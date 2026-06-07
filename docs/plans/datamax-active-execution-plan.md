# DataMax 当前唯一执行计划

**更新时间：** 2026-06-07 21:35 CST
**当前性质：** 下一阶段开发执行版；P0 主站可见视频/PPT smoke 与质量复核已完成证据记录，后续按 P1/P2 切片继续开发。本计划编写本身不部署 8 服务器。
**唯一 active plan：** `docs/plans/datamax-active-execution-plan.md`

## 1. 计划原则

- DataMax 继续只有一份 active plan；历史计划、执行回执和测试记录压缩到本文件的事实基线和后续队列中。
- 计划收拢阶段已结束；后续允许按本计划切片改代码、脚本和文档，但每个切片必须有对应验证证据。
- GitHub 同步可以包含通过验证的代码/脚本/文档提交；8 服务器只能在用户明确批准“发版/部署”后再 pull、build、restart。
- 120 服务器不在本计划范围内。
- 不记录或传播密钥、cookie、扫码会话、数据库 URL、provider payload、原始客户文件、原始客户行、私有 object path、内部下载地址。
- 视频 PPT 能力只面向“视频里已经播放 PPT/幻灯片/课件”的场景；不是把普通视频创作成 PPT。

## 2. 当前事实基线

### 2.1 代码与部署基线

- 当前 head 以 `git rev-parse HEAD` / `git rev-parse origin/main` 为准；最近已同步的计划收拢基线是 `33b6d9b Consolidate DataMax execution plan`。
- 已记录的 8 服务器最新部署基线：`b1abad9cc`，已启用 `INGEST_REMOTE_MEDIA_ENABLED=true` 和 `INGEST_REMOTE_MEDIA_CACHE_DIR=/srv/aiv3/remote-media-cache`。
- 8 服务器已知未跟踪文件：`?? mode`，继续保持不触碰。
- P0 主站可见视频/PPT smoke 在主站 `https://v3.elepcloud.com` 通过；本轮未重新部署 8 服务器，下一次部署必须单独获得批准。

### 2.2 已完成的主线事项

旧 active plan 中 Task 1-12 已进入收尾态，关键结果如下：

- 单计划归档完成：历史计划合并到 `docs/plans/datamax-active-execution-plan.md`。
- 视频/PPT 优先通道完成：上传视频、直连视频 URL、公开页面可解析直连视频资源，进入同一套 `VideoExtraction` 交付契约。
- 视频抽取 PPT 交付闭环完成：PPTX、Markdown、notes、manifest、published version history、前端下载动作、AssistantRun follow-up 已有确定性测试覆盖。
- 第三方视频素材说明完成：`.mp4`、`.mov`、`.m4v`、`.webm`、`.mkv`、`.avi` 可上传或登记为视频素材；“提取视频里的 PPT/幻灯片/课件”是特殊触发，不是普通解析默认动作。
- operator 观测页、小范围第三方数据库只读状态、row identity 自测、静态页/报表回归、被动回答质量语料、document enrichment/去重诊断等已按旧计划完成。
- 仍挂起的事项主要依赖 operator 凭据、业务决策或部署批准，不阻塞下一阶段视频/PPT专项推进。

### 2.3 视频/PPT 已交付修复

- `6a565f0 Auto-select PPT pages from video frames`
  从视频帧中自动选择稳定 PPT 页面关键帧，保留手动 keep-list 覆盖能力。
- `442ceec Download remote videos before frame extraction`
  远程视频先下载到抽取 session，再交给 ffmpeg 抽帧，降低网络输入不稳定性，同时保持 URL 脱敏。
- `f5902df Record controlled video PPT smoke`
  记录受控 PPT 播放视频 smoke。
- `ee24a26 Clarify video PPT extraction trigger`
  文档明确“视频素材登记”和“抽取视频中的 PPT”不是同一件事。
- `233c821 Parse inline video URLs in ReAct tools`
  修复中文提示中冒号/中文标点后的视频 URL 识别，避免用户发“提取这个视频里的 PPT：https://...”时漏掉直连 URL。
- `b1abad9 Allow video parse placeholders to reach PPT extraction`
  修复视频文档 `parse_video_media` 因普通文档 placeholder parse 状态被自动重解析门拒绝的问题，让视频可以继续进入 `extract_video_ppt`。

### 2.4 已完成的受控公开视频 smoke

公开视频样例：

- `https://v3.elepcloud.com/generated-artifacts/samples/react-in-5-minutes.mp4`

已记录 smoke 结果：

