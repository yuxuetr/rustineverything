#!/usr/bin/env bash
# 阶段 V：每天备份数据库和上传文件到 BACKUP_DIR，各保留最新 KEEP 份。服务器上由 cron 跑。
#
#   db-<时间>.dump          pg_dump 自定义格式（pg_restore 恢复）
#   uploads-<时间>.tar.gz   上传目录（app 容器的 /app/assets/uploads）
#
# 按份数而不是按天数清理：备份连续失败时旧的不会被删光。
# 恢复和演练见 scripts/restore_drill.sh。
#
# 环境变量：BACKUP_DIR（默认 /data/rie/backups）、KEEP（默认 14）、
#           PG_CONTAINER（默认 rie-postgres）、APP_CONTAINER（默认 rie-app）
# 退出码：0 = 两份都写好；1 = 有问题，不留半成品文件，也不清理旧备份。
set -euo pipefail

BACKUP_DIR=${BACKUP_DIR:-/data/rie/backups}
KEEP=${KEEP:-14}
PG_CONTAINER=${PG_CONTAINER:-rie-postgres}
APP_CONTAINER=${APP_CONTAINER:-rie-app}

log() { echo "backup: $*"; }
fail() { log "ERROR: $*" >&2; exit 1; }

mkdir -p "$BACKUP_DIR"
stamp=$(date +%Y%m%d-%H%M%S)
db="$BACKUP_DIR/db-$stamp.dump"
uploads="$BACKUP_DIR/uploads-$stamp.tar.gz"
trap 'rm -f "$db.tmp" "$uploads.tmp"' EXIT

# 容器里的 POSTGRES_USER / POSTGRES_DB 就是 compose 传进去的；本地 socket 免密
docker exec "$PG_CONTAINER" sh -c 'pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" -Fc' > "$db.tmp" \
  || fail "pg_dump 失败"
docker exec -i "$PG_CONTAINER" pg_restore -l < "$db.tmp" > /dev/null || fail "dump 读不回来"

# docker cp 输出 tar 流，容器停着也能拷
docker cp "$APP_CONTAINER:/app/assets/uploads" - | gzip > "$uploads.tmp" || fail "打包上传目录失败"
gzip -t "$uploads.tmp" || fail "上传目录的压缩包损坏"

mv "$db.tmp" "$db"
mv "$uploads.tmp" "$uploads"
log "已写入 $(basename "$db")（$(du -h "$db" | cut -f1)）、$(basename "$uploads")（$(du -h "$uploads" | cut -f1)）"

# 文件名里的时间可按字典序排序，保留最新 KEEP 份
for prefix in db- uploads-; do
  find "$BACKUP_DIR" -maxdepth 1 -name "$prefix*" ! -name '*.tmp' -printf '%f\n' | sort -r | tail -n +$((KEEP + 1)) \
    | while read -r old; do rm -f "$BACKUP_DIR/$old" && log "删除旧备份 $old"; done
done
