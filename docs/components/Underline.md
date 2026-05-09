# `<Underline />`

下划线文字（粗 2px、距离基线 3px）。

## 用法

```mdx
<Underline text="重点强调" />
```

## 属性

| 名 | 类型 | 必需 | 说明 |
|---|---|---|---|
| `text` | string | ✅ | 要加下划线的文字 |

## 输出

```html
<span style="text-decoration: underline; text-decoration-thickness: 2px; text-underline-offset: 3px">重点强调</span>
```

## 代码

`crates/widgets/src/components.rs::UnderlineComponent`。
