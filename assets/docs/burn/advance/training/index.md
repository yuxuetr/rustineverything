# 训练循环

使用 Burn 的 Learner API 进行模型训练：

```rust
let learner = LearnerBuilder::new("/tmp/burn")
    .with_optimizer(AdamConfig::new())
    .build(model, optim, lr_scheduler);

learner.fit(dataloader_train, dataloader_valid);
```
