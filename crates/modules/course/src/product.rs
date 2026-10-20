//! 订单卖的是什么（FEATURE_TIERS.md §6）：课程或打赏。client / server 共用。

use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
  use super::*;

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
