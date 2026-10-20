//! 报价与地区（阶段 I，docs/PAYMENT_SPEC.md §15）：client / server 共用的纯函数。
//!
//! 地区只决定「展示哪些支付方式、按哪种货币报价」；金额永远由服务端从课程数据
//! 算出（[`Pricing::quote`]），客户端声明的地区不构成绕过（DI2 / DI3）。

use serde::{Deserialize, Serialize};

/// 访客所在地区（按浏览器时区判断，可手动切换；DI2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Region {
  /// 中国大陆：支付宝 / 微信 / 银行卡，按人民币报价。SSR 首帧默认值。
  #[default]
  Mainland,
  /// 其余地区：Stripe，设了美元价时按美元报价。
  Overseas,
}

/// 中国大陆的 IANA 时区（含已并入 Asia/Shanghai 的旧名）。港澳台不在内：
/// 当地不用人民币，走 Stripe 更合适。
const MAINLAND_TIMEZONES: &[&str] =
  &["Asia/Shanghai", "Asia/Urumqi", "Asia/Chongqing", "Asia/Harbin", "Asia/Kashgar", "PRC"];

impl Region {
  pub fn from_timezone(tz: &str) -> Self {
    if MAINLAND_TIMEZONES.contains(&tz) {
      Region::Mainland
    } else {
      Region::Overseas
    }
  }

  /// server fn 参数的线上形态。
  pub fn as_str(self) -> &'static str {
    match self {
      Region::Mainland => "mainland",
      Region::Overseas => "overseas",
    }
  }

  pub fn parse(s: &str) -> Option<Self> {
    match s {
      "mainland" => Some(Region::Mainland),
      "overseas" => Some(Region::Overseas),
      _ => None,
    }
  }
}

/// 一个确定的报价：金额（最小货币单位：分 / 美分）+ 币种。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Price {
  pub amount: i64,
  pub currency: String,
}

impl Price {
  /// 展示文案：`¥99`、`¥99.50`、`$14.99`；未知币种为 `12.00 EUR`。
  pub fn label(&self) -> String {
    let major = self.amount / 100;
    let minor = self.amount % 100;
    let number = if minor == 0 { major.to_string() } else { format!("{major}.{minor:02}") };
    match self.currency.as_str() {
      "CNY" => format!("¥{number}"),
      "USD" => format!("${number}"),
      other => format!("{major}.{minor:02} {other}"),
    }
  }
}

/// 课程的价格数据（`course.yaml` 的 `price` / `currency` / `price_usd`）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Pricing {
  /// 主价格（分）。
  pub price: i64,
  /// 主价格币种，实际都是 CNY。
  pub currency: String,
  /// 海外价格（美分）；未设时海外也按主价格收。
  #[serde(default)]
  pub price_usd: Option<i64>,
}

impl Pricing {
  /// 该地区看到的标价。
  pub fn list_price(&self, region: Region) -> Price {
    match (region, self.price_usd) {
      (Region::Overseas, Some(usd)) => Price { amount: usd, currency: "USD".to_string() },
      _ => Price { amount: self.price, currency: self.currency.clone() },
    }
  }

  /// 下单报价（DI3）：支付宝 / 微信只收人民币；Stripe 收该地区的标价。
  /// 错误是 i18n key（`pay.err.*`）。
  pub fn quote(&self, provider: &str, region: Region) -> Result<Price, &'static str> {
    match provider {
      "alipay" | "wechat" if self.currency == "CNY" => {
        Ok(Price { amount: self.price, currency: "CNY".to_string() })
      }
      "alipay" | "wechat" => Err("pay.err.no_cny"),
      "stripe" => Ok(self.list_price(region)),
      _ => Err("pay.err.bad_request"),
    }
  }
}

/// 该地区给出的支付方式（按展示顺序），只保留服务端已配置的网关（DI4）。
pub fn methods_for(region: Region, configured: &[String]) -> Vec<String> {
  let order: &[&str] = match region {
    Region::Mainland => &["alipay", "wechat", "stripe"],
    Region::Overseas => &["stripe"],
  };
  order.iter().filter(|m| configured.iter().any(|c| c == *m)).map(|m| m.to_string()).collect()
}

/// 支付渠道名的 i18n key。
pub fn channel_key(provider: &str) -> &'static str {
  match provider {
    "alipay" => "pay.channel.alipay",
    "wechat" => "pay.channel.wechat",
    "stripe" => "pay.channel.stripe",
    _ => "pay.channel.other",
  }
}

/// 地区名的 i18n key。
pub fn region_key(region: Region) -> &'static str {
  match region {
    Region::Mainland => "pay.region.mainland",
    Region::Overseas => "pay.region.overseas",
  }
}

/// 「切换到某地区」按钮文案的 i18n key。
pub fn switch_key(target: Region) -> &'static str {
  match target {
    Region::Mainland => "pay.region.switch_to_mainland",
    Region::Overseas => "pay.region.switch_to_overseas",
  }
}

