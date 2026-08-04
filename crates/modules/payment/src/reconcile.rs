//! M5e 对账（server-only）：处理「回调丢失 / 用户未支付」的滞留 `pending` 单。
//!
//! 驱动方式：调用方（course）定时取 [`crate::pipeline::OrderStore::list_stale_pending`]，
//! 逐单经 [`crate::host::execute_query`] 向网关查单得到中立 [`PaymentEvent`]，
//! 再交本模块 [`reconcile_order`] 决策：
//!
//! - 网关已成功 → 走统一流水线 [`crate::pipeline::process_event`] 回填
//!   （金额核验 / 原子认领 / 注入发货 / pay_audit 全部复用，与回调同一条路径）；
//! - 未支付（含 `TRADE_NOT_EXIST` / `ORDER_NOT_EXIST`）且超过
//!   [`ReconcilePolicy::close_after`] → 条件关单 `pending → closed`
//!   （rows_affected = 0 表示回调恰好赶到，让位回调链路）；
//! - 尚在支付窗口内 → 保持 pending；
//! - 查单出错（网关错误 / 未配置）由调用方跳过，**绝不据此关单**（fail-safe）。
//!
//! 安全性：关单只覆盖 `pending`；即使误关，迟到的合法回调仍能经原子认领
//! `status != 'paid'` 把 closed 单认领为 paid 并发货——不会丢钱、不会双发货。

use crate::pipeline::{process_event, NotifyOutcome, OrderStore, StaleOrder};
use crate::PaymentEvent;

/// 对账策略。
#[derive(Debug, Clone, Copy)]
pub struct ReconcilePolicy {
  /// 创建后超过该时长仍未支付 → 关单（支付窗口）。
  pub close_after: chrono::Duration,
}

/// 单笔对账结果（计数 / 日志用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileAction {
  /// 网关已成功 → 已回填发货（含幂等重复确认）。
  Delivered,
  /// 未支付且超窗 → 已关单。
  Closed,
  /// 未支付且尚在支付窗口内 → 保持 pending。
  Pending,
  /// 关单/认领竞态（回调恰好赶到）→ 保持现状，由回调链路负责。
  Raced,
  /// 处理失败（原因串同流水线应答语义），下轮重试。
  Failed(&'static str),
}

/// 纯决策（无 IO）：给定网关事件是否成功 + 订单年龄，判定动作类别。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileDecision {
  Deliver,
  Close,
  Keep,
}

pub fn decide_reconcile(
  event_success: bool,
  created_at: chrono::DateTime<chrono::FixedOffset>,
  now: chrono::DateTime<chrono::FixedOffset>,
  policy: &ReconcilePolicy,
) -> ReconcileDecision {
  if event_success {
    return ReconcileDecision::Deliver;
  }
  if now - created_at >= policy.close_after {
    return ReconcileDecision::Close;
  }
  ReconcileDecision::Keep
}

