//! 论文的主题分类。固定列表：主题页的 URL、页面上的名字和模型分类用的说明都从这里来。
//!
//! 不用 arXiv 分类：2026-11-03 那天 46 篇里 38 篇落在 cs.CV / cs.AI / cs.LG / cs.CL 四类，
//! 读者没法靠它缩小范围；也不用 HF 的 `ai_keywords`：同一天 330 个关键词，大多只出现一次。

pub struct Topic {
  /// URL 段：`/papers/topic/<slug>`，也是 `papers.topics` 里存的值
  pub slug: &'static str,
  pub zh: &'static str,
  pub en: &'static str,
  /// 主题页的一句介绍；英文这句同时是给模型的分类说明
  pub about_zh: &'static str,
  pub about_en: &'static str,
}

/// 「其他」放最后：模型只在别的都不合适时选它。
pub const TOPICS: &[Topic] = &[
  Topic {
    slug: "llm",
    zh: "大模型与推理",
    en: "LLMs & Reasoning",
    about_zh: "大语言模型的预训练、架构、长上下文与推理能力。",
    about_en: "Large language models: pretraining, architecture, long context, reasoning.",
  },
  Topic {
    slug: "multimodal",
    zh: "多模态",
    en: "Multimodal",
    about_zh: "同时理解图像、视频、文本等多种输入的模型。",
    about_en: "Models that understand several modalities, e.g. vision-language models and video understanding.",
  },
  Topic {
    slug: "agents",
    zh: "智能体 Agent",
    en: "Agents",
    about_zh: "会调用工具、操作电脑或浏览器、完成多步任务的模型系统。",
    about_en: "LLM agents: tool use, computer/browser use, multi-step and multi-agent systems.",
  },
  Topic {
    slug: "generation",
    zh: "图像与视频生成",
    en: "Image & Video Generation",
    about_zh: "扩散模型、自回归生成、图像编辑与视频生成。",
    about_en: "Generating or editing images and video: diffusion, autoregressive generation, editing.",
  },
  Topic {
    slug: "3d-world",
    zh: "3D 与世界模型",
    en: "3D & World Models",
    about_zh: "3D 重建与生成、场景表示、可交互的世界模型。",
    about_en: "3D reconstruction and generation, scene representations, world models and simulation.",
  },
  Topic {
    slug: "rl-alignment",
    zh: "强化学习与对齐",
    en: "RL & Alignment",
    about_zh: "强化学习后训练、奖励模型、偏好对齐与安全。",
    about_en: "Reinforcement learning for post-training, reward models, preference alignment, safety.",
  },
  Topic {
    slug: "embodied",
    zh: "具身与机器人",
    en: "Embodied AI & Robotics",
    about_zh: "机器人操作、导航、视觉-语言-动作模型。",
    about_en: "Robotics, manipulation, navigation, vision-language-action models.",
  },
  Topic {
    slug: "audio",
    zh: "语音与音频",
    en: "Speech & Audio",
    about_zh: "语音识别与合成、音乐与音频生成。",
    about_en: "Speech recognition and synthesis, music and audio generation.",
  },
  Topic {
    slug: "efficiency",
    zh: "训练与推理效率",
    en: "Efficiency",
    about_zh: "量化、蒸馏、稀疏、推理加速与训练系统。",
    about_en: "Making training or inference cheaper: quantization, distillation, sparsity, serving, systems.",
  },
  Topic {
    slug: "evaluation",
    zh: "评测与数据集",
    en: "Benchmarks & Datasets",
    about_zh: "新基准、新数据集与评测方法。",
    about_en: "Papers whose main contribution is a benchmark, dataset or evaluation method.",
  },
  Topic {
    slug: "code",
    zh: "代码与软件工程",
    en: "Code & Software Engineering",
    about_zh: "代码生成、程序修复与软件工程中的模型。",
    about_en: "Code generation, program repair, models applied to software engineering.",
  },
  Topic {
    slug: "other",
    zh: "其他",
    en: "Other",
    about_zh: "不属于以上主题的论文。",
    about_en: "Only when none of the other topics fits.",
  },
];

/// 每篇最多几个主题。
pub const MAX_TOPICS: usize = 2;

pub fn find(slug: &str) -> Option<&'static Topic> {
  TOPICS.iter().find(|t| t.slug == slug)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn slugs_are_unique_and_url_safe() {
    for (i, t) in TOPICS.iter().enumerate() {
      assert!(t.slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
      assert!(TOPICS[..i].iter().all(|o| o.slug != t.slug), "duplicate {}", t.slug);
    }
    assert_eq!(TOPICS.last().map(|t| t.slug), Some("other"));
  }
}