- workflow id：`7bb6f92d-dbeb-4f4f-99e1-c2b029e063ba`
- task chain：`resolve_video_source`、`register_video_asset`、`parse_video_media`、`cleanup_document_facts`、`extract_video_ppt` 均成功。
- workflow 最终：`completed|succeeded|2026-06-07 20:48:56`
- `deliverable_status.state=final_pptx_ready`
- `frame_extraction.status=completed`
- `frame_count=1281`
- 生成文件：18 类，包括 `pptx`、`video_slides_markdown`、`slide_notes`、`slide_rectangles_manifest`、`selected_slides_manifest`、`frame_manifest`、`timestamp_map`、`contact_sheet_html`、各类 manifest。
- 关键缺口：`has_subtitle_page_map=false`，原因是样例没有可对齐字幕/转写；这是证据质量限制，不是抽帧/PPTX 失败。
- warning 包括：`full_frame_rectangle_fallback`、`missing_transcript_alignment`、`parse_partial`、`provider_failure`、`screenshot_based_pptx`、`selected_slide_duplicates_removed`、`speaker_notes_metadata_only`。
- 注意：这次是后端 workflow smoke，没有绑定旧聊天的 `assistant_run_id`，所以不能单独代表主站聊天可见性；该缺口已通过 2.5 的主站可见 smoke 补齐。

### 2.5 已完成的主站可见 smoke 与质量复核

主站样例：

- 主站：`https://v3.elepcloud.com`
- 提示词：`请提取这个视频里的 PPT：https://v3.elepcloud.com/generated-artifacts/samples/react-in-5-minutes.mp4`
- local thread：`video-ppt-main-visible-20260607-01`
- assistant run id：`46f57e74-85f9-4088-bad0-99f1ae0a6fea`

主站可见结果：

- `video_extraction.workflow_completed` 已进入 assistant-run-bound 链路。
- model completion follow-up 已 request、enqueue、consume，并追加模型后续轮。
- HTML artifact：`html-artifact-video-extraction-46f57e74-85f9-4088-bad0-99f1ae0a6fea-0c41706c-3e39-4c75-8f35-32d26f98e8b3`
- artifact `source_type=video_extraction`，`template_id=video_extraction_summary`，owner scope 绑定 assistant run。
- `/api/v3/html-artifacts?assistant_run_id=...&local_thread_id=...` 返回完整 manifest。
- `deliverable_status.state=final_pptx_ready`
- `generated_artifacts.status=completed`
- `frame_count=1281`
- `file_count=18`
- `has_pptx=true`
- `has_video_slides_markdown=true`
- `has_slide_notes=true`
- `has_slide_rectangles_manifest=true`
- `has_subtitle_page_map=false`

下载与质量复核：

- PPTX、`video_slides.md`、`final_deliverables_manifest.json`、`published_deliverable_manifest.json`、`published_version_history.json`、`extraction_artifacts_manifest.json` 均可通过 `/api/v3/html-artifacts/{artifact_id}/files/{index}` 下载。
- PPTX 是有效 OOXML，包含 `[Content_Types].xml`、`ppt/presentation.xml`、`ppt/slides/slide1.xml`。
- PPTX 含 30 个 slide XML 和 30 个 picture element。
- `video_slides.md` 含 30 个 `### Slide` 小节，与 PPTX 页数一致。
- selected slides manifest 报告 `selected_count=30`。
- 27 页带 DrawingML `a:srcRect` crop metadata。
- visual-similarity dedupe 移除了 7 个近重复候选。
- `has_subtitle_page_map=false` 对该样例可接受，因为没有可对齐字幕/转写证据。

质量结论：

- 该样例达到“可交付截图型 PPTX + Markdown deck + manifest”标准。
- warning 要按交付限制解释：`screenshot_based_pptx` 表示不是可编辑原生 PPT 重建，`missing_transcript_alignment` 表示没有字幕页映射，`full_frame_rectangle_fallback` 表示部分页面仍需人工复核 crop。
- 这次没有使用客户视频、登录态页面、视频号绕行、cookie、token、数据库 URL、provider payload 或私有 object path。
- 完整回执记录在 `docs/validation/video-ppt-deliverable-smoke.md`。

## 3. 当前能力边界

### 3.1 已支持

- 主站上传视频文件后，用户明确说“提取 PPT/幻灯片/课件”。
- 第三方上传或登记视频文件后，聊天事件授权对应文档/数据集，并明确要求“提取视频里的 PPT/幻灯片/课件”。
- 用户在主站或第三方消息中提供匿名可访问的直接视频 URL。
- 公开网页中可从 HTML 字段或网络可见资源解析出匿名可下载视频 URL。
- 输出截图型 PPTX、Markdown deck、slide notes、抽取 manifest、页面选择清单、帧清单、时间映射和质量 warning。