/// 对账处理一笔滞留订单（`event` 为已查单得到的中立事件）。
///
/// `on_paid` 与回调链路同一发货回调（幂等 upsert 权益）。
pub async fn reconcile_order<S, F, Fut>(
  store: &S,
  order: &StaleOrder,
  event: &PaymentEvent,
  now: chrono::DateTime<chrono::FixedOffset>,
  policy: &ReconcilePolicy,
  on_paid: F,
) -> ReconcileAction
where
  S: OrderStore + Sync,
  F: FnOnce(i32, String) -> Fut,
  Fut: std::future::Future<Output = Result<(), String>>,
{
  match decide_reconcile(event.is_success(), order.created_at, now, policy) {
    ReconcileDecision::Deliver => {
      // 防御：查单响应的订单号必须与本地滞留单一致（NOT_EXIST 等非成功
      // 事件可能缺 out_trade_no，但不会走到这里）。
      if event.out_trade_no != order.out_trade_no {
        tracing::warn!(
          target: "pay_audit",
          "reconcile: out_trade_no mismatch (local {}, gateway {})",
          order.out_trade_no,
          event.out_trade_no
        );
        return ReconcileAction::Failed("otn mismatch");
      }
      match process_event(store, event, on_paid).await {
        NotifyOutcome::Ok => {
          tracing::info!(
            target: "pay_audit",
            "reconcile: order {} backfilled via gateway query",
            order.out_trade_no
          );
          ReconcileAction::Delivered
        }
        NotifyOutcome::Failed(m) => ReconcileAction::Failed(m),
      }
    }
    ReconcileDecision::Close => match store.close_order(&order.out_trade_no).await {
      Ok(0) => ReconcileAction::Raced, // 回调恰好赶到并认领/处理
      Ok(_) => {
        tracing::info!(
          target: "pay_audit",
          "reconcile: order {} closed (unpaid past window, gateway status {})",
          order.out_trade_no,
          event.status
        );
        ReconcileAction::Closed
      }
      Err(_) => ReconcileAction::Failed("close failed"),
    },
    ReconcileDecision::Keep => ReconcileAction::Pending,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::pipeline::OrderSnapshot;
  use crate::EVENT_STATUS_SUCCESS;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::sync::Arc;

  fn policy() -> ReconcilePolicy {
    ReconcilePolicy { close_after: chrono::Duration::hours(2) }
  }

  fn now() -> chrono::DateTime<chrono::FixedOffset> {
    chrono::Utc::now().fixed_offset()
  }

  fn stale(age: chrono::Duration) -> StaleOrder {
    StaleOrder { out_trade_no: "RIE1".into(), provider: "alipay".into(), created_at: now() - age }
  }

  fn event(status: &str, amount: i64) -> PaymentEvent {
    PaymentEvent {
      provider: "alipay".into(),
      out_trade_no: "RIE1".into(),
      amount_cents: amount,
      status: status.into(),
      txn_id: Some("txn-1".into()),
    }
  }

  /// 对账 mock store：可注入查单快照 / 认领与关单行数，并统计关单次数。
  struct MockStore {
    order: Option<OrderSnapshot>,
    claim_rows: u64,
    close_rows: Result<u64, String>,
    closes: AtomicUsize,
  }

  impl OrderStore for MockStore {
    async fn find_order(&self, _otn: &str) -> Result<Option<OrderSnapshot>, String> {
      Ok(self.order.clone())
    }
    async fn claim_paid(&self, _otn: &str, _txn: Option<String>) -> Result<u64, String> {
      Ok(self.claim_rows)
    }
    async fn list_stale_pending(
      &self,
      _cutoff: chrono::DateTime<chrono::FixedOffset>,
      _limit: u64,
    ) -> Result<Vec<StaleOrder>, String> {
      Ok(vec![])
    }
    async fn close_order(&self, _otn: &str) -> Result<u64, String> {
      self.closes.fetch_add(1, Ordering::SeqCst);
      self.close_rows.clone()
    }
  }

  fn snapshot(status: &str) -> OrderSnapshot {
    OrderSnapshot {
      user_id: 7,
      course_slug: "rust-basics".into(),
      amount_cents: 9900,
      status: status.into(),
    }
  }

  async fn run(
    store: &MockStore,
    order: &StaleOrder,
    ev: &PaymentEvent,
  ) -> (ReconcileAction, usize) {
    let delivered = Arc::new(AtomicUsize::new(0));
    let d = delivered.clone();
    let action = reconcile_order(store, order, ev, now(), &policy(), |_uid, _slug| async move {
      d.fetch_add(1, Ordering::SeqCst);
      Ok(())
    })
    .await;
    (action, delivered.load(Ordering::SeqCst))
  }

  #[test]
  fn decide_matrix() {
    let p = policy();
    let t = now();
    // 网关已成功 → 回填（无论年龄）。
    assert_eq!(
      decide_reconcile(true, t - chrono::Duration::minutes(6), t, &p),
      ReconcileDecision::Deliver
    );
    // 未支付 + 超窗 → 关单。
    assert_eq!(
      decide_reconcile(false, t - chrono::Duration::hours(3), t, &p),
      ReconcileDecision::Close
    );
    // 未支付 + 窗口内 → 保持。
    assert_eq!(
      decide_reconcile(false, t - chrono::Duration::minutes(30), t, &p),
      ReconcileDecision::Keep
    );
  }

  #[tokio::test]
  async fn success_backfills_via_pipeline() {
    let store = MockStore {
      order: Some(snapshot("pending")),
      claim_rows: 1,
      close_rows: Ok(1),
      closes: AtomicUsize::new(0),
    };
    let (action, delivered) =
      run(&store, &stale(chrono::Duration::minutes(10)), &event(EVENT_STATUS_SUCCESS, 9900)).await;
    assert_eq!(action, ReconcileAction::Delivered);
    assert_eq!(delivered, 1, "回填必须走统一流水线发货一次");
    assert_eq!(store.closes.load(Ordering::SeqCst), 0);
  }

  #[tokio::test]
  async fn unpaid_past_window_closes() {
    let store = MockStore {
      order: Some(snapshot("pending")),
      claim_rows: 1,
      close_rows: Ok(1),
      closes: AtomicUsize::new(0),
    };
    let (action, delivered) =
      run(&store, &stale(chrono::Duration::hours(3)), &event("WAIT_BUYER_PAY", -1)).await;
    assert_eq!(action, ReconcileAction::Closed);
    assert_eq!(delivered, 0, "关单不得发货");
    assert_eq!(store.closes.load(Ordering::SeqCst), 1);
  }

  #[tokio::test]
  async fn unpaid_within_window_keeps_pending() {
    let store = MockStore {
      order: Some(snapshot("pending")),
      claim_rows: 1,
      close_rows: Ok(1),
      closes: AtomicUsize::new(0),
    };
    let (action, delivered) =
      run(&store, &stale(chrono::Duration::minutes(20)), &event("TRADE_NOT_EXIST", -1)).await;
    assert_eq!(action, ReconcileAction::Pending);
    assert_eq!(delivered, 0);
    assert_eq!(store.closes.load(Ordering::SeqCst), 0, "窗口内不触发关单");
  }

  #[tokio::test]
  async fn close_race_yields_to_notify() {
    // rows_affected = 0：回调恰好赶到把单认领走 → 让位，不覆盖。
    let store = MockStore {
      order: Some(snapshot("pending")),
      claim_rows: 1,
      close_rows: Ok(0),
      closes: AtomicUsize::new(0),
    };
    let (action, delivered) =
      run(&store, &stale(chrono::Duration::hours(3)), &event("NOTPAY", -1)).await;
    assert_eq!(action, ReconcileAction::Raced);
    assert_eq!(delivered, 0);
  }

  #[tokio::test]
  async fn amount_mismatch_backfill_rejected_by_pipeline() {
    // 网关说成功但金额与订单不符 → 流水线金额核验拒绝（S6 语义在对账路径同样生效）。
    let store = MockStore {
      order: Some(snapshot("pending")),
      claim_rows: 1,
      close_rows: Ok(1),
      closes: AtomicUsize::new(0),
    };
    let (action, delivered) =
      run(&store, &stale(chrono::Duration::minutes(10)), &event(EVENT_STATUS_SUCCESS, 100)).await;
    assert_eq!(action, ReconcileAction::Failed("amount mismatch"));
    assert_eq!(delivered, 0);
  }

  #[tokio::test]
  async fn otn_mismatch_rejected() {
    let store = MockStore {
      order: Some(snapshot("pending")),
      claim_rows: 1,
      close_rows: Ok(1),
      closes: AtomicUsize::new(0),
    };
    let mut ev = event(EVENT_STATUS_SUCCESS, 9900);
    ev.out_trade_no = "OTHER".into();
    let (action, delivered) = run(&store, &stale(chrono::Duration::minutes(10)), &ev).await;
    assert_eq!(action, ReconcileAction::Failed("otn mismatch"));
    assert_eq!(delivered, 0);
  }
}
