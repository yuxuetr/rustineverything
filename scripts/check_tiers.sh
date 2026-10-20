#!/usr/bin/env bash
# 功能分层门禁（docs/FEATURE_TIERS.md L6）：四档 × server / web 两端各 `cargo check -p app`，
# 任一档有 error 或 warning 即失败。warning 也算失败：漏写 cfg 最先表现为 unused import。
#
# 用法：scripts/check_tiers.sh            # 全部 8 项
#       scripts/check_tiers.sh basic      # 只查某一档（basic / accounts / course / full）
set -u

tier_flags() {
  case "$1" in
    basic) echo "--no-default-features" ;;
    accounts) echo "--no-default-features --features accounts" ;;
    course) echo "--no-default-features --features course" ;;
    full) echo "" ;; # 默认 feature：web + payments
    *) echo "unknown tier: $1" >&2; exit 2 ;;
  esac
}

tiers=("${@:-basic accounts course full}")
# shellcheck disable=SC2206
tiers=(${tiers[*]})

failed=()
for tier in "${tiers[@]}"; do
  base=$(tier_flags "$tier")
  for side in server web; do
    flags="$base --features $side"
    # full 的 web 端 = 默认 feature 本身（已含 web）
    [ "$tier" = full ] && [ "$side" = web ] && flags=""
    printf '%-9s %-7s ' "$tier" "$side"
    # shellcheck disable=SC2086
    out=$(CARGO_TERM_COLOR=never cargo check -q -p app $flags 2>&1)
    status=$?
    problems=$(printf '%s\n' "$out" | grep -E '^(warning|error)' | grep -v '^warning: .* generated [0-9]+ warning')
    if [ $status -ne 0 ] || [ -n "$problems" ]; then
      echo "FAIL"
      printf '%s\n' "$out" | grep -E -A6 '^(warning|error)' | head -40
      failed+=("$tier/$side")
    else
      echo "ok"
    fi
  done
done

if [ ${#failed[@]} -gt 0 ]; then
  echo "tier check failed: ${failed[*]}"
  exit 1
fi
echo "all tiers ok"
