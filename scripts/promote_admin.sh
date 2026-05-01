#!/usr/bin/env bash
# 把指定昵称的用户角色置为 admin。
# 用法: scripts/promote_admin.sh <昵称>
# 依赖: psql + DATABASE_URL 环境变量（或手动 export）。

set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "用法: $0 <昵称>"
  exit 1
fi

NICKNAME="$1"
DB_URL="${DATABASE_URL:-postgres://postgres:password@localhost/rustineverything}"

echo "将 nickname = ${NICKNAME} 设为 admin (DB: ${DB_URL})"
echo "更新前匹配的用户:"
psql "${DB_URL}" -c "SELECT id, nickname, role FROM users WHERE nickname = '${NICKNAME}';"

psql "${DB_URL}" -c "UPDATE users SET role = 'admin', updated_at = NOW() WHERE nickname = '${NICKNAME}';"

echo "更新后:"
psql "${DB_URL}" -c "SELECT id, nickname, role FROM users WHERE nickname = '${NICKNAME}';"
echo "完成。请刷新浏览器并重新加载 cookie 中的 JWT（重新登录一次即可生效）。"
