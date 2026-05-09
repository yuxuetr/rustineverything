# `<Blue />`

蓝色高亮文字（Mac Preview 标注色 `#3B82F6`）。

## 用法

```mdx
<Blue text="这段是蓝色高亮" />
```

## 属性

| 名 | 类型 | 必需 | 说明 |
|---|---|---|---|
| `text` | string | ✅ | 要高亮的文字内容 |

## 输出

```html
<span style="color: #3B82F6; font-weight: 600">这段是蓝色高亮</span>
```

## 代码

`crates/widgets/src/components.rs::ColorComponent`（`name = "Blue"`，
`color = "#3B82F6"`）。详见 [Yellow.md](./Yellow.md) 同系列说明。
