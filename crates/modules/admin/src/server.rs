use dioxus::prelude::*;
use rustineverything_core::session::{is_known_role, ROLE_ADMIN};
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
use std::path::PathBuf;

// =============================================================
// Public DTOs
// =============================================================

/// 后台首页统计数据
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AdminOverview {
  pub user_count: i64,
  pub admin_count: i64,
  pub comment_count: i64,
  pub topic_count: i64,
  pub reply_count: i64,
  pub annotation_count: i64,
  /// Phase 4.5：待复核的审核队列数量
  pub moderation_pending_count: i64,
}

/// 审核队列单行
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModerationQueueRow {
  pub id: i64,
  pub kind: String,
  pub ref_id: Option<i64>,
  pub ref_path: String,
  pub user_id: Option<i32>,
  pub user_nickname: Option<String>,
  pub content: String,
  pub images: Vec<String>,
  pub score: f32,
  pub label: String,
  pub reason: String,
  pub status: String,
  pub created_at: String,
  pub reviewer_nickname: Option<String>,
  pub reviewed_at: Option<String>,
}

/// 用户行（管理视角包含 role / 创建时间 / 绑定 provider）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdminUserRow {
  pub id: i32,
  pub nickname: String,
  pub avatar_url: Option<String>,
  pub role: String,
  pub providers: Vec<String>,
  pub created_at: String,
}

/// 评论行
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdminCommentRow {
  pub id: i32,
  pub blog_id: String,
  pub user_id: i32,
  pub author: String,
  pub content: String,
  pub created_at: String,
}

/// 话题行
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdminTopicRow {
  pub id: i32,
  pub title: String,
  pub tag: String,
  pub user_id: i32,
  pub author: String,
  pub reply_count: i32,
  pub created_at: String,
  pub last_reply_at: Option<String>,
}

/// 插件状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdminPluginRow {
  pub kind: String,            // "auth" | "theme" | "i18n" | "unknown"
  pub id: String,              // 来自 site.json 的 provider id 或文件名
  pub filename: String,        // wasm 文件名
  pub configured: bool,        // 在 site.json 里被启用
  pub credentials_ready: bool, // 仅 auth 插件相关：环境变量是否齐全
  pub present: bool,           // assets/plugins/<filename> 是否存在
  pub size_bytes: u64,
  pub modified: Option<String>,
}

/// 插件上传结果（Phase 5.1 hot reload）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginUploadResult {
  pub filename: String,
  pub plugin_id: String,
  pub capabilities: Vec<String>,
  pub size_bytes: u64,
  /// 是否覆盖了已存在的同名插件（true 时已生成 `.bak` 备份）。
  pub replaced_existing: bool,
}

/// 通用分页信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AdminPage<T> {
  pub items: Vec<T>,
  pub total: i64,
  pub page: u32,
  pub page_size: u32,
}

// =============================================================
// 纯逻辑工具（可单元测试）
// =============================================================

pub const ADMIN_PAGE_SIZE: u32 = 50;
pub const MAX_PAGE: u32 = 10_000;

/// 规范化分页参数：未提供时回落到第 0 页；过大值截断
pub fn clamp_page(page: Option<u32>) -> u32 {
  match page {
    Some(p) if p <= MAX_PAGE => p,
    Some(_) => MAX_PAGE,
    None => 0,
  }
}

/// 校验 role 入参是否合法
pub fn validate_role(role: &str) -> Result<(), String> {
  if !is_known_role(role) {
    return Err(format!("不支持的角色: {}", role));
  }
  Ok(())
}

/// 防止管理员把自己踢出管理员队列时，全站没有 admin。
/// 在 `set_user_role` 入口预检使用：
/// - 如果当前操作目标 ≠ 自己 → 总是允许
/// - 如果目标 == 自己 且新 role != admin 且这是最后一个 admin → 拒绝
pub fn check_self_role_change(
  target_user_id: i32,
  operator_user_id: i32,
  new_role: &str,
  other_admin_count: i64,
) -> Result<(), String> {
  if target_user_id == operator_user_id && new_role != ROLE_ADMIN && other_admin_count == 0 {
    return Err("不能取消自己的管理员角色：系统中没有其他管理员".to_string());
  }
  Ok(())
}

