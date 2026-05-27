//! 审核队列实体（Phase 4.5）。
//!
//! 一行 = 一次需要 admin 复核的 Flag 决定。Block 决定不入队（已在
//! comment/topic/reply 提交路径上被拒），但留下日志。
//!
//! ## 工作流
//! `status = "pending"` 初始入队 → admin 点 approve（保留内容）/ reject
//! （删除业务内容） → `status = "approved" | "rejected"` + `reviewed_at`
//! + `reviewer_user_id`。

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "moderation_queue")]
pub struct Model {
  #[sea_orm(primary_key)]
  pub id: i64,
  /// 业务类型：`comment` / `topic` / `reply` / `annotation`
  pub kind: String,
  /// 关联到具体业务行的 id（comment.id / topic.id / topic_reply.id / annotation.id）
  /// 用 i64 兼容 BIGSERIAL 表（annotation），i32 表直接 cast。
  pub ref_id: Option<i64>,
  /// 人类可读路径，例如 `blog:welcome` / `topic:42`
  pub ref_path: String,
  /// 提交者 user_id。用户被删时设 NULL（不丢失审核记录）。
  pub user_id: Option<i32>,
  /// 内容快照（避免业务行被删后失去上下文）
  pub content: String,
  /// 图片 URL 数组，JSON 字符串形式存储（避免引入 `with-json` feature）
  pub images: Option<String>,
  /// LLM 评分 0.0 ~ 1.0
  pub score: f32,
  /// `"flag"` | `"block"`（block 仍可记录用于审计，当前只有 flag 入队）
  pub label: String,
  pub reason: String,
  /// `"pending"` | `"approved"` | `"rejected"`
  pub status: String,
  /// 复核者 user_id。
  pub reviewer_user_id: Option<i32>,
  pub reviewer_note: Option<String>,
  pub created_at: DateTimeWithTimeZone,
  pub reviewed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
