#!/usr/bin/env bash
# 阶段 V：从 Dynadot 拉取续期后的证书，检查通过才替换并重启网关。服务器上每天跑一次（cron）。
#
# Dynadot 在证书到期前约一个月自动续期（域名用 Dynadot 的 DNS 时）；本脚本只负责把新证书取回来。
# 下载链接只给证书，私钥沿用 CERT_DIR 里现有的：新证书的公钥对不上现有私钥时拒绝替换并报错，
# 这时去 Dynadot 后台手动下载私钥。
#
# 环境变量：
#   DYNADOT_API_KEY  必填（从 CERT_ENV_FILE 读，默认 /data/rie/cert.env）
#   CERT_DIR         fullchain.pem / privkey.pem 所在目录，默认 /data/rie/certs
#   DOMAIN           默认 rustineverything.app
#   RESTART_CMD      替换后执行，默认 docker restart rie-gateway
#   CERT_URL         覆盖下载地址（测试用，可为 file://）；默认 Dynadot 永久下载链接
#   CA_FILE          覆盖信任根（测试用）；默认系统信任库
#
# 退出码：0 = 已替换或无需替换；1 = 有问题，现有证书不动。
set -euo pipefail

CERT_ENV_FILE=${CERT_ENV_FILE:-/data/rie/cert.env}
# shellcheck disable=SC1090
[ -f "$CERT_ENV_FILE" ] && . "$CERT_ENV_FILE"
CERT_DIR=${CERT_DIR:-/data/rie/certs}
DOMAIN=${DOMAIN:-rustineverything.app}
RESTART_CMD=${RESTART_CMD:-docker restart rie-gateway}

log() { echo "cert_refresh: $*"; }
fail() { log "ERROR: $*" >&2; exit 1; }

current="$CERT_DIR/fullchain.pem"
key="$CERT_DIR/privkey.pem"
[ -f "$current" ] && [ -f "$key" ] || fail "$CERT_DIR 里缺 fullchain.pem 或 privkey.pem"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
new="$tmp/fullchain.pem"

# key 只出现在 curl 的配置输入里，不进命令行参数（ps 看不到）
if [ -n "${CERT_URL:-}" ]; then
  url=$CERT_URL
else
  [ -n "${DYNADOT_API_KEY:-}" ] || fail "DYNADOT_API_KEY 未设置"
  url="https://www.dynadot.com/letsencrypt/download_cert?key=${DYNADOT_API_KEY}&domain=${DOMAIN}"
fi
printf 'url = "%s"\n' "$url" | curl -fsS --max-time 60 -K - -o "$new" \
  || fail "下载失败（403 多半是 Cloudflare 人机验证：这台机器的 IP 被拦）"
grep -q -- '-----BEGIN CERTIFICATE-----' "$new" || fail "下载内容不是 PEM 证书（$(head -c 80 "$new" | tr -d '\n')）"

leaf="$tmp/leaf.pem"
openssl x509 -in "$new" -out "$leaf" 2>/dev/null || fail "解析证书失败"

fingerprint() { openssl x509 -in "$1" -noout -fingerprint -sha256; }
if [ "$(fingerprint "$leaf")" = "$(fingerprint "$current")" ]; then
  log "证书没变，不处理"
  exit 0
fi

openssl x509 -in "$leaf" -noout -ext subjectAltName | tr ',' '\n' | sed 's/^ *//' | grep -qxF "DNS:${DOMAIN}" \
  || fail "新证书不含 $DOMAIN"

pub_of_cert=$(openssl x509 -in "$leaf" -noout -pubkey | openssl sha256)
pub_of_key=$(openssl pkey -in "$key" -pubout | openssl sha256)
[ "$pub_of_cert" = "$pub_of_key" ] || fail "新证书与现有私钥不配对：私钥换了，去 Dynadot 后台下载新私钥"

verify_args=(-untrusted "$new")
[ -n "${CA_FILE:-}" ] && verify_args+=(-CAfile "$CA_FILE")
openssl verify "${verify_args[@]}" "$leaf" >/dev/null 2>&1 || fail "证书链校验失败"

not_after() { openssl x509 -in "$1" -noout -enddate | cut -d= -f2; }
newer=$(python3 -c 'import ssl, sys; print(int(ssl.cert_time_to_seconds(sys.argv[1]) > ssl.cert_time_to_seconds(sys.argv[2])))' \
  "$(not_after "$leaf")" "$(not_after "$current")")
[ "$newer" = 1 ] || fail "新证书到期日（$(not_after "$leaf")）不晚于现有的（$(not_after "$current")）"

cp "$current" "$current.bak"
cp "$new" "$current.new"
mv "$current.new" "$current"
log "已替换：到期日 $(not_after "$leaf")"
sh -c "$RESTART_CMD" || fail "证书已替换，但重启网关失败：$RESTART_CMD"
log "已重启网关"
