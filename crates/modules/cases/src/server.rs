use crate::text::{
  compare_cases, humanize_slug, is_known_tag, matches_query, normalize_category, normalize_tags,
  CaseSortable,
};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use std::fs;
#[allow(unused_imports)]
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaseSummary {
  pub slug: String,
  pub name: String,
  pub description: String,
  pub category: String,
  pub tags: Vec<String>,
  pub repo: String,
  pub website: Option<String>,
  pub author: String,
  pub author_url: Option<String>,
  pub language: String,
  pub stars: i64,
  pub favorite: bool,
  pub date_added: String,
  pub cover_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Case {
  pub slug: String,
  pub name: String,
  pub description: String,
  pub category: String,
  pub tags: Vec<String>,
  pub repo: String,
  pub website: Option<String>,
  pub author: String,
  pub author_url: Option<String>,
  pub language: String,
  pub stars: i64,
  pub favorite: bool,
  pub date_added: String,
  pub cover_url: Option<String>,
  pub readme_md: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TagSummary {
  pub tag: String,
  pub count: usize,
  pub known: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CategorySummary {
  pub category: String,
  pub count: usize,
}

impl CaseSortable for CaseSummary {
  fn favorite(&self) -> bool {
    self.favorite
  }

  fn stars(&self) -> i64 {
    self.stars
  }

  fn date_added(&self) -> &str {
    &self.date_added
  }

  fn name(&self) -> &str {
    &self.name
  }
}

impl From<&Case> for CaseSummary {
  fn from(case: &Case) -> Self {
    Self {
      slug: case.slug.clone(),
      name: case.name.clone(),
      description: case.description.clone(),
      category: case.category.clone(),
      tags: case.tags.clone(),
      repo: case.repo.clone(),
      website: case.website.clone(),
      author: case.author.clone(),
      author_url: case.author_url.clone(),
      language: case.language.clone(),
      stars: case.stars,
      favorite: case.favorite,
      date_added: case.date_added.clone(),
      cover_url: case.cover_url.clone(),
    }
  }
}

#[cfg(feature = "server")]
#[derive(Debug, Deserialize, Default)]
struct CaseMeta {
  #[serde(default)]
  name: String,
  #[serde(default)]
  slug: String,
  #[serde(default)]
  description: String,
  #[serde(default)]
  category: String,
  #[serde(default)]
  tags: Vec<String>,
  #[serde(default)]
  repo: String,
  #[serde(default)]
  website: Option<String>,
  #[serde(default)]
  author: String,
  #[serde(default)]
  author_url: Option<String>,
  #[serde(default)]
  language: String,
  #[serde(default)]
  stars: i64,
  #[serde(default)]
  favorite: bool,
  #[serde(default)]
  date_added: String,
}

#[cfg(feature = "server")]
pub fn get_cases_root() -> PathBuf {
  let p = PathBuf::from("assets/cases");
  if p.exists() {
    p
  } else {
    PathBuf::from("../../assets/cases")
  }
}

#[cfg(feature = "server")]
fn skip_entry(name: &str) -> bool {
  name.starts_with('_') || name.starts_with('.')
}

#[cfg(feature = "server")]
fn normalize_language(raw: &str) -> String {
  match raw.trim().to_ascii_lowercase().as_str() {
    "rust" => "rust".to_string(),
    "wasm" => "wasm".to_string(),
    "mixed" => "mixed".to_string(),
    _ => "rust".to_string(),
  }
}

#[cfg(feature = "server")]
fn first_non_empty(value: String, fallback: String) -> String {
  if value.trim().is_empty() {
    fallback
  } else {
    value
  }
}

#[cfg(feature = "server")]
pub fn find_cover_image(dir: &Path) -> Option<String> {
  for name in ["cover.webp", "cover.jpg", "cover.png"] {
    if dir.join(name).is_file() {
      return Some(name.to_string());
    }
  }
  None
}

#[cfg(feature = "server")]
fn read_readme(dir: &Path, slug: &str) -> Option<String> {
  for name in ["README.md", "readme.md", "README.mdx", "readme.mdx"] {
    let path = dir.join(name);
    if path.is_file() {
      let raw = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(_) => return None,
      };
      return Some(rewrite_image_urls(&raw, &format!("/cases/{slug}")));
    }
  }
  None
}

#[cfg(feature = "server")]
pub fn rewrite_image_urls(markdown: &str, base: &str) -> String {
  let mut out = String::with_capacity(markdown.len());
  let bytes = markdown.as_bytes();
  let mut i = 0;
  while i < bytes.len() {
    if i + 1 < bytes.len() && bytes[i] == b'!' && bytes[i + 1] == b'[' {
      let mut j = i + 2;
      while j < bytes.len() && bytes[j] != b']' {
        j += 1;
      }
      if j + 1 < bytes.len() && bytes[j + 1] == b'(' {
        let url_start = j + 2;
        let mut k = url_start;
        while k < bytes.len() && bytes[k] != b')' {
          k += 1;
        }
        if k < bytes.len() {
          out.push_str(&markdown[i..url_start]);
          let raw_url = markdown[url_start..k].trim();
          out.push_str(&rewrite_one_url(raw_url, base));
          out.push(')');
          i = k + 1;
          continue;
        }
      }
    }
    let mut next = i + 1;
    while next < bytes.len() && !markdown.is_char_boundary(next) {
      next += 1;
    }
    out.push_str(&markdown[i..next]);
    i = next;
  }
  out
}

#[cfg(feature = "server")]
fn rewrite_one_url(url: &str, base: &str) -> String {
  if url.is_empty()
    || url.starts_with("http://")
    || url.starts_with("https://")
    || url.starts_with('/')
  {
    return url.to_string();
  }
  let stripped = match url.strip_prefix("./") {
    Some(rest) => rest,
    None => url,
  };
  format!("{}/{}", base.trim_end_matches('/'), stripped)
}

#[cfg(feature = "server")]
pub fn read_case_from_dir(dir: &Path) -> Result<Case, String> {
  let dir_slug = dir
    .file_name()
    .and_then(|name| name.to_str())
    .ok_or_else(|| "invalid case directory name".to_string())?
    .to_string();
  let yaml_path = dir.join("case.yaml");
  let raw = fs::read_to_string(&yaml_path).map_err(|e| e.to_string())?;
  let meta: CaseMeta = serde_yaml::from_str(&raw).map_err(|e| e.to_string())?;
  let slug = first_non_empty(meta.slug, dir_slug);
  let category = match normalize_category(&meta.category) {
    Some(value) => value,
    None => "tool".to_string(),
  };
  let tags = normalize_tags(&meta.tags);
  let cover_url = find_cover_image(dir).map(|name| format!("/cases/{slug}/{name}"));
  let readme_md = read_readme(dir, &slug);
  Ok(Case {
    slug: slug.clone(),
    name: first_non_empty(meta.name, humanize_slug(&slug)),
    description: meta.description,
    category,
    tags,
    repo: meta.repo,
    website: meta.website.filter(|v| !v.trim().is_empty()),
    author: first_non_empty(meta.author, "Unknown".to_string()),
    author_url: meta.author_url.filter(|v| !v.trim().is_empty()),
    language: normalize_language(&meta.language),
    stars: meta.stars.max(0),
    favorite: meta.favorite,
    date_added: first_non_empty(meta.date_added, "1970-01-01".to_string()),
    cover_url,
    readme_md,
  })
}

#[cfg(feature = "server")]
pub fn scan_cases_from_root(root: &Path) -> Vec<Case> {
  let entries = match fs::read_dir(root) {
    Ok(entries) => entries,
    Err(_) => return vec![],
  };
  let mut cases = Vec::new();
  for entry in entries.flatten() {
    let path = entry.path();
    if !path.is_dir() {
      continue;
    }
    let name = match path.file_name().and_then(|value| value.to_str()) {
      Some(value) => value,
      None => continue,
    };
    if skip_entry(name) {
      continue;
    }
    match read_case_from_dir(&path) {
      Ok(case) => cases.push(case),
      Err(e) => {
        tracing::warn!(path = %path.display(), error = %e, "cases: skipping unreadable entry")
      }
    }
  }
  cases.sort_by(|a, b| compare_cases(&CaseSummary::from(a), &CaseSummary::from(b)));
  cases
}

#[cfg(feature = "server")]
pub fn scan_cases() -> Vec<Case> {
  scan_cases_from_root(&get_cases_root())
}

fn filter_summaries(
  mut cases: Vec<CaseSummary>,
  tags: Option<Vec<String>>,
  category: Option<String>,
  q: Option<String>,
) -> Vec<CaseSummary> {
  let normalized_tags =
    tags.map(|values| normalize_tags(&values)).filter(|values| !values.is_empty());
  let normalized_category = category.and_then(|value| normalize_category(&value));
  let query = q.unwrap_or_default();
  cases.retain(|case| {
    let tag_match = match &normalized_tags {
      Some(selected) => selected.iter().any(|tag| case.tags.iter().any(|t| t == tag)),
      None => true,
    };
    let category_match = match &normalized_category {
      Some(selected) => &case.category == selected,
      None => true,
    };
    tag_match
      && category_match
      && matches_query(&case.name, &case.description, &case.category, &case.tags, &query)
  });
  cases.sort_by(compare_cases);
  cases
}

#[post("/api/cases/list")]
pub async fn list_cases(
  tags: Option<Vec<String>>,
  category: Option<String>,
  q: Option<String>,
) -> Result<Vec<CaseSummary>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    let summaries = scan_cases().iter().map(CaseSummary::from).collect();
    Ok(filter_summaries(summaries, tags, category, q))
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = (tags, category, q);
    Ok(vec![])
  }
}

