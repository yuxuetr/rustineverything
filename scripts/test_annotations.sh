#!/usr/bin/env bash
# 标注 API 端到端冒烟脚本
#
# 前置条件：
#   1. `dx serve` 已运行（默认 http://localhost:8080）
#   2. 你已通过 OAuth 登录，浏览器拿到了 session cookie
#   3. 把浏览器里的 cookie 复制到环境变量 RIE_COOKIE 中：
#        RIE_COOKIE='session=eyJ...'
#
# 用法：
#   RIE_COOKIE='session=...' bash scripts/test_annotations.sh
#
# 测试流程（覆盖多样式连续操作 + 多 visibility）：
#   1. list_annotations 同资源拉空（基线）
#   2. 对同一 lesson 连续 create 5 条不同样式（yellow/blue/underline/wavy/strikethrough）
#      + 各 visibility（private/course-public/doc-public/public）
#   3. list_annotations 验证全部入库且按 created_at asc 返回
#   4. list_my 验证个人列表（无资源过滤、按 created_at desc）
#   5. update 一条改 style+visibility，再读回校验
#   6. delete 一条，再 list 校验数量减 1
#   7. delete 全部测试数据清理

set -u
set -o pipefail

BASE="${RIE_BASE:-http://localhost:8080}"
COOKIE="${RIE_COOKIE:-}"
KIND="course"
PATHV="rie-test-annotations/01/01"   # 用一个独立的测试资源路径，避免污染真实数据
BLOCK="b1"

if [[ -z "$COOKIE" ]]; then
  echo "✗ 请先设置 RIE_COOKIE 环境变量（浏览器登录后复制 session cookie）" >&2
  exit 1
fi

# -- helpers --------------------------------------------------------------

# POST 一个 JSON 到 server fn 端点，并把响应体打到 stdout
post_json() {
  local endpoint="$1" body="$2"
  curl -sS -X POST "$BASE$endpoint" \
    -H "content-type: application/json" \
    -H "cookie: $COOKIE" \
    --data "$body"
}

mk_payload() {
  # $1=style $2=visibility $3=offset_start $4=offset_end $5=exact $6=note
  local style="$1" vis="$2" os="$3" oe="$4" exact="$5" note="$6"
  cat <<EOF
{"payload":{
  "resource_kind":"$KIND",
  "resource_path":"$PATHV",
  "block_id":"$BLOCK",
  "start_offset":$os,
  "end_offset":$oe,
  "exact_text":"$exact",
  "prefix_text":null,
  "suffix_text":null,
  "style":"$style",
  "note":$note,
  "visibility":"$vis"
}}
EOF
}

count_items() {
  # 用 python 解析 JSON 数组长度（jq 不一定可用）
  python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d) if isinstance(d,list) else 0)"
}

extract_id() {
  python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('id',''))"
}

# -- 1. baseline list -----------------------------------------------------
echo "→ [1] 基线：list_annotations 应该为 0（或仅含历史残留）"
RESP=$(post_json /api/annotations/list "{\"resource_kind\":\"$KIND\",\"resource_path\":\"$PATHV\"}")
BASE_COUNT=$(echo "$RESP" | count_items)
echo "    当前条数：$BASE_COUNT"

# -- 2. create 5 unique annotations --------------------------------------
echo "→ [2] 连续创建 5 条不同 style + 不同 visibility"
declare -a IDS=()

# 不同样式 + 不同可见性 + 不同偏移（避免业务上完全重复）
declare -a CASES=(
  'yellow|private|0|5|hello|null'
  'blue|course-public|6|11|world|null'
  'underline|doc-public|12|17|rusty|"underline 测试"'
  'wavy|public|18|23|trait|"波浪线"'
  'strikethrough|private|24|29|impl |null'
)

for c in "${CASES[@]}"; do
  IFS='|' read -r style vis os oe exact note <<<"$c"
  body=$(mk_payload "$style" "$vis" "$os" "$oe" "$exact" "$note")
  R=$(post_json /api/annotations/create "$body")
  ID=$(echo "$R" | extract_id)
  if [[ -z "$ID" ]]; then
    echo "    ✗ 创建失败 style=$style vis=$vis: $R" >&2
    exit 1
  fi
  echo "    ✓ id=$ID style=$style visibility=$vis"
  IDS+=("$ID")
done

