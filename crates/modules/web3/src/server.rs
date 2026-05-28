#[allow(unused_imports)]
use std::fs;
#[allow(unused_imports)]
use std::path::PathBuf;
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(unused_imports)]
use crate::text::{sort_by_date_desc, DatedArticle, BOARD_ID};

#[allow(dead_code)]
fn get_asset_root() -> PathBuf {
    let mut path = PathBuf::from("assets");
    if !path.exists() {
        path = PathBuf::from("../../assets");
    }
    path
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArticleSummary {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub date: String,
    pub subtopic: String,
    pub tags: Vec<String>,
}

impl DatedArticle for ArticleSummary {
    fn date(&self) -> &str {
        &self.date
    }
    fn title(&self) -> &str {
        &self.title
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
struct FrontMatter {
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    subtopic: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[allow(dead_code)]
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

/// 扫描 `assets/topics/ai/*/index.md`，按日期降序返回文章摘要。
#[server]
pub async fn list_web3_articles() -> Result<Vec<ArticleSummary>, ServerFnError> {
    let dir = get_asset_root().join("topics").join(BOARD_ID);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut items = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| ServerFnError::new(e.to_string()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let slug = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();

        let mdx = path.join("index.mdx");
        let md = path.join("index.md");
        let index_file = if mdx.exists() {
            mdx
        } else if md.exists() {
            md
        } else {
            continue;
        };

        let content = fs::read_to_string(&index_file).unwrap_or_default();
        let meta = parse_frontmatter(&content);
        items.push(ArticleSummary {
            slug,
            title: meta.title,
            description: meta.description,
            date: meta.date,
            subtopic: meta.subtopic,
            tags: meta.tags,
        });
    }

    sort_by_date_desc(&mut items);
    Ok(items)
}

/// 读取单篇文章的原始 markdown（含 frontmatter）。
#[server]
pub async fn get_web3_article(slug: String) -> Result<String, ServerFnError> {
    if slug.is_empty()
        || slug
            .chars()
            .any(|c| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
    {
        return Err(ServerFnError::new("无效的文章标识".to_string()));
    }
    let dir = get_asset_root().join("topics").join(BOARD_ID).join(&slug);
    let mdx = dir.join("index.mdx");
    let md = dir.join("index.md");
    let filepath = if mdx.exists() {
        mdx
    } else if md.exists() {
        md
    } else {
        return Err(ServerFnError::new(format!("文章未找到: {}", slug)));
    };
    fs::read_to_string(&filepath).map_err(|e| ServerFnError::new(format!("读取失败: {}", e)))
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_parses_fields() {
        let md = "---\ntitle: Candle 推理\ndescription: 本地大模型\ndate: 2026-05-01\nsubtopic: llm\ntags: [candle, llm]\n---\n# body";
        let fm = parse_frontmatter(md);
        assert_eq!(fm.title, "Candle 推理");
        assert_eq!(fm.subtopic, "llm");
        assert_eq!(fm.tags, vec!["candle".to_string(), "llm".to_string()]);
    }

    #[test]
    fn frontmatter_missing_is_default() {
        let fm = parse_frontmatter("no frontmatter here");
        assert!(fm.title.is_empty());
        assert!(fm.tags.is_empty());
    }
}
