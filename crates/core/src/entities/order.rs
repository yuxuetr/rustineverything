use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 课程购买订单（M5 在线支付）。
///
/// `out_trade_no` 是我方订单号（下单时生成、传给网关、回调据此对账，唯一）。
/// 订单 `status` 流转：pending → paid | failed | closed | refunded。
/// 支付成功后幂等写入 [`super::entitlement`]（source=purchase）。
/// 详见 docs/PAYMENT_SPEC.md §2。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "orders")]
pub struct Model {
  #[sea_orm(primary_key)]
  pub id: i64,
  #[sea_orm(unique)]
  pub out_trade_no: String,
  pub user_id: i32,
  pub course_slug: String,
  /// wechat | alipay
  pub provider: String,
  /// native | h5 | page | wap | qr
  pub scene: String,
  /// 金额（分），下单时从 course.price 快照，回调时核验。
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