### 3.2 不自动支持

- 微信视频号 `weixin.qq.com/sph/...`、`channels.weixin.qq.com/sph/...` 这类链接，除非后续能解析出匿名可下载视频直链。
- 扫码登录、cookie 注入、私有平台页、企业内网私有播放页、需要用户登录态才能播放的链接。
- 绕过 DRM、绕过平台权限、抓取私有接口 payload、持久化登录凭据或复用用户浏览器 cookie。
- 把普通视频“总结创作成 PPT”；当前能力是抽取视频中已经展示的 PPT/课件画面。

### 3.3 微信视频号研究结论

针对 `https://weixin.qq.com/sph/AhfmOtV8P5` 这类链接，当前结论：

- 这类链接是视频号分享入口，不等同于匿名可下载的 `.mp4` 直链。
- 当前代码已经把 `weixin.qq.com/sph/` 和 `channels.weixin.qq.com/sph/` 归类为 login-gated/unsupported，返回“需要直接视频 URL 或上传视频文件”的失败提示，这是正确的保守边界。
- 微信官方小程序 API `wx.openChannelsActivity` 是“打开视频号视频”的能力，需要 `finderUserName` 和 `feedId`；官方文档没有把它定义为视频文件下载接口。参考：<https://developers.weixin.qq.com/miniprogram/dev/api/open-api/channels/wx.openChannelsActivity.html>
- 因此，V3 自动从视频号链接提取 PPT 的第一阶段不应承诺“只给链接就必然能拿到视频文件”。
- 可以规划一个“授权播放录屏兜底”，但它不是解析直链能力，也不能绕过登录和平台权限。

### 3.4 录屏兜底可行性研究

受控录屏在工程上可行，但必须作为 operator-approved fallback：

- FFmpeg 官方设备文档支持 Linux `x11grab`、Windows `gdigrab`、macOS `avfoundation` 等屏幕/设备采集方式。参考：<https://www.ffmpeg.org/ffmpeg-devices.html>
- Playwright 官方支持浏览器 context 录制视频，录制文件在 page/context 关闭后可取。参考：<https://playwright.dev/docs/videos>
- Chrome DevTools Protocol `Page.startScreencast` 可以发送页面帧事件，适合低层帧抓取或诊断；该接口标记为 experimental。参考：<https://chromedevtools.github.io/devtools-protocol/tot/Page/#method-startScreencast>

计划采用的边界：

- 录屏结果只作为一个普通 `.mp4` 输入，后续复用现有 `VideoExtraction`，不另写一套 PPT 抽取管线。
- 不保存 cookie、二维码、登录态 storage、原始网络 payload、私有视频源 URL。
- 不在无用户授权或无 operator 批准的情况下自动打开、播放、录制第三方平台内容。
- 首选 jump-host/Mac 受控播放录制；如果要在 8 服务器内部录制，需要独立开关、隔离浏览器 profile、短时任务、容量上限、日志脱敏和保留期。

## 4. 下一阶段执行队列

### P0-0：本次计划收拢与 GitHub 同步

**状态：已完成，2026-06-07，提交 `33b6d9b`。**
**目标：** 把旧计划、视频测试、复核要求、视频号研究和录屏兜底设计合并成当前可执行方案。

执行项：

1. 重写 `docs/plans/datamax-active-execution-plan.md`。
2. 同步桌面副本 `/Users/manslive01/Desktop/datamax-active-execution-plan.md`。
3. 只运行文档级检查：`git diff --check`、计划关键词复核、桌面副本 `cmp`。
4. GitHub 同步仅提交/推送文档，不部署 8 服务器。

验收：

- Git diff 只包含计划文档。
- 桌面副本与仓库计划一致。
- GitHub 有 doc-only commit。
- 最终回复明确“未改代码、未发 8 服务器”。

后续说明：该 doc-only 阶段已经结束；当前计划进入功能开发执行期。

### P0-1：主站可见视频/PPT smoke

**状态：已完成，2026-06-07。**

**目标：** 用主站普通聊天链路跑一次 assistant-run-bound smoke，确认用户能在主站看到 PPT/Markdown 下载，而不是只看到后端 workflow 成功。

输入优先级：

1. 已公开样例：`https://v3.elepcloud.com/generated-artifacts/samples/react-in-5-minutes.mp4`
2. 用户上传的视频文件。
3. 第三方登记的视频素材。

