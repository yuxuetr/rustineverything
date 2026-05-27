//! Phase 7.1：初始 schema 迁移。
//!
//! 一对一移植根目录 `init.sql` 的 7 张表 + 索引到 SeaORM schema
//! builder。已有 SeaORM 实体（`crates/core/src/entities/*`）的列名、
//! 类型保持完全一致，避免 entity 与 schema 漂移。

use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20260527_000001_initial_schema"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    // ── users ───────────────────────────────────────────────
    manager
      .create_table(
        Table::create()
          .table(Users::Table)
          .if_not_exists()
          .col(pk_auto(Users::Id))
          .col(string_len(Users::Nickname, 255).not_null())
          .col(text_null(Users::AvatarUrl))
          .col(
            string_len(Users::Role, 50)
              .not_null()
              .default("member".to_string()),
          )
          .col(
            timestamp_with_time_zone(Users::CreatedAt)
              .not_null()
              .default(Expr::current_timestamp()),
          )
          .col(
            timestamp_with_time_zone(Users::UpdatedAt)
              .not_null()
              .default(Expr::current_timestamp()),
          )
          .to_owned(),
      )
      .await?;

    // ── comments ────────────────────────────────────────────
    manager
      .create_table(
        Table::create()
          .table(Comments::Table)
          .if_not_exists()
          .col(pk_auto(Comments::Id))
          .col(string_len(Comments::BlogId, 255).not_null())
          .col(integer(Comments::UserId).not_null())
          .col(text(Comments::Content).not_null())
          .col(
            timestamp_with_time_zone(Comments::CreatedAt)
              .not_null()
              .default(Expr::current_timestamp()),
          )
          .foreign_key(
            ForeignKey::create()
              .name("fk_comments_user")
              .from(Comments::Table, Comments::UserId)
              .to(Users::Table, Users::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .to_owned(),
      )
      .await?;

    // ── user_identities ─────────────────────────────────────
    manager
      .create_table(
        Table::create()
          .table(UserIdentities::Table)
          .if_not_exists()
          .col(pk_auto(UserIdentities::Id))
          .col(integer(UserIdentities::UserId).not_null())
          .col(string_len(UserIdentities::Provider, 50).not_null())
          .col(string_len(UserIdentities::ProviderUid, 255).not_null())
          .col(text_null(UserIdentities::AccessToken))
          .col(text_null(UserIdentities::RefreshToken))
          .col(
            timestamp_with_time_zone(UserIdentities::CreatedAt)
              .not_null()
              .default(Expr::current_timestamp()),
          )
          .foreign_key(
            ForeignKey::create()
              .name("fk_user_identities_user")
              .from(UserIdentities::Table, UserIdentities::UserId)
              .to(Users::Table, Users::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .index(
            Index::create()
              .name("uq_user_identities_provider_uid")
              .col(UserIdentities::Provider)
              .col(UserIdentities::ProviderUid)
              .unique(),
          )
          .to_owned(),
      )
      .await?;

    // ── course_progress（复合主键）──────────────────────────
    manager
      .create_table(
        Table::create()
          .table(CourseProgress::Table)
          .if_not_exists()
          .col(integer(CourseProgress::UserId).not_null())
          .col(string_len(CourseProgress::CourseSlug, 255).not_null())
          .col(text(CourseProgress::LessonPath).not_null())
          .col(
            boolean(CourseProgress::Completed)
              .not_null()
              .default(false),
          )
          .col(integer_null(CourseProgress::PositionSeconds))
          .col(
            timestamp_with_time_zone(CourseProgress::UpdatedAt)
              .not_null()
              .default(Expr::current_timestamp()),
          )
          .primary_key(
            Index::create()
              .col(CourseProgress::UserId)
              .col(CourseProgress::CourseSlug)
              .col(CourseProgress::LessonPath),
          )
          .foreign_key(
            ForeignKey::create()
              .name("fk_course_progress_user")
              .from(CourseProgress::Table, CourseProgress::UserId)
              .to(Users::Table, Users::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .to_owned(),
      )
      .await?;
    manager
      .create_index(
        Index::create()
          .name("idx_course_progress_user")
          .table(CourseProgress::Table)
          .col(CourseProgress::UserId)
          .to_owned(),
      )
      .await?;

    // ── annotations ─────────────────────────────────────────
    manager
      .create_table(
        Table::create()
          .table(Annotations::Table)
          .if_not_exists()
          .col(big_integer(Annotations::Id).not_null().auto_increment().primary_key())
          .col(integer(Annotations::UserId).not_null())
          .col(string_len(Annotations::ResourceKind, 32).not_null())
          .col(text(Annotations::ResourcePath).not_null())
          .col(string_len(Annotations::BlockId, 64).not_null())
          .col(integer(Annotations::StartOffset).not_null())
          .col(integer(Annotations::EndOffset).not_null())
          .col(text(Annotations::ExactText).not_null())
          .col(text_null(Annotations::PrefixText))
          .col(text_null(Annotations::SuffixText))
          .col(string_len(Annotations::Style, 32).not_null())
          .col(text_null(Annotations::Note))
          .col(
            string_len(Annotations::Visibility, 16)
              .not_null()
              .default("private".to_string()),
          )
          .col(
            timestamp_with_time_zone(Annotations::CreatedAt)
              .not_null()
              .default(Expr::current_timestamp()),
          )
          .col(
            timestamp_with_time_zone(Annotations::UpdatedAt)
              .not_null()
              .default(Expr::current_timestamp()),
          )
          .foreign_key(
            ForeignKey::create()
              .name("fk_annotations_user")
              .from(Annotations::Table, Annotations::UserId)
              .to(Users::Table, Users::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .to_owned(),
      )
      .await?;
    manager
      .create_index(
        Index::create()
          .name("idx_annotations_resource")
          .table(Annotations::Table)
          .col(Annotations::ResourceKind)
          .col(Annotations::ResourcePath)
          .to_owned(),
      )
      .await?;
    manager
      .create_index(
        Index::create()
          .name("idx_annotations_user")
          .table(Annotations::Table)
          .col(Annotations::UserId)
          .to_owned(),
      )
      .await?;

    // ── topics ──────────────────────────────────────────────
    manager
      .create_table(
        Table::create()
          .table(Topics::Table)
          .if_not_exists()
          .col(pk_auto(Topics::Id))
          .col(string_len(Topics::Title, 255).not_null())
          .col(string_len(Topics::Tag, 64).not_null())
          .col(text(Topics::Content).not_null())
          .col(integer(Topics::UserId).not_null())
          .col(integer(Topics::ReplyCount).not_null().default(0))
          .col(timestamp_with_time_zone_null(Topics::LastReplyAt))
          .col(string_len_null(Topics::RefKind, 32))
          .col(text_null(Topics::RefPath))
          .col(
            timestamp_with_time_zone(Topics::CreatedAt)
              .not_null()
              .default(Expr::current_timestamp()),
          )
          .col(
            timestamp_with_time_zone(Topics::UpdatedAt)
              .not_null()
              .default(Expr::current_timestamp()),
          )
          .foreign_key(
            ForeignKey::create()
              .name("fk_topics_user")
              .from(Topics::Table, Topics::UserId)
              .to(Users::Table, Users::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .to_owned(),
      )
      .await?;
    manager
      .create_index(
        Index::create()
          .name("idx_topics_tag")
          .table(Topics::Table)
          .col(Topics::Tag)
          .to_owned(),
      )
      .await?;
    manager
      .create_index(
        Index::create()
          .name("idx_topics_user")
          .table(Topics::Table)
          .col(Topics::UserId)
          .to_owned(),
      )
      .await?;
    // recency 索引在 SeaORM 中不直接支持 NULLS LAST，只能按列顺序近似。
    // 实际查询若要严格 NULLS LAST，可保留 init.sql 行为或后续追加 raw SQL 迁移。
    manager
      .create_index(
        Index::create()
          .name("idx_topics_recency")
          .table(Topics::Table)
          .col((Topics::LastReplyAt, IndexOrder::Desc))
          .col((Topics::CreatedAt, IndexOrder::Desc))
          .to_owned(),
      )
      .await?;
    manager
      .create_index(
        Index::create()
          .name("idx_topics_ref")
          .table(Topics::Table)
          .col(Topics::RefKind)
          .col(Topics::RefPath)
          .to_owned(),
      )
      .await?;

    // ── topic_replies ───────────────────────────────────────
    manager
      .create_table(
        Table::create()
          .table(TopicReplies::Table)
          .if_not_exists()
          .col(pk_auto(TopicReplies::Id))
          .col(integer(TopicReplies::TopicId).not_null())
          .col(integer(TopicReplies::UserId).not_null())
          .col(text(TopicReplies::Content).not_null())
          .col(
            timestamp_with_time_zone(TopicReplies::CreatedAt)
              .not_null()
              .default(Expr::current_timestamp()),
          )
          .foreign_key(
            ForeignKey::create()
              .name("fk_topic_replies_topic")
              .from(TopicReplies::Table, TopicReplies::TopicId)
              .to(Topics::Table, Topics::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .foreign_key(
            ForeignKey::create()
              .name("fk_topic_replies_user")
              .from(TopicReplies::Table, TopicReplies::UserId)
              .to(Users::Table, Users::Id)
              .on_delete(ForeignKeyAction::Cascade),
          )
          .to_owned(),
      )
      .await?;
    manager
      .create_index(
        Index::create()
          .name("idx_topic_replies_topic")
          .table(TopicReplies::Table)
          .col(TopicReplies::TopicId)
          .col(TopicReplies::CreatedAt)
          .to_owned(),
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    // 逆序拆，避免外键依赖。
    for table in [
      TopicReplies::Table.into_iden(),
      Topics::Table.into_iden(),
      Annotations::Table.into_iden(),
      CourseProgress::Table.into_iden(),
      UserIdentities::Table.into_iden(),
      Comments::Table.into_iden(),
      Users::Table.into_iden(),
    ] {
      manager
        .drop_table(Table::drop().table(table).if_exists().to_owned())
        .await?;
    }
    Ok(())
  }
}

// ────────────────────────────────────────────────────────────
// Iden 定义。列名与 init.sql / SeaORM 实体保持字面一致。
// ────────────────────────────────────────────────────────────

#[derive(DeriveIden)]
enum Users {
  Table,
  Id,
  Nickname,
  AvatarUrl,
  Role,
  CreatedAt,
  UpdatedAt,
}

#[derive(DeriveIden)]
enum Comments {
  Table,
  Id,
  BlogId,
  UserId,
  Content,
  CreatedAt,
}

#[derive(DeriveIden)]
enum UserIdentities {
  Table,
  Id,
  UserId,
  Provider,
  ProviderUid,
  AccessToken,
  RefreshToken,
  CreatedAt,
}

#[derive(DeriveIden)]
enum CourseProgress {
  Table,
  UserId,
  CourseSlug,
  LessonPath,
  Completed,
  PositionSeconds,
  UpdatedAt,
}

#[derive(DeriveIden)]
enum Annotations {
  Table,
  Id,
  UserId,
  ResourceKind,
  ResourcePath,
  BlockId,
  StartOffset,
  EndOffset,
  ExactText,
  PrefixText,
  SuffixText,
  Style,
  Note,
  Visibility,
  CreatedAt,
  UpdatedAt,
}

#[derive(DeriveIden)]
enum Topics {
  Table,
  Id,
  Title,
  Tag,
  Content,
  UserId,
  ReplyCount,
  LastReplyAt,
  RefKind,
  RefPath,
  CreatedAt,
  UpdatedAt,
}

#[derive(DeriveIden)]
enum TopicReplies {
  Table,
  Id,
  TopicId,
  UserId,
  Content,
  CreatedAt,
}