/// 生成一个安全的插件文件名：去除目录、强制 `.wasm` 后缀、仅保留 ASCII
/// 字母 / 数字 / `_` / `-`，并统一小写（与 site.json 中的插件命名约定一致）。
///
/// hot reload（Phase 5.1）用：上传的 wasm 必须覆盖到一个可预测的安全路径，
/// 杜绝 `../` 路径穿越与异常字符。
pub fn safe_plugin_filename(original: &str) -> Result<String, String> {
  use std::path::Path;

  let name = Path::new(original)
    .file_name()
    .and_then(|s| s.to_str())
    .ok_or_else(|| "无效的文件名".to_string())?
    .to_ascii_lowercase();

  let stem = name.strip_suffix(".wasm").ok_or_else(|| "插件文件必须以 .wasm 结尾".to_string())?;

  let safe: String =
    stem.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-')).collect();

  if safe.is_empty() {
    return Err("文件名为空或仅含非法字符".to_string());
  }
  if safe.len() > 80 {
    return Err("文件名过长（≤ 80 字符）".to_string());
  }
  Ok(format!("{}.wasm", safe))
}

/// 统一识别插件类型（按文件名后缀启发式判断）
pub fn classify_plugin_kind(filename: &str) -> &'static str {
  let lower = filename.to_lowercase();
  if lower.contains("auth") {
    "auth"
  } else if lower.contains("theme") {
    "theme"
  } else if lower.contains("i18n") {
    "i18n"
  } else {
    "unknown"
  }
}

// =============================================================
// Server-only helpers
// =============================================================

#[cfg(feature = "server")]
fn get_asset_root() -> PathBuf {
  let p = PathBuf::from("assets");
  if p.exists() {
    p
  } else {
    PathBuf::from("../../assets")
  }
}

#[cfg(feature = "server")]
async fn open_db() -> Result<sea_orm::DatabaseConnection, ServerFnError> {
  rustineverything_core::db::get_or_init_pool().await.map_err(|e| ServerFnError::new(e.to_string()))
}

#[cfg(feature = "server")]
fn fmt_dt(dt: chrono::DateTime<chrono::FixedOffset>) -> String {
  dt.format("%Y-%m-%d %H:%M").to_string()
}

#[cfg(feature = "server")]
fn fmt_system_time(t: std::time::SystemTime) -> Option<String> {
  let dt: chrono::DateTime<chrono::Utc> = t.into();
  Some(dt.format("%Y-%m-%d %H:%M").to_string())
}

// =============================================================
// Server functions
// =============================================================

#[post("/api/admin/overview")]
pub async fn admin_overview() -> Result<AdminOverview, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use rustineverything_core::entities::{
      annotation, comment, moderation_queue, topic, topic_reply, user as user_entity,
    };
    use rustineverything_core::session::require_admin;
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

    let _ = require_admin()?;
    let db = open_db().await?;

    let user_count =
      user_entity::Entity::find().count(&db).await.map_err(|e| ServerFnError::new(e.to_string()))?
        as i64;
    let admin_count = user_entity::Entity::find()
      .filter(user_entity::Column::Role.eq(ROLE_ADMIN))
      .count(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))? as i64;
    let comment_count =
      comment::Entity::find().count(&db).await.map_err(|e| ServerFnError::new(e.to_string()))?
        as i64;
    let topic_count =
      topic::Entity::find().count(&db).await.map_err(|e| ServerFnError::new(e.to_string()))? as i64;
    let reply_count =
      topic_reply::Entity::find().count(&db).await.map_err(|e| ServerFnError::new(e.to_string()))?
        as i64;
    let annotation_count =
      annotation::Entity::find().count(&db).await.map_err(|e| ServerFnError::new(e.to_string()))?
        as i64;
    let moderation_pending_count = moderation_queue::Entity::find()
      .filter(moderation_queue::Column::Status.eq("pending"))
      .count(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))? as i64;

    Ok(AdminOverview {
      user_count,
      admin_count,
      comment_count,
      topic_count,
      reply_count,
      annotation_count,
      moderation_pending_count,
    })
  }
  #[cfg(not(feature = "server"))]
  {
    Ok(AdminOverview::default())
  }
}