触发语：

- “提取这个视频里的 PPT”
- “提取视频中的幻灯片/课件”
- “把这个课程视频里播放的 PPT 抽出来”

验收：

- 主站 assistant run 有状态卡或后续消息。
- 能看到 `video_extraction_summary`。
- 下载动作至少包含 PPTX、`video_slides.md`、交付 manifest。
- 如果缺字幕映射，要显示为 `missing_transcript_alignment`，不能误报失败。
- 不暴露本地路径、原始 source URL、cookie、token、provider payload。

不做：

- 不用微信视频号链接替代公开视频直链 smoke。
- 不跑登录态/私有视频。
- 不部署新代码，除非 smoke 证明当前部署缺必要修复且用户批准发版。

### P0-2：抽取效果复核标准化

**状态：已完成，2026-06-07。**

**目标：** 把“抽出来了”升级为“抽取质量可判断、可复核、可交付”。

复核维度：

- PPT 页数量是否接近视频中实际播放页数。
- 是否存在重复页，`selected_slide_duplicates_removed` 是否解释清楚。
- 截图是否截到完整课件区域；`full_frame_rectangle_fallback` 是否需要人工裁剪复核。
- `video_slides.md` 是否按页列出时间点、帧文件名、crop 状态、可用旁白。
- `slide_notes` 是否只是 metadata-only，还是有字幕/转写内容。
- `subtitle_page_map` 缺失时是否有明确原因。
- PPTX 是否是有效 OOXML，slide 数量与 selected slides 一致。

验收输出：

- 一份 `docs/validation/video-ppt-deliverable-smoke.md` 追加回执。
- smoke 中记录：输入类型、触发方式、frame_count、selected_count、文件清单、warning 解读、人工复核结论。
- 如果主站未暴露下载链接，记录为 P0 阻塞，不用后端成功掩盖前端失败。

### P1-1：公开页面视频资源解析增强

**目标：** 支持“客户提供链接，系统先尝试从公开网页中找到匿名可下载视频文件”，但不越过登录和平台权限。

解析顺序：

1. URL 后缀/MIME 判断：`.mp4`、`.mov`、`.m4v`、`.webm`、`.mkv`、`.avi`。
2. HTML 静态字段：`<video src>`、`<source src>`、OpenGraph `og:video`、Twitter video、JSON-LD 中的 contentUrl/embedUrl。
3. 公开页面网络探测：只记录候选 URL 的 redacted metadata，命中匿名可下载视频后下载到 remote media cache。
4. 若页面要求登录、跳转二维码、返回平台壳页或没有匿名 media URL，进入 unsupported/handoff。

验收：

- 新增本地 fixtures：直接 video tag、OG video、JSON-LD、相对 URL、无视频页面、登录态页面。
- 主站消息 “从这个链接提取视频里的 PPT” 能先走 resolver，再进入 `VideoExtraction`。
- 失败原因区分 `public_page_no_video_asset`、`login_gated_video_source_not_supported`、`direct_video_url_required`。
- 不持久化完整 source URL；manifest 只保留 redacted 来源审计。

### P1-2：微信视频号/登录态来源的产品 handoff

**状态：已完成，2026-06-07。**

**目标：** 对视频号链接给出可执行下一步，而不是泛泛失败。

产品行为：

- 如果检测到 `weixin.qq.com/sph/` 或 `channels.weixin.qq.com/sph/`：
  - 明确提示当前不能自动从该链接拿到视频文件。
  - 给出三个选项：上传视频文件、提供匿名直连视频 URL、申请授权录屏处理。
  - 不要求用户提供 cookie、扫码截图或账号密码。
- 如果用户已上传同一视频文件，再说“提取 PPT”，则直接进入视频 PPT 特殊触发。
- 如果第三方系统能先把视频文件存到它自己的对象存储，并给 DataMax 一个匿名 HTTPS 下载 URL，则按直连 URL 处理。

验收：

- 主站/第三方返回文案统一，不再让用户误以为 DataMax 已经看过视频内容。
- unsupported 状态带 `failure_reason=login_gated_video_source_not_supported`。
- API 文档保留“视频号不属于自动解析范围”的说明。

完成证据：

- 后端 `wechat_video_login_handoff` artifact 会绑定 assistant run，带 `failure_reason=login_gated_video_source_not_supported`，并给出上传视频文件、提供匿名直连视频 URL、申请授权录屏处理三选项。
- ReAct `resolve_video_url` 对视频号/登录态来源返回同一失败原因和三选项 next action。
- 前端 HTML artifact 安全渲染“视频来源受限”卡片，不显示二维码、扫码交接、原始链接、cookie 或登录态获取要求。
- 第三方 API 文档和纯第三方指南已同步三选项 handoff。

