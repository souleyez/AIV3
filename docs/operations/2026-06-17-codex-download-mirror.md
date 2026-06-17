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

Version: `20260617-163249`

```text
33be41cfb2da3ea19290769115558fc9885fd23d50090ccbf73a9ae66411e14c  V3企业定制agent终端-Windows.zip
bfd2c63b7879c3511bf74f49e7a54bcd25627a432a36b4e117c32146019739ed  V3企业定制agent终端-macOS.zip
ab7d7864801e62f92aa3abf878e3b0aaa8b9f7de03120bc81b029f67281c1842  openai-codex-install.ps1
f2ff8df0c74616b4484d412df9ee0621f71dd2fff1f83e6693ac053cd595ded5  codex-package-x86_64-pc-windows-msvc.tar.gz
3e95ceba558ba258dc4ad930905ce595f2bf21dfbcef7613b25965054cdf0f0f  codex-package-aarch64-pc-windows-msvc.tar.gz
7aac6479f93a8383fb481d3ed16f731d98bf75facfa84b00d88f30ef676b9411  codex-package_SHA256SUMS
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

## Backup Points

- Nginx and previous mirror snapshot: `/srv/backups/codex-download-mirror/20260617T090514`
- Previous V3 page files: `/srv/backups/codex-download-mirror/20260617T090735-aiv3-web-files`
- Previous V3 landing file: `/srv/backups/codex-download-mirror/20260617T091523-v3-landing`
- Previous package rename page files: `/srv/backups/codex-download-mirror/20260617T093451-package-rename-pages`
- Current client activation package snapshot: `/srv/backups/codex-download-mirror/20260617T155424-client-activation-current`
- Previous client activation package before heartbeat release: `/srv/backups/codex-download-mirror/20260617T163249-terminal-heartbeat`
