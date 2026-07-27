//! 统一 notify 流水线（server-only，PM3）。
//!
//! 合并 `module-course` 两处 ~90% 重复的 notify 处理，S6 加固成果完整收敛：
//!
//! 1. **验签 / 解密在宿主侧**（provider 模块 + 调用方 handler）——进入本流水线
//!    的 [`PaymentEvent`] 必须已通过验签（微信另含解密 + appid/mchid 校验）；
//! 2. 非成功状态 → 确认收到但不发货（避免网关无谓重试）；
//! 3. 金额核验：以 **DB 订单快照**为准，网关回报金额不符必拒；
//! 4. 幂等快路径：已 `paid` 直接确认；
//! 5. **原子认领**：`UPDATE … WHERE out_trade_no = ? AND status != 'paid'`，
//!    `rows_affected = 0` 视为已处理——并发回调 / 重放只有一次发货生效；
//! 6. 发货：**注入回调** `on_paid(user_id, course_slug)`——payment crate 不
//!    反向依赖 course 的 entitlement（§11 依赖方向：db 句柄由调用方提供，
//!    订单实体下沉 app-core）；
//! 7. `target=pay_audit` 审计日志（关键字段留痕，不含买家敏感信息）。
//!
//! 存储经 [`OrderStore`] 抽象（[`sea_orm::DatabaseConnection`] 自带实现），
//! 单测用纯 mock store 覆盖幂等 / 金额不匹配 / 并发认领语义。

use std::future::Future;

use crate::PaymentEvent;

/// 订单快照（金额核验 / 幂等判定 / 发货入参；从 orders 行提取）。
#[derive(Debug, Clone, PartialEq)]
pub struct OrderSnapshot {
  pub user_id: i32,
  pub course_slug: String,
  /// 下单时快照的金额（分）——金额核验以此为准。
  pub amount_cents: i64,
  pub status: String,
}

/// 流水线的最小存储边界：查单 + 原子认领。
///
/// [`sea_orm::DatabaseConnection`] 有默认实现；单测注入纯 mock。
pub trait OrderStore {
  /// 按我方订单号查单；`Ok(None)` = 不存在，`Err` = 存储错误。
  fn find_order(
    &self,
    out_trade_no: &str,
  ) -> impl Future<Output = Result<Option<OrderSnapshot>, String>> + Send;

  /// 原子认领：`status → paid` + 回填网关流水号 / 支付时间，条件
  /// `status != 'paid'`。返回受影响行数（0 = 已被并发回调认领）。
  fn claim_paid(
    &self,
    out_trade_no: &str,
    txn_id: Option<String>,
  ) -> impl Future<Output = Result<u64, String>> + Send;
}

impl OrderStore for sea_orm::DatabaseConnection {
  async fn find_order(&self, out_trade_no: &str) -> Result<Option<OrderSnapshot>, String> {
    use app_core::entities::order;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    order::Entity::find()
      .filter(order::Column::OutTradeNo.eq(out_trade_no))
      .one(self)
      .await
      .map(|row| {
        row.map(|m| OrderSnapshot {
          user_id: m.user_id,
          course_slug: m.course_slug,
          amount_cents: m.amount,
          status: m.status,
        })
      })
      .map_err(|e| e.to_string())
  }

  async fn claim_paid(&self, out_trade_no: &str, txn_id: Option<String>) -> Result<u64, String> {
    use app_core::entities::order;
    use sea_orm::sea_query::Expr;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    order::Entity::update_many()
      .col_expr(order::Column::Status, Expr::value("paid"))
      .col_expr(order::Column::ProviderTxn, Expr::value(txn_id))
      .col_expr(order::Column::PaidAt, Expr::value(Some(chrono::Utc::now().fixed_offset())))
      .filter(order::Column::OutTradeNo.eq(out_trade_no))
      .filter(order::Column::Status.ne("paid"))
      .exec(self)
      .await
      .map(|res| res.rows_affected)
      .map_err(|e| e.to_string())
  }
}

/// 纯决策（无 IO，便于单测）：给定已验签事件 + 订单快照，判定下一步。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotifyDecision {
  /// 非成功状态：确认收到但不发货。
  AckNotSuccess,
  /// 网关回报金额与订单快照不符：拒绝（触发网关重试 + 人工排查）。
  RejectAmountMismatch,
  /// 订单已 paid：幂等快路径，直接确认。
  AlreadyPaid,
  /// 认领并发货。
  Claim,
}

pub fn decide(event: &PaymentEvent, snapshot: &OrderSnapshot) -> NotifyDecision {
  if !event.is_success() {
    return NotifyDecision::AckNotSuccess;
  }
  if event.amount_cents != snapshot.amount_cents {
    return NotifyDecision::RejectAmountMismatch;
  }
  if snapshot.status == "paid" {
    return NotifyDecision::AlreadyPaid;
  }
  NotifyDecision::Claim
}

/// 流水线结果：调用方据此构造网关应答（支付宝纯文本 / 微信 JSON）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotifyOutcome {
  /// 处理成功（含幂等重复、非成功状态确认）→ 应答成功，网关停止重试。
  Ok,
  /// 处理失败 → 应答失败触发网关重试；原因串与旧 wechat handler 应答一致。
  Failed(&'static str),
}