### P1-3：产物下载与发布可见性审计

**目标：** 确认不同入口的最终产物都能被用户拿到。

覆盖入口：

- 主站上传视频。
- 主站消息直接给视频 URL。
- 第三方上传/登记视频文件后触发。
- 后端 workflow smoke。

验收：

- assistant-run-bound 的产物链接通过 `/api/v3/html-artifacts/{artifact_id}/files/{index}` 或等价公开 surface 暴露。
- 没有 assistant_run_id 的后端 smoke 要标注“仅后端证据，不代表主站聊天可见”。
- 下载链接权限正确：同一 run/thread 可访问，跨 scope 不泄露。

### P2-1：授权录屏兜底 MVP 设计

**目标：** 在不破坏主线能力、不绕过平台权限的前提下，支持“拿不到直链但 operator 已批准可播放”的视频源。

推荐方案：

- **优先路径：jump-host/Mac 录制。** 适合需要人工打开微信、扫码或客户端播放的场景，录完生成普通 `.mp4`，再上传到 DataMax。
- **可选路径：8 服务器内部隔离录制。** 只适合服务器能合法访问并播放的网页，必须显式开关启用。

8 服务器内部录制最小组件：

- `CAPTURE_FALLBACK_ENABLED=false` 默认关闭。
- Playwright/Chrome 独立 profile，禁用持久化登录态。
- Xvfb 或等价虚拟显示。
- FFmpeg `x11grab` 录制画面；如需要声音，单独评估 PulseAudio/PipeWire。
- 任务级最大时长、最大文件大小、并发 1、临时目录隔离。
- 捕获完成后输出 `.mp4`，注册为视频素材，复用现有 `parse_video_media` 和 `extract_video_ppt`。
- 临时文件默认短期保留，建议 7 天内清理；保留期由 operator 配置确认。

安全门：

- 每条 capture job 必须记录授权确认：谁批准、来源、用途、最大时长、清理策略。
- 不导出浏览器 storage、cookie、HAR、登录二维码截图、网络 payload。
- 不对 DRM/禁止录制内容做规避。
- 日志只记录 redacted URL host、任务 id、时长、输出文件 hash/size、清理状态。

验收：

- 先用本地公开测试页录制一个 30-60 秒样例，证明录制文件可被现有 `VideoExtraction` 生成 `final_pptx_ready`。
- 再由 operator 提供一个已授权、可播放但无直链的样例做受控 smoke。
- smoke 失败时要能说明是播放失败、录制失败、视频无 PPT、抽帧失败、PPT 质量不足，不能统一报“解析失败”。

### P2-2：视频/PPT 质量增强

**目标：** 提高截图型 PPT 的可读性，减少人工复核成本。

候选方向：

- 更强的课件区域检测，减少 `full_frame_rectangle_fallback`。
- 画面稳定区间识别，避免转场帧和讲师遮挡帧。
- OCR/字幕/转写对齐，补齐 `subtitle_page_map`。
- 多语言 slide notes 与逐页讲稿备注。
- 质量评分：清晰度、重复度、遮挡、裁剪风险、字幕缺失。

验收：

- 用 3 个样例集复核：合成 PPT 视频、公开视频课程、真实客户上传视频。
- 每个样例输出 PPTX、Markdown、质量报告和人工复核结论。

## 5. 逐项执行 Runbook

本节把上面的 P0/P1/P2 切片转成可直接执行的开发动作。除 P0-0 外，任何代码改动都应独立分支或独立提交，且不自动部署 8 服务器。

### 5.1 通用预检

每个开发切片开始前先运行：

```bash
git status --short --branch
git rev-parse --short HEAD
rg -n "VideoExtraction|extract_video_ppt|video_slides|video_extraction_summary|resolve_video_url|login_gated_video_source_not_supported|screen_recording_bypass" crates apps scripts tools docs -S
```

预期：

- 本地没有未知的非本任务改动；如有用户改动，先记录，不覆盖。
- 能定位视频 URL 解析、视频 PPT 特殊触发、产物发布、前端下载和 unsupported/handoff 文案路径。
- 本任务只改与当前切片直接相关的文件，不做顺手重构。

通用收尾：

```bash
cargo fmt --check
git diff --check
git status --short --branch
```

如果切片涉及 Web：

```bash
npm run build
node --test app/lib/html-artifact-manifest.test.mjs
```

