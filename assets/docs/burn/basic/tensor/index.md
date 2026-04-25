# 张量操作

Burn 的张量 API 类似 PyTorch：

```rust
use burn::tensor::Tensor;

let x = Tensor::<Backend, 2>::random([2, 3], Distribution::Default, &device);
let y = x.matmul(x.transpose());
```
