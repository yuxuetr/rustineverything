use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
use std::fs;
#[allow(unused_imports)]
use std::path::{Path, PathBuf};

// =============================================================
// Public types (shared between server and client)
// =============================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LessonKind {
    Doc,
    Video,
    Audio,
    Code,
}

impl LessonKind {
    pub fn as_str(self) -> &'static str {
        match self {
            LessonKind::Doc => "doc",
            LessonKind::Video => "video",
            LessonKind::Audio => "audio",
            LessonKind::Code => "code",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            LessonKind::Doc => "\u{1F4C4}",   // 📄
            LessonKind::Video => "\u{1F3AC}", // 🎬
            LessonKind::Audio => "\u{1F3A7}", // 🎧
            LessonKind::Code => "\u{1F4BB}",  // 💻
        }
    }
}

/// 媒体引用（音/视频）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediaRef {
    pub url: String,
    #[serde(default)]
    pub duration: Option<String>,
    #[serde(default)]
    pub poster: Option<String>,
}

/// 单个代码文件
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodeFile {
    pub name: String,
    pub lang: String,
    pub content: String,
    pub raw_url: String,
}

/// 可下载附件
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DownloadFile {
    pub name: String,
    pub size_bytes: u64,
    pub url: String,
}

/// 章节文档体（Doc Lesson 或带 index.md 的 Video/Audio/Code Lesson）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DocBody {
    pub markdown: String,
    pub title: String,
    pub description: String,
}

/// Lesson 摘要（出现在课程详情页章节列表里，不含正文/资源）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LessonSummary {
    pub slug: String,
    pub title: String,
    pub kind: LessonKind,
    pub order: i32,
    #[serde(default)]
    pub duration: Option<String>,
}

/// 完整 Lesson（PR-B 中由 LessonPage 使用）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Lesson {
    pub slug: String,
    pub title: String,
    pub kind: LessonKind,
    pub order: i32,
    #[serde(default)]
    pub doc: Option<DocBody>,
    #[serde(default)]
    pub audio: Option<MediaRef>,
    #[serde(default)]
    pub video: Option<MediaRef>,
    #[serde(default)]
    pub code: Vec<CodeFile>,
    #[serde(default)]
    pub downloads: Vec<DownloadFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Chapter {
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub order: i32,
    pub lessons: Vec<LessonSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Course {
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub level: Option<String>,
    pub order: i32,
    pub chapters: Vec<Chapter>,
}

/// 课程列表摘要（不含 chapters 详情，仅做卡片网格）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CourseSummary {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub cover: Option<String>,
    pub tags: Vec<String>,
    pub level: Option<String>,
    pub order: i32,
    pub chapter_count: usize,
    pub lesson_count: usize,
}

impl From<&Course> for CourseSummary {
    fn from(c: &Course) -> Self {
        let lesson_count = c.chapters.iter().map(|ch| ch.lessons.len()).sum();
        CourseSummary {
            slug: c.slug.clone(),
            title: c.title.clone(),
            description: c.description.clone(),
            cover: c.cover.clone(),
            tags: c.tags.clone(),
            level: c.level.clone(),
            order: c.order,
            chapter_count: c.chapters.len(),
            lesson_count,
        }
    }
}

// =============================================================
// YAML metadata (server-only deserialization helpers)
// =============================================================

