//! 板块元数据 + 纯逻辑（无 IO / 无 dioxus），可独立单测。

pub const BOARD_ID: &str = "cli";
pub const BOARD_LABEL: &str = "CLI";
pub const BOARD_ROUTE: &str = "/cli";
pub const BOARD_TAGLINE: &str = "打造一流命令行工具：参数解析、TUI、进度反馈与分发。";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subtopic {
    pub slug: &'static str,
    pub label: &'static str,
    pub blurb: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeaturedCrate {
    pub name: &'static str,
    pub blurb: &'static str,
    pub url: &'static str,
}

pub const SUBTOPICS: &[Subtopic] = &[
    Subtopic {
        slug: "args",
        label: "参数解析",
        blurb: "用 clap 的 derive 宏声明子命令、标志与校验。",
    },
    Subtopic {
        slug: "tui",
        label: "终端 UI",
        blurb: "ratatui 构建全屏交互式终端界面。",
    },
    Subtopic {
        slug: "output",
        label: "进度与输出",
        blurb: "进度条、彩色输出与人性化的状态反馈。",
    },
    Subtopic {
        slug: "config",
        label: "配置",
        blurb: "分层配置：默认值、配置文件、环境变量、命令行。",
    },
    Subtopic {
        slug: "testing",
        label: "测试",
        blurb: "用 assert_cmd / trycmd 对 CLI 做端到端断言。",
    },
    Subtopic {
        slug: "distribution",
        label: "分发",
        blurb: "交叉编译、静态链接与一键安装脚本。",
    },
];

pub const FEATURED_CRATES: &[FeaturedCrate] = &[
    FeaturedCrate {
        name: "clap",
        blurb: "功能完备的命令行参数解析器",
        url: "https://github.com/clap-rs/clap",
    },
    FeaturedCrate {
        name: "ratatui",
        blurb: "构建终端用户界面（TUI）",
        url: "https://ratatui.rs",
    },
    FeaturedCrate {
        name: "indicatif",
        blurb: "进度条与 spinner",
        url: "https://github.com/console-rs/indicatif",
    },
    FeaturedCrate {
        name: "console",
        blurb: "终端样式、颜色与交互工具",
        url: "https://github.com/console-rs/console",
    },
    FeaturedCrate {
        name: "crossterm",
        blurb: "跨平台终端操作库",
        url: "https://github.com/crossterm-rs/crossterm",
    },
    FeaturedCrate {
        name: "assert_cmd",
        blurb: "CLI 端到端测试断言",
        url: "https://github.com/assert-rs/assert_cmd",
    },
];

pub trait DatedArticle {
    fn date(&self) -> &str;
    fn title(&self) -> &str;
}

pub fn normalize_tag(raw: &str) -> String {
    raw.trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

pub fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> = tags
        .iter()
        .map(|t| normalize_tag(t))
        .filter(|t| !t.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

pub fn subtopic_label(slug: &str) -> Option<&'static str> {
    SUBTOPICS
        .iter()
        .find(|s| s.slug == slug)
        .map(|s| s.label)
}

pub fn matches_query(title: &str, description: &str, tags: &[String], query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    title.to_lowercase().contains(&q)
        || description.to_lowercase().contains(&q)
        || tags.iter().any(|t| t.to_lowercase().contains(&q))
}

pub fn sort_by_date_desc<T: DatedArticle>(items: &mut [T]) {
    items.sort_by(|a, b| {
        b.date()
            .cmp(a.date())
            .then_with(|| a.title().to_lowercase().cmp(&b.title().to_lowercase()))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    struct A {
        date: String,
        title: String,
    }
    impl DatedArticle for A {
        fn date(&self) -> &str {
            &self.date
        }
        fn title(&self) -> &str {
            &self.title
        }
    }

    #[test]
    fn board_constants_well_formed() {
        assert_eq!(BOARD_ID, "cli");
        assert!(BOARD_ROUTE.starts_with('/'));
        assert!(!BOARD_LABEL.is_empty());
        assert!(!BOARD_TAGLINE.is_empty());
    }

    #[test]
    fn subtopics_have_unique_slugs() {
        let mut slugs: Vec<&str> = SUBTOPICS.iter().map(|s| s.slug).collect();
        let n = slugs.len();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), n, "子主题 slug 应唯一");
    }

    #[test]
    fn subtopics_are_non_empty() {
        assert!(SUBTOPICS.len() >= 4);
        for s in SUBTOPICS {
            assert!(!s.slug.is_empty());
            assert!(!s.label.is_empty());
            assert!(!s.blurb.is_empty());
        }
    }

    #[test]
    fn subtopic_slugs_are_url_safe() {
        for s in SUBTOPICS {
            assert_eq!(normalize_tag(s.slug), s.slug, "slug 应已是归一化形态");
        }
    }

    #[test]
    fn featured_crates_use_https() {
        assert!(FEATURED_CRATES.len() >= 4);
        for c in FEATURED_CRATES {
            assert!(c.url.starts_with("https://"), "{} 应为 https URL", c.name);
            assert!(!c.name.is_empty());
            assert!(!c.blurb.is_empty());
        }
    }

    #[test]
    fn featured_crates_unique_names() {
        let mut names: Vec<&str> = FEATURED_CRATES.iter().map(|c| c.name).collect();
        let n = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), n);
    }

    #[test]
    fn normalize_tag_lowercases_and_strips() {
        assert_eq!(normalize_tag(" Clap!! "), "clap");
        assert_eq!(normalize_tag("assert_cmd"), "assert_cmd");
    }

    #[test]
    fn normalize_tags_dedups_and_drops_empty() {
        let tags = vec![
            "Clap".to_string(),
            "clap".to_string(),
            "   ".to_string(),
            "Ratatui".to_string(),
        ];
        assert_eq!(normalize_tags(&tags), vec!["clap".to_string(), "ratatui".to_string()]);
    }

    #[test]
    fn subtopic_label_known_and_unknown() {
        assert_eq!(subtopic_label("tui"), Some("终端 UI"));
        assert_eq!(subtopic_label("does-not-exist"), None);
    }

    #[test]
    fn matches_query_empty_returns_true() {
        assert!(matches_query("t", "d", &[], ""));
        assert!(matches_query("t", "d", &[], "   "));
    }

    #[test]
    fn matches_query_hits_title_description_tags() {
        let tags = vec!["clap".to_string(), "args".to_string()];
        assert!(matches_query("Clap 入门", "参数解析", &tags, "clap"));
        assert!(matches_query("Clap 入门", "参数解析", &tags, "参数"));
        assert!(matches_query("Clap 入门", "参数解析", &tags, "args"));
        assert!(!matches_query("Clap 入门", "参数解析", &tags, "solana"));
    }

    #[test]
    fn matches_query_supports_chinese() {
        let tags = vec!["tui".to_string()];
        assert!(matches_query("终端界面", "ratatui 实战", &tags, "终端"));
    }

    #[test]
    fn sort_by_date_desc_orders_newest_first_then_title() {
        let mut items = vec![
            A { date: "2026-01-01".into(), title: "beta".into() },
            A { date: "2026-03-01".into(), title: "zeta".into() },
            A { date: "2026-01-01".into(), title: "alpha".into() },
        ];
        sort_by_date_desc(&mut items);
        let order: Vec<&str> = items.iter().map(|a| a.title.as_str()).collect();
        assert_eq!(order, vec!["zeta", "alpha", "beta"]);
    }
}
