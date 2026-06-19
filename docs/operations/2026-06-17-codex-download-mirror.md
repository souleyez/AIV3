# Codex Download Mirror On Server 8

Date: 2026-06-17

## Public Entry

- V3 home entry: `https://v3.elepcloud.com/`
- Download page: `https://v3.elepcloud.com/downloads/codex/`
- Codex installer mirror: `https://v3.elepcloud.com/downloads/codex/openai-codex-install.ps1`
- Windows terminal package: `https://v3.elepcloud.com/downloads/codex/V3企业定制agent终端-Windows.zip`
- macOS terminal package: `https://v3.elepcloud.com/downloads/codex/V3企业定制agent终端-macOS.zip`
- Official Codex Windows x64 package: `https://v3.elepcloud.com/downloads/codex/codex-package-x86_64-pc-windows-msvc.tar.gz`
- Official Codex Windows ARM64 package: `https://v3.elepcloud.com/downloads/codex/codex-package-aarch64-pc-windows-msvc.tar.gz`
- Checksums: `https://v3.elepcloud.com/downloads/codex/SHA256SUMS.txt`
- Machine manifest: `https://v3.elepcloud.com/downloads/codex/latest.json`

The mirror hosts the Codex Windows installer script, official Codex Windows release packages, and SoulEye's `V3企业定制agent终端` configurator/client shell packages.

## Server Layout

- Host: `8服务器` / `8.155.8.7`
- Static root: `/srv/aiv3-downloads/codex`
- Nginx global limit zone: `/etc/nginx/conf.d/00-codex-download-limit.conf`
- V3 host route: `/etc/nginx/conf.d/v3-elepcloud.conf`

The nginx location is mounted under `/downloads/codex/` on `v3.elepcloud.com`.

## Traffic Limit

Download concurrency is limited by nginx:

```nginx
limit_conn_zone $server_name zone=codex_download_total:1m;

location /downloads/codex/ {
    alias /srv/aiv3-downloads/codex/;
    limit_conn codex_download_total 3;
    limit_conn_status 503;
}
```

This is a total concurrent connection cap for the mirror path on the V3 host. A fourth simultaneous download receives HTTP `503`.

## Current Manifest Packages

Version: `20260619-165520`

```text
997e000ec6d482cb019c7a6cb3412d3cccbdf9228b9440c2ab920150e3a7579d  V3企业定制agent终端-Windows.zip
ebcdab966aefec43f7a4a71b0026059ae169e6eb2931307fd18a48349a39e7c7  V3企业定制agent终端-macOS.zip
ab7d7864801e62f92aa3abf878e3b0aaa8b9f7de03120bc81b029f67281c1842  openai-codex-install.ps1
6695999e3681ad9138ecc6dbbed90f2a428b258a56405b2fc6207a5b0a4fa66a  codex-package-x86_64-pc-windows-msvc.tar.gz
56d1bb8d15471ddc496f513f0638316fcdb78b1fe01052e6d264afe735c93e8e  codex-package-aarch64-pc-windows-msvc.tar.gz
94062ac8bd49941fae39e6846a4fcb01b8a1ace2c588ccd5e5ffa8fb74013ab5  codex-package_SHA256SUMS
a37f1688f69b38b1ced056086d233e64784740fe19a2a15e3adc1c6828e1b933  codex-package-aarch64-apple-darwin.tar.gz
b5bb1af9c823306b682ca8d5c2f18610d5f0c3d5dbe4674b0b5f18eecd3986a7  codex-package-x86_64-apple-darwin.tar.gz
```

Legacy files still hosted but no longer listed in `latest.json`:

```text
f2d8ff0c54831dbcaf0d56459d25d832b0ef52abe3063fdec5209d6f649ecbe9  SoulEye-Codex-Config-Manager-Windows.zip
d3a8fb331fc6ddaa83f8dbfe981a707353d274415bfb05280a1ccab681979642  SoulEye-Codex-Config-Manager-macOS.zip
15893c38d89231747af9f709422114b9a4a610874d2a725ef961bd708916fbed  codex-executor-intro.html
```

## V3 Page Entry

`https://v3.elepcloud.com/` includes a short `V3企业定制agent终端` area with:

- Codex mirror
- Download center
- V3 enterprise terminal for Windows
- V3 enterprise terminal for macOS
- SHA256 checksums

`/external-integrations` also keeps the Codex Executor Mirror entry.

## Verification

Expected checks:

```bash
curl -I https://v3.elepcloud.com/downloads/codex/
curl -I https://v3.elepcloud.com/downloads/codex/openai-codex-install.ps1
curl -I https://v3.elepcloud.com/downloads/codex/V3%E4%BC%81%E4%B8%9A%E5%AE%9A%E5%88%B6agent%E7%BB%88%E7%AB%AF-Windows.zip
curl -I https://v3.elepcloud.com/downloads/codex/V3%E4%BC%81%E4%B8%9A%E5%AE%9A%E5%88%B6agent%E7%BB%88%E7%AB%AF-macOS.zip
curl -I https://v3.elepcloud.com/downloads/codex/codex-package-x86_64-pc-windows-msvc.tar.gz
curl -I https://v3.elepcloud.com/downloads/codex/codex-package-aarch64-pc-windows-msvc.tar.gz
curl -fsS https://v3.elepcloud.com/downloads/codex/latest.json
curl -fsS https://v3.elepcloud.com/
curl -fsS https://v3.elepcloud.com/external-integrations
curl -fsS https://v3.elepcloud.com/v1/datasets
nginx -t
systemctl is-active nginx aiv3-web.service aiv3-platform-api.service
```

Concurrency smoke used four parallel Range downloads; three returned `206`, one returned `503`.

2026-06-19 enterprise-billing client release verification:

- `latest.json` returned public version `20260619-165520`.
- Windows terminal package returned HTTP `200`, `Content-Length: 5605979`, and `X-Download-Concurrency-Limit: 3`.
- macOS terminal package returned HTTP `200`, `Content-Length: 11031909`, and `X-Download-Concurrency-Limit: 3`.
- Windows jump-host fast smoke passed with tenant `v3-jump-smoke-20260619170147`: public package download, private Codex runtime install, terminal activation, heartbeat, quota summary, and temporary revoke all succeeded.

## Backup Points

- Nginx and previous mirror snapshot: `/srv/backups/codex-download-mirror/20260617T090514`
- Previous V3 page files: `/srv/backups/codex-download-mirror/20260617T090735-aiv3-web-files`
- Previous V3 landing file: `/srv/backups/codex-download-mirror/20260617T091523-v3-landing`
- Previous package rename page files: `/srv/backups/codex-download-mirror/20260617T093451-package-rename-pages`
- Current client activation package snapshot: `/srv/backups/codex-download-mirror/20260617T155424-client-activation-current`
- Previous client activation package before heartbeat release: `/srv/backups/codex-download-mirror/20260617T163249-terminal-heartbeat`
- Previous enterprise-billing client package before 20260619 release: `/srv/aiv3-downloads/codex/backup-20260619-170008-enterprise-billing`