#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
struct CourseMeta {
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    cover: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    level: Option<String>,
    #[serde(default)]
    order: Option<i32>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
struct ChapterMeta {
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    order: Option<i32>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
pub(crate) struct LessonFrontmatter {
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    audio_url: String,
    #[serde(default)]
    video_url: String,
    #[serde(default)]
    duration: Option<String>,
    #[serde(default)]
    sidebar_position: Option<i32>,
}

// =============================================================
// File-extension whitelists
// =============================================================

pub const AUDIO_EXTS: &[&str] =
    &["mp3", "m4a", "wav", "ogg", "flac", "aac", "opus", "mpeg"];
pub const VIDEO_EXTS: &[&str] = &["mp4", "webm", "mov", "mkv"];
pub const COVER_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp"];
pub const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "svg"];
pub const CODE_EXTS: &[&str] = &[
    "rs", "toml", "ts", "tsx", "js", "jsx", "py", "sh", "bash", "zsh", "yaml", "yml", "json",
    "sql", "go", "java", "kt", "swift", "c", "cpp", "h", "hpp", "html", "css",
];

// =============================================================
// Pure helpers (no fs/server feature needed)
// =============================================================

/// 用文件扩展名判断属于哪个白名单
pub fn ext_in(name: &str, exts: &[&str]) -> bool {
    let lower = name.to_ascii_lowercase();
    let dot = match lower.rfind('.') {
        Some(i) => i,
        None => return false,
    };
    let ext = &lower[dot + 1..];
    exts.iter().any(|e| *e == ext)
}

/// 以"数字前缀 + 分隔符"驱动顺序：`01-foo` → (1, "foo")；无前缀返回 (i32::MAX, slug)
pub fn parse_order_prefix(slug: &str) -> (i32, String) {
    let bytes = slug.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 {
        return (i32::MAX, slug.to_string());
    }
    let num: i32 = slug[..i].parse().unwrap_or(i32::MAX);
    let mut rest_start = i;
    while rest_start < bytes.len() && (bytes[rest_start] == b'-' || bytes[rest_start] == b'_') {
        rest_start += 1;
    }
    let rest = slug[rest_start..].to_string();
    (num, rest)
}

/// 标题美化：把目录名 `01-rust-basics` 转成 `Rust Basics`
pub fn humanize_title(slug: &str) -> String {
    let (_, rest) = parse_order_prefix(slug);
    let base = if rest.is_empty() { slug.to_string() } else { rest };
    base.split(|c: char| c == '-' || c == '_')
        .filter(|s| !s.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// 推测代码文件语言（基础版；前端再交给 PrismJS 高亮）
pub fn lang_from_ext(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "sh" | "bash" | "zsh" => "bash",
        "yaml" | "yml" => "yaml",
        "json" => "json",
        "toml" => "toml",
        "sql" => "sql",
        "go" => "go",
        "java" => "java",
        "kt" => "kotlin",
        "swift" => "swift",
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" | "hh" => "cpp",
        "html" => "html",
        "css" => "css",
        other => {
            if other.is_empty() {
                "text"
            } else {
                "text"
            }
        }
    }
    .to_string()
}

/// 把 markdown 里的相对路径图片重写为绝对 URL
/// - `![](./diagram.png)` / `![](images/diagram.png)` → `<base>/diagram.png` 或 `<base>/images/diagram.png`
/// - `![](/foo)` / `![](http(s)://...)` 原样保留
/// 仅支持 `![alt](url)` 格式（不处理 reference link 与 HTML img）
pub fn rewrite_image_urls(markdown: &str, base: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let bytes = markdown.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // 找 "![" 起始
        if i + 1 < bytes.len() && bytes[i] == b'!' && bytes[i + 1] == b'[' {
            // 找 alt 关闭 ']'
            let mut j = i + 2;
            let mut depth = 1;
            while j < bytes.len() && depth > 0 {
                match bytes[j] {
                    b'[' => depth += 1,
                    b']' => depth -= 1,
                    _ => {}
                }
                j += 1;
            }
            // 此时 j 指向 ']' 之后；要求紧跟 '('
            if depth == 0 && j < bytes.len() && bytes[j] == b'(' {
                // 找匹配 ')'
                let url_start = j + 1;
                let mut k = url_start;
                let mut pdepth = 1;
                while k < bytes.len() && pdepth > 0 {
                    match bytes[k] {
                        b'(' => pdepth += 1,
                        b')' => pdepth -= 1,
                        _ => {}
                    }
                    if pdepth > 0 {
                        k += 1;
                    }
                }
                if pdepth == 0 && k <= bytes.len() {
                    // 把 ![alt] 部分原样写入
                    out.push_str(&markdown[i..url_start]);
                    let raw_url_with_title = &markdown[url_start..k];
                    // 拆 url 与 title（"url title"）
                    let (raw_url, title_part) = match raw_url_with_title.find(char::is_whitespace) {
                        Some(p) => (&raw_url_with_title[..p], &raw_url_with_title[p..]),
                        None => (raw_url_with_title, ""),
                    };
                    let rewritten = rewrite_one_url(raw_url.trim(), base);
                    out.push_str(&rewritten);
                    out.push_str(title_part);
                    out.push(')');
                    i = k + 1;
                    continue;
                }
            }
        }
        // 按 UTF-8 char 边界拷贝切片，避免将多字节字符拆成单字节造成乱码。
        let mut next = i + 1;
        while next < bytes.len() && !markdown.is_char_boundary(next) {
            next += 1;
        }
        out.push_str(&markdown[i..next]);
        i = next;
    }
    out
}

fn rewrite_one_url(url: &str, base: &str) -> String {
    if url.is_empty() {
        return url.to_string();
    }
    if url.starts_with("http://") || url.starts_with("https://") || url.starts_with('/') {
        return url.to_string();
    }
    let stripped = url.strip_prefix("./").unwrap_or(url);
    let trimmed_base = base.trim_end_matches('/');
    format!("{}/{}", trimmed_base, stripped)
}

/// 解析 markdown 中的 frontmatter (YAML between ---)
pub(crate) fn parse_frontmatter_lesson(content: &str) -> (LessonFrontmatter, String) {
    if !content.starts_with("---") {
        return (LessonFrontmatter::default(), content.to_string());
    }
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return (LessonFrontmatter::default(), content.to_string());
    }
    #[cfg(feature = "server")]
    {
        let meta: LessonFrontmatter = serde_yaml::from_str(parts[1]).unwrap_or_default();
        return (meta, parts[2].to_string());
    }
    #[cfg(not(feature = "server"))]
    {
        // 客户端不解析 YAML（避免引入 serde_yaml 到 client target）
        return (LessonFrontmatter::default(), parts[2].to_string());
    }
}

// =============================================================
// Server-only filesystem scanners
// =============================================================

#[cfg(feature = "server")]
pub fn get_courses_root() -> PathBuf {
    let mut p = PathBuf::from("assets/courses");
    if !p.exists() {
        p = PathBuf::from("../../assets/courses");
    }
    p
}

#[cfg(feature = "server")]
fn skip_entry(name: &str) -> bool {
    name.starts_with('_') || name.starts_with('.')
}

/// 在目录中查找第一个白名单扩展名的文件，按字母升序
#[cfg(feature = "server")]
fn find_first_with_ext(dir: &Path, exts: &[&str]) -> Option<String> {
    let mut found: Vec<String> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            if ext_in(&name, exts) {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    found.sort();
    found.into_iter().next()
}

#[cfg(feature = "server")]
pub fn find_audio_file(dir: &Path) -> Option<String> {
    find_first_with_ext(dir, AUDIO_EXTS)
}

#[cfg(feature = "server")]
pub fn find_video_file(dir: &Path) -> Option<String> {
    find_first_with_ext(dir, VIDEO_EXTS)
}

#[cfg(feature = "server")]
pub fn find_cover_image(dir: &Path) -> Option<String> {
    let candidates = ["cover", "Cover", "COVER"];
    for stem in candidates {
        for ext in COVER_EXTS {
            let p = dir.join(format!("{stem}.{ext}"));
            if p.exists() {
                return Some(format!("{stem}.{ext}"));
            }
        }
    }
    None
}

/// 解析媒体相对/绝对/http URL -> 最终 URL
/// - http(s):// 或 / 开头 → 原样
/// - 否则相对于 base
#[cfg(feature = "server")]
pub fn resolve_media_url(value: &str, base: &str) -> Option<String> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }
    if v.starts_with("http://") || v.starts_with("https://") || v.starts_with('/') {
        return Some(v.to_string());
    }
    let trimmed_base = base.trim_end_matches('/');
    let trimmed_v = v.strip_prefix("./").unwrap_or(v);
    Some(format!("{}/{}", trimmed_base, trimmed_v))
}