如果切片涉及视频/PPT：

```bash
npm run test:video-deliverables
CC=clang CXX=clang++ cargo test -p media-worker frame_extraction --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete
CC=clang CXX=clang++ cargo test -p platform-api video_url_resolution --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
```

如果切片涉及 ingest-worker 视频登记/解析：

```bash
CC=clang CXX=clang++ cargo test -p ingest-worker --bin ingest-worker video_parse_media_placeholder -- --nocapture
CC=clang CXX=clang++ cargo test -p ingest-worker --bin ingest-worker auto_reparse_decision -- --nocapture
```

### 5.2 P0-1 主站可见 smoke 执行步骤

目的不是再次证明后端 workflow 能跑，而是证明主站用户能拿到可见产物。
本节现在作为新版本回归或重新部署后的复现 runbook 使用；当前 P0-1 已完成，证据见 2.5。

执行步骤：

1. 在主站普通聊天中发送公开视频直链和明确触发语，例如：
   - `请提取这个视频里的 PPT：https://v3.elepcloud.com/generated-artifacts/samples/react-in-5-minutes.mp4`
2. 记录 assistant run id、workflow id、状态卡、最终消息和产物链接。
3. 如果只出现“已登记视频素材”而没有进入 PPT 抽取，判定为触发/编排缺口。
4. 如果后端成功但主站无下载链接，判定为发布可见性缺口。
5. 如果产物可见，下载 PPTX 和 Markdown，做最小复核。

复核命令和检查点：

```bash
rg -n "video_extraction_summary|download_exports|html-artifacts|video_slides_screenshot_based|video_slides.md" apps crates docs -S
npm run test:video-deliverables
git diff --check
```

验收记录写入：

- `docs/validation/video-ppt-deliverable-smoke.md`
- 必填字段：输入 URL 类型、assistant_run_id、workflow_id、frame_count、selected_count、ready file kinds、download link surface、warning codes、人工复核结论。

失败分流：

- `direct_video_url_required`：URL 没被识别，优先回看 `first_url_in_text` / resolver。
- `login_gated_video_source_not_supported`：输入不是公开视频直链，按 P1-2 handoff。
- `final_pptx_ready` 但无下载：按 P1-3 artifact visibility。
- `has_subtitle_page_map=false`：如果样例无字幕，不作为失败；如果样例有字幕，进入 P2-2。

### 5.3 P0-2 抽取效果复核执行步骤

本节现在作为后续样例复核 runbook 使用；当前公开样例的 P0-2 已完成，证据见 2.5 和 `docs/validation/video-ppt-deliverable-smoke.md`。

执行步骤：

1. 取主站可见 smoke 的 PPTX、`video_slides.md`、`selected_slides_manifest`、`slide_rectangles_manifest`、`frame_manifest`。
2. 统计 PPT slide 数和 selected slide 数是否一致。
3. 抽样检查每张 slide 是否是视频中稳定的课件页，而不是转场/黑屏/人物大头/重复页。
4. 对 warning 做解释，不把 warning 当成失败，也不忽略影响交付质量的 warning。
5. 写一份“可交付/需人工复核/不可交付”的结论。

建议质量判定：

- `可交付`：PPTX 可打开，页数合理，主要课件内容可读，重复页少，Markdown 有逐页索引。
- `需人工复核`：PPTX 可打开，但 crop fallback 多、遮挡明显、重复页明显、缺字幕映射。
- `不可交付`：PPTX 无效、关键页缺失、抽到非课件画面、无法下载、产物泄露路径或 URL。

验收记录写入：

- `docs/validation/video-ppt-deliverable-smoke.md`
- 不提交生成的 PPTX、帧图、原始视频或私有下载链接。

### 5.4 P1-1 公开页面 resolver 增强执行步骤

目标文件优先级：

- `crates/platform-api/src/react_agent_tools.rs`
- `crates/tool-registry/src/lib.rs`
- `crates/platform-api/src/lib.rs`
- `docs/integrations/third-party-integration-api.zh-CN.md`
- `docs/integrations/pure-third-party-integration-guide.zh-CN.md`

开发步骤：

1. 先补测试 fixture，不先改生产逻辑：
   - 直接 `<video src="/a.mp4">`
   - `<source src="/a.webm" type="video/webm">`
   - `og:video`
   - JSON-LD `contentUrl`
   - 相对 URL
   - 无视频页面
   - 登录态/二维码页面
2. resolver 只提取匿名可下载候选，不请求登录态，不读取 cookie。
3. 候选 URL 进入现有 remote media download/cache；manifest 只记录 redacted 审计。
4. unsupported 状态要稳定、可测试、可展示。