#[post("/api/cases/tags")]
pub async fn list_case_tags() -> Result<Vec<TagSummary>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for case in scan_cases() {
      for tag in case.tags {
        let next = counts.get(&tag).copied().map_or(1, |count| count + 1);
        counts.insert(tag, next);
      }
    }
    Ok(
      counts
        .into_iter()
        .map(|(tag, count)| TagSummary { known: is_known_tag(&tag), tag, count })
        .collect(),
    )
  }
  #[cfg(not(feature = "server"))]
  {
    Ok(vec![])
  }
}

#[post("/api/cases/categories")]
pub async fn list_case_categories() -> Result<Vec<CategorySummary>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for case in scan_cases() {
      let next = counts.get(&case.category).copied().map_or(1, |count| count + 1);
      counts.insert(case.category, next);
    }
    Ok(counts.into_iter().map(|(category, count)| CategorySummary { category, count }).collect())
  }
  #[cfg(not(feature = "server"))]
  {
    Ok(vec![])
  }
}

#[post("/api/cases/get")]
pub async fn get_case(slug: String) -> Result<Option<Case>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    let requested = slug.trim();
    if requested.is_empty() {
      return Ok(None);
    }
    Ok(scan_cases().into_iter().find(|case| case.slug == requested))
  }
  #[cfg(not(feature = "server"))]
  {
    let _ = slug;
    Ok(None)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
      if let Err(e) = fs::create_dir_all(parent) {
        panic!("failed to create test dir: {}", e);
      }
    }
    if let Err(e) = fs::write(path, content) {
      panic!("failed to write test file: {}", e);
    }
  }

  fn temp_root() -> tempfile::TempDir {
    match tempfile::tempdir() {
      Ok(dir) => dir,
      Err(e) => panic!("failed to create tempdir: {}", e),
    }
  }

  #[test]
  fn find_cover_image_prefers_webp() {
    let tmp = temp_root();
    write_file(&tmp.path().join("cover.png"), "");
    write_file(&tmp.path().join("cover.jpg"), "");
    write_file(&tmp.path().join("cover.webp"), "");
    assert_eq!(find_cover_image(tmp.path()).as_deref(), Some("cover.webp"));
  }

  #[test]
  fn read_case_defaults_slug_name_category_and_language() {
    let tmp = temp_root();
    let dir = tmp.path().join("axum-realworld");
    write_file(
            &dir.join("case.yaml"),
            "description: API\nrepo: https://github.com/example/axum-realworld\ntags: [Axum, Sea ORM]\nstars: 5\n",
        );
    let case = match read_case_from_dir(&dir) {
      Ok(case) => case,
      Err(e) => panic!("failed to read case: {}", e),
    };
    assert_eq!(case.slug, "axum-realworld");
    assert_eq!(case.name, "Axum Realworld");
    assert_eq!(case.category, "tool");
    assert_eq!(case.language, "rust");
    assert_eq!(case.tags, vec!["axum".to_string(), "seaorm".to_string()]);
  }

  #[test]
  fn read_case_uses_cover_and_rewrites_readme_images() {
    let tmp = temp_root();
    let dir = tmp.path().join("demo");
    write_file(
      &dir.join("case.yaml"),
      "name: Demo\ncategory: frontend\nrepo: https://github.com/example/demo\n",
    );
    write_file(&dir.join("cover.jpg"), "");
    write_file(&dir.join("README.md"), "![shot](./shot.png)\n![remote](https://example.com/a.png)");
    let case = match read_case_from_dir(&dir) {
      Ok(case) => case,
      Err(e) => panic!("failed to read case: {}", e),
    };
    assert_eq!(case.cover_url.as_deref(), Some("/cases/demo/cover.jpg"));
    let readme = match case.readme_md {
      Some(value) => value,
      None => panic!("expected readme"),
    };
    assert!(readme.contains("](/cases/demo/shot.png)"));
    assert!(readme.contains("](https://example.com/a.png)"));
  }

  #[test]
  fn scan_cases_skips_bad_yaml() {
    let tmp = temp_root();
    write_file(
      &tmp.path().join("good/case.yaml"),
      "name: Good\nrepo: https://github.com/example/good\n",
    );
    write_file(&tmp.path().join("bad/case.yaml"), "name: [");
    let cases = scan_cases_from_root(tmp.path());
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].slug, "good");
  }

  #[test]
  fn filter_summaries_uses_or_tags_query_category_and_sort() {
    let cases = vec![
      CaseSummary {
        slug: "a".to_string(),
        name: "Dioxus App".to_string(),
        description: "Frontend".to_string(),
        category: "frontend".to_string(),
        tags: vec!["dioxus".to_string()],
        repo: String::new(),
        website: None,
        author: String::new(),
        author_url: None,
        language: "rust".to_string(),
        stars: 1,
        favorite: false,
        date_added: "2025-01-01".to_string(),
        cover_url: None,
      },
      CaseSummary {
        slug: "b".to_string(),
        name: "Axum API".to_string(),
        description: "Backend".to_string(),
        category: "backend".to_string(),
        tags: vec!["axum".to_string()],
        repo: String::new(),
        website: None,
        author: String::new(),
        author_url: None,
        language: "rust".to_string(),
        stars: 100,
        favorite: true,
        date_added: "2024-01-01".to_string(),
        cover_url: None,
      },
    ];
    let filtered = filter_summaries(
      cases,
      Some(vec!["dioxus".to_string(), "axum".to_string()]),
      Some("backend".to_string()),
      Some("api".to_string()),
    );
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].slug, "b");
  }

  #[test]
  fn every_board_domain_has_at_least_three_real_cases() {
    // 验收 Phase 6.6：用真实 parser 扫描仓库 assets/cases，每板块需 ≥3 真实案例。
    // 同时校验全部 case.yaml 能被 serde_yaml 正确解析（解析失败的目录会被跳过，
    // 从而使对应板块计数不足而触发断言）。板块归类：embedded/ai/web3/cli 用
    // category，wasm 用 tag。
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../assets/cases");
    let cases = scan_cases_from_root(&root);
    assert!(!cases.is_empty(), "应能从 {} 扫描到案例", root.display());
    for domain in ["embedded", "ai", "web3", "cli"] {
      let n = cases.iter().filter(|c| c.category == domain).count();
      assert!(n >= 3, "板块 {domain} 应有 ≥3 真实案例，实际 {n}");
    }
    let wasm = cases.iter().filter(|c| c.tags.iter().any(|t| t == "wasm")).count();
    assert!(wasm >= 3, "wasm 板块（tag）应有 ≥3 真实案例，实际 {wasm}");
  }
}