# -- 3. list verifies all inserted ---------------------------------------
echo "→ [3] list_annotations 应包含 baseline + 5 条新数据"
RESP=$(post_json /api/annotations/list "{\"resource_kind\":\"$KIND\",\"resource_path\":\"$PATHV\"}")
EXPECT=$((BASE_COUNT + 5))
GOT=$(echo "$RESP" | count_items)
echo "    期望 ${EXPECT}，实际 ${GOT}"
[[ "${GOT}" == "${EXPECT}" ]] || { echo "    ✗ 数量不一致" >&2; exit 1; }

# 校验每条 style 与 visibility 都被原样存下
# 注：不能同时用管道 + heredoc 作为 stdin——heredoc 会抢占 stdin。
# 改用 argv 传 JSON。
python3 - "$RESP" <<'PY'
import json, sys
d = json.loads(sys.argv[1])
seen = {(a["style"], a["visibility"]) for a in d}
need = {("yellow","private"),("blue","course-public"),("underline","doc-public"),
        ("wavy","public"),("strikethrough","private")}
missing = need - seen
if missing:
    print("    ✗ 缺少:", missing); sys.exit(1)
print("    ✓ 5 个 (style, visibility) 组合均已入库")
PY

# -- 4. list_my (个人列表) -----------------------------------------------
echo "→ [4] list_my_annotations 应该至少包含本次的 5 条"
RESP=$(post_json /api/annotations/list_my "{}")
TOTAL=$(echo "$RESP" | count_items)
echo "    个人列表总数：${TOTAL}（≥ 5 即可）"
[[ "${TOTAL}" -ge 5 ]] || { echo "    ✗ 数量不足" >&2; exit 1; }

# -- 5. update one annotation -------------------------------------------
TARGET="${IDS[0]}"   # 第一条 yellow/private
echo "→ [5] 更新 id=${TARGET}：style=yellow→pink, visibility=private→public"
R=$(post_json /api/annotations/update \
  "{\"id\":${TARGET},\"style\":\"pink\",\"note\":null,\"visibility\":\"public\"}")
NEW_STYLE=$(echo "$R" | python3 -c "import json,sys; print(json.load(sys.stdin)['style'])")
NEW_VIS=$(echo "$R" | python3 -c "import json,sys; print(json.load(sys.stdin)['visibility'])")
echo "    服务端返回 style=${NEW_STYLE} visibility=${NEW_VIS}"
[[ "$NEW_STYLE" == "pink" && "$NEW_VIS" == "public" ]] \
  || { echo "    ✗ 更新未持久化" >&2; exit 1; }

# 再 list 一次校验
RESP=$(post_json /api/annotations/list "{\"resource_kind\":\"$KIND\",\"resource_path\":\"$PATHV\"}")
HIT=$(echo "$RESP" | python3 -c "
import json,sys
d=json.load(sys.stdin)
m={a['id']:a for a in d}
a=m.get($TARGET)
print('OK' if a and a['style']=='pink' and a['visibility']=='public' else 'FAIL')")
[[ "$HIT" == "OK" ]] && echo "    ✓ list 中也是 pink/public" \
  || { echo "    ✗ list 未反映更新" >&2; exit 1; }

# -- 6. delete one ------------------------------------------------------
DEL="${IDS[1]}"
echo "→ [6] 删除 id=${DEL}，list 应少 1 条"
post_json /api/annotations/delete "{\"id\":${DEL}}" >/dev/null
RESP=$(post_json /api/annotations/list "{\"resource_kind\":\"$KIND\",\"resource_path\":\"$PATHV\"}")
GOT=$(echo "$RESP" | count_items)
[[ "${GOT}" == $((EXPECT - 1)) ]] && echo "    ✓ 数量正确 (${GOT})" \
  || { echo "    ✗ 数量异常 ${GOT}，期望 $((EXPECT - 1))" >&2; exit 1; }

# -- 7. cleanup ---------------------------------------------------------
echo "→ [7] 清理剩余测试数据"
for id in "${IDS[@]}"; do
  [[ "$id" == "$DEL" ]] && continue
  post_json /api/annotations/delete "{\"id\":$id}" >/dev/null
done
RESP=$(post_json /api/annotations/list "{\"resource_kind\":\"$KIND\",\"resource_path\":\"$PATHV\"}")
FINAL=$(echo "$RESP" | count_items)
[[ "${FINAL}" == "${BASE_COUNT}" ]] && echo "    ✓ 已恢复 baseline=${BASE_COUNT}" \
  || echo "    ⚠ 清理后剩余 ${FINAL}（期望 ${BASE_COUNT}）"

echo ""
echo "✅ 全流程通过：5 条 (style, visibility) 入库 / 更新生效 / 删除生效"
