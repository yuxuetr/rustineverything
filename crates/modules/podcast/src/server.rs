use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
use std::fs;
#[allow(unused_imports)]
use std::path::PathBuf;

/// 单个 podcast 节目的元数据 + 运行时信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Episode {
  pub id: i32,
  pub slug: String,
  pub title: String,
  pub description: String,
  pub duration: String,
  pub date: String,
  pub url: String,
  #[serde(default)]
  pub guest: Option<String>,
  #[serde(default)]
  pub tags: Vec<String>,
}

/// YAML frontmatter 文件中的字段
#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
struct EpisodeMeta {
  #[serde(default)]
  id: i32,
  #[serde(default)]
  title: String,
  #[serde(default)]
  description: String,
  #[serde(default)]
  duration: String,
  #[serde(default)]
  date: String,
  #[serde(default)]
  audio_url: String,
  #[serde(default)]
  guest: Option<String>,
  #[serde(default)]
  tags: Vec<String>,
}

/// 自动探测 podcasts 资产根目录
#[cfg(feature = "server")]
fn get_podcasts_root() -> PathBuf {
  let mut path = PathBuf::from("assets/podcasts");
  if !path.exists() {
    path = PathBuf::from("../../assets/podcasts");
  }
  path
}

/// 受支持的音频文件扩展名
const AUDIO_EXTENSIONS: &[&str] = &["m4a", "mp3", "wav", "ogg", "flac", "aac", "opus", "mpeg"];

/// server-only: 扫描节目目录中的音频文件，按文件名升序返回第一个
#[cfg(feature = "server")]
fn find_audio_file(dir: &std::path::Path) -> Option<String> {
  let mut audio_files: Vec<String> = fs::read_dir(dir)
    .ok()?
    .flatten()
    .filter(|e| e.path().is_file())
    .filter_map(|e| {
      let path = e.path();
      let ext = path.extension()?.to_str()?.to_lowercase();
      if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
        Some(e.file_name().to_str()?.to_string())
      } else {
        None
      }
    })
    .collect();
  audio_files.sort();
  audio_files.into_iter().next()
}

/// 给 slug 生成一个稳定的正整数 id（用于无 YAML 场景）
#[cfg(feature = "server")]
fn slug_to_id(slug: &str) -> i32 {
  use std::collections::hash_map::DefaultHasher;
  use std::hash::{Hash, Hasher};
  let mut hasher = DefaultHasher::new();
  slug.hash(&mut hasher);
  let h = hasher.finish();
  // 取低 31 位保证为正数
  ((h & 0x7fff_ffff) as i32).max(1)
}

/// 文件 mtime 转为 YYYY-MM-DD 字符串
#[cfg(feature = "server")]
fn file_date(path: &std::path::Path) -> String {
  use chrono::{DateTime, Utc};
  fs::metadata(path)
    .and_then(|m| m.modified())
    .ok()
    .map(|t| {
      let dt: DateTime<Utc> = t.into();
      dt.format("%Y-%m-%d").to_string()
    })
    .unwrap_or_default()
}

/// 解析 audio_url 取得最终 url。优先级：
/// 1. http(s) 或 / 开头 -> 原样保留
/// 2. 相对路径 -> /podcasts/<slug>/<file>
/// 3. 空 -> 扫描目录中的音频文件
#[cfg(feature = "server")]
fn resolve_audio_url(slug: &str, dir: &std::path::Path, audio_url: &str) -> Option<String> {
  if audio_url.starts_with('/') || audio_url.starts_with("http") {
    Some(audio_url.to_string())
  } else if !audio_url.is_empty() {
    Some(format!("/podcasts/{}/{}", slug, audio_url))
  } else {
    // 扫描目录中的音频文件
    find_audio_file(dir).map(|f| format!("/podcasts/{}/{}", slug, f))
  }
}

