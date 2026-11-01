#!/bin/sh
# 异地副本的接收端：放在备份机上，作为 authorized_keys 里的强制命令，
# 推送方的 key 只能干这一件事：
#
#   restrict,command="/usr/local/bin/rie-offsite-recv" ssh-ed25519 AAAA... rie-offsite
#
# 推送方把文件名当命令传（ssh host db-20261101-033001.dump.age < 文件），
# 这里只认 backup.sh 产生的两种文件名，只收 age 加密过的内容，写完各保留最新 KEEP 份。
# 备份机拿不到私钥，看不到明文。
#
# 环境变量：OFFSITE_DIR（默认 $HOME/rie-offsite）、KEEP（默认 30）
set -eu

dir=${OFFSITE_DIR:-$HOME/rie-offsite}
keep=${KEEP:-30}
name=${SSH_ORIGINAL_COMMAND:-}

fail() { echo "offsite-recv: $*" >&2; exit 1; }

echo "$name" | grep -Eqx '(db-[0-9]{8}-[0-9]{6}\.dump|uploads-[0-9]{8}-[0-9]{6}\.tar\.gz)\.age' \
  || fail "拒收文件名：$name"

mkdir -p "$dir"
tmp="$dir/.$name.tmp"
trap 'rm -f "$tmp"' EXIT
cat > "$tmp"
# age 二进制格式以这一行开头；明文（推送方忘了加密）直接拒收
[ "$(head -c 21 "$tmp")" = "age-encryption.org/v1" ] || fail "不是 age 加密文件：$name"
mv "$tmp" "$dir/$name"

for prefix in db- uploads-; do
  ls "$dir" | grep "^$prefix" | sort -r | tail -n +$((keep + 1)) | while read -r old; do rm -f "$dir/$old"; done
done
echo "offsite-recv: 已收 $name"
