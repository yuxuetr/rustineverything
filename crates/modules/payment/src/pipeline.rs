//! 统一 notify 流水线（server-only）。
//!
//! PM3：合并 `module-course` 两处 ~90% 重复的 notify 处理——
//! 验签/解密（宿主侧）→ `parse_notify` → 金额核验（以 DB 订单为准）→
//! 原子认领（条件 UPDATE，rows_affected=0 幂等）→ 发货回调（注入）→
//! pay_audit 审计日志。S6 加固成果完整收敛于此。
