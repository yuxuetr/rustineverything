//! 订单卖的是什么（FEATURE_TIERS.md §6）：课程或打赏。client / server 共用。

use serde::{Deserialize, Serialize};

use crate::pricing::Pricing;

/// 订单商品种类（`orders.product_kind`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProductKind {
  /// `product_ref` = 课程 slug；付款后发课程权益。
  Course,
  /// `product_ref` = 打赏发生的页面路径；付款后不发货。
  Tip,
}

impl ProductKind {
  pub fn as_str(self) -> &'static str {
    match self {
      ProductKind::Course => "course",
      ProductKind::Tip => "tip",
    }
  }

  pub fn parse(s: &str) -> Option<Self> {
    match s {
      "course" => Some(ProductKind::Course),
      "tip" => Some(ProductKind::Tip),
      _ => None,
    }
  }

  /// 付款后是否发课程权益（退款时同样据此撤销）。
  pub fn grants_entitlement(self) -> bool {
    match self {
      ProductKind::Course => true,
      ProductKind::Tip => false,
    }
  }
}

/// 打赏档位（人民币分, 美分）：¥5 / 10 / 20 / 50，海外 $1 / 3 / 5 / 10（DL6）。
/// 客户端只传档位序号，金额由服务端查表。
pub const TIP_TIERS: [(i64, i64); 4] = [(500, 100), (1000, 300), (2000, 500), (5000, 1000)];

/// 档位的价格数据，交给 [`Pricing::quote`] 按网关与地区报价（与课程同一规则）。
pub fn tip_pricing(tier: usize) -> Option<Pricing> {
  TIP_TIERS.get(tier).map(|&(cny, usd)| Pricing {
    price: cny,
    currency: "CNY".to_string(),
    price_usd: Some(usd),
  })
}

/// 打赏能发生的页面：`/podcast` 或 `/blog/<slug>`。它也是付款后的返回路径，
/// 只收这两种，免得订单里存进任意 URL。
pub fn is_tip_source(path: &str) -> bool {
  path == "/podcast" || path.strip_prefix("/blog/").is_some_and(app_core::utils::is_safe_slug)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::pricing::{Price, Region};

  fn price(amount: i64, currency: &str) -> Price {
    Price { amount, currency: currency.into() }
  }

  #[test]
  fn tip_quotes_follow_course_rules() {
    for (tier, &(cny, usd)) in TIP_TIERS.iter().enumerate() {
      let p = tip_pricing(tier).unwrap();
      for region in [Region::Mainland, Region::Overseas] {
        assert_eq!(p.quote("alipay", region), Ok(price(cny, "CNY")));
        assert_eq!(p.quote("wechat", region), Ok(price(cny, "CNY")));
      }
      assert_eq!(p.quote("stripe", Region::Mainland), Ok(price(cny, "CNY")));
      assert_eq!(p.quote("stripe", Region::Overseas), Ok(price(usd, "USD")));
    }
    assert_eq!(tip_pricing(1).unwrap().list_price(Region::Overseas).label(), "$3");
    assert_eq!(tip_pricing(3).unwrap().list_price(Region::Mainland).label(), "¥50");
    assert!(tip_pricing(TIP_TIERS.len()).is_none());
  }

  #[test]
  fn tip_sources() {
    assert!(is_tip_source("/podcast"));
    assert!(is_tip_source("/blog/welcome"));
    assert!(is_tip_source("/blog/rust_2024-recap"));
    for bad in [
      "",
      "/",
      "/blog",
      "/blog/",
      "/blog/a/b",
      "/blog/../x",
      "https://evil.com",
      "/course/rust-basics",
      "/podcast/1",
    ] {
      assert!(!is_tip_source(bad), "{bad}");
    }
  }

  #[test]
  fn kind_wire_roundtrip() {
    for k in [ProductKind::Course, ProductKind::Tip] {
      assert_eq!(ProductKind::parse(k.as_str()), Some(k));
    }
    assert_eq!(ProductKind::parse("membership"), None);
  }

  #[test]
  fn only_courses_grant_entitlements() {
    assert!(ProductKind::Course.grants_entitlement());
    assert!(!ProductKind::Tip.grants_entitlement());
  }
}
