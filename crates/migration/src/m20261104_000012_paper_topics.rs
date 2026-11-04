//! 论文的主题与中文标题：`papers` 加 `topics`、`title_zh`。
//!
//! - `topics`：主题 slug 的 JSON 数组文本（列表见 module-papers 的 `topics.rs`）；
//!   NULL 表示还没分类，`papers-ingest --classify` 补。
//! - `title_zh`：模型翻译的中文标题，中文页面的标题和搜索结果用。

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261104_000012_paper_topics"
  }
}

#[derive(DeriveIden)]
enum Papers {
  Table,
  Topics,
  TitleZh,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Papers::Table)
          .add_column(ColumnDef::new(Papers::Topics).text())
          .add_column(ColumnDef::new(Papers::TitleZh).text())
          .to_owned(),
      )
      .await
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Papers::Table)
          .drop_column(Papers::Topics)
          .drop_column(Papers::TitleZh)
          .to_owned(),
      )
      .await
  }
}