#[post("/api/admin/users/list")]
pub async fn admin_list_users(page: Option<u32>) -> Result<AdminPage<AdminUserRow>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use rustineverything_core::entities::{user as user_entity, user_identity};
    use rustineverything_core::session::require_admin;
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};

    let _ = require_admin()?;
    let db = open_db().await?;
    let page = clamp_page(page);

    let pager = user_entity::Entity::find()
      .order_by_desc(user_entity::Column::CreatedAt)
      .paginate(&db, ADMIN_PAGE_SIZE as u64);
    let total = pager.num_items().await.map_err(|e| ServerFnError::new(e.to_string()))? as i64;
    let rows =
      pager.fetch_page(page as u64).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    // 一次性查 user_identity，按 user_id 聚合 provider
    let user_ids: Vec<i32> = rows.iter().map(|u| u.id).collect();
    let identities = if user_ids.is_empty() {
      vec![]
    } else {
      user_identity::Entity::find()
        .filter(user_identity::Column::UserId.is_in(user_ids.clone()))
        .all(&db)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };
    let mut providers_map: std::collections::HashMap<i32, Vec<String>> =
      std::collections::HashMap::new();
    for ident in identities {
      providers_map.entry(ident.user_id).or_default().push(ident.provider);
    }

    let items = rows
      .into_iter()
      .map(|u| {
        let providers = providers_map.remove(&u.id).unwrap_or_default();
        AdminUserRow {
          id: u.id,
          nickname: u.nickname,
          avatar_url: u.avatar_url,
          role: u.role,
          providers,
          created_at: fmt_dt(u.created_at),
        }
      })
      .collect();

    Ok(AdminPage { items, total, page, page_size: ADMIN_PAGE_SIZE })
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = page;
    Ok(AdminPage::default())
  }
}

#[post("/api/admin/users/set-role")]
pub async fn admin_set_user_role(
  user_id: i32,
  role: String,
) -> Result<AdminUserRow, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use chrono::Utc;
    use rustineverything_core::entities::user as user_entity;
    use rustineverything_core::session::require_admin;
    use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

    validate_role(&role).map_err(ServerFnError::new)?;
    let operator = require_admin()?;
    let db = open_db().await?;

    // 找到目标用户
    let target = user_entity::Entity::find_by_id(user_id)
      .one(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?
      .ok_or_else(|| ServerFnError::new("用户不存在".to_string()))?;

    // 自我降权保护：如果目标 == 自己 且要把自己踢出 admin → 必须有其他 admin
    let other_admin_count = user_entity::Entity::find()
      .filter(user_entity::Column::Role.eq(ROLE_ADMIN))
      .filter(user_entity::Column::Id.ne(operator.id))
      .count(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))? as i64;
    check_self_role_change(target.id, operator.id, &role, other_admin_count)
      .map_err(ServerFnError::new)?;

    let mut am: user_entity::ActiveModel = target.clone().into();
    am.role = Set(role.clone());
    am.updated_at = Set(Utc::now().fixed_offset());
    let updated = user_entity::Entity::update(am)
      .exec(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(AdminUserRow {
      id: updated.id,
      nickname: updated.nickname,
      avatar_url: updated.avatar_url,
      role: updated.role,
      providers: vec![],
      created_at: fmt_dt(updated.created_at),
    })
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = (user_id, role);
    Err(ServerFnError::new("server only".to_string()))
  }
}

#[post("/api/admin/comments/list")]
pub async fn admin_list_comments(
  page: Option<u32>,
) -> Result<AdminPage<AdminCommentRow>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use rustineverything_core::entities::{comment, user as user_entity};
    use rustineverything_core::session::require_admin;
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};

    let _ = require_admin()?;
    let db = open_db().await?;
    let page = clamp_page(page);

    let pager = comment::Entity::find()
      .order_by_desc(comment::Column::CreatedAt)
      .paginate(&db, ADMIN_PAGE_SIZE as u64);
    let total = pager.num_items().await.map_err(|e| ServerFnError::new(e.to_string()))? as i64;
    let rows =
      pager.fetch_page(page as u64).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut user_ids: Vec<i32> = rows.iter().map(|c| c.user_id).collect();
    user_ids.sort();
    user_ids.dedup();
    let users = if user_ids.is_empty() {
      vec![]
    } else {
      user_entity::Entity::find()
        .filter(user_entity::Column::Id.is_in(user_ids))
        .all(&db)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };
    let user_map: std::collections::HashMap<i32, user_entity::Model> =
      users.into_iter().map(|u| (u.id, u)).collect();

    let items = rows
      .into_iter()
      .map(|c| AdminCommentRow {
        id: c.id,
        blog_id: c.blog_id,
        user_id: c.user_id,
        author: user_map
          .get(&c.user_id)
          .map(|u| u.nickname.clone())
          .unwrap_or_else(|| "已注销".to_string()),
        content: c.content,
        created_at: fmt_dt(c.created_at),
      })
      .collect();

    Ok(AdminPage { items, total, page, page_size: ADMIN_PAGE_SIZE })
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = page;
    Ok(AdminPage::default())
  }
}