impl NotifyOutcome {
  pub fn is_ok(&self) -> bool {
    matches!(self, NotifyOutcome::Ok)
  }
}

/// 处理一条**已验签（及解密）**的支付事件：查单 → 金额核验 → 原子认领 →
/// 发货回调 → 审计日志。
///
/// `on_paid(user_id, course_slug)`：认领成功后的发货动作（幂等要求由回调方
/// 保证，如 upsert 权益）；返回 `Err` 时应答失败，网关将重试（认领已落库，
/// 重试进幂等快路径后不再二次发货——与 S6 前行为一致，发货失败需人工对账）。
pub async fn process_event<S, F, Fut>(store: &S, event: &PaymentEvent, on_paid: F) -> NotifyOutcome
where
  S: OrderStore + Sync,
  F: FnOnce(i32, String) -> Fut,
  Fut: Future<Output = Result<(), String>>,
{
  let provider = event.provider.as_str();
  // 2) 仅成功状态发货；其它状态确认收到（避免无谓重试）但不发货。
  //    先于查单判定，非成功回调零 DB 开销。
  if !event.is_success() {
    return NotifyOutcome::Ok;
  }
  let snapshot = match store.find_order(&event.out_trade_no).await {
    Ok(Some(s)) => s,
    _ => return NotifyOutcome::Failed("order not found"),
  };
  match decide(event, &snapshot) {
    NotifyDecision::AckNotSuccess | NotifyDecision::AlreadyPaid => return NotifyOutcome::Ok,
    NotifyDecision::RejectAmountMismatch => {
      // 3) 金额核验失败：留痕 + 拒绝。
      tracing::warn!(
        target: "pay_audit",
        "{} notify: amount mismatch for {}",
        provider,
        event.out_trade_no
      );
      return NotifyOutcome::Failed("amount mismatch");
    }
    NotifyDecision::Claim => {}
  }
  // 5) S6：原子认领——条件 UPDATE 取代「读-判-写」，并发回调只有一个能认领。
  match store.claim_paid(&event.out_trade_no, event.txn_id.clone()).await {
    Ok(0) => return NotifyOutcome::Ok, // 并发回调已处理
    Ok(_) => {}
    Err(_) => return NotifyOutcome::Failed("update failed"),
  }
  // 6) 发货：注入回调（course 侧写权益，幂等 upsert）。
  if on_paid(snapshot.user_id, snapshot.course_slug).await.is_err() {
    return NotifyOutcome::Failed("grant failed");
  }
  tracing::info!(
    target: "pay_audit",
    "{} notify: order {} paid + entitlement granted",
    provider,
    event.out_trade_no
  );
  NotifyOutcome::Ok
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::EVENT_STATUS_SUCCESS;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::sync::Arc;

  fn event(amount: i64, status: &str) -> PaymentEvent {
    PaymentEvent {
      provider: "alipay".into(),
      out_trade_no: "RIE1".into(),
      amount_cents: amount,
      status: status.into(),
      txn_id: Some("txn-1".into()),
    }
  }

  fn snapshot(amount: i64, status: &str) -> OrderSnapshot {
    OrderSnapshot {
      user_id: 7,
      course_slug: "rust-basics".into(),
      amount_cents: amount,
      status: status.into(),
    }
  }

  /// 纯 mock store：可注入查单结果 / 认领行数 / 存储错误，并统计调用次数。
  struct MockStore {
    order: Option<OrderSnapshot>,
    claim_rows: Result<u64, String>,
    finds: AtomicUsize,
    claims: AtomicUsize,
  }

  impl MockStore {
    fn new(order: Option<OrderSnapshot>, claim_rows: Result<u64, String>) -> Self {
      Self { order, claim_rows, finds: AtomicUsize::new(0), claims: AtomicUsize::new(0) }
    }
  }

  impl OrderStore for MockStore {
    async fn find_order(&self, out_trade_no: &str) -> Result<Option<OrderSnapshot>, String> {
      assert_eq!(out_trade_no, "RIE1");
      self.finds.fetch_add(1, Ordering::SeqCst);
      Ok(self.order.clone())
    }
    async fn claim_paid(&self, out_trade_no: &str, txn_id: Option<String>) -> Result<u64, String> {
      assert_eq!(out_trade_no, "RIE1");
      assert_eq!(txn_id.as_deref(), Some("txn-1"));
      self.claims.fetch_add(1, Ordering::SeqCst);
      self.claim_rows.clone()
    }
  }

  /// 跑一次流水线，返回 (结果, 发货次数)。发货回调即「mock 发货」。
  async fn run(store: &MockStore, ev: PaymentEvent, deliver_ok: bool) -> (NotifyOutcome, usize) {
    let delivered = Arc::new(AtomicUsize::new(0));
    let d = delivered.clone();
    let outcome = process_event(store, &ev, |user_id, slug| async move {
      assert_eq!((user_id, slug.as_str()), (7, "rust-basics"));
      d.fetch_add(1, Ordering::SeqCst);
      if deliver_ok {
        Ok(())
      } else {
        Err("boom".to_string())
      }
    })
    .await;
    (outcome, delivered.load(Ordering::SeqCst))
  }

  // ── 纯决策 ──

  #[test]
  fn decide_matrix() {
    let snap = snapshot(9900, "pending");
    // 非成功状态 → 确认不发货。
    assert_eq!(decide(&event(9900, "WAIT_BUYER_PAY"), &snap), NotifyDecision::AckNotSuccess);
    // 金额不匹配（含解析失败的 -1）→ 拒绝。
    assert_eq!(
      decide(&event(1, EVENT_STATUS_SUCCESS), &snap),
      NotifyDecision::RejectAmountMismatch
    );
    assert_eq!(
      decide(&event(-1, EVENT_STATUS_SUCCESS), &snap),
      NotifyDecision::RejectAmountMismatch
    );
    // 正常认领。
    assert_eq!(decide(&event(9900, EVENT_STATUS_SUCCESS), &snap), NotifyDecision::Claim);
    // 幂等：已 paid。
    assert_eq!(
      decide(&event(9900, EVENT_STATUS_SUCCESS), &snapshot(9900, "paid")),
      NotifyDecision::AlreadyPaid
    );
  }

  // ── process_event（mock store + mock 发货回调）──

  #[tokio::test]
  async fn happy_path_claims_and_delivers() {
    let store = MockStore::new(Some(snapshot(9900, "pending")), Ok(1));
    let (outcome, delivered) = run(&store, event(9900, EVENT_STATUS_SUCCESS), true).await;
    assert_eq!(outcome, NotifyOutcome::Ok);
    assert_eq!(delivered, 1, "claim 成功必须发货一次");
    assert_eq!(store.claims.load(Ordering::SeqCst), 1);
  }

  #[tokio::test]
  async fn idempotent_paid_order_acks_without_delivery() {
    let store = MockStore::new(Some(snapshot(9900, "paid")), Ok(1));
    let (outcome, delivered) = run(&store, event(9900, EVENT_STATUS_SUCCESS), true).await;
    assert_eq!(outcome, NotifyOutcome::Ok);
    assert_eq!(delivered, 0, "已 paid 的重放不得二次发货");
    assert_eq!(store.claims.load(Ordering::SeqCst), 0, "幂等快路径不触发 UPDATE");
  }

  #[tokio::test]
  async fn amount_mismatch_rejected() {
    let store = MockStore::new(Some(snapshot(9900, "pending")), Ok(1));
    let (outcome, delivered) = run(&store, event(100, EVENT_STATUS_SUCCESS), true).await;
    assert_eq!(outcome, NotifyOutcome::Failed("amount mismatch"));
    assert_eq!(delivered, 0);
    assert_eq!(store.claims.load(Ordering::SeqCst), 0);
  }

  #[tokio::test]
  async fn concurrent_claim_loser_acks_without_delivery() {
    // rows_affected = 0：另一并发回调已认领 → 确认成功但不发货。
    let store = MockStore::new(Some(snapshot(9900, "pending")), Ok(0));
    let (outcome, delivered) = run(&store, event(9900, EVENT_STATUS_SUCCESS), true).await;
    assert_eq!(outcome, NotifyOutcome::Ok);
    assert_eq!(delivered, 0, "并发认领失败方不得发货");
  }

  #[tokio::test]
  async fn non_success_event_acks_without_store_access() {
    let store = MockStore::new(Some(snapshot(9900, "pending")), Ok(1));
    let (outcome, delivered) = run(&store, event(9900, "NOTPAY"), true).await;
    assert_eq!(outcome, NotifyOutcome::Ok);
    assert_eq!(delivered, 0);
    assert_eq!(store.finds.load(Ordering::SeqCst), 0, "非成功状态必须在查单前返回");
  }

  #[tokio::test]
  async fn missing_order_fails() {
    let store = MockStore::new(None, Ok(1));
    let (outcome, delivered) = run(&store, event(9900, EVENT_STATUS_SUCCESS), true).await;
    assert_eq!(outcome, NotifyOutcome::Failed("order not found"));
    assert_eq!(delivered, 0);
  }

  #[tokio::test]
  async fn claim_error_maps_to_update_failed() {
    let store = MockStore::new(Some(snapshot(9900, "pending")), Err("db down".into()));
    let (outcome, delivered) = run(&store, event(9900, EVENT_STATUS_SUCCESS), true).await;
    assert_eq!(outcome, NotifyOutcome::Failed("update failed"));
    assert_eq!(delivered, 0);
  }

  #[tokio::test]
  async fn delivery_failure_maps_to_grant_failed() {
    let store = MockStore::new(Some(snapshot(9900, "pending")), Ok(1));
    let (outcome, delivered) = run(&store, event(9900, EVENT_STATUS_SUCCESS), false).await;
    assert_eq!(outcome, NotifyOutcome::Failed("grant failed"));
    assert_eq!(delivered, 1);
  }
}
