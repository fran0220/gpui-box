#!/usr/bin/env bash
# Idempotently install GPUI Box's isolated BWG service boundary.
set -euo pipefail

[[ "$(id -u)" -eq 0 ]] || { echo "bootstrap must run as root" >&2; exit 1; }
source_dir="${1:?bootstrap needs its uploaded source directory}"
for command in curl file flock jq nginx python3 readelf runuser systemctl; do
  command -v "$command" >/dev/null || { echo "missing host command: $command" >&2; exit 1; }
done
[[ -s /etc/nginx/ssl/origingame.crt && -s /etc/nginx/ssl/origingame.key ]] || {
  echo "the BWG wildcard origingame.dev certificate is unavailable" >&2
  exit 1
}

if ! getent passwd gpui-box >/dev/null; then
  useradd --system --home-dir /nonexistent --shell /usr/sbin/nologin gpui-box
fi
install -d -o root -g root -m 0755 /opt/gpui-box /opt/gpui-box/releases
install -d -o root -g root -m 0700 /var/lib/gpui-box /var/lib/gpui-box/incoming
install -o root -g root -m 0755 "$source_dir/receive-release.sh" /usr/local/sbin/receive-gpui-box-release
install -o root -g root -m 0755 "$source_dir/install-release.sh" /usr/local/sbin/install-gpui-box-release
install -o root -g root -m 0644 "$source_dir/gpui-box-mcp.service" /etc/systemd/system/gpui-box-mcp.service
install -o root -g root -m 0644 "$source_dir/gpui-box.nginx.conf" /etc/nginx/conf.d/gpui-box.conf

read -r key_type key_body _ < "$source_dir/deploy-key.pub"
[[ "$key_type" == ssh-ed25519 && -n "$key_body" ]] || { echo "invalid deploy key" >&2; exit 1; }
install -d -o root -g root -m 0700 /root/.ssh
touch /root/.ssh/authorized_keys
chmod 0600 /root/.ssh/authorized_keys
temporary="$(mktemp /root/.ssh/authorized_keys.XXXXXX)"
grep -v ' gpui-box-github-production$' /root/.ssh/authorized_keys > "$temporary" || true
printf 'restrict,command="/usr/local/sbin/receive-gpui-box-release" %s %s gpui-box-github-production\n' \
  "$key_type" "$key_body" >> "$temporary"
chown root:root "$temporary"
chmod 0600 "$temporary"
mv "$temporary" /root/.ssh/authorized_keys

verification_unit="$source_dir/gpui-box-mcp-verify.service"
sed 's#^ExecStart=.*#ExecStart=/bin/true#' /etc/systemd/system/gpui-box-mcp.service \
  > "$verification_unit"
systemd-analyze verify "$verification_unit"
rm -f "$verification_unit"
systemctl daemon-reload
systemctl enable gpui-box-mcp.service
nginx -t
systemctl reload nginx
echo "GPUI Box host boundary is installed; deploy a release to start the MCP"