#[post("/api/admin/comments/delete")]
pub async fn admin_delete_comment(id: i32) -> Result<(), ServerFnError> {
  #[cfg(feature = "server")]
  {
    use rustineverything_core::entities::comment;
    use rustineverything_core::session::require_admin;
    use sea_orm::EntityTrait;

    let _ = require_admin()?;
    let db = open_db().await?;
    comment::Entity::delete_by_id(id)
      .exec(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = id;
    Err(ServerFnError::new("server only".to_string()))
  }
}

#[post("/api/admin/topics/list")]
pub async fn admin_list_topics(
  page: Option<u32>,
) -> Result<AdminPage<AdminTopicRow>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use rustineverything_core::entities::{topic, user as user_entity};
    use rustineverything_core::session::require_admin;
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};

    let _ = require_admin()?;
    let db = open_db().await?;
    let page = clamp_page(page);

    let pager = topic::Entity::find()
      .order_by_desc(topic::Column::CreatedAt)
      .paginate(&db, ADMIN_PAGE_SIZE as u64);
    let total = pager.num_items().await.map_err(|e| ServerFnError::new(e.to_string()))? as i64;
    let rows =
      pager.fetch_page(page as u64).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut user_ids: Vec<i32> = rows.iter().map(|t| t.user_id).collect();
    user_ids.sort();
    user_ids.dedup();
    let users = if user_ids.is_empty() {
      vec![]
    } else {
      user_entity::Entity::find()
        .filter(user_entity::Column::Id.is_in(user_ids))
        .all(&db)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };
    let user_map: std::collections::HashMap<i32, user_entity::Model> =
      users.into_iter().map(|u| (u.id, u)).collect();

    let items = rows
      .into_iter()
      .map(|t| AdminTopicRow {
        id: t.id,
        title: t.title,
        tag: t.tag,
        user_id: t.user_id,
        author: user_map
          .get(&t.user_id)
          .map(|u| u.nickname.clone())
          .unwrap_or_else(|| "已注销".to_string()),
        reply_count: t.reply_count,
        created_at: fmt_dt(t.created_at),
        last_reply_at: t.last_reply_at.map(fmt_dt),
      })
      .collect();

    Ok(AdminPage { items, total, page, page_size: ADMIN_PAGE_SIZE })
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = page;
    Ok(AdminPage::default())
  }
}

#[post("/api/admin/topics/delete")]
pub async fn admin_delete_topic(id: i32) -> Result<(), ServerFnError> {
  #[cfg(feature = "server")]
  {
    use rustineverything_core::entities::topic;
    use rustineverything_core::session::require_admin;
    use sea_orm::EntityTrait;

    let _ = require_admin()?;
    let db = open_db().await?;
    // topic_replies 通过 ON DELETE CASCADE 自动清理
    topic::Entity::delete_by_id(id)
      .exec(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = id;
    Err(ServerFnError::new("server only".to_string()))
  }
}

#[post("/api/admin/replies/delete")]
pub async fn admin_delete_reply(id: i32) -> Result<(), ServerFnError> {
  #[cfg(feature = "server")]
  {
    use rustineverything_core::entities::{topic, topic_reply};
    use rustineverything_core::session::require_admin;
    use sea_orm::{
      ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, TransactionTrait,
    };

    let _ = require_admin()?;
    let db = open_db().await?;

    // 先查到 reply 以拿到 topic_id
    let reply = topic_reply::Entity::find_by_id(id)
      .one(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?
      .ok_or_else(|| ServerFnError::new("回复不存在".to_string()))?;
    let topic_id = reply.topic_id;

    let txn = db.begin().await.map_err(|e| ServerFnError::new(e.to_string()))?;

    topic_reply::Entity::delete_by_id(id)
      .exec(&txn)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;

    // 重新计算 reply_count
    let new_count = topic_reply::Entity::find()
      .filter(topic_reply::Column::TopicId.eq(topic_id))
      .count(&txn)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))? as i32;
    let target = topic::Entity::find_by_id(topic_id)
      .one(&txn)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;
    if let Some(t) = target {
      let mut am: topic::ActiveModel = t.into();
      am.reply_count = Set(new_count);
      topic::Entity::update(am).exec(&txn).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    }

    txn.commit().await.map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = id;
    Err(ServerFnError::new("server only".to_string()))
  }
}

