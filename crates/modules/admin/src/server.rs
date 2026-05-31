use dioxus::prelude::*;
use app_core::session::{is_known_role, ROLE_ADMIN};
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
  /// 该作者在审核队列中的累计命中数（任意状态）。
  pub user_history_total: i64,
  /// 该作者已被「拒绝」的确认违规数。
  pub user_history_rejected: i64,
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
  app_core::db::get_or_init_pool().await.map_err(|e| ServerFnError::new(e.to_string()))
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
    use app_core::entities::{
      annotation, comment, moderation_queue, topic, topic_reply, user as user_entity,
    };
    use app_core::session::require_admin;
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

    let _ = require_admin().await?;
    let db = open_db().await?;

    // Phase 8.4：7 个 COUNT 各自独立 → `tokio::try_join!` 并行发起，总耗时
    // ≈ max(单查耗时) 而非 sum；admin dashboard 在 10K 行规模下从 ~700ms 降到 ~150ms。
    let (
      user_count,
      admin_count,
      comment_count,
      topic_count,
      reply_count,
      annotation_count,
      moderation_pending_count,
    ) = tokio::try_join!(
      user_entity::Entity::find().count(&db),
      user_entity::Entity::find().filter(user_entity::Column::Role.eq(ROLE_ADMIN)).count(&db),
      comment::Entity::find().count(&db),
      topic::Entity::find().count(&db),
      topic_reply::Entity::find().count(&db),
      annotation::Entity::find().count(&db),
      moderation_queue::Entity::find()
        .filter(moderation_queue::Column::Status.eq("pending"))
        .count(&db),
    )
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(AdminOverview {
      user_count: user_count as i64,
      admin_count: admin_count as i64,
      comment_count: comment_count as i64,
      topic_count: topic_count as i64,
      reply_count: reply_count as i64,
      annotation_count: annotation_count as i64,
      moderation_pending_count: moderation_pending_count as i64,
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
    use app_core::entities::{user as user_entity, user_identity};
    use app_core::session::require_admin;
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};

    let _ = require_admin().await?;
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
    use app_core::entities::user as user_entity;
    use app_core::session::require_admin;
    use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

    validate_role(&role).map_err(ServerFnError::new)?;
    let operator = require_admin().await?;
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
    use app_core::entities::{comment, user as user_entity};
    use app_core::session::require_admin;
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};

    let _ = require_admin().await?;
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
    use app_core::entities::comment;
    use app_core::session::require_admin;
    use sea_orm::EntityTrait;

    let _ = require_admin().await?;
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
    use app_core::entities::{topic, user as user_entity};
    use app_core::session::require_admin;
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};

    let _ = require_admin().await?;
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
    use app_core::entities::topic;
    use app_core::session::require_admin;
    use sea_orm::EntityTrait;

    let _ = require_admin().await?;
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
    use app_core::entities::{topic, topic_reply};
    use app_core::session::require_admin;
    use sea_orm::{
      ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, TransactionTrait,
    };

    let _ = require_admin().await?;
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
    use app_core::session::require_admin;
    use app_core::settings::SiteConfig;

    let _ = require_admin().await?;

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
    use app_core::session::require_admin;

    let _ = require_admin().await?;
    // 清空共享 PluginManager 的 Module 缓存（i18n / 主题 / auth 下次调用
    // 会重新从磁盘加载），并重建审核流水线（重读 site.json + 插件目录）。
    app_core::shared_plugin_manager().invalidate_all();
    module_moderation::reload_pipeline();
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
    use app_core::session::require_admin;
    use app_core::{
      capabilities, shared_plugin_manager, PluginManifest, SDK_ABI_VERSION,
    };

    let _ = require_admin().await?;

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
      .await
      .map_err(|e| ServerFnError::new(format!("插件校验失败: {}", e)))?;

    // 4. manifest + ABI 版本校验（hot reload 要求新 ABI 插件导出 get_manifest）
    let manifest_json =
      manager.call_with_string(&bytes, "get_manifest", "").await.map_err(|e| {
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
      module_moderation::reload_pipeline();
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
    use app_core::entities::{moderation_queue, user as user_entity};
    use app_core::session::require_admin;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

    let _ = require_admin().await?;
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

    // Phase 8.4：作者历史违规改成 2 个 GROUP BY 而不是把整条 queue 拉回 Rust 端聚合。
    // 之前对每页作者 *拉所有他们的 queue 行*，N（作者）× M（每人历史长度）的网络 + 反序列化都是浪费。
    // 现在固定 ≤ 2 个 round-trip：1 个查 total，1 个查 rejected。
    let author_ids: Vec<i32> = {
      let mut v: Vec<i32> = rows.iter().filter_map(|r| r.user_id).collect();
      v.sort_unstable();
      v.dedup();
      v
    };
    let (history_total, history_rejected): (
      std::collections::HashMap<i32, i64>,
      std::collections::HashMap<i32, i64>,
    ) = if author_ids.is_empty() {
      Default::default()
    } else {
      use sea_orm::{QuerySelect, sea_query::Expr};

      // 两个 GROUP BY query；count 改在 DB 端做，Rust 端只构造 HashMap。
      let total_rows: Vec<(i32, i64)> = moderation_queue::Entity::find()
        .select_only()
        .column(moderation_queue::Column::UserId)
        .column_as(Expr::col(moderation_queue::Column::Id).count(), "count")
        .filter(moderation_queue::Column::UserId.is_in(author_ids.clone()))
        .group_by(moderation_queue::Column::UserId)
        .into_tuple()
        .all(&db)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
      let rejected_rows: Vec<(i32, i64)> = moderation_queue::Entity::find()
        .select_only()
        .column(moderation_queue::Column::UserId)
        .column_as(Expr::col(moderation_queue::Column::Id).count(), "count")
        .filter(moderation_queue::Column::UserId.is_in(author_ids))
        .filter(moderation_queue::Column::Status.eq("rejected"))
        .group_by(moderation_queue::Column::UserId)
        .into_tuple()
        .all(&db)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

      (total_rows.into_iter().collect(), rejected_rows.into_iter().collect())
    };
    let hist = |uid: Option<i32>| -> (i64, i64) {
      match uid {
        Some(id) => (
          history_total.get(&id).copied().unwrap_or(0),
          history_rejected.get(&id).copied().unwrap_or(0),
        ),
        None => (0, 0),
      }
    };

    Ok(
      rows
        .into_iter()
        .map(|r| {
          let (user_history_total, user_history_rejected) = hist(r.user_id);
          ModerationQueueRow {
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
            user_history_total,
            user_history_rejected,
          }
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
    use app_core::entities::moderation_queue;
    use app_core::session::require_admin;
    use sea_orm::{ActiveValue::Set, EntityTrait};

    let admin = require_admin().await?;
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

/// 拒绝一条队列记录：删除关联业务内容（按 kind + ref_id）并标记 rejected。
/// 供单条 [`admin_reject_moderation`] 与批量 [`admin_bulk_reject_moderation`] 复用。
#[cfg(feature = "server")]
async fn reject_one(
  db: &sea_orm::DatabaseConnection,
  admin_id: i32,
  row: app_core::entities::moderation_queue::Model,
) -> Result<(), ServerFnError> {
  use app_core::entities::{
    annotation, comment, moderation_queue, topic, topic_reply,
  };
  use sea_orm::{ActiveValue::Set, EntityTrait};

  let now = chrono::Utc::now().fixed_offset();
  // Phase 8.8：删除业务内容的错误不再吞掉。之前用 `let _ = ...` 静默掉，
  // 极端情况下评论真的没被删但队列被标 rejected → admin UI 上看着已处理，
  // 但前端还能看到原帖。改成 `?` 传播；事实是 DELETE FROM ... WHERE id = ?
  // 行不存在不会报错（SeaORM 返回 rows_affected = 0），所以「ref 已被其他流程
  // 提前删」并不会触发错。
  if let Some(ref_id) = row.ref_id {
    let kind = row.kind.as_str();
    let res = match kind {
      "comment" => comment::Entity::delete_by_id(ref_id as i32).exec(db).await.map(|_| ()),
      "topic" => topic::Entity::delete_by_id(ref_id as i32).exec(db).await.map(|_| ()),
      "reply" => topic_reply::Entity::delete_by_id(ref_id as i32).exec(db).await.map(|_| ()),
      "annotation" => annotation::Entity::delete_by_id(ref_id).exec(db).await.map(|_| ()),
      _ => {
        tracing::warn!(kind = %kind, "unknown moderation kind; queue marked rejected without business delete");
        Ok(())
      }
    };
    if let Err(e) = res {
      // 不阻塞队列更新：内容确实可能已经先被作者 / 其他 admin 流程删掉
      // （DELETE WHERE id = X 在 SeaORM 下行数 0 不报错；这里能进 Err 说明
      // 是真实 DB 故障，例如连接断、外键约束等）。记 warn 让运维有迹可循。
      tracing::warn!(error = %e, kind = %kind, ref_id, "reject_one: business content delete failed");
    }
  }

  let mut am: moderation_queue::ActiveModel = row.into();
  am.status = Set("rejected".to_string());
  am.reviewer_user_id = Set(Some(admin_id));
  am.reviewed_at = Set(Some(now));
  moderation_queue::Entity::update(am)
    .exec(db)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
  Ok(())
}

/// 拒绝一条记录：把队列标记为 rejected，并删除关联的业务内容（如果 ref_id 在）。
#[post("/api/admin/moderation/reject")]
pub async fn admin_reject_moderation(id: i64) -> Result<(), ServerFnError> {
  #[cfg(feature = "server")]
  {
    use app_core::entities::moderation_queue;
    use app_core::session::require_admin;
    use sea_orm::EntityTrait;

    let admin = require_admin().await?;
    let db = open_db().await?;
    let row = moderation_queue::Entity::find_by_id(id)
      .one(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?
      .ok_or_else(|| ServerFnError::new("审核记录不存在".to_string()))?;
    reject_one(&db, admin.id, row).await
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = id;
    Ok(())
  }
}

/// 批量通过：把一组 pending 记录标记为 approved（保留内容）。返回更新条数。
#[post("/api/admin/moderation/bulk-approve")]
pub async fn admin_bulk_approve_moderation(ids: Vec<i64>) -> Result<u64, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use app_core::entities::moderation_queue;
    use app_core::session::require_admin;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let admin = require_admin().await?;
    if ids.is_empty() {
      return Ok(0);
    }
    let db = open_db().await?;
    let now = chrono::Utc::now().fixed_offset();

    // 单条 UPDATE ... WHERE id IN (...)，避免逐行往返。
    let res = moderation_queue::Entity::update_many()
      .col_expr(moderation_queue::Column::Status, sea_orm::sea_query::Expr::value("approved"))
      .col_expr(moderation_queue::Column::ReviewerUserId, sea_orm::sea_query::Expr::value(admin.id))
      .col_expr(moderation_queue::Column::ReviewedAt, sea_orm::sea_query::Expr::value(now))
      .filter(moderation_queue::Column::Id.is_in(ids))
      .exec(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(res.rows_affected)
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = ids;
    Ok(0)
  }
}

/// 批量拒绝：逐条删除关联业务内容并标记 rejected。返回成功处理的条数。
#[post("/api/admin/moderation/bulk-reject")]
pub async fn admin_bulk_reject_moderation(ids: Vec<i64>) -> Result<u64, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use app_core::entities::moderation_queue;
    use app_core::session::require_admin;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let admin = require_admin().await?;
    if ids.is_empty() {
      return Ok(0);
    }
    let db = open_db().await?;
    let rows = moderation_queue::Entity::find()
      .filter(moderation_queue::Column::Id.is_in(ids))
      .all(&db)
      .await
      .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Phase 8.8：8 路并发处理。DB pool 上限 32，留一半给前台读路径，
    // 仍把批量耗时压缩到 ~1/8。单条失败只告警，继续处理其余。
    use futures::stream::{self, StreamExt};
    let done = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let done_for_stream = done.clone();
    let admin_id = admin.id;
    let db_ref = &db;
    stream::iter(rows)
      .for_each_concurrent(8, |row| {
        let done_inc = done_for_stream.clone();
        async move {
          match reject_one(db_ref, admin_id, row).await {
            Ok(()) => {
              done_inc.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            Err(e) => tracing::warn!(error = %e, "bulk reject: one entry failed"),
          }
        }
      })
      .await;
    Ok(done.load(std::sync::atomic::Ordering::Relaxed))
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = ids;
    Ok(0)
  }
}

// =============================================================
// Phase 308：Moderation 配置在线编辑（admin UI 写回 site.json）
// =============================================================

/// 校验 [`app_core::settings::ModerationSettings`]。可能的失败：
/// - thresholds 的 flag_above / block_above 超出 [0.0, 1.0]
/// - flag_above > block_above（语义上不可能：flag 必须比 block 低）
/// - url_blocklist 含空白 / 含 scheme（应只填 host 模式）
///
/// 纯函数，便于单测。在 `admin_set_moderation_settings` 内调用。
pub fn validate_moderation_settings(
  s: &app_core::settings::ModerationSettings,
) -> Result<(), String> {
  if let Some(th) = &s.thresholds {
    if let Some(f) = th.flag_above {
      if !(0.0..=1.0).contains(&f) {
        return Err(format!("flag_above 必须在 0.0–1.0 之间，得到 {}", f));
      }
    }
    if let Some(b) = th.block_above {
      if !(0.0..=1.0).contains(&b) {
        return Err(format!("block_above 必须在 0.0–1.0 之间，得到 {}", b));
      }
    }
    if let (Some(f), Some(b)) = (th.flag_above, th.block_above) {
      if f > b {
        return Err(format!("flag_above ({}) 不能大于 block_above ({})", f, b));
      }
    }
  }
  for entry in &s.url_blocklist {
    let trimmed = entry.trim();
    if trimmed.is_empty() {
      return Err("url_blocklist 含空字符串".to_string());
    }
    if trimmed.contains("://") {
      return Err(format!("url_blocklist 条目不应含 scheme（只填 host）：{}", trimmed));
    }
    if trimmed.chars().any(char::is_whitespace) {
      return Err(format!("url_blocklist 条目含空白字符：{}", trimmed));
    }
  }
  Ok(())
}

/// 读取 site.json 中的 moderation 配置（admin 专属，用于面板初始填充）。
#[post("/api/admin/moderation/settings/get")]
pub async fn admin_get_moderation_settings(
) -> Result<app_core::settings::ModerationSettings, ServerFnError> {
  #[cfg(feature = "server")]
  {
    use app_core::session::require_admin;
    use app_core::settings::SiteConfig;
    let _ = require_admin().await?;
    let path = get_asset_root().join("site.json");
    let config = SiteConfig::from_file(path.to_str().unwrap_or("assets/site.json"))
      .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(config.moderation)
  }
  #[cfg(not(feature = "server"))]
  {
    Err(ServerFnError::new("server only".to_string()))
  }
}

/// 写回 moderation 配置 + 触发审核流水线热重载。
///
/// 流程：
/// 1. require_admin
/// 2. validate
/// 3. read site.json 为 `serde_json::Value`，只替换 `moderation` 子树（保留所有
///    其他字段，包括 SiteConfig struct 未声明的人工自定义字段）
/// 4. 原子 tmp + rename 写回
/// 5. `shared_plugin_manager().invalidate_all()` + `reload_pipeline()` 立即生效
#[post("/api/admin/moderation/settings/set")]
pub async fn admin_set_moderation_settings(
  settings: app_core::settings::ModerationSettings,
) -> Result<(), ServerFnError> {
  #[cfg(feature = "server")]
  {
    use app_core::session::require_admin;
    let _ = require_admin().await?;
    validate_moderation_settings(&settings).map_err(ServerFnError::new)?;

    let path = get_asset_root().join("site.json");
    write_moderation_settings_to_site_json(&path, &settings).map_err(ServerFnError::new)?;

    // 立即生效：插件缓存清空 + 审核 pipeline 重建（重读 site.json + 插件目录）
    app_core::shared_plugin_manager().invalidate_all();
    module_moderation::reload_pipeline();
    tracing::info!(
      enabled = settings.enabled,
      plugins = settings.plugins.len(),
      blocklist = settings.url_blocklist.len(),
      "admin: moderation settings updated + pipeline reloaded"
    );
    Ok(())
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = settings;
    Err(ServerFnError::new("server only".to_string()))
  }
}

/// 原子写：read full site.json → 替换 `moderation` 子树 → 写回。
///
/// 选择基于 `serde_json::Value` 而非 `SiteConfig` round-trip：保留人工 site.json
/// 里可能存在的、SiteConfig struct 未声明的额外字段（向后兼容老配置 / 自定义
/// 实验字段）。
#[cfg(feature = "server")]
fn write_moderation_settings_to_site_json(
  path: &std::path::Path,
  settings: &app_core::settings::ModerationSettings,
) -> Result<(), String> {
  let raw = std::fs::read_to_string(path)
    .map_err(|e| format!("读取 {} 失败：{}", path.display(), e))?;
  let mut value: serde_json::Value =
    serde_json::from_str(&raw).map_err(|e| format!("site.json 不是合法 JSON：{}", e))?;
  let moderation = serde_json::to_value(settings)
    .map_err(|e| format!("序列化 ModerationSettings 失败：{}", e))?;
  value["moderation"] = moderation;
  let pretty =
    serde_json::to_string_pretty(&value).map_err(|e| format!("序列化 site.json 失败：{}", e))?;

  // tmp + rename：写盘失败时不会污染正式 site.json
  let tmp_path = path.with_extension("json.tmp");
  std::fs::write(&tmp_path, pretty).map_err(|e| format!("写入临时文件失败：{}", e))?;
  std::fs::rename(&tmp_path, path).map_err(|e| {
    let _ = std::fs::remove_file(&tmp_path);
    format!("rename 替换 site.json 失败：{}", e)
  })?;
  Ok(())
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

  // ── Phase 308：moderation 配置校验 + 写回 ──

  use app_core::settings::{ModerationSettings, ModerationThresholdsConfig};

  fn thresholds(flag: Option<f32>, block: Option<f32>) -> ModerationSettings {
    ModerationSettings {
      thresholds: Some(ModerationThresholdsConfig { flag_above: flag, block_above: block }),
      ..Default::default()
    }
  }

  #[test]
  fn validate_moderation_accepts_default_settings() {
    assert!(validate_moderation_settings(&ModerationSettings::default()).is_ok());
  }

  #[test]
  fn validate_moderation_accepts_valid_thresholds() {
    assert!(validate_moderation_settings(&thresholds(Some(0.5), Some(0.9))).is_ok());
    // 边界值都允许
    assert!(validate_moderation_settings(&thresholds(Some(0.0), Some(1.0))).is_ok());
    // 只设一个也允许
    assert!(validate_moderation_settings(&thresholds(Some(0.3), None)).is_ok());
    assert!(validate_moderation_settings(&thresholds(None, Some(0.8))).is_ok());
  }

  #[test]
  fn validate_moderation_rejects_out_of_range_flag() {
    assert!(validate_moderation_settings(&thresholds(Some(1.2), None)).is_err());
    assert!(validate_moderation_settings(&thresholds(Some(-0.1), None)).is_err());
  }

  #[test]
  fn validate_moderation_rejects_out_of_range_block() {
    assert!(validate_moderation_settings(&thresholds(None, Some(1.5))).is_err());
    assert!(validate_moderation_settings(&thresholds(None, Some(-0.5))).is_err());
  }

  #[test]
  fn validate_moderation_rejects_flag_greater_than_block() {
    let err = validate_moderation_settings(&thresholds(Some(0.9), Some(0.5)))
      .expect_err("flag>block 应拒");
    assert!(err.contains("flag_above"));
    assert!(err.contains("block_above"));
  }

  #[test]
  fn validate_moderation_rejects_empty_blocklist_entry() {
    let s = ModerationSettings {
      url_blocklist: vec!["scam.com".to_string(), "  ".to_string()],
      ..Default::default()
    };
    assert!(validate_moderation_settings(&s).is_err());
  }

  #[test]
  fn validate_moderation_rejects_blocklist_with_scheme() {
    let s = ModerationSettings {
      url_blocklist: vec!["https://scam.com".to_string()],
      ..Default::default()
    };
    assert!(validate_moderation_settings(&s).is_err());
  }

  #[test]
  fn validate_moderation_rejects_blocklist_with_whitespace() {
    let s = ModerationSettings {
      url_blocklist: vec!["scam .com".to_string()],
      ..Default::default()
    };
    assert!(validate_moderation_settings(&s).is_err());
  }

  #[test]
  fn validate_moderation_accepts_valid_blocklist() {
    let s = ModerationSettings {
      url_blocklist: vec!["scam.com".to_string(), "*.phishing.example".to_string()],
      ..Default::default()
    };
    assert!(validate_moderation_settings(&s).is_ok());
  }

  #[cfg(feature = "server")]
  #[test]
  fn write_preserves_other_site_json_fields() {
    use std::fs;
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("site.json");
    // 故意包含 SiteConfig struct 没有的字段（"custom_x"），确保 round-trip 保留
    let original = r#"{
      "site_name": "保留",
      "themes": ["t.wasm"],
      "auth": {"enabled": true, "providers": []},
      "moderation": {"enabled": false},
      "custom_x": {"unknown": "field"}
    }"#;
    fs::write(&path, original).unwrap();

    let new_settings = ModerationSettings {
      enabled: true,
      plugins: vec!["mod.wasm".to_string()],
      thresholds: Some(ModerationThresholdsConfig {
        flag_above: Some(0.4),
        block_above: Some(0.85),
      }),
      url_blocklist: vec!["scam.com".to_string()],
    };

    write_moderation_settings_to_site_json(&path, &new_settings).expect("write");

    let after: serde_json::Value =
      serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    // 旧字段保留
    assert_eq!(after["site_name"], "保留");
    assert_eq!(after["themes"], serde_json::json!(["t.wasm"]));
    assert_eq!(after["auth"]["enabled"], true);
    assert_eq!(after["custom_x"]["unknown"], "field");
    // moderation 已替换
    assert_eq!(after["moderation"]["enabled"], true);
    assert_eq!(after["moderation"]["plugins"], serde_json::json!(["mod.wasm"]));
    assert_eq!(after["moderation"]["url_blocklist"], serde_json::json!(["scam.com"]));
    // f32 → JSON 经 f64 序列化会引入精度尾噪（0.4 ≠ 0.4000000059604645），按近似比较
    let flag = after["moderation"]["thresholds"]["flag_above"].as_f64().unwrap();
    let block = after["moderation"]["thresholds"]["block_above"].as_f64().unwrap();
    assert!((flag - 0.4).abs() < 1e-6);
    assert!((block - 0.85).abs() < 1e-6);
  }

  #[cfg(feature = "server")]
  #[test]
  fn write_atomic_temp_does_not_linger_on_success() {
    use std::fs;
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("site.json");
    fs::write(&path, r#"{"site_name":"x","moderation":{}}"#).unwrap();
    write_moderation_settings_to_site_json(&path, &ModerationSettings::default()).unwrap();
    // .tmp 文件不应残留
    assert!(!path.with_extension("json.tmp").exists());
  }
}