/// 推断 lesson 类型；返回 None 表示该目录应被跳过
#[cfg(feature = "server")]
pub fn infer_lesson_kind(dir: &Path) -> Option<LessonKind> {
    if dir.join("index.md").exists() || dir.join("index.mdx").exists() {
        return Some(LessonKind::Doc);
    }
    if find_video_file(dir).is_some() {
        return Some(LessonKind::Video);
    }
    if find_audio_file(dir).is_some() {
        return Some(LessonKind::Audio);
    }
    // code: code/ 目录有白名单文件，或目录里直接有代码文件
    let code_dir = dir.join("code");
    if code_dir.is_dir() {
        let any = fs::read_dir(&code_dir)
            .ok()
            .map(|it| {
                it.flatten().any(|e| {
                    e.path().is_file()
                        && e.file_name()
                            .to_str()
                            .map(|n| ext_in(n, CODE_EXTS))
                            .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        if any {
            return Some(LessonKind::Code);
        }
    }
    let direct_code = fs::read_dir(dir)
        .ok()
        .map(|it| {
            it.flatten().any(|e| {
                e.path().is_file()
                    && e.file_name()
                        .to_str()
                        .map(|n| ext_in(n, CODE_EXTS))
                        .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    if direct_code {
        return Some(LessonKind::Code);
    }
    None
}

/// 扫描一个 lesson 目录的 attachments
#[cfg(feature = "server")]
pub fn scan_attachments(dir: &Path, base_url: &str) -> Vec<DownloadFile> {
    let att = dir.join("attachments");
    if !att.is_dir() {
        return vec![];
    }
    let mut out: Vec<DownloadFile> = fs::read_dir(&att)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            if name.starts_with('.') {
                return None;
            }
            let size = e.metadata().ok().map(|m| m.len()).unwrap_or(0);
            let url = format!("{}/attachments/{}", base_url.trim_end_matches('/'), name);
            Some(DownloadFile {
                name,
                size_bytes: size,
                url,
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// 扫描代码文件（先看 code/，再看 lesson 根目录里的代码文件）
#[cfg(feature = "server")]
pub fn scan_code_files(dir: &Path, base_url: &str) -> Vec<CodeFile> {
    let mut out: Vec<CodeFile> = vec![];
    let code_dir = dir.join("code");
    let mut sources: Vec<(PathBuf, String)> = vec![];
    if code_dir.is_dir() {
        if let Ok(it) = fs::read_dir(&code_dir) {
            for e in it.flatten() {
                let p = e.path();
                if !p.is_file() {
                    continue;
                }
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    if !name.starts_with('.') && ext_in(name, CODE_EXTS) {
                        sources.push((p.clone(), format!("code/{name}")));
                    }
                }
            }
        }
    } else {
        // 仅 Code-only Lesson 时退化扫 lesson 根目录
        if let Ok(it) = fs::read_dir(dir) {
            for e in it.flatten() {
                let p = e.path();
                if !p.is_file() {
                    continue;
                }
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    if !name.starts_with('.')
                        && !name.eq_ignore_ascii_case("index.md")
                        && !name.eq_ignore_ascii_case("index.mdx")
                        && ext_in(name, CODE_EXTS)
                    {
                        sources.push((p.clone(), name.to_string()));
                    }
                }
            }
        }
    }
    sources.sort_by(|a, b| a.1.cmp(&b.1));
    for (path, rel) in sources {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();
        let content = fs::read_to_string(&path).unwrap_or_default();
        let raw_url = format!("{}/{}", base_url.trim_end_matches('/'), rel);
        out.push(CodeFile {
            name: name.clone(),
            lang: lang_from_ext(&name),
            content,
            raw_url,
        });
    }
    out
}

/// 读取一个 lesson 目录的完整内容（含正文 / 媒体 / 代码 / 下载）
#[cfg(feature = "server")]
pub fn read_lesson(course_slug: &str, chapter_slug: &str, lesson_slug: &str) -> Option<Lesson> {
    let root = get_courses_root();
    let dir = root.join(course_slug).join(chapter_slug).join(lesson_slug);
    if !dir.is_dir() {
        return None;
    }
    let kind = infer_lesson_kind(&dir)?;
    let base_url = format!("/courses/{}/{}/{}", course_slug, chapter_slug, lesson_slug);
    let (order, _) = parse_order_prefix(lesson_slug);

    // 解析 index.md 正文（如有）
    let mut doc_body: Option<DocBody> = None;
    let mut frontmatter = LessonFrontmatter::default();
    let md_path = if dir.join("index.md").exists() {
        Some(dir.join("index.md"))
    } else if dir.join("index.mdx").exists() {
        Some(dir.join("index.mdx"))
    } else {
        None
    };
    if let Some(p) = md_path {
        let raw = fs::read_to_string(&p).unwrap_or_default();
        let (fm, body) = parse_frontmatter_lesson(&raw);
        let rewritten = rewrite_image_urls(&body, &base_url);
        let title = if !fm.title.is_empty() {
            fm.title.clone()
        } else {
            humanize_title(lesson_slug)
        };
        doc_body = Some(DocBody {
            markdown: rewritten,
            title,
            description: fm.description.clone(),
        });
        frontmatter = fm;
    }

    // 媒体
    let audio = match resolve_media_url(&frontmatter.audio_url, &base_url) {
        Some(u) => Some(MediaRef {
            url: u,
            duration: frontmatter.duration.clone(),
            poster: None,
        }),
        None => find_audio_file(&dir).map(|f| MediaRef {
            url: format!("{}/{}", base_url, f),
            duration: frontmatter.duration.clone(),
            poster: None,
        }),
    };
    let video = match resolve_media_url(&frontmatter.video_url, &base_url) {
        Some(u) => Some(MediaRef {
            url: u,
            duration: frontmatter.duration.clone(),
            poster: None,
        }),
        None => find_video_file(&dir).map(|f| MediaRef {
            url: format!("{}/{}", base_url, f),
            duration: frontmatter.duration.clone(),
            poster: None,
        }),
    };

    let code = scan_code_files(&dir, &base_url);
    let downloads = scan_attachments(&dir, &base_url);

    let title = doc_body
        .as_ref()
        .map(|d| d.title.clone())
        .unwrap_or_else(|| humanize_title(lesson_slug));

    Some(Lesson {
        slug: lesson_slug.to_string(),
        title,
        kind,
        order,
        doc: doc_body,
        audio,
        video,
        code,
        downloads,
    })
}

/// 扫描一个 chapter 目录，返回带 LessonSummary 的 Chapter
#[cfg(feature = "server")]
pub fn read_chapter(course_slug: &str, chapter_slug: &str) -> Option<Chapter> {
    let root = get_courses_root();
    let dir = root.join(course_slug).join(chapter_slug);
    if !dir.is_dir() {
        return None;
    }

    // 元数据：_chapter.yaml 优先；否则从目录名推断
    let meta_path = dir.join("_chapter.yaml");
    let alt_meta_path = dir.join("_chapter.yml");
    let mut meta = ChapterMeta::default();
    let path = if meta_path.exists() {
        Some(meta_path)
    } else if alt_meta_path.exists() {
        Some(alt_meta_path)
    } else {
        None
    };
    if let Some(p) = path {
        if let Ok(content) = fs::read_to_string(&p) {
            meta = serde_yaml::from_str(&content).unwrap_or_default();
        }
    }
    let (prefix_order, _) = parse_order_prefix(chapter_slug);
    let order = meta.order.unwrap_or(prefix_order);
    let title = if !meta.title.is_empty() {
        meta.title.clone()
    } else {
        humanize_title(chapter_slug)
    };

    // 扫描 lesson 子目录
    let mut lessons: Vec<LessonSummary> = vec![];
    if let Ok(it) = fs::read_dir(&dir) {
        for e in it.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let name = match p.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if skip_entry(&name) {
                continue;
            }
            let kind = match infer_lesson_kind(&p) {
                Some(k) => k,
                None => continue,
            };
            let (lorder, _) = parse_order_prefix(&name);

            // 标题：优先取 index.md frontmatter title，再退化目录名
            let mut lesson_title = humanize_title(&name);
            let mut duration: Option<String> = None;
            let md = p.join("index.md");
            let mdx = p.join("index.mdx");
            let md_path = if md.exists() {
                Some(md)
            } else if mdx.exists() {
                Some(mdx)
            } else {
                None
            };
            if let Some(mp) = md_path {
                if let Ok(raw) = fs::read_to_string(&mp) {
                    let (fm, _) = parse_frontmatter_lesson(&raw);
                    if !fm.title.is_empty() {
                        lesson_title = fm.title;
                    }
                    duration = fm.duration;
                }
            }

            lessons.push(LessonSummary {
                slug: name,
                title: lesson_title,
                kind,
                order: lorder,
                duration,
            });
        }
    }
    lessons.sort_by(|a, b| a.order.cmp(&b.order).then(a.slug.cmp(&b.slug)));

    Some(Chapter {
        slug: chapter_slug.to_string(),
        title,
        description: meta.description.clone(),
        order,
        lessons,
    })
}

/// 读取单个课程（含全部章节与 lesson summary）
#[cfg(feature = "server")]
pub fn read_course(course_slug: &str) -> Option<Course> {
    let root = get_courses_root();
    let dir = root.join(course_slug);
    if !dir.is_dir() {
        return None;
    }

    // course.yaml 元数据
    let mut meta = CourseMeta::default();
    let yaml = dir.join("course.yaml");
    let yml = dir.join("course.yml");
    let path = if yaml.exists() {
        Some(yaml)
    } else if yml.exists() {
        Some(yml)
    } else {
        None
    };
    if let Some(p) = path {
        if let Ok(c) = fs::read_to_string(&p) {
            meta = serde_yaml::from_str(&c).unwrap_or_default();
        }
    }
    let (prefix_order, _) = parse_order_prefix(course_slug);
    let order = meta.order.unwrap_or(prefix_order);
    let title = if !meta.title.is_empty() {
        meta.title.clone()
    } else {
        humanize_title(course_slug)
    };

    // 封面：course.yaml.cover > 自动探测
    let cover_url = if !meta.cover.is_empty() {
        if meta.cover.starts_with("http") || meta.cover.starts_with('/') {
            Some(meta.cover.clone())
        } else {
            Some(format!(
                "/courses/{}/{}",
                course_slug,
                meta.cover.trim_start_matches("./")
            ))
        }
    } else {
        find_cover_image(&dir).map(|f| format!("/courses/{}/{}", course_slug, f))
    };

    // 扫章节
    let mut chapters: Vec<Chapter> = vec![];
    if let Ok(it) = fs::read_dir(&dir) {
        for e in it.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let name = match p.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if skip_entry(&name) {
                continue;
            }
            if let Some(ch) = read_chapter(course_slug, &name) {
                if !ch.lessons.is_empty() {
                    chapters.push(ch);
                }
            }
        }
    }
    chapters.sort_by(|a, b| a.order.cmp(&b.order).then(a.slug.cmp(&b.slug)));

    Some(Course {
        slug: course_slug.to_string(),
        title,
        description: meta.description.clone(),
        cover: cover_url,
        tags: meta.tags.clone(),
        level: meta.level.clone(),
        order,
        chapters,
    })
}

/// 扫描整个 courses 根目录
#[cfg(feature = "server")]
pub fn scan_courses(root: &Path) -> Vec<Course> {
    if !root.exists() {
        return vec![];
    }
    let mut out: Vec<Course> = vec![];
    if let Ok(it) = fs::read_dir(root) {
        for e in it.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let name = match p.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if skip_entry(&name) {
                continue;
            }
            if let Some(c) = read_course(&name) {
                if !c.chapters.is_empty() {
                    out.push(c);
                }
            }
        }
    }
    out.sort_by(|a, b| a.order.cmp(&b.order).then(a.slug.cmp(&b.slug)));
    out
}

// =============================================================
// Server functions exposed to the frontend
// =============================================================

#[post("/api/courses/list")]
pub async fn list_courses() -> Result<Vec<CourseSummary>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let root = get_courses_root();
        let courses = scan_courses(&root);
        Ok(courses.iter().map(CourseSummary::from).collect())
    }
    #[cfg(not(feature = "server"))]
    {
        Ok(vec![])
    }
}

#[post("/api/courses/get")]
pub async fn get_course(slug: String) -> Result<Option<Course>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        Ok(read_course(&slug))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = slug;
        Ok(None)
    }
}

#[post("/api/courses/lesson")]
pub async fn get_lesson(
    slug: String,
    chapter: String,
    lesson: String,
) -> Result<Option<Lesson>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        Ok(read_lesson(&slug, &chapter, &lesson))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (slug, chapter, lesson);
        Ok(None)
    }
}

