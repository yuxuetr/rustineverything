#!/usr/bin/env bash
# 恢复演练：把一份备份导入临时库，逐表比对行数；上传包比对文件数。不碰线上库。
# 线上在备份之后又有写入时，行数会对不上，挑没人写的时段跑，或拿刚做的备份跑。
#
# 用法：scripts/restore_drill.sh [db-<时间>.dump]   不给就用 BACKUP_DIR 里最新的一份
# 同名的 uploads-<时间>.tar.gz 一起检查。
#
# 真正恢复（停 app 后）：
#   docker exec -i rie-postgres sh -c 'pg_restore -U "$POSTGRES_USER" -d "$POSTGRES_DB" --clean --if-exists --no-owner' < db-<时间>.dump
#   docker run --rm -v rustineverything_app-uploads:/data -v "$PWD":/in:ro alpine \
#     tar xzf /in/uploads-<时间>.tar.gz -C /data --strip-components=1
#
# 环境变量同 backup.sh。退出码：0 = 全部对上；1 = 有出入或导入失败。
set -euo pipefail

BACKUP_DIR=${BACKUP_DIR:-/data/rie/backups}
PG_CONTAINER=${PG_CONTAINER:-rie-postgres}
APP_CONTAINER=${APP_CONTAINER:-rie-app}
DRILL_DB=rie_restore_drill

log() { echo "restore_drill: $*"; }
fail() { log "ERROR: $*" >&2; exit 1; }

db=${1:-$(find "$BACKUP_DIR" -maxdepth 1 -name 'db-*.dump' | sort | tail -1)}
[ -f "$db" ] || fail "找不到备份：${db:-$BACKUP_DIR 里没有 db-*.dump}"
uploads="$(dirname "$db")/uploads-$(basename "$db" .dump | sed 's/^db-//').tar.gz"
[ -f "$uploads" ] || fail "缺同一时间的上传包 $(basename "$uploads")"

psql_in() { docker exec -i "$PG_CONTAINER" sh -c "psql -U \"\$POSTGRES_USER\" -d $1 -v ON_ERROR_STOP=1 -qAt"; }
live_db=$(docker exec "$PG_CONTAINER" sh -c 'echo "$POSTGRES_DB"')

# 每张表一行「表名|行数」
COUNT_SQL="select table_name || '|' || (xpath('/row/c/text()', query_to_xml(format('select count(*) as c from public.%I', table_name), false, true, '')))[1]::text
  from information_schema.tables where table_schema = 'public' and table_type = 'BASE TABLE' order by 1;"

echo "drop database if exists $DRILL_DB; create database $DRILL_DB;" | psql_in postgres
trap 'echo "drop database if exists $DRILL_DB;" | psql_in postgres || true' EXIT
docker exec -i "$PG_CONTAINER" sh -c "pg_restore -U \"\$POSTGRES_USER\" -d $DRILL_DB --no-owner --exit-on-error" < "$db" \
  || fail "导入 $(basename "$db") 失败"

restored=$(echo "$COUNT_SQL" | psql_in "$DRILL_DB")
live=$(echo "$COUNT_SQL" | psql_in "$live_db")
[ -n "$restored" ] || fail "导入后一张表都没有"
if [ "$restored" != "$live" ]; then
  diff <(echo "$live") <(echo "$restored") | sed 's/^/  /' >&2 || true
  fail "表或行数对不上（< 线上，> 备份）"
fi
log "数据库：$(echo "$restored" | wc -l) 张表、共 $(echo "$restored" | awk -F'|' '{ n += $2 } END { print n }') 行，与线上一致"

in_backup=$(gunzip -c "$uploads" | tar -t | grep -vc '/$' || true)
in_live=$(docker exec "$APP_CONTAINER" find /app/assets/uploads -type f | wc -l)
[ "$in_backup" = "$in_live" ] || fail "上传文件数对不上：线上 $in_live，备份 $in_backup"
log "上传文件：$in_backup 个，与线上一致"
