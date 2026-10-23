#!/usr/bin/env bash
# 线上证书还剩几天：少于 MIN_DAYS 天（默认 20）或连不上就失败。
# 由 .github/workflows/cert-check.yml 每天跑；Dynadot 约在到期前 30 天续期，cert_refresh.sh 每天取回，
# 所以正常情况下剩余天数不会低于 30 太多。低于 20 说明续期或替换链路断了。
#
# 用法：scripts/check_cert_expiry.sh <host> [MIN_DAYS]
set -euo pipefail

host=${1:?用法：check_cert_expiry.sh <host> [MIN_DAYS]}
min_days=${2:-20}

end=$(echo | openssl s_client -connect "$host:443" -servername "$host" 2>/dev/null \
  | openssl x509 -noout -enddate 2>/dev/null | cut -d= -f2) || true
[ -n "$end" ] || { echo "check_cert_expiry: 连不上 $host:443 或拿不到证书" >&2; exit 1; }

days=$(python3 -c 'import ssl, sys, time; print(int((ssl.cert_time_to_seconds(sys.argv[1]) - time.time()) // 86400))' "$end")
echo "check_cert_expiry: $host 证书到期 $end，剩 $days 天（门槛 $min_days）"
[ "$days" -ge "$min_days" ]
