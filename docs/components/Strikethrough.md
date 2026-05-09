# `<Strikethrough />`

删除线文字（粗 2px）。

## 用法

```mdx
<Strikethrough text="已废弃" />
```

## 属性

| 名 | 类型 | 必需 | 说明 |
|---|---|---|---|
| `text` | string | ✅ | 要加删除线的文字 |

## 输出

```html
<span style="text-decoration: line-through; text-decoration-thickness: 2px">已废弃</span>
```

## 代码

`crates/widgets/src/components.rs::StrikethroughComponent`。

## 与 GFM `~~text~~` 的区别

GFM 的 `~~text~~` 同样产生删除线，但样式更细且依赖 `prose` typography
默认值。`<Strikethrough text="…" />` 给出固定的 2px 粗细，更适合需要
视觉强调的标注场景。
