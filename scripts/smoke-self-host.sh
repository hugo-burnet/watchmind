#!/usr/bin/env sh
set -eu

project_name="watchmind-smoke"
port="18080"
backup="/data/watchmind-smoke-backup.json"

WATCHMIND_PORT="$port" docker compose -p "$project_name" up -d --build
trap 'WATCHMIND_PORT="$port" docker compose -p "$project_name" down -v' EXIT

attempt=0
until curl --fail --silent "http://127.0.0.1:$port/api/health" >/dev/null; do
  attempt=$((attempt + 1))
  [ "$attempt" -lt 30 ] || exit 1
  sleep 2
done

WATCHMIND_PORT="$port" docker compose -p "$project_name" exec -T api watchmind-api backup "$backup"
WATCHMIND_PORT="$port" docker compose -p "$project_name" exec -T api watchmind-api restore "$backup"
curl --fail --silent "http://127.0.0.1:$port/api/export?format=json" >/dev/null