#[post("/api/admin/plugins/list")]
pub async fn admin_list_plugins() -> Result<Vec<AdminPluginRow>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use rustineverything_core::session::require_admin;
    use rustineverything_core::settings::SiteConfig;

    let _ = require_admin()?;

    let asset_root = get_asset_root();
    let plugin_dir = asset_root.join("plugins");
    let site =
      SiteConfig::from_file(asset_root.join("site.json").to_str().unwrap_or("assets/site.json"))
        .unwrap_or_default();

    let mut rows: Vec<AdminPluginRow> = Vec::new();

    // 1. site.json 中显式配置的 auth providers
    for entry in &site.auth.providers {
      let path = plugin_dir.join(&entry.plugin);
      let (present, size, modified) = stat_plugin(&path);
      let upper = entry.id.to_uppercase();
      let creds_ready = std::env::var(format!("{}_CLIENT_ID", upper)).is_ok()
        && std::env::var(format!("{}_CLIENT_SECRET", upper)).is_ok();
      rows.push(AdminPluginRow {
        kind: "auth".to_string(),
        id: entry.id.clone(),
        filename: entry.plugin.clone(),
        configured: site.auth.enabled,
        credentials_ready: creds_ready,
        present,
        size_bytes: size,
        modified,
      });
    }

    // 2. active_theme
    if !site.active_theme.is_empty() {
      let path = plugin_dir.join(&site.active_theme);
      let (present, size, modified) = stat_plugin(&path);
      rows.push(AdminPluginRow {
        kind: "theme".to_string(),
        id: "active_theme".to_string(),
        filename: site.active_theme.clone(),
        configured: true,
        credentials_ready: true,
        present,
        size_bytes: size,
        modified,
      });
    }

    // 3. 文件系统中其他未在 site.json 列举的 wasm 插件 → 标记 configured=false
    let known: std::collections::HashSet<String> =
      rows.iter().map(|r| r.filename.clone()).collect();
    if let Ok(entries) = std::fs::read_dir(&plugin_dir) {
      for entry in entries.flatten() {
        let name = match entry.file_name().to_str() {
          Some(n) => n.to_string(),
          None => continue,
        };
        if !name.ends_with(".wasm") || known.contains(&name) {
          continue;
        }
        let path = entry.path();
        let (present, size, modified) = stat_plugin(&path);
        rows.push(AdminPluginRow {
          kind: classify_plugin_kind(&name).to_string(),
          id: name.trim_end_matches(".wasm").to_string(),
          filename: name,
          configured: false,
          credentials_ready: true,
          present,
          size_bytes: size,
          modified,
        });
      }
    }

    Ok(rows)
  }
  #[cfg(not(feature = "server"))]
  {
    Ok(vec![])
  }
}

#[cfg(feature = "server")]
fn stat_plugin(path: &std::path::Path) -> (bool, u64, Option<String>) {
  match std::fs::metadata(path) {
    Ok(meta) => {
      let modified = meta.modified().ok().and_then(fmt_system_time);
      (true, meta.len(), modified)
    }
    Err(_) => (false, 0, None),
  }
}

#[post("/api/admin/plugins/reload")]
pub async fn admin_reload_plugins() -> Result<String, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use rustineverything_core::session::require_admin;

    let _ = require_admin()?;
    // 清空共享 PluginManager 的 Module 缓存（i18n / 主题 / auth 下次调用
    // 会重新从磁盘加载），并重建审核流水线（重读 site.json + 插件目录）。
    rustineverything_core::shared_plugin_manager().invalidate_all();
    rustineverything_module_moderation::reload_pipeline();
    tracing::info!("admin: plugin caches invalidated + moderation pipeline reloaded");
    Ok("已清空插件缓存并重建审核流水线（无需重启）".to_string())
  }
  #[cfg(not(feature = "server"))]
  {
    Err(ServerFnError::new("server only".to_string()))
  }
}

/// 允许上传的插件最大字节数（16MB）。当前最大的内置插件（审核 LLM）约 158KB，
/// 留足余量同时阻断异常大的负载。
#[cfg(feature = "server")]
const PLUGIN_MAX_BYTES: usize = 16 * 1024 * 1024;

