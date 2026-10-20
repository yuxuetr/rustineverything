#!/bin/bash
# O3 验收：各路径 <title> 互不相同、不是纯站名、都以「 - 站名」结尾。退出码 0 = 通过。
export LC_ALL=C  # CJK 在部分 locale 下无排序权重，uniq 会把不同标题判为相同
BASE=${BASE:-http://localhost:8080}
paths="/ /blog /docs /podcast /course /case /topics /embedded /ai /web3 /cli /ecosystem/rust"
case1=$(curl -s --noproxy '*' $BASE/case | grep -o 'href="/case/[a-z0-9-]*"' | head -1 | cut -d'"' -f2); paths="$paths $case1"
course1=$(curl -s --noproxy '*' $BASE/course | grep -o 'href="/course/[a-z0-9-]*"' | head -1 | cut -d'"' -f2); paths="$paths $course1"
blog1=$(curl -s --noproxy '*' $BASE/blog | grep -o 'href="/blog/[a-z0-9-]*"' | head -1 | cut -d'"' -f2); paths="$paths $blog1"
out=""
for p in $paths; do
  t=$(curl -s --noproxy '*' $BASE$p | grep -o '<title>[^<]*</title>' | head -1 | sed 's/<[^>]*>//g')
  d=$(curl -s --noproxy '*' $BASE$p | grep -c '<link rel="canonical"')
  echo "$p | $t | canonical=$d"
  out="$out$t"$'\n'
done
bad=$(printf '%s' "$out" | grep -cxE '(Rust in Everything)?')
# 每页都带站名后缀（文章页的 frontmatter 标题也一样）
nosuffix=$(printf '%s' "$out" | grep -cv ' - Rust in Everything$')
dups=$(printf '%s' "$out" | sort | uniq -d | wc -l | tr -d ' ')
printf "%s" "$out" | sort | uniq -d | sed "s/^/DUP: /"
echo "site-name-only=$bad no-suffix=$nosuffix dups=$dups"
[ "$bad" -eq 0 ] && [ "$nosuffix" -eq 0 ] && [ "$dups" -eq 0 ]
