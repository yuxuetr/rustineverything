# `<YouTube />`

嵌入一个 16:9 的 YouTube 视频。

## 用法

```mdx
<YouTube id="dQw4w9WgXcQ" />
```

## 属性

| 名 | 类型 | 必需 | 说明 |
|---|---|---|---|
| `id` | string | ✅ | YouTube 视频 ID（URL 末尾那段，如 `dQw4w9WgXcQ`） |

## 输出

```html
<div class="not-prose aspect-video my-8 overflow-hidden rounded-2xl shadow-2xl border border-slate-200 dark:border-slate-800">
  <iframe class="w-full h-full" src="https://www.youtube.com/embed/{id}" allowfullscreen></iframe>
</div>
```

## 代码

`crates/widgets/src/components.rs::YouTubeComponent`。

由 `widgets::register_default_components()` 在 app
启动期注册。