目标测试：

```bash
CC=clang CXX=clang++ cargo test -p platform-api video_url_resolution --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
npm run check:pure-third-party-guide-html
git diff --check
```

验收：

- 公开视频页解析成功时能进入现有 `VideoExtraction`。
- 登录态/二维码/视频号页面不会被误判为可解析。
- API 文档与纯第三方指南同步。

### 5.5 P1-2 视频号 handoff 执行步骤

目标文件优先级：

- `crates/platform-api/src/react_agent_tools.rs`
- `crates/tool-registry/src/lib.rs`
- `crates/platform-api/src/lib.rs`
- `crates/ingest-worker/src/lib.rs`
- `apps/web/app/HomePageClient.js`
- `docs/integrations/third-party-integration-api.zh-CN.md`
- `docs/integrations/pure-third-party-integration-guide.zh-CN.md`

开发步骤：

1. 保持当前拒绝逻辑，不把视频号链接加入自动解析 allowlist。
2. 把失败提示升级成三选项 handoff：
   - 上传视频文件；
   - 提供匿名可下载直连视频 URL；
   - 申请授权录屏处理。
3. 确保同一用户后续上传 `.mp4` 后再说“提取 PPT”能正常触发，不被上一条视频号失败状态污染。
4. 第三方响应字段只新增可选字段，不改 URL、鉴权、必填字段和已有字段语义。

目标测试：

```bash
CC=clang CXX=clang++ cargo test -p platform-api video_url_resolution --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
CC=clang CXX=clang++ cargo test -p ingest-worker --bin ingest-worker video_parse_media_placeholder -- --nocapture
npm run check:pure-third-party-guide-html
git diff --check
```

验收：

- `weixin.qq.com/sph/...` 和 `channels.weixin.qq.com/sph/...` 都返回 `login_gated_video_source_not_supported`。
- UI/第三方文案不要求 cookie、扫码、账号密码。
- 文案明确“当前没有拿到视频内容，不能声称已看过或已生成 PPT”。

### 5.6 P1-3 artifact 可见性审计执行步骤

目标文件优先级：

- `apps/web/app/lib/html-artifact-manifest.js`
- `apps/web/app/lib/html-artifact-manifest.test.mjs`
- `apps/web/app/HomePageClient.js`
- `crates/platform-api/src/lib.rs`
- `crates/media-worker/src/lib.rs`

开发步骤：

1. 以 assistant-run-bound smoke 为准，不以后端裸 workflow 为准。
2. 复核 `video_extraction_summary` 是否暴露 prioritized download actions。
3. 复核生成项目卡和打开后的 HTML artifact toolbar 是否复用同一组下载链接。
4. 复核 scope：assistant run 或 local thread 可访问，跨 scope 不泄露。
5. 复核 redaction：下载 metadata 不出现本地路径、source URL、cookie/token。

目标测试：

```bash
node --test app/lib/html-artifact-manifest.test.mjs
node --check app/lib/html-artifact-manifest.js
npm run build
npm run test:video-deliverables
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
git diff --check
```

验收：

- 主站用户可直接拿到 PPTX 和 Markdown。
- 后端 workflow smoke 与主站可见 smoke 在文档中明确区分。

### 5.7 P2-1 授权录屏 MVP 执行步骤

推荐先做 isolated script，不直接塞进核心 worker：

- `scripts/capture-authorized-video.mjs`
- `docs/operations/video-capture-fallback-runbook.md`
- `docs/validation/video-ppt-deliverable-smoke.md`

第一阶段只做本地/受控 host：

1. 用 Playwright 打开 operator 指定 URL。
2. 使用隔离 browser profile，不加载用户日常浏览器 profile。
3. 用 FFmpeg 从虚拟显示或系统采集设备录制固定时长。
4. 输出 `.mp4` 到临时目录。
5. 将 `.mp4` 作为普通视频素材进入现有抽取流程。
6. 清理临时 profile 和录屏文件，或按 operator 配置保留短期文件。

第二阶段再评审是否接入 8 服务器：

- 默认 `CAPTURE_FALLBACK_ENABLED=false`。
- 只允许并发 1。
- 每次 job 必须有 explicit approval id。
- 最大录制时长和最大文件大小硬限制。
- 日志脱敏；不存 cookie/storage/HAR。

验证命令建议：

```bash
node --check scripts/capture-authorized-video.mjs
npm run test:video-deliverables
CC=clang CXX=clang++ cargo test -p media-worker frame_extraction --lib
git diff --check
```

