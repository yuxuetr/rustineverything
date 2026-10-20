use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 在线支付订单（M5）：课程购买或打赏（FEATURE_TIERS.md §6）。
///
/// `out_trade_no` 是我方订单号（下单时生成、传给网关、回调据此对账，唯一）。
/// 订单 `status` 流转：pending → paid | failed | closed | refunded。
/// 课程订单支付成功后幂等写入 [`super::entitlement`]（source=purchase）；打赏不发货。
/// 详见 docs/PAYMENT_SPEC.md §2。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "orders")]
pub struct Model {
  #[sea_orm(primary_key)]
  pub id: i64,
  #[sea_orm(unique)]
  pub out_trade_no: String,
  pub user_id: i32,
  /// course | tip
  pub product_kind: String,
  /// 课程：课程 slug；打赏：打赏发生的页面路径（`/blog/<slug>`、`/podcast`）。
  pub product_ref: String,
  /// alipay | wechat | stripe
  pub provider: String,
  /// native | h5 | page | wap | qr
  pub scene: String,
  /// 金额（最小货币单位），下单时报价快照，回调时核验。
  pub amount: i64,
  pub currency: String,
  /// pending | paid | failed | closed | refunded
  pub status: String,
  /// 网关流水号（transaction_id / trade_no），支付后回填。
  pub provider_txn: Option<String>,
  pub created_at: DateTimeWithTimeZone,
  pub paid_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
  #[sea_orm(
    belongs_to = "super::user::Entity",
    from = "Column::UserId",
    to = "super::user::Column::Id"
  )]
  User,
}

impl Related<super::user::Entity> for Entity {
  fn to() -> RelationDef {
    Relation::User.def()
  }
}

impl ActiveModelBehavior for ActiveModel {}