/// server-only: 从某个节目目录读取 Episode（优先读 YAML，否则仅从音频文件推断）
#[cfg(feature = "server")]
pub fn read_episode_from_dir(dir: &std::path::Path) -> Option<Episode> {
  let slug = dir.file_name()?.to_str()?.to_string();
  let yaml_path = dir.join("episode.yaml");
  let yml_path = dir.join("episode.yml");
  let yaml_file = if yaml_path.exists() {
    Some(yaml_path)
  } else if yml_path.exists() {
    Some(yml_path)
  } else {
    None
  };

  if let Some(path) = yaml_file {
    // === 完整 YAML 元数据 ===
    let content = fs::read_to_string(&path).ok()?;
    let meta: EpisodeMeta = serde_yaml::from_str(&content).ok()?;

    let url = resolve_audio_url(&slug, dir, &meta.audio_url)
      .unwrap_or_else(|| format!("/podcasts/{}/audio.mp3", slug));

    let id = if meta.id != 0 { meta.id } else { slug_to_id(&slug) };

    Some(Episode {
      id,
      slug,
      title: meta.title,
      description: meta.description,
      duration: meta.duration,
      date: meta.date,
      url,
      guest: meta.guest,
      tags: meta.tags,
    })
  } else {
    // === 仅有音频文件，无 YAML：推断元数据 ===
    let audio_file = find_audio_file(dir)?;
    let audio_path = dir.join(&audio_file);
    let url = format!("/podcasts/{}/{}", slug, audio_file);

    // 标题 = 文件名（去扩展名）
    let title = std::path::Path::new(&audio_file)
      .file_stem()
      .and_then(|s| s.to_str())
      .unwrap_or(&slug)
      .to_string();

    Some(Episode {
      id: slug_to_id(&slug),
      slug,
      title,
      description: String::new(),
      duration: String::new(),
      date: file_date(&audio_path),
      url,
      guest: None,
      tags: vec![],
    })
  }
}

/// server-only: 扫描 podcasts 目录返回按日期降序的 Episode 列表
#[cfg(feature = "server")]
pub fn scan_episodes(root: &std::path::Path) -> Vec<Episode> {
  if !root.exists() {
    return vec![];
  }
  let mut episodes: Vec<Episode> = fs::read_dir(root)
    .into_iter()
    .flatten()
    .flatten()
    .filter(|e| e.path().is_dir())
    .filter_map(|e| {
      let name = e.file_name().to_str()?.to_string();
      if name.starts_with('_') || name.starts_with('.') {
        return None;
      }
      read_episode_from_dir(&e.path())
    })
    .collect();
  // 按日期降序（最新在前），日期相同时 id 大的在前
  episodes.sort_by(|a, b| {
    let by_date = b.date.cmp(&a.date);
    if by_date == std::cmp::Ordering::Equal {
      b.id.cmp(&a.id)
    } else {
      by_date
    }
  });
  episodes
}

#[post("/api/podcasts/list")]
pub async fn list_episodes() -> Result<Vec<Episode>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    Ok(scan_episodes(&get_podcasts_root()))
  }
  #[cfg(not(feature = "server"))]
  {
    Ok(vec![])
  }
}

#[post("/api/podcasts/get")]
pub async fn get_episode_by_id(id: i32) -> Result<Option<Episode>, ServerFnError> {
  #[cfg(feature = "server")]
  {
    Ok(scan_episodes(&get_podcasts_root()).into_iter().find(|e| e.id == id))
  }
  #[cfg(not(feature = "server"))]
  {
    Ok(None)
  }
}

// ========== Tests ==========

#[cfg(all(test, feature = "server"))]
mod tests {
  use super::*;
  use std::path::Path;
  use tempfile::TempDir;

