#!/usr/bin/env bash
# backup.sh / restore_drill.sh 的测试：起一个临时 postgres 和一个放上传文件的容器，
# 覆盖「该过 / 该拦」的各种情况。用完删掉容器。
# 用法：scripts/test_backup.sh   （需要 docker；会拉 postgres:16-alpine、debian:trixie-slim，
#       拉不到时用 PG_IMAGE / APP_IMAGE 指定镜像源，如 PG_IMAGE=docker.1ms.run/library/postgres:16-alpine）
set -u

here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
pg=rie-bktest-pg
app=rie-bktest-app
cleanup() { docker rm -f "$pg" "$app" > /dev/null 2>&1; rm -rf "$work"; }
trap cleanup EXIT
cleanup
mkdir -p "$work"

docker run -d --name "$pg" -e POSTGRES_USER=rie -e POSTGRES_PASSWORD=x -e POSTGRES_DB=rie \
  "${PG_IMAGE:-postgres:16-alpine}" > /dev/null || exit 1
docker run -d --name "$app" "${APP_IMAGE:-debian:trixie-slim}" \
  sh -c 'mkdir -p /app/assets/uploads/2026 && sleep infinity' > /dev/null || exit 1
for _ in $(seq 30); do docker exec "$pg" pg_isready -U rie -d rie > /dev/null 2>&1 && break; sleep 1; done
sleep 2 # pg_isready 在初始化阶段的临时实例上也会通过，等正式实例起来
sql() { docker exec -i "$pg" psql -U rie -d rie -v ON_ERROR_STOP=1 -q; }
echo "create table users (id int primary key, name text); create table posts (id int, title text);
  insert into users select g, 'u' || g from generate_series(1, 50) g;
  insert into posts select g, 'p' || g from generate_series(1, 7) g;" | sql || exit 1
docker exec "$app" sh -c 'echo a > /app/assets/uploads/2026/a.png; echo b > /app/assets/uploads/b.png'

export BACKUP_DIR="$work/backups" PG_CONTAINER=$pg APP_CONTAINER=$app

pass=0
fail=0
# check <名字> <期望退出码> <命令...>
check() {
  name=$1 want=$2
  shift 2
  out=$("$@" 2>&1)
  code=$?
  if [ "$code" = "$want" ]; then
    pass=$((pass + 1))
    printf 'ok    %-34s %s\n' "$name" "$(echo "$out" | tail -1)"
  else
    fail=$((fail + 1))
    printf 'FAIL  %-34s exit=%s want=%s\n%s\n' "$name" "$code" "$want" "$out"
  fi
}
count() { find "$BACKUP_DIR" -maxdepth 1 -name "$1" | wc -l; }
latest() { find "$BACKUP_DIR" -maxdepth 1 -name 'db-*.dump' | sort | tail -1; }

check "备份 → 两份文件" 0 bash "$here/backup.sh"
check "  db 和 uploads 各一份" 0 test "$(count 'db-*.dump')$(count 'uploads-*.tar.gz')" = 11
check "演练：刚做的备份 → 对上" 0 bash "$here/restore_drill.sh"

echo "insert into users values (51, 'late');" | sql
check "备份后线上多一行 → 拦" 1 bash "$here/restore_drill.sh"

sleep 1
check "重新备份" 0 bash "$here/backup.sh"
check "演练：新备份 → 对上" 0 bash "$here/restore_drill.sh"
docker exec "$app" sh -c 'echo c > /app/assets/uploads/c.png'
check "备份后多一个上传文件 → 拦" 1 bash "$here/restore_drill.sh"

cp "$(latest)" "$work/full.dump"
head -c 2000 "$work/full.dump" > "$(latest)"
check "dump 被截断 → 拦" 1 bash "$here/restore_drill.sh"
cp "$work/full.dump" "$(latest)"
check "  临时库已删掉" 0 test -z "$(echo "select 1 from pg_database where datname = 'rie_restore_drill';" | docker exec -i "$pg" psql -U rie -d rie -qAt)"

before=$(count '*')
check "postgres 容器不存在 → 失败" 1 env PG_CONTAINER=rie-bktest-none bash "$here/backup.sh"
check "  不留半成品" 0 test "$(count '*')" = "$before"
check "app 容器不存在 → 失败" 1 env APP_CONTAINER=rie-bktest-none bash "$here/backup.sh"
check "  不留半成品" 0 test "$(count '*')" = "$before"

# OPERATIONS.md 里的上传恢复命令：解到一个新卷，文件数与内容对上
vol=rie-bktest-restore
docker volume rm "$vol" > /dev/null 2>&1
up=$(find "$BACKUP_DIR" -maxdepth 1 -name 'uploads-*.tar.gz' | sort | tail -1)
check "恢复上传包到新卷" 0 docker run --rm -v "$vol:/data" -v "$BACKUP_DIR:/in:ro" "${APP_IMAGE:-debian:trixie-slim}" \
  tar xzf "/in/$(basename "$up")" -C /data --strip-components=1
check "  2026/a.png 内容一致" 0 test "$(docker run --rm -v "$vol:/data:ro" "${APP_IMAGE:-debian:trixie-slim}" cat /data/2026/a.png)" = a
docker volume rm "$vol" > /dev/null 2>&1

sleep 1
check "KEEP=2 再备份一次" 0 env KEEP=2 bash "$here/backup.sh"
check "  各剩 2 份" 0 test "$(count 'db-*.dump')$(count 'uploads-*.tar.gz')" = 22

echo "pass=$pass fail=$fail"
[ "$fail" = 0 ]
