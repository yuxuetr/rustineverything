# `<PodcastCard />`

在 MDX 中嵌入一张播客卡片：拉取播客元数据 + 嵌入 `<audio>` 控件。

## 用法

```mdx
<PodcastCard id="3" />
```

## 属性

| 名 | 类型 | 必需 | 说明 |
|---|---|---|---|
| `id` | integer (string-encoded) | ✅ | 播客剧集 ID（i32），对应 `assets/podcasts/<id>` |

## 解析失败

若 `id` 不能 parse 为 i32，渲染降级为黄色提示横幅：

```html
<div class="not-prose my-8 p-4 rounded-lg border border-amber-200 bg-amber-50 text-sm text-amber-800">
  PodcastCard 需提供有效的 id。
</div>
```

## 输出（成功）

调用 server fn `get_episode_by_id(id)`：

- 找到 → 渲染卡片 + audio 控件（剧集标题 / 时长 / 日期 / `<audio src=...>`）
- 找不到 → 红色错误横幅
- 加载中 → 灰色 placeholder

详细 RSX 见 `crates/modules/podcast/src/podcast.rs::PodcastCard`。

## 代码与注册

- 实现：`crates/modules/podcast/src/podcast.rs::PodcastCard`
- MDX 包装：`crates/modules/podcast/src/lib.rs::PodcastCardComponent`
- 注册：`module_podcast::register_components()` 在
  `crates/app/src/main.rs` 启动期调用一次

## 与 widgets 的解耦

widgets crate **不**直接依赖 podcast crate；PodcastCard 通过
[全局组件注册表](../MDX_SPEC.md#42-业务模块贡献的组件) 注入，因此
podcast 也成为「可关闭模块」的一员（详见 `ModuleEngine`）。