  fn write_yaml(dir: &Path, content: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("episode.yaml"), content).unwrap();
  }

  fn write_audio(dir: &Path, filename: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(filename), b"fake audio data").unwrap();
  }

  #[test]
  fn test_read_episode_basic() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("ep1");
    write_yaml(
      &dir,
      "id: 1\ntitle: T\ndescription: D\nduration: \"10:00\"\ndate: 2024-01-01\naudio_url: a.mp3\n",
    );
    let ep = read_episode_from_dir(&dir).unwrap();
    assert_eq!(ep.id, 1);
    assert_eq!(ep.slug, "ep1");
    assert_eq!(ep.title, "T");
    // 相对 audio_url 解析为 /podcasts/<slug>/<file>
    assert_eq!(ep.url, "/podcasts/ep1/a.mp3");
  }

  #[test]
  fn test_audio_url_absolute_kept_as_is() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("ep");
    write_yaml(
      &dir,
      "id: 2\ntitle: T\nduration: \"5:00\"\ndate: 2024-02-01\naudio_url: /audio/foo.m4a\n",
    );
    let ep = read_episode_from_dir(&dir).unwrap();
    assert_eq!(ep.url, "/audio/foo.m4a");
  }

  #[test]
  fn test_audio_url_http_kept_as_is() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("ep");
    write_yaml(&dir, "id: 3\ntitle: T\nduration: \"5:00\"\ndate: 2024-02-01\naudio_url: https://cdn.example.com/x.mp3\n");
    let ep = read_episode_from_dir(&dir).unwrap();
    assert_eq!(ep.url, "https://cdn.example.com/x.mp3");
  }

  #[test]
  fn test_audio_url_fallback_when_missing_and_no_files() {
    // 无 audio_url 且目录中没有音频文件时，退化到默认 audio.mp3
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("noaudio");
    write_yaml(&dir, "id: 4\ntitle: T\nduration: \"5:00\"\ndate: 2024-02-01\n");
    let ep = read_episode_from_dir(&dir).unwrap();
    assert_eq!(ep.url, "/podcasts/noaudio/audio.mp3");
  }

  #[test]
  fn test_yaml_with_no_audio_url_auto_detects_file() {
    // YAML 没写 audio_url，但目录中有 .m4a 文件——应被自动拾取
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("auto");
    write_yaml(&dir, "id: 10\ntitle: T\ndate: 2024-04-01\n");
    write_audio(&dir, "my-show.m4a");
    let ep = read_episode_from_dir(&dir).unwrap();
    assert_eq!(ep.url, "/podcasts/auto/my-show.m4a");
  }

  #[test]
  fn test_audio_only_no_yaml() {
    // 只有 mp3 文件，没有 YAML——仍能生成节目
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("talk-001");
    write_audio(&dir, "深度对谈.mp3");
    let ep = read_episode_from_dir(&dir).unwrap();
    assert_eq!(ep.slug, "talk-001");
    assert_eq!(ep.title, "深度对谈");
    assert_eq!(ep.url, "/podcasts/talk-001/深度对谈.mp3");
    assert!(ep.id > 0);
    // date 应为文件创建时间（近期）
    assert!(!ep.date.is_empty(), "应该从文件 mtime 推导出日期");
  }

  #[test]
  fn test_audio_extensions_supported() {
    // 所有支持的音频格式都能被识别
    let exts = ["m4a", "mp3", "wav", "ogg", "flac", "aac", "opus"];
    for ext in exts {
      let tmp = TempDir::new().unwrap();
      let dir = tmp.path().join("ep");
      write_audio(&dir, &format!("audio.{}", ext));
      let found = find_audio_file(&dir);
      assert_eq!(found, Some(format!("audio.{}", ext)), "未识别扩展名 {}", ext);
    }
  }

  #[test]
  fn test_audio_picked_alphabetically() {
    // 有多个音频文件时，选择字母顺序第一个
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("ep");
    write_audio(&dir, "zzz.mp3");
    write_audio(&dir, "aaa.mp3");
    write_audio(&dir, "middle.mp3");
    let found = find_audio_file(&dir);
    assert_eq!(found, Some("aaa.mp3".to_string()));
  }

  #[test]
  fn test_non_audio_files_ignored() {
    // 非音频文件不被选中
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("ep");
    write_audio(&dir, "readme.txt"); // 重用辅助函数写入任意文件
    write_audio(&dir, "cover.png");
    let found = find_audio_file(&dir);
    assert_eq!(found, None);
  }

  #[test]
  fn test_slug_to_id_is_stable() {
    // 同一个 slug 应该产生相同的 id
    assert_eq!(slug_to_id("hello"), slug_to_id("hello"));
    assert!(slug_to_id("hello") > 0);
    // 不同 slug 产生不同 id
    assert_ne!(slug_to_id("foo"), slug_to_id("bar"));
  }

  #[test]
  fn test_scan_episodes_sorted_by_date_desc() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_yaml(
      &root.join("a"),
      "id: 1\ntitle: A\nduration: \"10:00\"\ndate: 2024-01-01\naudio_url: a.mp3\n",
    );
    write_yaml(
      &root.join("b"),
      "id: 2\ntitle: B\nduration: \"10:00\"\ndate: 2024-03-15\naudio_url: b.mp3\n",
    );
    write_yaml(
      &root.join("c"),
      "id: 3\ntitle: C\nduration: \"10:00\"\ndate: 2024-02-10\naudio_url: c.mp3\n",
    );

    let eps = scan_episodes(root);
    assert_eq!(eps.len(), 3);
    assert_eq!(eps[0].slug, "b"); // 2024-03-15
    assert_eq!(eps[1].slug, "c"); // 2024-02-10
    assert_eq!(eps[2].slug, "a"); // 2024-01-01
  }

  #[test]
  fn test_scan_skips_underscore_and_hidden() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_yaml(
      &root.join("visible"),
      "id: 1\ntitle: V\nduration: \"5:00\"\ndate: 2024-01-01\naudio_url: a.mp3\n",
    );
    write_yaml(
      &root.join("_draft"),
      "id: 2\ntitle: D\nduration: \"5:00\"\ndate: 2024-01-02\naudio_url: d.mp3\n",
    );
    write_yaml(
      &root.join(".hidden"),
      "id: 3\ntitle: H\nduration: \"5:00\"\ndate: 2024-01-03\naudio_url: h.mp3\n",
    );

    let eps = scan_episodes(root);
    assert_eq!(eps.len(), 1);
    assert_eq!(eps[0].slug, "visible");
  }

  #[test]
  fn test_scan_skips_dirs_without_yaml_or_audio() {
    // 既没 YAML 也没音频文件的目录被跳过
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_yaml(
      &root.join("withyaml"),
      "id: 1\ntitle: T\nduration: \"5:00\"\ndate: 2024-01-01\naudio_url: a.mp3\n",
    );
    std::fs::create_dir_all(root.join("empty")).unwrap();

    let eps = scan_episodes(root);
    assert_eq!(eps.len(), 1);
    assert_eq!(eps[0].slug, "withyaml");
  }

  #[test]
  fn test_scan_includes_audio_only_dirs() {
    // 只有音频文件的目录也会被收录
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_yaml(&root.join("with-yaml"), "id: 1\ntitle: T\ndate: 2024-01-01\naudio_url: a.mp3\n");
    write_audio(&root.join("audio-only"), "talk.m4a");

    let eps = scan_episodes(root);
    assert_eq!(eps.len(), 2);
    assert!(eps.iter().any(|e| e.slug == "audio-only"));
    assert!(eps.iter().any(|e| e.slug == "with-yaml"));
  }

  #[test]
  fn test_episode_optional_fields() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("ep");
    write_yaml(&dir,
            "id: 1\ntitle: T\nduration: \"5:00\"\ndate: 2024-01-01\naudio_url: a.mp3\nguest: Alice\ntags: [rust, ai]\n");
    let ep = read_episode_from_dir(&dir).unwrap();
    assert_eq!(ep.guest, Some("Alice".to_string()));
    assert_eq!(ep.tags, vec!["rust", "ai"]);
  }

  #[test]
  fn test_empty_root_returns_empty_list() {
    let tmp = TempDir::new().unwrap();
    let eps = scan_episodes(tmp.path());
    assert!(eps.is_empty());
  }

  #[test]
  fn test_nonexistent_root_returns_empty() {
    let path = std::path::Path::new("/this/path/should/not/exist/xyz123");
    let eps = scan_episodes(path);
    assert!(eps.is_empty());
  }

  #[test]
  fn test_same_date_breaks_tie_by_id_desc() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_yaml(
      &root.join("a"),
      "id: 1\ntitle: A\nduration: \"5:00\"\ndate: 2024-01-01\naudio_url: a.mp3\n",
    );
    write_yaml(
      &root.join("b"),
      "id: 5\ntitle: B\nduration: \"5:00\"\ndate: 2024-01-01\naudio_url: b.mp3\n",
    );
    write_yaml(
      &root.join("c"),
      "id: 3\ntitle: C\nduration: \"5:00\"\ndate: 2024-01-01\naudio_url: c.mp3\n",
    );

    let eps = scan_episodes(root);
    // 同日期下 id 大的在前
    assert_eq!(eps.iter().map(|e| e.id).collect::<Vec<_>>(), vec![5, 3, 1]);
  }
}
