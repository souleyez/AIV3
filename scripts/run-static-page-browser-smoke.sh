#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

report_dir="${STATIC_PAGE_BROWSER_SMOKE_REPORT_DIR:-${repo_root}/target/static-page-browser-smoke}"
report_basename="static-page-browser-smoke-$(date -u +%Y%m%dT%H%M%SZ)"
artifact_dir="${report_dir}/${report_basename}-artifact"
screenshot_dir="${report_dir}/${report_basename}-screenshots"
report_json="${report_dir}/${report_basename}.json"
report_md="${report_dir}/${report_basename}.md"
cargo_bin="${CARGO_BIN:-cargo}"

if ! command -v "${cargo_bin}" >/dev/null 2>&1; then
  for candidate in \
    "${HOME:-}/.cargo/bin/cargo" \
    "${HOME:-}/.cargo/bin/cargo.exe" \
    "/mnt/c/Users/${USER:-}/.cargo/bin/cargo.exe"; do
    if [[ -n "${candidate}" && -x "${candidate}" ]]; then
      cargo_bin="${candidate}"
      break
    fi
  done
fi

if ! command -v "${cargo_bin}" >/dev/null 2>&1 && [[ ! -x "${cargo_bin}" ]]; then
  echo "cargo was not found. Install Rust or set CARGO_BIN=/path/to/cargo." >&2
  exit 1
fi

mkdir -p "${report_dir}" "${screenshot_dir}"

head_short="$(git rev-parse --short HEAD)"

echo "Static page browser smoke started"
echo "Repository: ${repo_root}"
echo "HEAD: ${head_short}"
echo "Artifact directory: ${artifact_dir}"
echo "Screenshot directory: ${screenshot_dir}"

"${cargo_bin}" run -p static-page-renderer --example static_page_render_smoke -- "${artifact_dir}"

node tools/static-page-browser-smoke.mjs \
  --html "${artifact_dir}/index.html" \
  --manifest "${artifact_dir}/asset-manifest.json" \
  --screenshot-dir "${screenshot_dir}" \
  --report-json "${report_json}" \
  --report-md "${report_md}" \
  ${CHROME_BIN:+--chrome-bin "${CHROME_BIN}"}

echo ""
echo "Static page browser smoke report: ${report_json}"
echo "Static page browser smoke summary: ${report_md}"
echo "Static page browser smoke screenshots: ${screenshot_dir}"
echo "OK static-page-browser smoke completed."

