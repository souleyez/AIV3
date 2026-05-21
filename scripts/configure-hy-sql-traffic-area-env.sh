#!/usr/bin/env bash
set -euo pipefail

env_file="${AIV3_ENV_FILE:-/etc/aiv3/aiv3.env}"
env_name="${HY_SQL_TRAFFIC_ENV_NAME:-THIRD_PARTY_HY_SQL_DATABASE_URL}"
restart_services=true

if [[ "${1:-}" == "--no-restart" ]]; then
  restart_services=false
fi

if [[ "$(id -u)" -ne 0 ]]; then
  echo "Run this script as root on the 8 server." >&2
  exit 1
fi

read_default() {
  local prompt="$1"
  local default_value="$2"
  local value
  read -r -p "${prompt} [${default_value}]: " value
  printf '%s' "${value:-$default_value}"
}

host="$(read_default "MySQL host" "8.155.12.154")"
port="$(read_default "MySQL port" "23306")"
database="$(read_default "MySQL database" "hy_sql")"
user="$(read_default "MySQL user" "hy_root")"
charset="$(read_default "MySQL charset" "utf8mb4")"

read -r -s -p "MySQL password: " password
printf '\n'
if [[ -z "$password" ]]; then
  echo "Password cannot be empty." >&2
  exit 1
fi

mysql_url="$(
  DB_HOST="$host" \
  DB_PORT="$port" \
  DB_NAME="$database" \
  DB_USER="$user" \
  DB_PASSWORD="$password" \
  DB_CHARSET="$charset" \
  python3 - <<'PY'
import os
from urllib.parse import quote

host = os.environ["DB_HOST"]
port = os.environ["DB_PORT"]
database = os.environ["DB_NAME"]
user = os.environ["DB_USER"]
password = os.environ["DB_PASSWORD"]
charset = os.environ["DB_CHARSET"]

print(
    "mysql://"
    + quote(user, safe="")
    + ":"
    + quote(password, safe="")
    + "@"
    + host
    + ":"
    + port
    + "/"
    + quote(database, safe="")
    + "?charset="
    + quote(charset, safe="")
)
PY
)"

install -d -m 700 "$(dirname "$env_file")"
touch "$env_file"
chmod 600 "$env_file"

backup_dir="$(dirname "$env_file")/backups"
install -d -m 700 "$backup_dir"
backup_file="${backup_dir}/$(basename "$env_file").$(date -u +%Y%m%dT%H%M%SZ)"
cp -p "$env_file" "$backup_file"

tmp="$(mktemp)"
awk -v key="$env_name" -v value="${env_name}=${mysql_url}" '
  BEGIN { written = 0 }
  $0 ~ "^" key "=" {
    if (!written) {
      print value
      written = 1
    }
    next
  }
  { print }
  END {
    if (!written) {
      print value
    }
  }
' "$env_file" > "$tmp"
install -m 600 "$tmp" "$env_file"
rm -f "$tmp"

unset password mysql_url DB_PASSWORD

echo "Configured ${env_name} in ${env_file}."
echo "Backup: ${backup_file}"

if "$restart_services"; then
  systemctl restart aiv3-platform-api.service aiv3-external-source-worker.service
  systemctl is-active aiv3-platform-api.service aiv3-external-source-worker.service
else
  echo "Skipped service restart. Restart aiv3-platform-api.service and aiv3-external-source-worker.service before smoke testing."
fi
