# `<Bilibili />`

嵌入一个 16:9 的 Bilibili 视频。

## 用法

```mdx
<Bilibili id="BV1xx411c7m1" />
```

## 属性

| 名 | 类型 | 必需 | 说明 |
|---|---|---|---|
| `id` | string | ✅ | Bilibili BV 号（如 `BV1xx411c7m1`） |

## 输出

```html
<div class="not-prose aspect-video my-8 overflow-hidden rounded-2xl shadow-2xl border border-slate-200 dark:border-slate-800">
  <iframe class="w-full h-full border-0"
          src="//player.bilibili.com/player.html?bvid={id}&page=1&high_quality=1"
          allowfullscreen></iframe>
</div>
```

## 代码

`crates/widgets/src/components.rs::BilibiliComponent`。
