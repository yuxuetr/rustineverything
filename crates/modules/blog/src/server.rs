use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Comment {
  pub id: String,
  pub blog_id: String,
  pub content: String,
  pub author: String,
  pub date: String,
}

/// 自动探测资产根目录
fn get_asset_root() -> PathBuf {
    let mut path = PathBuf::from("assets");
    if !path.exists() {
        path = PathBuf::from("../../assets");
    }
    path
}

#[server]
pub async fn get_comments(blog_id: String) -> Result<Vec<Comment>, ServerFnError> {
  let path = get_asset_root().join("data/comments.json");
  if !path.exists() {
    return Ok(Vec::new());
  }

  let content = fs::read_to_string(path)
    .map_err(|e| ServerFnError::new(e.to_string()))?;

  let comments: Vec<Comment> = serde_json::from_str(&content).unwrap_or_default();

  let mut filtered: Vec<Comment> = comments
    .into_iter()
    .filter(|c| c.blog_id == blog_id)
    .collect();
  filtered.reverse();
  Ok(filtered)
}

#[server]
pub async fn post_comment(blog_id: String, content: String) -> Result<Vec<Comment>, ServerFnError> {
  let db_dir = get_asset_root().join("data");
  let db_path = db_dir.join("comments.json");
  
  if !db_dir.exists() {
      fs::create_dir_all(&db_dir).map_err(|e| ServerFnError::new(e.to_string()))?;
  }

  let mut comments: Vec<Comment> = if db_path.exists() {
    let c = fs::read_to_string(&db_path).unwrap_or_else(|_| "[]".to_string());
    serde_json::from_str(&c).unwrap_or_default()
  } else {
    Vec::new()
  };

  let new_comment = Comment {
    id: chrono::Utc::now().timestamp_micros().to_string(),
    blog_id: blog_id.clone(),
    content,
    author: "访客".to_string(),
    date: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
  };

  comments.push(new_comment);

  let json = serde_json::to_string_pretty(&comments)
    .map_err(|e| ServerFnError::new(e.to_string()))?;

  fs::write(db_path, json)
    .map_err(|e| ServerFnError::new(e.to_string()))?;

  get_comments(blog_id).await
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlogPostSummary {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub date: String,
    pub tags: Vec<String>,
}

/// 扫描 assets/posts/ 下所有博客目录，解析 frontmatter 返回列表
#[server]
pub async fn list_blog_posts() -> Result<Vec<BlogPostSummary>, ServerFnError> {
    let posts_dir = get_asset_root().join("posts");
    if !posts_dir.exists() {
        return Ok(Vec::new());
    }

    let mut posts = Vec::new();
    let entries = fs::read_dir(&posts_dir)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() { continue; }

        let slug = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();

        // 查找 index.mdx 或 index.md
        let mdx = path.join("index.mdx");
        let md = path.join("index.md");
        let index_file = if mdx.exists() { mdx } else if md.exists() { md } else { continue };

        let content = fs::read_to_string(&index_file).unwrap_or_default();
        let meta = parse_frontmatter(&content);

        posts.push(BlogPostSummary {
            slug,
            title: meta.title,
            description: meta.description,
            date: meta.date,
            tags: meta.tags,
        });
    }

    // 按日期降序排列
    posts.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(posts)
}

#[derive(Debug, Deserialize, Default)]
struct FrontMatter {
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    tags: Vec<String>,
}

fn parse_frontmatter(content: &str) -> FrontMatter {
    if !content.starts_with("---") {
        return FrontMatter::default();
    }
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return FrontMatter::default();
    }
    serde_yaml::from_str(parts[1]).unwrap_or_default()
}

#[server]
pub async fn get_blog_content(id: String) -> Result<String, ServerFnError> {
    let posts_dir = get_asset_root().join("posts").join(&id);

    // 尝试 index.mdx 和 index.md
    let mdx = posts_dir.join("index.mdx");
    let md = posts_dir.join("index.md");
    let filepath = if mdx.exists() { mdx } else if md.exists() { md } else {
        return Err(ServerFnError::new(format!("文章未找到: {}", id)));
    };

    fs::read_to_string(&filepath)
        .map_err(|e| ServerFnError::new(format!("读取失败: {}", e)))
}