/// Phase 5.1：admin 上传 wasm 插件 → 沙箱校验 → 备份 → 原子替换 → 失效缓存。
///
/// 安全流程：
/// 1. 仅 admin 可调。
/// 2. 文件名经 [`safe_plugin_filename`] 清洗，杜绝路径穿越。
/// 3. 字节先在临时 [`wasmi`] Store 上编译 + 实例化（[`validate_plugin_bytes`]），
///    再读 `get_manifest` 校验 ABI 版本——不兼容直接拒绝，文件不落盘。
/// 4. 旧文件复制为 `<name>.bak`，新字节写入 `<name>.tmp` 后 `rename` 原子替换。
///    任一 IO 步骤失败都会回滚到备份。
/// 5. 替换成功后失效插件缓存；审核类插件额外触发审核流水线重建。
#[post("/api/admin/plugins/upload")]
pub async fn admin_upload_plugin(
  name: String,
  data_base64: String,
) -> Result<PluginUploadResult, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use base64::Engine as _;
    use rustineverything_core::session::require_admin;
    use rustineverything_core::{
      capabilities, shared_plugin_manager, PluginManifest, SDK_ABI_VERSION,
    };

    let _ = require_admin()?;

    // 1. 安全文件名
    let filename = safe_plugin_filename(&name).map_err(ServerFnError::new)?;

    // 2. 解码 base64（容忍 data:URL 前缀）+ 大小限制
    let base64_str = data_base64.split(',').next_back().unwrap_or(&data_base64);
    let estimated = base64_str.len().saturating_mul(3) / 4;
    if estimated > PLUGIN_MAX_BYTES {
      return Err(ServerFnError::new(format!(
        "插件过大（限制 {} MB）",
        PLUGIN_MAX_BYTES / 1024 / 1024
      )));
    }
    let bytes = base64::engine::general_purpose::STANDARD
      .decode(base64_str)
      .map_err(|e| ServerFnError::new(format!("解码失败: {}", e)))?;
    if bytes.len() > PLUGIN_MAX_BYTES {
      return Err(ServerFnError::new(format!(
        "插件过大（限制 {} MB）",
        PLUGIN_MAX_BYTES / 1024 / 1024
      )));
    }

    // 3. 沙箱结构校验：能编译 + 实例化 + 具备内存管理 ABI
    let manager = shared_plugin_manager();
    manager
      .validate_plugin_bytes(&bytes)
      .map_err(|e| ServerFnError::new(format!("插件校验失败: {}", e)))?;

    // 4. manifest + ABI 版本校验（hot reload 要求新 ABI 插件导出 get_manifest）
    let manifest_json = manager.call_with_string(&bytes, "get_manifest", "").map_err(|e| {
      ServerFnError::new(format!("插件缺少 get_manifest 导出，无法识别 ABI: {}", e))
    })?;
    let manifest: PluginManifest = serde_json::from_str(&manifest_json)
      .map_err(|e| ServerFnError::new(format!("manifest 解析失败: {}", e)))?;
    if !manifest.is_compatible() {
      return Err(ServerFnError::new(format!(
        "插件 ABI 版本不兼容：期望 {}，得到 {}。请用最新 SDK 重新构建。",
        SDK_ABI_VERSION, manifest.abi_version
      )));
    }

    // 5. 落盘：备份 + 原子替换 + 失败回滚
    let plugin_dir = get_asset_root().join("plugins");
    if !plugin_dir.exists() {
      std::fs::create_dir_all(&plugin_dir)
        .map_err(|e| ServerFnError::new(format!("创建插件目录失败: {}", e)))?;
    }
    let target = plugin_dir.join(&filename);
    let replaced_existing = target.exists();
    let backup = plugin_dir.join(format!("{}.bak", filename));
    if replaced_existing {
      std::fs::copy(&target, &backup)
        .map_err(|e| ServerFnError::new(format!("备份旧插件失败: {}", e)))?;
    }

    let tmp = plugin_dir.join(format!("{}.tmp", filename));
    let swap_result = std::fs::write(&tmp, &bytes).and_then(|_| std::fs::rename(&tmp, &target));
    if let Err(e) = swap_result {
      // 回滚：清理残留 tmp，恢复备份
      let _ = std::fs::remove_file(&tmp);
      if replaced_existing {
        let _ = std::fs::rename(&backup, &target);
      }
      return Err(ServerFnError::new(format!("写入插件失败，已回滚: {}", e)));
    }

    // 6. 失效缓存 → 下次调用重新加载新插件
    manager.invalidate(&target);

    // 7. 审核类插件 → 重建审核流水线立即生效
    if manifest.has_capability(capabilities::MODERATION_PROVIDER) {
      rustineverything_module_moderation::reload_pipeline();
    }

    tracing::info!(
        plugin = %manifest.id,
        file = %filename,
        replaced = replaced_existing,
        "admin: plugin uploaded and hot-reloaded"
    );

    Ok(PluginUploadResult {
      filename,
      plugin_id: manifest.id,
      capabilities: manifest.capabilities,
      size_bytes: bytes.len() as u64,
      replaced_existing,
    })
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = (name, data_base64);
    Err(ServerFnError::new("server only".to_string()))
  }
}

// =============================================================
// Phase 4.5：审核队列 (Moderation Queue)
// =============================================================