// ------------------------------------------------------------
// 当前访客的地区（界面状态）
// ------------------------------------------------------------

/// `None` = 还没判断（SSR 与 hydration 前），按默认地区渲染，保证两端首帧一致。
static REGION: dioxus::prelude::GlobalSignal<Option<Region>> =
  dioxus::prelude::Signal::global(|| None);

/// 记住手动切换的地区（每位访客自己的偏好，丢了就回到按时区判断）。
const REGION_STORAGE_KEY: &str = "pay_region";

/// 当前地区。hydration 后取手动选择，没有则按浏览器时区判断（DI2）。
pub fn use_region() -> Region {
  use dioxus::prelude::*;
  use_effect(|| {
    if REGION.peek().is_none() {
      let stored = widgets::browser::storage_get(REGION_STORAGE_KEY);
      let region = stored.as_deref().and_then(Region::parse).unwrap_or_else(|| {
        widgets::browser::time_zone().map(|tz| Region::from_timezone(&tz)).unwrap_or_default()
      });
      *REGION.write() = Some(region);
    }
  });
  REGION().unwrap_or_default()
}

/// 手动切换地区（购买弹窗里的「切换地区」）。
pub fn set_region(region: Region) {
  *REGION.write() = Some(region);
  widgets::browser::storage_set(REGION_STORAGE_KEY, region.as_str());
}

#[cfg(test)]
mod tests {
  use super::*;

  fn pricing(usd: Option<i64>) -> Pricing {
    Pricing { price: 9900, currency: "CNY".into(), price_usd: usd }
  }

  fn price(amount: i64, currency: &str) -> Price {
    Price { amount, currency: currency.into() }
  }

  #[test]
  fn region_from_timezone() {
    assert_eq!(Region::from_timezone("Asia/Shanghai"), Region::Mainland);
    assert_eq!(Region::from_timezone("Asia/Urumqi"), Region::Mainland);
    assert_eq!(Region::from_timezone("Asia/Hong_Kong"), Region::Overseas);
    assert_eq!(Region::from_timezone("America/New_York"), Region::Overseas);
    // 取不到时区（空串）按海外：大陆用户的浏览器都能给出时区
    assert_eq!(Region::from_timezone(""), Region::Overseas);
  }

  #[test]
  fn region_wire_roundtrip() {
    for r in [Region::Mainland, Region::Overseas] {
      assert_eq!(Region::parse(r.as_str()), Some(r));
    }
    assert_eq!(Region::parse("mars"), None);
  }

  #[test]
  fn quote_matrix() {
    let (with_usd, cny_only) = (pricing(Some(1499)), pricing(None));
    for p in [&with_usd, &cny_only] {
      for region in [Region::Mainland, Region::Overseas] {
        // 支付宝 / 微信不论地区都按人民币
        assert_eq!(p.quote("alipay", region), Ok(price(9900, "CNY")));
        assert_eq!(p.quote("wechat", region), Ok(price(9900, "CNY")));
      }
      assert_eq!(p.quote("stripe", Region::Mainland), Ok(price(9900, "CNY")));
      assert!(p.quote("paypal", Region::Overseas).is_err());
    }
    assert_eq!(with_usd.quote("stripe", Region::Overseas), Ok(price(1499, "USD")));
    // 没设美元价：海外也按人民币
    assert_eq!(cny_only.quote("stripe", Region::Overseas), Ok(price(9900, "CNY")));
  }

  #[test]
  fn domestic_gateways_reject_non_cny_course() {
    let usd_course = Pricing { price: 1500, currency: "USD".into(), price_usd: None };
    assert!(usd_course.quote("alipay", Region::Mainland).is_err());
    assert_eq!(usd_course.quote("stripe", Region::Mainland), Ok(price(1500, "USD")));
  }

  #[test]
  fn methods_follow_region_and_configuration() {
    let all: Vec<String> = ["stripe", "wechat", "alipay"].iter().map(|s| s.to_string()).collect();
    assert_eq!(methods_for(Region::Mainland, &all), ["alipay", "wechat", "stripe"]);
    assert_eq!(methods_for(Region::Overseas, &all), ["stripe"]);
    let domestic = vec!["alipay".to_string(), "wechat".to_string()];
    assert_eq!(methods_for(Region::Mainland, &domestic), ["alipay", "wechat"]);
    // 海外且没配 Stripe：没有可用方式（界面提示切换地区）
    assert!(methods_for(Region::Overseas, &domestic).is_empty());
  }

  #[test]
  fn price_labels() {
    assert_eq!(price(9900, "CNY").label(), "¥99");
    assert_eq!(price(9950, "CNY").label(), "¥99.50");
    assert_eq!(price(1499, "USD").label(), "$14.99");
    assert_eq!(price(1500, "USD").label(), "$15");
    assert_eq!(price(1205, "EUR").label(), "12.05 EUR");
  }
}