// =============================================================
// Progress tracking (PR-C)
// =============================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct LessonProgress {
    pub lesson_path: String,
    pub completed: bool,
    pub position_seconds: Option<i32>,
}

/// server-only: 从请求 cookie 提取当前用户
#[cfg(feature = "server")]
fn current_session_user() -> Option<rustineverything_core::session::SessionUser> {
    use dioxus::fullstack::FullstackContext;
    use rustineverything_core::session::parse_session_from_cookie_header;

    let ctx = FullstackContext::current()?;
    let parts = ctx.parts_mut();
    let cookie_str = parts
        .headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    drop(parts);
    parse_session_from_cookie_header(cookie_str.as_deref())
}

/// 限制：仅 admin / member 可写进度与标注
#[cfg(feature = "server")]
fn require_writer() -> Result<rustineverything_core::session::SessionUser, ServerFnError> {
    let user =
        current_session_user().ok_or_else(|| ServerFnError::new("请先登录".to_string()))?;
    if user.role == "admin" || user.role == "member" {
        Ok(user)
    } else {
        Err(ServerFnError::new(
            "当前角色无权执行该操作".to_string(),
        ))
    }
}

#[cfg(feature = "server")]
async fn open_db() -> Result<sea_orm::DatabaseConnection, ServerFnError> {
    rustineverything_core::db::get_or_init_pool()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[post("/api/courses/progress/list")]
pub async fn get_progress(slug: String) -> Result<Vec<LessonProgress>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use rustineverything_core::entities::course_progress;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
        let user = match current_session_user() {
            Some(u) => u,
            None => return Ok(vec![]),
        };
        let db = open_db().await?;
        let rows = course_progress::Entity::find()
            .filter(course_progress::Column::UserId.eq(user.id))
            .filter(course_progress::Column::CourseSlug.eq(&slug))
            .all(&db)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| LessonProgress {
                lesson_path: r.lesson_path,
                completed: r.completed,
                position_seconds: r.position_seconds,
            })
            .collect())
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = slug;
        Ok(vec![])
    }
}

