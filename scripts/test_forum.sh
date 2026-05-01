#!/usr/bin/env bash
# 论坛 API 端到端冒烟脚本
#
# 前置条件：
#   1. `dx serve` 已运行（默认 http://localhost:8080）
#   2. 你已通过 OAuth 登录，浏览器拿到了 session cookie
#   3. 把浏览器里的 cookie 复制到环境变量 RIE_COOKIE 中：
#        RIE_COOKIE='session=eyJ...'
#
# 用法：
#   RIE_COOKIE='session=...' bash scripts/test_forum.sh
#
# 流程：
#   1. baseline list_topics
#   2. create_topic 一条独立话题
#   3. create_topic 一条带 ref 的话题（ref_kind=blog, ref_path=test-post）
#   4. list_topics 验证两条都返回，并验证带 ref 的有 reference 字段
#   5. list_topics_by_ref 应返回带 ref 的那条
#   6. list_tags 应包含两个 tag
#   7. post_reply 给第一条话题；get_topic 验证 reply_count=1
#   8. list_my_topics 应包含两条
#
# 注意：本脚本会创建测试数据且不自动清理（DB 里没有 delete_topic 接口）。
#       请人工删除或在测试 DB 上运行。

set -u
set -o pipefail

BASE="${RIE_BASE:-http://localhost:8080}"
COOKIE="${RIE_COOKIE:-}"
TAG_PLAIN="rie-test-forum"
TAG_REF="rie-test-forum-ref"
REF_KIND="blog"
REF_PATH="test-post"

if [[ -z "$COOKIE" ]]; then
  echo "✗ 请先设置 RIE_COOKIE 环境变量（浏览器登录后复制 session cookie）" >&2
  exit 1
fi

post_json() {
  local endpoint="$1" body="$2"
  curl -sS -X POST "$BASE$endpoint" \
    -H "content-type: application/json" \
    -H "cookie: $COOKIE" \
    --data "$body"
}

count_items() {
  python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d) if isinstance(d,list) else 0)"
}

extract_id() {
  python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('id',''))"
}

# -- 1. baseline ---------------------------------------------------------
echo "→ [1] baseline list_topics"
RESP=$(post_json /api/topics/list '{"tag":null,"page":0}')
BASE_COUNT=$(echo "$RESP" | count_items)
echo "    当前总数：$BASE_COUNT"

# -- 2. create plain topic ----------------------------------------------
echo "→ [2] 创建独立话题（无 ref）"
BODY=$(cat <<EOF
{"input":{
  "title":"Forum 冒烟：基础发帖",
  "tag":"$TAG_PLAIN",
  "content":"测试正文 — Hello forum!",
  "ref_kind":null,
  "ref_path":null
}}
EOF
)
R=$(post_json /api/topics/create "$BODY")
ID_PLAIN=$(echo "$R" | extract_id)
[[ -n "$ID_PLAIN" ]] && echo "    ✓ id=$ID_PLAIN" || { echo "    ✗ create 失败: $R" >&2; exit 1; }

# -- 3. create ref topic ------------------------------------------------
echo "→ [3] 创建带 ref 的话题（kind=$REF_KIND, path=$REF_PATH）"
BODY=$(cat <<EOF
{"input":{
  "title":"Forum 冒烟：博客联动",
  "tag":"$TAG_REF",
  "content":"讨论博客 $REF_PATH 的细节",
  "ref_kind":"$REF_KIND",
  "ref_path":"$REF_PATH"
}}
EOF
)
R=$(post_json /api/topics/create "$BODY")
ID_REF=$(echo "$R" | extract_id)
[[ -n "$ID_REF" ]] && echo "    ✓ id=$ID_REF" || { echo "    ✗ create 失败: $R" >&2; exit 1; }

# -- 4. list_topics 应包含两条 -----------------------------------------
echo "→ [4] list_topics 应包含两条新话题"
RESP=$(post_json /api/topics/list '{"tag":null,"page":0}')
GOT=$(echo "$RESP" | count_items)
[[ "$GOT" -ge "$((BASE_COUNT + 2))" ]] && echo "    ✓ 数量 $GOT >= $((BASE_COUNT + 2))" \
  || { echo "    ✗ 数量异常 $GOT" >&2; exit 1; }

python3 - "$RESP" "$ID_REF" <<'PY'
import json, sys
d = json.loads(sys.argv[1])
target = int(sys.argv[2])
for t in d:
    if t["id"] == target:
        if t.get("reference") and t["reference"].get("kind") == "blog":
            print("    ✓ ref topic has reference field")
            sys.exit(0)
        print("    ✗ ref topic missing reference"); sys.exit(1)
print("    ✗ ref topic not found in list"); sys.exit(1)
PY

# -- 5. list_topics_by_ref ---------------------------------------------
echo "→ [5] list_topics_by_ref 反查"
RESP=$(post_json /api/topics/list-by-ref "{\"kind\":\"$REF_KIND\",\"path\":\"$REF_PATH\"}")
HIT=$(echo "$RESP" | python3 -c "
import json, sys
d=json.load(sys.stdin)
ids=[t['id'] for t in d]
print('OK' if $ID_REF in ids else 'FAIL')")
[[ "$HIT" == "OK" ]] && echo "    ✓ 反查命中 id=$ID_REF" \
  || { echo "    ✗ 反查未命中" >&2; exit 1; }

# -- 6. list_tags -------------------------------------------------------
echo "→ [6] list_tags 应包含 $TAG_PLAIN 和 $TAG_REF"
RESP=$(post_json /api/topics/tags '{}')
HIT=$(echo "$RESP" | python3 -c "
import json, sys
d=json.load(sys.stdin)
tags={t['tag'] for t in d}
need={'$TAG_PLAIN','$TAG_REF'}
print('OK' if need.issubset(tags) else 'FAIL')")
[[ "$HIT" == "OK" ]] && echo "    ✓ 标签云包含两个测试 tag" \
  || { echo "    ✗ 标签云缺失" >&2; exit 1; }

# -- 7. post_reply ------------------------------------------------------
echo "→ [7] post_reply 后 reply_count 应为 1"
post_json /api/topics/reply "{\"topic_id\":$ID_PLAIN,\"content\":\"+1 同意\"}" >/dev/null
RESP=$(post_json /api/topics/get "{\"id\":$ID_PLAIN}")
RC=$(echo "$RESP" | python3 -c "import json,sys; print(json.load(sys.stdin)['reply_count'])")
[[ "$RC" == "1" ]] && echo "    ✓ reply_count=1" \
  || { echo "    ✗ reply_count=$RC（期望 1）" >&2; exit 1; }

# -- 8. list_my_topics --------------------------------------------------
echo "→ [8] list_my_topics 应包含两条新话题"
RESP=$(post_json /api/topics/mine '{}')
TOTAL=$(echo "$RESP" | count_items)
[[ "$TOTAL" -ge 2 ]] && echo "    ✓ 我的话题数 $TOTAL >= 2" \
  || { echo "    ✗ 我的话题数 $TOTAL" >&2; exit 1; }

echo ""
echo "✅ 论坛全流程通过：创建 / 引用 / 反查 / 回复 / 个人列表"
echo "（注意：本脚本未自动清理测试数据，请按需手动删除）"
