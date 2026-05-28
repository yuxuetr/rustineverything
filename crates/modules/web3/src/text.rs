//! 板块元数据 + 纯逻辑（无 IO / 无 dioxus），可独立单测。

pub const BOARD_ID: &str = "web3";
pub const BOARD_LABEL: &str = "Web3";
pub const BOARD_ROUTE: &str = "/web3";
pub const BOARD_TAGLINE: &str = "区块链与去中心化应用的 Rust 工具链：以太坊、Solana、Substrate 与智能合约。";

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
        slug: "evm",
        label: "以太坊 / EVM",
        blurb: "用 alloy 读写链上状态、发交易、解析事件日志。",
    },
    Subtopic {
        slug: "solana",
        label: "Solana",
        blurb: "Solana 程序与客户端开发，高吞吐链上逻辑。",
    },
    Subtopic {
        slug: "substrate",
        label: "Substrate",
        blurb: "用 polkadot-sdk 搭建自定义区块链与 runtime pallet。",
    },
    Subtopic {
        slug: "contracts",
        label: "智能合约",
        blurb: "ink! / Solana 程序 / EVM 字节码与合约交互。",
    },
    Subtopic {
        slug: "wallet",
        label: "钱包与签名",
        blurb: "密钥管理、签名方案与交易构造。",
    },
    Subtopic {
        slug: "indexing",
        label: "链上索引",
        blurb: "区块/事件扫描、状态重建与数据服务。",
    },
];

pub const FEATURED_CRATES: &[FeaturedCrate] = &[
    FeaturedCrate {
        name: "alloy",
        blurb: "以太坊互操作的现代 Rust 工具集",
        url: "https://github.com/alloy-rs/alloy",
    },
    FeaturedCrate {
        name: "revm",
        blurb: "高性能纯 Rust EVM 实现",
        url: "https://github.com/bluealloy/revm",
    },
    FeaturedCrate {
        name: "solana-sdk",
        blurb: "Solana 链上程序与客户端 SDK",
        url: "https://github.com/solana-labs/solana",
    },
    FeaturedCrate {
        name: "anchor",
        blurb: "Solana 程序开发框架",
        url: "https://www.anchor-lang.com",
    },
    FeaturedCrate {
        name: "polkadot-sdk",
        blurb: "Substrate / Polkadot 区块链框架",
        url: "https://github.com/paritytech/polkadot-sdk",
    },
    FeaturedCrate {
        name: "ink!",
        blurb: "面向 Substrate 的智能合约 eDSL",
        url: "https://use.ink",
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
        assert_eq!(BOARD_ID, "web3");
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
        assert_eq!(normalize_tag(" Alloy!! "), "alloy");
        assert_eq!(normalize_tag("solana-sdk"), "solana-sdk");
    }

    #[test]
    fn normalize_tags_dedups_and_drops_empty() {
        let tags = vec![
            "Alloy".to_string(),
            "alloy".to_string(),
            "   ".to_string(),
            "Solana".to_string(),
        ];
        assert_eq!(normalize_tags(&tags), vec!["alloy".to_string(), "solana".to_string()]);
    }

    #[test]
    fn subtopic_label_known_and_unknown() {
        assert_eq!(subtopic_label("solana"), Some("Solana"));
        assert_eq!(subtopic_label("does-not-exist"), None);
    }

    #[test]
    fn matches_query_empty_returns_true() {
        assert!(matches_query("t", "d", &[], ""));
        assert!(matches_query("t", "d", &[], "   "));
    }

    #[test]
    fn matches_query_hits_title_description_tags() {
        let tags = vec!["alloy".to_string(), "evm".to_string()];
        assert!(matches_query("Alloy 入门", "以太坊交互", &tags, "alloy"));
        assert!(matches_query("Alloy 入门", "以太坊交互", &tags, "以太坊"));
        assert!(matches_query("Alloy 入门", "以太坊交互", &tags, "evm"));
        assert!(!matches_query("Alloy 入门", "以太坊交互", &tags, "candle"));
    }

    #[test]
    fn matches_query_supports_chinese() {
        let tags = vec!["substrate".to_string()];
        assert!(matches_query("自建链", "substrate runtime", &tags, "自建"));
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