#[post("/api/courses/progress/complete")]
pub async fn mark_lesson_complete(
    slug: String,
    lesson_path: String,
    completed: bool,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "server")]
    {
        use chrono::Utc;
        use rustineverything_core::entities::course_progress;
        use sea_orm::{
            sea_query::OnConflict, ActiveValue::Set, EntityTrait,
        };
        let user = require_writer()?;
        let db = open_db().await?;
        let am = course_progress::ActiveModel {
            user_id: Set(user.id),
            course_slug: Set(slug),
            lesson_path: Set(lesson_path),
            completed: Set(completed),
            position_seconds: Set(None),
            updated_at: Set(Utc::now().fixed_offset()),
        };
        course_progress::Entity::insert(am)
            .on_conflict(
                OnConflict::columns([
                    course_progress::Column::UserId,
                    course_progress::Column::CourseSlug,
                    course_progress::Column::LessonPath,
                ])
                .update_columns([
                    course_progress::Column::Completed,
                    course_progress::Column::UpdatedAt,
                ])
                .to_owned(),
            )
            .exec(&db)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        Ok(())
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (slug, lesson_path, completed);
        Ok(())
    }
}

#[post("/api/courses/progress/last")]
pub async fn get_last_lesson(slug: String) -> Result<Option<String>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use rustineverything_core::entities::course_progress;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
        let user = match current_session_user() {
            Some(u) => u,
            None => return Ok(None),
        };
        let db = open_db().await?;
        let row = course_progress::Entity::find()
            .filter(course_progress::Column::UserId.eq(user.id))
            .filter(course_progress::Column::CourseSlug.eq(&slug))
            .order_by_desc(course_progress::Column::UpdatedAt)
            .one(&db)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        Ok(row.map(|r| r.lesson_path))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = slug;
        Ok(None)
    }
}

// =============================================================
// Annotations (PR-D)
// =============================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Annotation {
    pub id: i64,
    pub user_id: i32,
    pub resource_kind: String,
    pub resource_path: String,
    pub block_id: String,
    pub start_offset: i32,
    pub end_offset: i32,
    pub exact_text: String,
    pub prefix_text: Option<String>,
    pub suffix_text: Option<String>,
    pub style: String,
    pub note: Option<String>,
    pub visibility: String,
    pub created_at: String,
    /// 仅在该标注为"他人公开标注"时填充，帮助 UI 显示作者。本人标注为 None。
    #[serde(default)]
    pub author_nickname: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnnotationCreate {
    pub resource_kind: String,
    pub resource_path: String,
    pub block_id: String,
    pub start_offset: i32,
    pub end_offset: i32,
    pub exact_text: String,
    pub prefix_text: Option<String>,
    pub suffix_text: Option<String>,
    pub style: String,
    pub note: Option<String>,
    /// 'private' | 'course-public' | 'doc-public' | 'public'。缺省 'private'。
    #[serde(default)]
    pub visibility: Option<String>,
}

/// 表示合法的 visibility 取值
#[cfg(feature = "server")]
fn normalize_visibility(v: Option<&str>) -> String {
    match v.unwrap_or("private") {
        "public" => "public".to_string(),
        "course-public" => "course-public".to_string(),
        "doc-public" => "doc-public".to_string(),
        _ => "private".to_string(),
    }
}

/// 读 site.json 中 annotations 开关
#[cfg(feature = "server")]
fn read_annotations_switch(kind: &str) -> bool {
    use std::path::PathBuf;
    let mut p = PathBuf::from("assets/site.json");
    if !p.exists() {
        p = PathBuf::from("../../assets/site.json");
    }
    let raw = match fs::read_to_string(&p) {
        Ok(s) => s,
        Err(_) => return default_annotation_enabled(kind),
    };
    let v: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return default_annotation_enabled(kind),
    };
    v.get("annotations")
        .and_then(|a| a.get(kind))
        .and_then(|x| x.as_bool())
        .unwrap_or_else(|| default_annotation_enabled(kind))
}

#[cfg(feature = "server")]
fn default_annotation_enabled(kind: &str) -> bool {
    matches!(kind, "course" | "doc")
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnnotationsConfig {
    pub course: bool,
    pub doc: bool,
    pub blog: bool,
}

#[post("/api/annotations/config")]
pub async fn get_annotations_config() -> Result<AnnotationsConfig, ServerFnError> {
    #[cfg(feature = "server")]
    {
        Ok(AnnotationsConfig {
            course: read_annotations_switch("course"),
            doc: read_annotations_switch("doc"),
            blog: read_annotations_switch("blog"),
        })
    }
    #[cfg(not(feature = "server"))]
    {
        Ok(AnnotationsConfig {
            course: true,
            doc: true,
            blog: false,
        })
    }
}

#[cfg(feature = "server")]
fn model_to_annotation(m: rustineverything_core::entities::annotation::Model) -> Annotation {
    Annotation {
        id: m.id,
        user_id: m.user_id,
        resource_kind: m.resource_kind,
        resource_path: m.resource_path,
        block_id: m.block_id,
        start_offset: m.start_offset,
        end_offset: m.end_offset,
        exact_text: m.exact_text,
        prefix_text: m.prefix_text,
        suffix_text: m.suffix_text,
        style: m.style,
        note: m.note,
        visibility: m.visibility,
        created_at: m.created_at.format("%Y-%m-%d %H:%M").to_string(),
        author_nickname: None,
    }
}

/// 列出当前登录用户的全部标注（不受资源过滤，供个人标注列表页使用）。
/// 未登录返回空。
#[post("/api/annotations/list_my")]
pub async fn list_my_annotations() -> Result<Vec<Annotation>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use rustineverything_core::entities::annotation;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
        let user = match current_session_user() {
            Some(u) => u,
            None => return Ok(vec![]),
        };
        let db = open_db().await?;
        let rows = annotation::Entity::find()
            .filter(annotation::Column::UserId.eq(user.id))
            .order_by_desc(annotation::Column::CreatedAt)
            .all(&db)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        Ok(rows.into_iter().map(model_to_annotation).collect())
    }
    #[cfg(not(feature = "server"))]
    {
        Ok(vec![])
    }
}

