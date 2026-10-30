//! 论文资讯（module-papers）的三张表。
//!
//! - `papers`：一篇论文一行，主键是不带版本的 arXiv id；HF 列表里再次出现时只更新
//!   `last_seen_on` / `hf_upvotes` / `arxiv_version`。
//! - `paper_summaries`：一篇论文每种语言一行解读，`digest` 是 JSON 文本。
//! - `paper_ingest_runs`：采集程序每跑一次记一行，用来看篇数、token 用量和耗时。

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261029_000011_papers"
  }
}

#[derive(DeriveIden)]
enum Papers {
  Table,
  ArxivId,
  Title,
  Abstract,
  Authors,
  ArxivVersion,
  PublishedAt,
  FirstSeenOn,
  LastSeenOn,
  HfUpvotes,
  GithubRepo,
  HasHtml,
  Status,
  Error,
  CreatedAt,
  UpdatedAt,
}

#[derive(DeriveIden)]
enum PaperSummaries {
  Table,
  ArxivId,
  Lang,
  Digest,
  Model,
  PromptVersion,
  UnverifiedNumbers,
  CreatedAt,
}

#[derive(DeriveIden)]
enum PaperIngestRuns {
  Table,
  Id,
  RunOn,
  Fetched,
  Inserted,
  Generated,
  Failed,
  InputTokens,
  OutputTokens,
  DurationMs,
  StartedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Papers::Table)
          .col(ColumnDef::new(Papers::ArxivId).string_len(32).not_null().primary_key())
          .col(ColumnDef::new(Papers::Title).text().not_null())
          .col(ColumnDef::new(Papers::Abstract).text().not_null())
          // JSON 数组文本（同 moderation_queue.images，不引入 with-json）
          .col(ColumnDef::new(Papers::Authors).text().not_null())
          .col(ColumnDef::new(Papers::ArxivVersion).integer())
          .col(ColumnDef::new(Papers::PublishedAt).timestamp_with_time_zone().not_null())
          .col(ColumnDef::new(Papers::FirstSeenOn).date().not_null())
          .col(ColumnDef::new(Papers::LastSeenOn).date().not_null())
          .col(ColumnDef::new(Papers::HfUpvotes).integer().not_null().default(0))
          .col(ColumnDef::new(Papers::GithubRepo).text())
          // 取全文之前不知道有没有 HTML
          .col(ColumnDef::new(Papers::HasHtml).boolean())
          // pending / done / failed / skipped
          .col(ColumnDef::new(Papers::Status).string_len(16).not_null().default("pending"))
          .col(ColumnDef::new(Papers::Error).text())
          .col(ColumnDef::new(Papers::CreatedAt).timestamp_with_time_zone().not_null())
          .col(ColumnDef::new(Papers::UpdatedAt).timestamp_with_time_zone().not_null())
          .to_owned(),
      )
      .await?;
    // `/papers` 按日期列出
    manager
      .create_index(
        Index::create()
          .name("idx_papers_first_seen_on")
          .table(Papers::Table)
          .col(Papers::FirstSeenOn)
          .to_owned(),
      )
      .await?;

    manager
      .create_table(
        Table::create()
          .table(PaperSummaries::Table)
          .col(ColumnDef::new(PaperSummaries::ArxivId).string_len(32).not_null())
          .col(ColumnDef::new(PaperSummaries::Lang).string_len(8).not_null())
          .col(ColumnDef::new(PaperSummaries::Digest).text().not_null())
          .col(ColumnDef::new(PaperSummaries::Model).string_len(64).not_null())
          .col(ColumnDef::new(PaperSummaries::PromptVersion).integer().not_null())
          // 原文里找不到的数字，JSON 数组文本
          .col(ColumnDef::new(PaperSummaries::UnverifiedNumbers).text().not_null().default("[]"))
          .col(ColumnDef::new(PaperSummaries::CreatedAt).timestamp_with_time_zone().not_null())
          .primary_key(Index::create().col(PaperSummaries::ArxivId).col(PaperSummaries::Lang))
          .foreign_key(
            ForeignKey::create()
              .name("fk_paper_summaries_paper")
              .from(PaperSummaries::Table, PaperSummaries::ArxivId)
              .to(Papers::Table, Papers::ArxivId)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .to_owned(),
      )
      .await?;

    manager
      .create_table(
        Table::create()
          .table(PaperIngestRuns::Table)
          .col(
            ColumnDef::new(PaperIngestRuns::Id)
              .big_integer()
              .not_null()
              .auto_increment()
              .primary_key(),
          )
          .col(ColumnDef::new(PaperIngestRuns::RunOn).date().not_null())
          .col(ColumnDef::new(PaperIngestRuns::Fetched).integer().not_null())
          .col(ColumnDef::new(PaperIngestRuns::Inserted).integer().not_null())
          .col(ColumnDef::new(PaperIngestRuns::Generated).integer().not_null())
          .col(ColumnDef::new(PaperIngestRuns::Failed).integer().not_null())
          .col(ColumnDef::new(PaperIngestRuns::InputTokens).big_integer().not_null())
          .col(ColumnDef::new(PaperIngestRuns::OutputTokens).big_integer().not_null())
          .col(ColumnDef::new(PaperIngestRuns::DurationMs).big_integer().not_null())
          .col(ColumnDef::new(PaperIngestRuns::StartedAt).timestamp_with_time_zone().not_null())
          .to_owned(),
      )
      .await
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    for table in [
      PaperIngestRuns::Table.into_iden(),
      PaperSummaries::Table.into_iden(),
      Papers::Table.into_iden(),
    ] {
      manager.drop_table(Table::drop().table(table).to_owned()).await?;
    }
    Ok(())
  }
}
