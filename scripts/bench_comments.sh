#!/usr/bin/env bash
#
# bench_comments.sh — 评论列表 (`POST /api/comments/list`) 延迟压测 (Phase 1A.2/1A.6)
#
# 前置环境：
#   1. app 服务运行中（本地：`cd crates/app && dx serve`，默认 http://localhost:8080）
#   2. PostgreSQL 可达，`.env` 的 DATABASE_URL 已迁移
#   3. 目标 blog_id 已有评论数据（量越大越有意义）。本地种子示例：
#        set -a; . ./.env; set +a
#        psql "$DATABASE_URL" -c "INSERT INTO comments (blog_id,user_id,content,created_at)
#          SELECT 'bench-test', (SELECT min(id) FROM users), 'bench #'||g, now()-(g||' seconds')::interval
#          FROM generate_series(1,5000) g;"
#      用完清理：psql "$DATABASE_URL" -c "DELETE FROM comments WHERE blog_id='bench-test';"
#
# 用法：
#   scripts/bench_comments.sh [N] [BLOG_ID] [BASE_URL]
#   N        请求数（默认 200）
#   BLOG_ID  目标博客（默认 bench-test，或 env BENCH_BLOG_ID）
#   BASE_URL 服务地址（默认 http://localhost:8080，或 env BENCH_BASE_URL）
#
# 若安装了 `oha`，自动用它（keep-alive，并发更真实）；否则回退到
# curl 顺序循环 + awk 百分位（每请求新建连接，本地 http 开销极小，可作上界估计）。
set -euo pipefail

N="${1:-200}"
BLOG_ID="${2:-${BENCH_BLOG_ID:-bench-test}}"
BASE_URL="${3:-${BENCH_BASE_URL:-http://localhost:8080}}"
ENDPOINT="${BASE_URL%/}/api/comments/list"
PAYLOAD="{\"blog_id\":\"${BLOG_ID}\"}"

echo "POST ${ENDPOINT}"
echo "blog_id=${BLOG_ID}  N=${N}"

# 预检：一次请求确认端点可用且返回 200
code="$(curl -s -o /dev/null -w '%{http_code}' -X POST "${ENDPOINT}" \
  -H 'Content-Type: application/json' -d "${PAYLOAD}")"
if [ "${code}" != "200" ]; then
  echo "预检失败：端点返回 HTTP ${code}（服务未起 / DB 不可达 / blog_id 无数据？）" >&2
  exit 1
fi

# 优先用 oha（更精确）
if command -v oha >/dev/null 2>&1; then
  echo "→ 使用 oha"
  exec oha -n "${N}" -c "${BENCH_CONCURRENCY:-10}" --no-tui \
    -m POST -H 'Content-Type: application/json' -d "${PAYLOAD}" "${ENDPOINT}"
fi

echo "→ oha 未安装，使用 curl 循环 + awk 百分位"
tmp="$(mktemp)"
trap 'rm -f "${tmp}"' EXIT

fail=0
for ((i = 0; i < N; i++)); do
  out="$(curl -s -o /dev/null -w '%{http_code} %{time_total}' -X POST "${ENDPOINT}" \
    -H 'Content-Type: application/json' -d "${PAYLOAD}")"
  rc="${out%% *}"
  t="${out##* }"
  if [ "${rc}" != "200" ]; then
    fail=$((fail + 1))
    continue
  fi
  awk -v t="${t}" 'BEGIN { printf "%d\n", t * 1000 }' >>"${tmp}"
done

count="$(wc -l <"${tmp}" | tr -d ' ')"
if [ "${count}" -eq 0 ]; then
  echo "无成功样本（fail=${fail}）" >&2
  exit 1
fi

sort -n "${tmp}" | awk -v fail="${fail}" '
  function pct(p,   idx) { idx = int((p / 100.0) * n + 0.999999); if (idx < 1) idx = 1; if (idx > n) idx = n; return a[idx] }
  { a[NR] = $1; sum += $1 }
  END {
    n = NR
    printf "samples=%d  fail=%d\n", n, fail
    printf "min=%dms  p50=%dms  p95=%dms  p99=%dms  max=%dms  avg=%.1fms\n", a[1], pct(50), pct(95), pct(99), a[n], sum / n
  }'