#[post("/api/annotations/list")]
pub async fn list_annotations(
    resource_kind: String,
    resource_path: String,
) -> Result<Vec<Annotation>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        if !read_annotations_switch(&resource_kind) {
            return Ok(vec![]);
        }
        use rustineverything_core::entities::{annotation, user as user_entity};
        use sea_orm::{
            sea_query::Expr, ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder,
        };
        let me = current_session_user();
        let db = open_db().await?;

        // 同资源路径下：本人全部标注 + 他人不为 private 的标注。
        // 未登录只能看他人公开标注。
        let me_id = me.as_ref().map(|u| u.id).unwrap_or(-1);
        let visible_cond = Condition::any()
            .add(Expr::col(annotation::Column::UserId).eq(me_id))
            .add(Expr::col(annotation::Column::Visibility).ne("private"));

        let rows = annotation::Entity::find()
            .filter(annotation::Column::ResourceKind.eq(&resource_kind))
            .filter(annotation::Column::ResourcePath.eq(&resource_path))
            .filter(visible_cond)
            .order_by_asc(annotation::Column::CreatedAt)
            .all(&db)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        // 收集需要查询 nickname 的 user_id（仅他人标注需要增量查询）
        let mut other_ids: Vec<i32> = rows
            .iter()
            .filter(|r| r.user_id != me_id)
            .map(|r| r.user_id)
            .collect();
        other_ids.sort();
        other_ids.dedup();

        let mut nick_map: std::collections::HashMap<i32, String> =
            std::collections::HashMap::new();
        if !other_ids.is_empty() {
            let users = user_entity::Entity::find()
                .filter(user_entity::Column::Id.is_in(other_ids))
                .all(&db)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            for u in users {
                nick_map.insert(u.id, u.nickname);
            }
        }

        Ok(rows
            .into_iter()
            .map(|m| {
                let mut a = model_to_annotation(m);
                if a.user_id != me_id {
                    a.author_nickname = nick_map.get(&a.user_id).cloned();
                }
                a
            })
            .collect())
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (resource_kind, resource_path);
        Ok(vec![])
    }
}

#[post("/api/annotations/create")]
pub async fn create_annotation(
    payload: AnnotationCreate,
) -> Result<Annotation, ServerFnError> {
    #[cfg(feature = "server")]
    {
        if !read_annotations_switch(&payload.resource_kind) {
            return Err(ServerFnError::new(
                "当前资源未启用标注".to_string(),
            ));
        }
        use chrono::Utc;
        use rustineverything_core::engines::moderation::ModerationLabel;
        use rustineverything_core::entities::annotation;
        use rustineverything_module_moderation::{
            enqueue_if_flagged, evaluate_submission,
        };
        use rustineverything_sdk::ModerationSubmission;
        use sea_orm::{ActiveValue::Set, EntityTrait};
        let user = require_writer()?;

        // ── 审核：只对 note 非空时调；exact_text 是被引用的原文，不是用户内容 ──
        // resource_kind + resource_path 组成 ref_path（便于 admin 复核时跳回原文）
        let ref_path = format!("{}:{}", payload.resource_kind, payload.resource_path);
        let note_content = payload.note.clone().unwrap_or_default();
        let mod_outcome = if !note_content.trim().is_empty() {
            let submission = ModerationSubmission::new(&note_content)
                .with_kind("annotation")
                .with_ref_path(&ref_path);
            let verdict = evaluate_submission(submission).await;
            if verdict.label == ModerationLabel::Block {
                tracing::warn!(
                    user = %user.nickname,
                    ref_path = %ref_path,
                    score = verdict.score,
                    reason = %verdict.reason,
                    "moderation: annotation BLOCKED"
                );
                return Err(ServerFnError::new(format!(
                    "标注被审核拒绝：{}",
                    if verdict.reason.is_empty() {
                        "未通过内容审核".to_string()
                    } else {
                        verdict.reason
                    }
                )));
            }
            Some(verdict)
        } else {
            None
        };

        let db = open_db().await?;
        let now = Utc::now().fixed_offset();
        let visibility = normalize_visibility(payload.visibility.as_deref());
        let am = annotation::ActiveModel {
            user_id: Set(user.id),
            resource_kind: Set(payload.resource_kind),
            resource_path: Set(payload.resource_path),
            block_id: Set(payload.block_id),
            start_offset: Set(payload.start_offset),
            end_offset: Set(payload.end_offset),
            exact_text: Set(payload.exact_text),
            prefix_text: Set(payload.prefix_text),
            suffix_text: Set(payload.suffix_text),
            style: Set(payload.style),
            note: Set(payload.note),
            visibility: Set(visibility),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };
        let res = annotation::Entity::insert(am)
            .exec_with_returning(&db)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        // Flag note → 入审核队列（annotation.id 是 i64 BIGSERIAL）
        if let Some(verdict) = mod_outcome {
            enqueue_if_flagged(
                &db,
                &verdict,
                "annotation",
                Some(res.id),
                &ref_path,
                Some(user.id),
                &note_content,
                &[],
            )
            .await;
        }

        Ok(model_to_annotation(res))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = payload;
        Err(ServerFnError::new("server only".to_string()))
    }
}

#[post("/api/annotations/delete")]
pub async fn delete_annotation(id: i64) -> Result<(), ServerFnError> {
    #[cfg(feature = "server")]
    {
        use rustineverything_core::entities::annotation;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
        let user = require_writer()?;
        let db = open_db().await?;
        annotation::Entity::delete_many()
            .filter(annotation::Column::Id.eq(id))
            .filter(annotation::Column::UserId.eq(user.id))
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

#[post("/api/annotations/update")]
pub async fn update_annotation(
    id: i64,
    style: Option<String>,
    note: Option<String>,
    visibility: Option<String>,
) -> Result<Annotation, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use chrono::Utc;
        use rustineverything_core::entities::annotation;
        use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
        let user = require_writer()?;
        let db = open_db().await?;
        let row = annotation::Entity::find_by_id(id)
            .filter(annotation::Column::UserId.eq(user.id))
            .one(&db)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .ok_or_else(|| ServerFnError::new("未找到标注".to_string()))?;
        let mut am: annotation::ActiveModel = row.into();
        if let Some(s) = style {
            am.style = Set(s);
        }
        if let Some(n) = note {
            am.note = Set(Some(n));
        }
        if let Some(v) = visibility {
            am.visibility = Set(normalize_visibility(Some(&v)));
        }
        am.updated_at = Set(Utc::now().fixed_offset());
        let updated = annotation::Entity::update(am)
            .exec(&db)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        Ok(model_to_annotation(updated))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (id, style, note, visibility);
        Err(ServerFnError::new("server only".to_string()))
    }
}

