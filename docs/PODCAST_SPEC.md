# Podcast 动态化系统

## 概述

Podcast 模块支持从 `assets/podcasts/` 目录动态加载节目数据，无需修改代码即可发布新节目。提供三种使用模式，从最简单的「零配置」到完整的 YAML 元数据。

## 目录结构

```
assets/podcasts/
├── ai-deep-dive/
│   ├── episode.yaml        # 完整元数据
│   └── audio.m4a
├── prog-paradigms/
│   ├── episode.yaml        # YAML 不写 audio_url
│   └── deep-dive.m4a       # 自动检测
└── talk-001/
    └── 深度对谈.mp3        # 零配置：仅音频文件
```

## 三种使用模式

### 模式一：完整 YAML（推荐生产）

```yaml
# assets/podcasts/ai-deep-dive/episode.yaml
id: 1
title: 神经网络大揭秘：从零件到架构
description: 深入探讨神经网络的内部工作机制
duration: "24:15"
date: 2024-01-15
audio_url: /audio/foo.m4a    # 可选，省略则自动扫描
guest: 张三                   # 可选
tags: [AI, 神经网络, 深度学习]   # 可选
```

`audio_url` 支持三种格式：
- **绝对路径** `/audio/foo.m4a` — 原样保留
- **HTTP URL** `https://cdn.example.com/x.mp3` — 原样保留
- **相对路径** `audio.m4a` — 自动拼接为 `/podcasts/<slug>/audio.m4a`

### 模式二：YAML + 自动检测音频

```yaml
# episode.yaml 不写 audio_url
id: 10
title: 我的播客
date: 2024-04-01
```

```
my-show/
├── episode.yaml
└── deep-dive.m4a    # 自动作为播放源
```

### 模式三：零配置（最快上手）

只需把音频文件放进目录：

```
talk-001/
└── 深度对谈.mp3
```

自动推断：

| 字段 | 值 |
|------|---|
| `slug` | `talk-001`（目录名） |
| `title` | `深度对谈`（文件名去扩展名） |
| `id` | 由 slug 稳定哈希生成 |
| `date` | 文件 mtime（YYYY-MM-DD） |
| `url` | `/podcasts/talk-001/深度对谈.mp3` |

## 支持的音频格式

`m4a`、`mp3`、`wav`、`ogg`、`flac`、`aac`、`opus`、`mpeg`

## 多音频文件处理

当目录中有多个音频文件时，按文件名**字母顺序**选第一个。可以使用 `01-intro.mp3`、`02-main.mp3` 控制顺序。

## 节目排序

节目列表自动按 **日期降序** 排序（最新在前）。同日期按 id 倒序排列。

## 跳过规则

以下目录不会被扫描：
- `_` 开头的目录（如 `_drafts`）
- `.` 开头的目录（如 `.tmp`）
- 既无 YAML 又无音频文件的目录

## API

| Server Function | 说明 |
|-----------------|------|
| `list_episodes()` | 返回所有节目，按日期降序 |
| `get_episode_by_id(id: i32)` | 根据 id 查询单个节目 |

## 前端用法

### Podcast 页面（自动加载）

`/podcast` 页面通过 `use_resource(list_episodes)` 自动加载，无需手动配置。

### MDX 中嵌入 PodcastCard

在博客 `.mdx` 文件中：

```mdx
<PodcastCard id="1" />
```

会渲染一张播放卡片，自动从 server 加载该 id 的节目。

## 静态文件路由

`assets/podcasts/<slug>/<file>` 通过 axum 的 `/podcasts` nest_service 暴露为 `http://host/podcasts/<slug>/<file>`。

## 数据结构

```rust
pub struct Episode {
    pub id: i32,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub duration: String,
    pub date: String,             // YYYY-MM-DD
    pub url: String,              // 播放 URL
    pub guest: Option<String>,
    pub tags: Vec<String>,
}
```

## 测试

`crates/modules/podcast/src/server.rs` 包含 18 个单元测试：

- YAML 解析与各字段
- 三种 audio_url 格式
- 自动检测音频文件
- 零配置模式（无 YAML）
- 7 种音频扩展名
- 多音频文件按字母序选择
- 非音频文件被忽略
- slug 哈希稳定性
- 按日期降序排序
- 同日期 id 倒序
- 跳过 `_` 和 `.` 目录
- 跳过空目录
- 收录仅音频文件的目录
- 空根目录 / 不存在的根目录

运行测试：

```bash
cargo test -p module-podcast --features server --lib
```
