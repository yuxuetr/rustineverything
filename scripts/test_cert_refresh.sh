#!/usr/bin/env bash
# cert_refresh.sh 的测试：自建 CA 签几张证书，覆盖「该换 / 不该换」的各种情况。
# 用法：scripts/test_cert_refresh.sh   （需要 openssl、python3、curl）
set -u

here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work" || exit 1

DOMAIN=example.test

# 自建 CA 与另一个不受信的 CA
mkca() {
  openssl req -x509 -newkey rsa:2048 -nodes -keyout "$1.key" -out "$1.pem" -days 30 -subj "/CN=$1" \
    -addext basicConstraints=critical,CA:TRUE -addext keyUsage=critical,keyCertSign 2>/dev/null
}
mkca ca
mkca rogue
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out site.key 2>/dev/null
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out other.key 2>/dev/null

# leaf <out> <key> <ca> <days> <san>：输出「叶子 + CA」的 fullchain
leaf() {
  openssl req -new -key "$2" -subj "/CN=$5" -out "$1.csr" 2>/dev/null
  printf 'subjectAltName=DNS:%s\n' "$5" > "$1.ext"
  openssl x509 -req -in "$1.csr" -CA "$3.pem" -CAkey "$3.key" -CAcreateserial -days "$4" \
    -extfile "$1.ext" -out "$1.leaf" 2>/dev/null
  cat "$1.leaf" "$3.pem" > "$1.pem"
}
leaf old site.key ca 10 "$DOMAIN"
leaf renewed site.key ca 20 "$DOMAIN"
leaf shorter site.key ca 5 "$DOMAIN"
leaf newkey other.key ca 20 "$DOMAIN"
leaf wrongname site.key ca 20 other.test
leaf untrusted site.key rogue 20 "$DOMAIN"
echo '<!DOCTYPE html><title>Just a moment...</title>' > challenge.pem

pass=0
fail=0
# run <名字> <下载内容> <期望退出码> <期望是否替换 yes/no>
run() {
  rm -rf certs restarted
  mkdir certs
  cp old.pem certs/fullchain.pem
  cp site.key certs/privkey.pem
  out=$(CERT_ENV_FILE=/nonexistent CERT_DIR="$work/certs" DOMAIN=$DOMAIN CA_FILE="$work/ca.pem" \
    CERT_URL="file://$work/$2" RESTART_CMD="touch $work/restarted" bash "$here/cert_refresh.sh" 2>&1)
  code=$?
  if cmp -s certs/fullchain.pem old.pem; then replaced=no; else replaced=yes; fi
  restarted=no
  [ -e restarted ] && restarted=yes
  if [ "$code" = "$3" ] && [ "$replaced" = "$4" ] && [ "$restarted" = "$4" ]; then
    pass=$((pass + 1))
    printf 'ok    %-30s %s\n' "$1" "$(echo "$out" | tail -1)"
  else
    fail=$((fail + 1))
    printf 'FAIL  %-30s exit=%s replaced=%s restarted=%s\n%s\n' "$1" "$code" "$replaced" "$restarted" "$out"
  fi
}

run "续期后的证书 → 替换并重启" renewed.pem 0 yes
run "和现有的相同 → 不动" old.pem 0 no
run "到期日更早 → 拒绝" shorter.pem 1 no
run "私钥换了 → 拒绝" newkey.pem 1 no
run "域名不对 → 拒绝" wrongname.pem 1 no
run "不受信的 CA → 拒绝" untrusted.pem 1 no
run "人机验证页 → 拒绝" challenge.pem 1 no
run "下载失败 → 拒绝" missing.pem 1 no

echo "pass=$pass fail=$fail"
[ "$fail" = 0 ]
