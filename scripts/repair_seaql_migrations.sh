#!/usr/bin/env bash
# Phase 7.1 + 4.5 一次性修复：把 init.sql 时代建好的旧 schema 与
# sea-orm-migration 的 `seaql_migrations` 跟踪表对齐。
#
# 适用条件：你的 postgres 当前的表是通过 `init.sql` 直接灌入的（而不是
# 通过 `Migrator::up` 跑出来的），导致 `seaql_migrations` 里记不到
# `m20260527_000001_initial_schema`，应用每次启动 Migrator 都会尝试重
# 跑初始迁移并失败（`relation "..." already exists`）。
#
# 修复策略：直接给 `seaql_migrations` 写一条 "initial migration applied"
# 记录，让 Migrator 跳过它，从而能正常跑后续迁移（如 moderation_queue）。
#
# 使用：
#   ./scripts/repair_seaql_migrations.sh
#
# 读取 `.env` 中的 `DATABASE_URL`。

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="$ROOT/.env"

if [ ! -f "$ENV_FILE" ]; then
  echo "错误：找不到 $ENV_FILE" >&2
  exit 1
fi

# 取出 DATABASE_URL，不导出别的（避免污染 shell）
DATABASE_URL="$(grep -E '^DATABASE_URL=' "$ENV_FILE" | head -n1 | cut -d= -f2-)"
if [ -z "$DATABASE_URL" ]; then
  echo "错误：.env 中 DATABASE_URL 为空" >&2
  exit 1
fi

# 用 psql 跑修复语句。先建 seaql_migrations（如果没建过），再 upsert 初始记录。
# 注意：sea-orm-migration v1 的表 schema 是 (version VARCHAR(255), applied BIGINT)。
psql "$DATABASE_URL" <<'SQL'
CREATE TABLE IF NOT EXISTS seaql_migrations (
    version VARCHAR(255) PRIMARY KEY,
    applied BIGINT NOT NULL
);

INSERT INTO seaql_migrations (version, applied)
VALUES ('m20260527_000001_initial_schema', extract(epoch from now())::bigint)
ON CONFLICT (version) DO NOTHING;

-- 打印当前状态便于确认
SELECT version, to_timestamp(applied) AS applied_at FROM seaql_migrations ORDER BY version;
SQL

echo
echo "[OK] 修复完成。下次启动应用时 Migrator 将跳过 initial_schema，"
echo "     并跑 m20260530_000002_moderation_queue 等新迁移。"