/// 列出审核队列。filter_status 可选 `"pending"` / `"approved"` / `"rejected"`，
/// 留空表示全部。limit 默认 100（防止数据爆炸）。
#[post("/api/admin/moderation/list")]
pub async fn admin_list_moderation_queue(
  filter_status: Option<String>,
  limit: Option<u64>,
) -> Result<Vec<ModerationQueueRow>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use rustineverything_core::entities::{moderation_queue, user as user_entity};
    use rustineverything_core::session::require_admin;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

    let _ = require_admin()?;
    let db = open_db().await?;

    let mut q = moderation_queue::Entity::find();
    if let Some(status) = filter_status.as_ref().filter(|s| !s.is_empty()) {
      q = q.filter(moderation_queue::Column::Status.eq(status.clone()));
    }
    let rows = q
      .order_by_desc(moderation_queue::Column::CreatedAt)
      .limit(limit.unwrap_or(100).min(500))
      .all(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;

    // 批量查 user / reviewer 昵称
    let mut user_ids: Vec<i32> = rows.iter().filter_map(|r| r.user_id).collect();
    user_ids.extend(rows.iter().filter_map(|r| r.reviewer_user_id));
    user_ids.sort_unstable();
    user_ids.dedup();
    let users = if user_ids.is_empty() {
      Vec::new()
    } else {
      user_entity::Entity::find()
        .filter(user_entity::Column::Id.is_in(user_ids))
        .all(&db)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };
    let lookup = |uid: Option<i32>| {
      uid.and_then(|id| users.iter().find(|u| u.id == id).map(|u| u.nickname.clone()))
    };

    Ok(
      rows
        .into_iter()
        .map(|r| ModerationQueueRow {
          id: r.id,
          kind: r.kind,
          ref_id: r.ref_id,
          ref_path: r.ref_path,
          user_id: r.user_id,
          user_nickname: lookup(r.user_id),
          content: r.content,
          images: r
            .images
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_default(),
          score: r.score,
          label: r.label,
          reason: r.reason,
          status: r.status,
          created_at: fmt_dt(r.created_at),
          reviewer_nickname: lookup(r.reviewer_user_id),
          reviewed_at: r.reviewed_at.map(fmt_dt),
        })
        .collect(),
    )
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = (filter_status, limit);
    Ok(vec![])
  }
}