上线门槛：

- 先有公开视频录屏自测。
- 再有 operator 授权样例。
- 再评审 8 服务器部署窗口。
- 未批准前不部署、不启用。

### 5.8 部署门槛

本计划本轮不部署。后续如用户明确要求发 8 服务器，按以下门槛走：

1. 本地 tests 通过。
2. GitHub 已推送对应 commit。
3. 8 服务器先只读检查：

```bash
ssh <8-server-host> 'cd /srv/aiv3/repo && git status --short --branch && git rev-parse --short HEAD && systemctl is-active aiv3-platform-api.service aiv3-web.service aiv3-media-worker.service aiv3-ingest-worker.service aiv3-assistant-run-worker.service'
```

4. 仅 pull 当前 commit，不碰 `?? mode`。
5. 仅 build/restart 受影响服务。
6. 部署后记录服务 active、queue stats、smoke 结果和回滚点。

## 6. 验证矩阵

| 场景 | 输入 | 预期 | 当前状态 | 下一步 |
| --- | --- | --- | --- | --- |
| 后端公开视频 smoke | `react-in-5-minutes.mp4` 直链 | `final_pptx_ready`，PPTX/Markdown/manifest 生成 | 已通过，workflow `7bb6f92d-dbeb-4f4f-99e1-c2b029e063ba` | 作为后端回归基线保留 |
| 主站可见 smoke | 同一公开视频直链 | 用户在主站看到下载动作 | 已通过，assistant run `46f57e74-85f9-4088-bad0-99f1ae0a6fea`，PPTX/Markdown/manifest 可下载 | 纳入 release gate |
| 主站上传视频 | 用户上传 `.mp4/.mov/...` | 上传登记后明确触发 PPT 抽取 | 能力已说明，仍需上传入口 smoke | P1-3 |
| 第三方视频登记 | 第三方 `content_url` 或 attachment | 特殊触发后进入 `VideoExtraction` | 文档已说明，需端到端 smoke | P1-3 |
| 公开视频页 | HTML 暴露 video/OG/JSON-LD | 解析候选并抽取 PPT | 基础能力已说明，需增强 fixtures 和失败分流 | P1-1 |
| 微信视频号链接 | `weixin.qq.com/sph/...` | 自动解析拒绝，给上传/直链/授权录屏选项 | P1-2 handoff 已实现，拒绝原因稳定为 `login_gated_video_source_not_supported` | 后续用主站/第三方 smoke 复核展示 |
| 授权录屏兜底 | operator 已批准可播放页面 | 录制 `.mp4` 后复用现有抽取 | 仅研究方案 | P2-1 |
| 普通视频转 PPT | 没有 PPT/课件画面的普通视频 | 不触发或提示不适用 | 已明确边界 | 保持 |

## 7. 决策门

进入 P1/P2 前需要确认：

- 是否允许在 8 服务器部署 capture fallback；如果允许，是否只针对公开可播放页面，还是允许人工登录后的短时受控 session。
- 录屏产物保留期：默认建议 7 天，是否需要更短或按客户配置。
- 授权记录格式：由谁批准、批准哪条链接、最大录制时长、是否允许音频、清理要求。
- 第三方系统是否愿意优先提供自己的视频文件直链，避免 DataMax 做平台录屏。
- 主站 UI 是否需要新增“申请授权录屏处理”按钮，还是先用人工流程。

没有上述确认前，录屏兜底只写方案，不进入生产实现。

## 8. 下一次执行建议

按风险和收益排序：

1. 做 P1-1 公开页面 resolver 增强，覆盖 video tag、source tag、OG video、JSON-LD、相对 URL 和登录态拒绝 fixtures。
2. 做 P1-3 artifact 可见性审计的回归测试，把新增 `smoke:video-ppt-main-visible` 纳入后续 release gate，并补主站上传视频、第三方视频登记 smoke。
3. 再推进 P2-1 授权录屏兜底 MVP 的 isolated script / runbook 评审。
4. 最后在明确授权和部署窗口后，评审是否允许 8 服务器内部录屏开关。

本计划完成当前阶段的定义：

- active plan 已收敛为当前可执行方案。
- 后端公开视频 smoke、主站可见 smoke、抽取效果复核、视频号限制、授权录屏兜底都已纳入执行队列和验收矩阵。
- GitHub 同步策略：计划/验证文档可按 doc-only 提交；功能或脚本提交必须绑定对应测试证据；任何 GitHub 同步都不等于 8 服务器发版。
- 8 服务器未发版、未重启、未触碰 `mode`。