// =============================================================
// Tests
// =============================================================

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;
    use std::fs as stdfs;
    use tempfile::TempDir;

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            stdfs::create_dir_all(parent).unwrap();
        }
        stdfs::write(path, content).unwrap();
    }

    fn touch(path: &Path) {
        write(path, "");
    }

    #[test]
    fn test_parse_order_prefix_numeric() {
        assert_eq!(parse_order_prefix("01-foo"), (1, "foo".to_string()));
        assert_eq!(parse_order_prefix("10_bar"), (10, "bar".to_string()));
        assert_eq!(parse_order_prefix("3-baz-qux"), (3, "baz-qux".to_string()));
    }

    #[test]
    fn test_parse_order_prefix_no_prefix() {
        assert_eq!(parse_order_prefix("foo"), (i32::MAX, "foo".to_string()));
        assert_eq!(parse_order_prefix("rust-basics"), (i32::MAX, "rust-basics".to_string()));
    }

    #[test]
    fn test_humanize_title() {
        assert_eq!(humanize_title("01-rust-basics"), "Rust Basics");
        assert_eq!(humanize_title("ownership"), "Ownership");
        assert_eq!(humanize_title("01-what_is-rust"), "What Is Rust");
    }

    #[test]
    fn test_ext_in() {
        assert!(ext_in("foo.MP3", AUDIO_EXTS));
        assert!(ext_in("hello.rs", CODE_EXTS));
        assert!(!ext_in("README", CODE_EXTS));
        assert!(!ext_in("foo.txt", CODE_EXTS));
    }

    #[test]
    fn test_lang_from_ext() {
        assert_eq!(lang_from_ext("hello.rs"), "rust");
        assert_eq!(lang_from_ext("Cargo.toml"), "toml");
        assert_eq!(lang_from_ext("script.sh"), "bash");
        assert_eq!(lang_from_ext("noext"), "text");
    }

    #[test]
    fn test_rewrite_one_url_keeps_absolute() {
        assert_eq!(rewrite_one_url("/foo.png", "/base"), "/foo.png");
        assert_eq!(
            rewrite_one_url("https://x.com/a.png", "/base"),
            "https://x.com/a.png"
        );
    }

    #[test]
    fn test_rewrite_one_url_relative() {
        assert_eq!(rewrite_one_url("foo.png", "/base"), "/base/foo.png");
        assert_eq!(rewrite_one_url("./foo.png", "/base"), "/base/foo.png");
        assert_eq!(rewrite_one_url("img/foo.png", "/base/"), "/base/img/foo.png");
    }

    #[test]
    fn test_rewrite_image_urls_basic() {
        let md = "Hello\n![diagram](./diagram.png)\nworld\n![](images/x.jpg \"caption\")";
        let out = rewrite_image_urls(md, "/courses/c/ch/le");
        assert!(out.contains("![diagram](/courses/c/ch/le/diagram.png)"));
        assert!(out.contains("![](/courses/c/ch/le/images/x.jpg \"caption\")"));
    }

    #[test]
    fn test_rewrite_image_urls_preserves_utf8() {
        // 中文不能被拆成单字节 — 上一个版本发生过 mojibake
        let md = "# 安装与环境\n\nRust 是一门系统编程语言。\n\n![架构](./d.png)\n";
        let out = rewrite_image_urls(md, "/courses/c/ch/le");
        assert!(out.contains("安装与环境"));
        assert!(out.contains("Rust 是一门系统编程语言。"));
        assert!(out.contains("![架构](/courses/c/ch/le/d.png)"));
    }

    #[test]
    fn test_rewrite_image_urls_keeps_external_and_absolute() {
        let md = "![](/already/abs.png)\n![](https://cdn.example.com/x.png)";
        let out = rewrite_image_urls(md, "/courses/c/ch/le");
        assert!(out.contains("/already/abs.png"));
        assert!(out.contains("https://cdn.example.com/x.png"));
        assert!(!out.contains("/courses/c/ch/le/already"));
    }

    #[test]
    fn test_infer_lesson_kind_doc() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("index.md"), "# hi");
        assert_eq!(infer_lesson_kind(tmp.path()), Some(LessonKind::Doc));
    }

    #[test]
    fn test_infer_lesson_kind_video() {
        let tmp = TempDir::new().unwrap();
        touch(&tmp.path().join("clip.mp4"));
        assert_eq!(infer_lesson_kind(tmp.path()), Some(LessonKind::Video));
    }

    #[test]
    fn test_infer_lesson_kind_audio() {
        let tmp = TempDir::new().unwrap();
        touch(&tmp.path().join("show.m4a"));
        assert_eq!(infer_lesson_kind(tmp.path()), Some(LessonKind::Audio));
    }

    #[test]
    fn test_infer_lesson_kind_code_via_subdir() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("code/main.rs"), "fn main() {}");
        assert_eq!(infer_lesson_kind(tmp.path()), Some(LessonKind::Code));
    }

    #[test]
    fn test_infer_lesson_kind_code_via_root() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("solution.py"), "print('hi')");
        assert_eq!(infer_lesson_kind(tmp.path()), Some(LessonKind::Code));
    }

    #[test]
    fn test_infer_lesson_kind_skip_empty() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("README.txt"), "ignored");
        assert_eq!(infer_lesson_kind(tmp.path()), None);
    }

    #[test]
    fn test_infer_priority_doc_over_video() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("index.md"), "# h");
        touch(&tmp.path().join("clip.mp4"));
        assert_eq!(infer_lesson_kind(tmp.path()), Some(LessonKind::Doc));
    }

    #[test]
    fn test_resolve_media_url() {
        assert_eq!(
            resolve_media_url("audio.mp3", "/courses/a/ch/le"),
            Some("/courses/a/ch/le/audio.mp3".to_string())
        );
        assert_eq!(
            resolve_media_url("/abs.mp3", "/courses/a/ch/le"),
            Some("/abs.mp3".to_string())
        );
        assert_eq!(
            resolve_media_url("https://cdn/x.mp3", "/courses/a/ch/le"),
            Some("https://cdn/x.mp3".to_string())
        );
        assert_eq!(resolve_media_url("", "/courses/a/ch/le"), None);
    }

    #[test]
    fn test_scan_attachments_orders_alphabetically() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("attachments/zeta.pdf"), "z");
        write(&tmp.path().join("attachments/alpha.zip"), "a");
        write(&tmp.path().join("attachments/.hidden"), "");
        let list = scan_attachments(tmp.path(), "/base");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "alpha.zip");
        assert_eq!(list[1].name, "zeta.pdf");
        assert_eq!(list[0].url, "/base/attachments/alpha.zip");
    }

    #[test]
    fn test_scan_code_files_under_code_dir() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("code/main.rs"), "fn main() {}");
        write(&tmp.path().join("code/Cargo.toml"), "[package]");
        let files = scan_code_files(tmp.path(), "/base");
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f.name == "main.rs" && f.lang == "rust"));
        assert!(files.iter().any(|f| f.name == "Cargo.toml" && f.lang == "toml"));
        let main = files.iter().find(|f| f.name == "main.rs").unwrap();
        assert_eq!(main.raw_url, "/base/code/main.rs");
    }

    #[test]
    fn test_scan_code_files_root_when_no_code_dir() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("hello.rs"), "fn main() {}");
        let files = scan_code_files(tmp.path(), "/base");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].raw_url, "/base/hello.rs");
    }

    #[test]
    fn test_read_lesson_doc_with_assets() {
        let tmp = TempDir::new().unwrap();
        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let res = (|| {
            let lesson_dir = tmp
                .path()
                .join("assets/courses/rust-basics/01-fundamentals/01-what-is-rust");
            stdfs::create_dir_all(&lesson_dir).unwrap();
            write(
                &lesson_dir.join("index.md"),
                "---\ntitle: What is Rust\nduration: '5:00'\n---\n# heading\n![](./d.png)\n",
            );
            touch(&lesson_dir.join("audio.mp3"));
            write(&lesson_dir.join("attachments/slides.pdf"), "pdfdata");
            write(&lesson_dir.join("code/hello.rs"), "fn main(){}");

            let lesson = read_lesson("rust-basics", "01-fundamentals", "01-what-is-rust")
                .expect("lesson should be readable");

            assert_eq!(lesson.kind, LessonKind::Doc);
            assert_eq!(lesson.title, "What is Rust");
            assert_eq!(lesson.order, 1);
            let doc = lesson.doc.expect("doc body");
            assert!(doc
                .markdown
                .contains("/courses/rust-basics/01-fundamentals/01-what-is-rust/d.png"));
            let audio = lesson.audio.expect("audio");
            assert_eq!(
                audio.url,
                "/courses/rust-basics/01-fundamentals/01-what-is-rust/audio.mp3"
            );
            assert_eq!(audio.duration.as_deref(), Some("5:00"));
            assert_eq!(lesson.code.len(), 1);
            assert_eq!(lesson.downloads.len(), 1);
            assert_eq!(lesson.downloads[0].size_bytes, "pdfdata".len() as u64);
            Ok::<(), ()>(())
        })();
        std::env::set_current_dir(cwd).unwrap();
        res.unwrap();
    }

    #[test]
    fn test_scan_courses_full_tree() {
        let tmp = TempDir::new().unwrap();
        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let res = (|| {
            let root = tmp.path().join("assets/courses");
            // course rust-basics with two chapters
            write(
                &root.join("rust-basics/course.yaml"),
                "title: Rust Basics\ndescription: hello\norder: 1\ntags: [rust]\n",
            );
            write(&root.join("rust-basics/01-fundamentals/01-intro/index.md"), "# i");
            touch(&root.join("rust-basics/01-fundamentals/02-install/clip.mp4"));
            write(&root.join("rust-basics/02-ownership/01-borrow/index.md"), "# b");
            // course async-rust without yaml
            write(&root.join("async-rust/01-tokio/01-runtime/index.md"), "# r");
            // 隐藏目录应被跳过
            write(&root.join("_drafts/garbage/lesson/index.md"), "x");

            let courses = scan_courses(&root);
            assert_eq!(courses.len(), 2);
            let rust_basics = courses.iter().find(|c| c.slug == "rust-basics").unwrap();
            assert_eq!(rust_basics.title, "Rust Basics");
            assert_eq!(rust_basics.order, 1);
            assert_eq!(rust_basics.chapters.len(), 2);
            let ch1 = &rust_basics.chapters[0];
            assert_eq!(ch1.lessons.len(), 2);
            assert_eq!(ch1.lessons[0].kind, LessonKind::Doc);
            assert_eq!(ch1.lessons[1].kind, LessonKind::Video);

            let async_rust = courses.iter().find(|c| c.slug == "async-rust").unwrap();
            assert_eq!(async_rust.title, "Async Rust");
            Ok::<(), ()>(())
        })();
        std::env::set_current_dir(cwd).unwrap();
        res.unwrap();
    }

    #[test]
    fn test_normalize_visibility_known_values() {
        assert_eq!(normalize_visibility(Some("public")), "public");
        assert_eq!(normalize_visibility(Some("course-public")), "course-public");
        assert_eq!(normalize_visibility(Some("doc-public")), "doc-public");
        assert_eq!(normalize_visibility(Some("private")), "private");
    }

    #[test]
    fn test_normalize_visibility_fallback() {
        // 未知值 → private
        assert_eq!(normalize_visibility(Some("")), "private");
        assert_eq!(normalize_visibility(Some("PUBLIC")), "private");
        assert_eq!(normalize_visibility(Some("hacker-attempt")), "private");
        // None → private
        assert_eq!(normalize_visibility(None), "private");
    }

    #[test]
    fn test_default_annotation_enabled() {
        assert!(default_annotation_enabled("course"));
        assert!(default_annotation_enabled("doc"));
        assert!(!default_annotation_enabled("blog"));
        assert!(!default_annotation_enabled("unknown"));
    }

    #[test]
    fn test_scan_skips_courses_without_lessons() {
        let tmp = TempDir::new().unwrap();
        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let res = (|| {
            let root = tmp.path().join("assets/courses");
            // course 没有任何 lesson
            stdfs::create_dir_all(&root.join("empty/01-ch")).unwrap();
            // course 有一个 lesson
            write(&root.join("real/01-ch/01-le/index.md"), "# r");

            let courses = scan_courses(&root);
            assert_eq!(courses.len(), 1);
            assert_eq!(courses[0].slug, "real");
            Ok::<(), ()>(())
        })();
        std::env::set_current_dir(cwd).unwrap();
        res.unwrap();
    }
}