/// 标记一条记录为已批准（保留内容）。
#[post("/api/admin/moderation/approve")]
pub async fn admin_approve_moderation(id: i64) -> Result<(), ServerFnError> {
  #[cfg(feature = "server")]
  {
    use rustineverything_core::entities::moderation_queue;
    use rustineverything_core::session::require_admin;
    use sea_orm::{ActiveValue::Set, EntityTrait};

    let admin = require_admin()?;
    let db = open_db().await?;
    let now = chrono::Utc::now().fixed_offset();

    let row = moderation_queue::Entity::find_by_id(id)
      .one(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?
      .ok_or_else(|| ServerFnError::new("审核记录不存在".to_string()))?;

    let mut am: moderation_queue::ActiveModel = row.into();
    am.status = Set("approved".to_string());
    am.reviewer_user_id = Set(Some(admin.id));
    am.reviewed_at = Set(Some(now));
    moderation_queue::Entity::update(am)
      .exec(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = id;
    Ok(())
  }
}

/// 拒绝一条记录：把队列标记为 rejected，并删除关联的业务内容（如果 ref_id 在）。
#[post("/api/admin/moderation/reject")]
pub async fn admin_reject_moderation(id: i64) -> Result<(), ServerFnError> {
  #[cfg(feature = "server")]
  {
    use rustineverything_core::entities::{
      annotation, comment, moderation_queue, topic, topic_reply,
    };
    use rustineverything_core::session::require_admin;
    use sea_orm::{ActiveValue::Set, EntityTrait};

    let admin = require_admin()?;
    let db = open_db().await?;
    let now = chrono::Utc::now().fixed_offset();

    let row = moderation_queue::Entity::find_by_id(id)
      .one(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?
      .ok_or_else(|| ServerFnError::new("审核记录不存在".to_string()))?;

    // 尝试按 kind + ref_id 删除业务内容；找不到 ref_id 时跳过删除，仅打标记。
    if let Some(ref_id) = row.ref_id {
      match row.kind.as_str() {
        "comment" => {
          let _ = comment::Entity::delete_by_id(ref_id as i32).exec(&db).await;
        }
        "topic" => {
          let _ = topic::Entity::delete_by_id(ref_id as i32).exec(&db).await;
        }
        "reply" => {
          let _ = topic_reply::Entity::delete_by_id(ref_id as i32).exec(&db).await;
        }
        "annotation" => {
          let _ = annotation::Entity::delete_by_id(ref_id).exec(&db).await;
        }
        _ => {
          tracing::warn!(kind = %row.kind, "unknown moderation kind; queue marked rejected without business delete");
        }
      }
    }

    let mut am: moderation_queue::ActiveModel = row.into();
    am.status = Set("rejected".to_string());
    am.reviewer_user_id = Set(Some(admin.id));
    am.reviewed_at = Set(Some(now));
    moderation_queue::Entity::update(am)
      .exec(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = id;
    Ok(())
  }
}

// =============================================================
// Tests（纯逻辑）
// =============================================================

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn clamp_page_handles_none() {
    assert_eq!(clamp_page(None), 0);
  }

  #[test]
  fn clamp_page_passes_through_valid() {
    assert_eq!(clamp_page(Some(0)), 0);
    assert_eq!(clamp_page(Some(7)), 7);
    assert_eq!(clamp_page(Some(MAX_PAGE)), MAX_PAGE);
  }

  #[test]
  fn clamp_page_caps_at_max() {
    assert_eq!(clamp_page(Some(MAX_PAGE + 1)), MAX_PAGE);
    assert_eq!(clamp_page(Some(u32::MAX)), MAX_PAGE);
  }

  #[test]
  fn validate_role_accepts_known() {
    assert!(validate_role("admin").is_ok());
    assert!(validate_role("member").is_ok());
    assert!(validate_role("guest").is_ok());
  }

  #[test]
  fn validate_role_rejects_unknown() {
    assert!(validate_role("").is_err());
    assert!(validate_role("Admin").is_err());
    assert!(validate_role("super").is_err());
    assert!(validate_role(" admin ").is_err());
  }

  #[test]
  fn check_self_role_change_allows_other_target() {
    assert!(check_self_role_change(2, 1, "member", 0).is_ok());
    assert!(check_self_role_change(2, 1, "guest", 0).is_ok());
  }

  #[test]
  fn check_self_role_change_allows_self_kept_admin() {
    assert!(check_self_role_change(1, 1, "admin", 0).is_ok());
  }

  #[test]
  fn check_self_role_change_blocks_last_admin_self_demotion() {
    assert!(check_self_role_change(1, 1, "member", 0).is_err());
    assert!(check_self_role_change(1, 1, "guest", 0).is_err());
  }

  #[test]
  fn check_self_role_change_allows_with_other_admins() {
    assert!(check_self_role_change(1, 1, "member", 1).is_ok());
    assert!(check_self_role_change(1, 1, "guest", 5).is_ok());
  }

  #[test]
  fn classify_plugin_kind_basic() {
    assert_eq!(classify_plugin_kind("github_auth_plugin.wasm"), "auth");
    assert_eq!(classify_plugin_kind("theme_ocean_plugin.wasm"), "theme");
    assert_eq!(classify_plugin_kind("i18n_fluent_plugin.wasm"), "i18n");
    assert_eq!(classify_plugin_kind("foo.wasm"), "unknown");
  }

  #[test]
  fn classify_plugin_kind_case_insensitive() {
    assert_eq!(classify_plugin_kind("GitHub_Auth.wasm"), "auth");
    assert_eq!(classify_plugin_kind("THEME_neo.wasm"), "theme");
  }

  // ─── Phase 5.1 安全文件名 ───────────────────────────────

  #[test]
  fn safe_plugin_filename_keeps_valid_name() {
    assert_eq!(safe_plugin_filename("theme_ocean_plugin.wasm").unwrap(), "theme_ocean_plugin.wasm");
    assert_eq!(safe_plugin_filename("github-auth.wasm").unwrap(), "github-auth.wasm");
  }

  #[test]
  fn safe_plugin_filename_strips_path_traversal() {
    let name = safe_plugin_filename("../../etc/evil_plugin.wasm").unwrap();
    assert_eq!(name, "evil_plugin.wasm");
    assert!(!name.contains(".."));
    assert!(!name.contains('/'));
  }

  #[test]
  fn safe_plugin_filename_lowercases() {
    assert_eq!(safe_plugin_filename("Theme_Ocean.WASM").unwrap(), "theme_ocean.wasm");
  }

  #[test]
  fn safe_plugin_filename_requires_wasm_ext() {
    assert!(safe_plugin_filename("plugin.exe").is_err());
    assert!(safe_plugin_filename("plugin").is_err());
    assert!(safe_plugin_filename("plugin.wasm.exe").is_err());
  }

  #[test]
  fn safe_plugin_filename_rejects_empty_stem() {
    assert!(safe_plugin_filename(".wasm").is_err());
    assert!(safe_plugin_filename("中文.wasm").is_err());
  }

  #[test]
  fn safe_plugin_filename_strips_unsafe_chars() {
    assert_eq!(safe_plugin_filename("my plugin@v1!.wasm").unwrap(), "mypluginv1.wasm");
  }
}
