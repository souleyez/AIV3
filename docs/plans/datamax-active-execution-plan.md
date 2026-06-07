# DataMax 当前唯一执行计划

**更新时间：** 2026-06-08 05:49 CST
**当前性质：** 开发执行版；当前入口是第 0.7 节，尤其是第 0.7.13 节的全量验收闭环。P2-2E-2A public candidate 本地交付契约修复已完成，P2-2E-2B 已补 public-course deliverables 专用质量矩阵入口，selected manifest 已拆清去重前 requested indices 与去重后 final indices，稀疏文字/build 状态自动选页和稳定段 best-sharpness 代表帧选择也已补本地回归。P2-2E-2B-Next 已完成保守 visual-shape duplicate 去重切片、页面级复核和 shape-dedupe public candidate 复跑；后续第二/第三公开视频探测补充了 `Nix in Space` 降级样例和 `Layered Nix Stores` 白底 build 保护回归，修复了 bright template 被 shape duplicate 误删 build 页的问题。EP6 单页输出复核风险已完成，`Nix in Space` 复跑会在 `slide_quality_report.json` 标记 `single_slide_output_review_required`，但仍保持 `final_pptx_ready`。当前公开视频样例仍应作为 `needs_manual_review` 真实样例，不强行标为 clean deliverable。第 0.7 节把后续工作收口为可执行包，覆盖主站上传、第三方登记、微信视频号/登录态 handoff、授权录屏兜底、客户授权质量矩阵、GitHub 同步和 8 服务器部署门槛；本轮仍不部署 8 服务器。
**状态摘要：**

- P0 主站可见视频/PPT smoke、P1-1 公开页面 resolver、P1-2 视频号 handoff、P1-3 direct URL release gate 已有证据；P1-3 主站上传视频 smoke 和第三方视频登记 special-trigger smoke 均已补离线 self-test 下载校验，视频号/登录态 handoff smoke 脚本已实现，并已补离线负向 fixture gate，证明不会把登录态来源误报为已提取成功、不会暴露下载或 artifact link。
- P1-3D 主站 handoff 已改为 deterministic early return，handler 级测试证明不会走 provider 且 html-artifacts 可列出 handoff；第三方 `/events` 入口也已补 deterministic unsupported-source card，endpoint 级测试证明首次投递、幂等重复和 reply 查询都不会走 provider。
- P2-1 授权录屏兜底 runbook 和 isolated script 已实现，self-test/dry-run/授权门禁通过；self-test 已补授权负向用例和可复制的脱敏 `sharedReceipt`；没有执行 live capture，没有接入 8 服务器生产服务。
- P2-2A/B/C/D/F 的本地质量切片已完成：质量报告、bright-canvas detector、讲师小窗/外部前景 crop、暗色/亮色/纯色低信息过滤、短动画转场过滤、OCR evidence 进入 notes/Markdown、清晰度/可读性风险提示均有本地验证。
- P2-2E 三样例质量矩阵 self-test scaffold、本地 `generated_artifacts/` 输入适配、public-course deliverables 分类入口、customer-authorized deliverables 输入入口和三输入组合矩阵已完成；quality matrix 可用 `--synthetic-deliverables <path>` 复核合成类本地产物，用 `--public-course-deliverables <path>` 把匿名公开视频样例归到 `public_course_video`，也可在获得明确授权后用 `--customer-deliverables <path> --customer-approval-id <approval_id>` 复核客户/operator 授权样例；三类输入同时提供时可生成 `matrix_complete=true` 的完整三类报告，approval id 不写入报告。
- 当前无授权离线验收 rollup 已通过：主站上传 self-test、第三方视频 PPT self-test、视频号/登录态 handoff self-test、授权录屏 helper self-test、质量矩阵 self-test 和 video deliverables validator 19 项均通过；这不替代 live smoke、8 服务器部署验收或客户授权样例。
- S1 公开视频 slides/presentation 候选访问和画面探测已完成；S2A 已完成 public manifest redaction 修复和复跑：公开视频候选生成 `final_pptx_ready`、96 帧、PPTX、`video_slides.md`、notes 和 `slide_quality_report.json`，`validate-video-deliverables` 已通过。S2B 已补 `--public-course-deliverables` 分类入口，quality matrix 现在把该样例归到 `public_course_video`；selected manifest 语义已修正为 `requested_selected_candidate_indices` 记录去重前请求，`selected_candidate_indices` 记录最终入选页；32x32 visual signature + changed-sample guard 已让 public candidate 从 5 页提升到 7 页，覆盖更多 build 内容；best-sharpness 代表帧选择把质量分提升到 61、sharpness high 页从 4 降到 3；visual-shape duplicate 已补保守去重路径和回归，shape-dedupe 复跑仍为 9 requested、7 selected、2 visual near duplicates、0 shape duplicates。第二/第三 public probe 证明额外匿名 slides 视频可跑通交付契约，其中 `Layered Nix Stores` 暴露并修复了白底模板 build 被 shape duplicate 误删的问题，修复后为 4 requested、3 selected、0 shape duplicates、quality score 68。当前 public 类样例可作为真实公开视频回执，但质量结论仍是需人工复核，不是无条件可交付。
- EP6 单页输出风险提示已完成：弱 public probe `Nix in Space` 复跑仍是 20 帧、1 页、`final_pptx_ready`，validator 通过；质量报告新增 `single_slide_output=true` 和 `single_slide_output_review_required`，public-course matrix 仍判 `needs_manual_review`。
- deliverables validator 已补 `slide_quality_report.risk_flags` 结构校验：risk flag 必须有有效 `code`、`severity`、`count` 和 `review_action`；`single_slide_output=true` 必须与 `single_slide_output_review_required` 风险一致。
- P0-3 字幕页映射契约本地切片已完成：无 transcript/subtitle evidence 的包不再硬性要求 `subtitle_page_map.json`，但文件存在或 manifest 声明存在时仍严格校验 mapped schema/redaction。
- live 上传/第三方回执仍待授权或凭据，P1-3D live pass 待部署后复跑，P2-1 live 授权样例未执行，P2-2E customer 质量矩阵仍待授权。本轮未部署 8 服务器。
**唯一 active plan：** `docs/plans/datamax-active-execution-plan.md`

## 0. 下一阶段执行总览

这一节是后续开发的入口，下面第 1-9 节保留事实基线、详细 runbook、验证矩阵和历史回执。后续执行时先看本节确认边界，再跳到对应 runbook。

### 0.1 执行边界

- 后续开发可按本计划切片修改代码、脚本、测试和文档；每个切片必须有对应验证证据。
- 允许把通过验证的切片同步到 GitHub；GitHub 同步不等于 8 服务器发版。
- 8 服务器只能在用户明确批准“发版/部署/重启服务”后再 pull、build、restart；本轮 P2-2E-1 不做这些动作。
- 不触碰 120 服务器。
- 继续不记录密钥、cookie、扫码会话、数据库 URL、provider payload、原始客户文件、原始客户行、私有 object path 或内部下载地址。
- “视频 PPT”只表示从视频里已经播放的 PPT/幻灯片/课件画面中抽取截图型 PPTX、Markdown、notes 和 manifest；不是把普通视频创作成可编辑原生 PPT。

### 0.2 当前已证明的能力

- 公开视频直链可进入 `VideoExtraction`，并生成 `final_pptx_ready`、截图型 PPTX、`video_slides.md`、slide notes、rectangle manifest、发布 manifest 和 version history。
- media.ccc / NixCon 2023 public slides candidate 已证明真实课件视频可离线抽帧并生成截图型 PPTX；public manifest redaction 修复后已通过 deliverables validator，但质量矩阵结论为 `needs_manual_review`。
- 主站 assistant-run-bound 可见链路已证明：用户能通过 `/api/v3/html-artifacts/{artifact_id}/files/{index}` 下载 PPTX、Markdown 和 manifests。
- 没有 transcript/subtitle evidence 的包不再硬性要求 `subtitle_page_map.json`；但只要文件存在、状态位为 true 或 manifest 声明该 artifact，就必须严格校验 mapped schema 和 redaction。
- `.mp4`、`.mov`、`.m4v`、`.webm`、`.mkv`、`.avi` 可作为视频素材上传或登记；“提取 PPT/幻灯片/课件”仍是特殊触发。
- 公开网页只能在暴露匿名可下载视频资源时自动解析；视频号/登录态链接保持 handoff，不抓取、不绕过登录、不声称已经看过视频。
- 授权录屏兜底已有 runbook 和 isolated helper，但只通过 self-test/dry-run；没有执行 live capture，也没有接入 8 服务器生产服务。
- 讲师小窗/外部前景遮挡的本地端到端 fixture 已证明 `foreground_component_v1` crop 能进入 rectangle manifest、selected slides、quality report 和 PPTX `a:srcRect`，减少 full-frame fallback。
- `slide_quality_report.json` 已支持可选 sharpness/readability 字段：可解码帧记录 measured score/risk，不可解码帧记录 `unavailable/unknown`，低对比字迹会进入 high-risk review，不泄露本地 frame path。
- 自动选页已拒绝短时不稳定段、短动画转场帧、暗色低信息稳定段、亮色低信息稳定段和近乎纯色低信息稳定段，避免黑屏、白屏、灰屏、空白转场和短动画帧被误选为 PPT 页；深色主题但有可见内容的课件页已有正向保留回归。
- `smoke:video-ppt-quality-matrix -- --self-test` 已固定 P2-2E 的三类样例矩阵结构；`--synthetic-deliverables <path>`、`--public-course-deliverables <path>` 和 `--customer-deliverables <path> --customer-approval-id <approval_id>` 已可读取本地 `generated_artifacts/`，复用 `validate-video-deliverables` 生成脱敏复核结论，也可组合成完整三输入报告。公开视频课程样例已归入 public category 并判为 `needs_manual_review`；customer 模式只在客户/operator 授权输入存在时使用，且必须提供授权引用，真实客户授权样例仍待授权。

### 0.3 下一阶段优先顺序

| 优先级 | 切片 | 现在是否可做 | 需要输入/授权 | 交付物 | 验收口径 |
| ---: | --- | --- | --- | --- | --- |
| 1 | P1-3B 主站上传视频 controlled smoke | 待授权后可做 | 用户批准在主站写入一条非客户公开视频 smoke 记录 | live smoke 回执，含 dataset/document/assistant run/artifact/download 证据 | `smoke:video-ppt-upload-main` 产出 `final_pptx_ready`，PPTX/Markdown/manifests 可下载 |
| 2 | P1-3C 第三方视频登记 special-trigger smoke | 待凭据后可做 | inbound bearer、`connection_id`、`source_id`、公开视频或上传文件 | live 第三方回执，区分“素材登记”和“PPT 抽取触发” | `smoke:external-video-ppt` 证明第三方 surface 可见；缺下载出口则记录为发布可见性缺口 |
| 3 | P1-3D 视频号/登录态 handoff live pass | 待部署窗口后可做 | 用户批准 8 服务器部署；第三方模式还需 bearer/context | main/external handoff live 回执 | 只返回 `login_gated_video_source_not_supported` 三选项 handoff，不抓视频、不抽帧、不走 provider |
| 4 | P2-1C 授权录屏样例 | 待 operator 授权后可做 | approval id、可播放来源、最大时长、音频策略、保留期、handoff 目标 | 录制 MP4 的脱敏回执，随后复用上传或第三方视频 PPT 抽取 | 失败能分流为播放/录制/无 PPT/抽帧/质量问题；不绕过平台权限 |
| 5 | P2-2E-3 客户授权视频样例 | 待授权后可做 | 客户/operator 授权、输入来源和保留策略 | 客户样例脱敏回执与质量结论；产物不进 Git | 区分可交付、需人工复核、不可交付，并记录 source access/video has no PPT/frame/crop/subtitle/artifact visibility 归因 |
| 6 | P2-2B/P2-2C/P2-2F 继续本地质量 fixture | 可立即做；不跑 live | 无 live 授权；只用本地 fixture 或第二个公开视频样例 | 已补深色主题内容页防误伤、低对比字迹质量报告、讲师小窗 crop、短动画转场过滤、shape duplicate 保守去重 | 不替代主站 live 或客户样例，只减少后续质量回归风险 |

### 0.4 可直接继续的本地开发切片

P2-2E-1、P2-2E-2A 和 P2-2E-2B-Next 已完成。若暂时没有主站上传授权、第三方 bearer 或 8 服务器部署窗口，后续本地工作只能继续做更窄的质量 fixture、第二个公开视频样例或客户授权前的准备；任何本地 fixture 都不能替代 live 验收：

1. P2-2E-2B-Next 完成结论：public candidate shape-dedupe 复跑仍是 9 requested、7 selected、2 visual near duplicates、0 shape duplicates，quality score 61，质量矩阵仍判 `needs_manual_review`；保守 shape duplicate 规则没有误删 build 页。
2. 第二/第三 public probe 完成结论：`Nix in Space` 只选出 1 页，降级为弱对照；`Layered Nix Stores` 先暴露 bright template build 被 shape duplicate 误删，新增 `visual_shape_duplicate_max_avg_luma` guard 后复跑为 4 requested、3 selected、1 visual near duplicate、0 shape duplicate、quality score 68。
3. P2-2B/P2-2C/P2-2F 的基础本地质量 fixture 已覆盖暗色/亮色/纯色低信息稳定转场、深色主题内容页防误伤、低对比字迹质量报告、讲师小窗/外部前景 crop、短动画转场和白底 build 防误删；后续更复杂动画 build/真实遮挡仍需真实样例复核。
4. 若 public candidate validator 通过但质量分仍低，按 `slide_quality_report.json` 的 risk flags 判断是继续 crop/dedupe/清晰度修复，还是标记为 `需人工复核`。
5. 已有 public candidate `需人工复核` 回执；客户授权样例仍缺失前，不声称三样例质量矩阵完成。
6. 任何本地质量切片都不跑 live smoke、不下载私有视频、不上传文件、不部署；验证通过后追加 `docs/validation/video-ppt-deliverable-smoke.md` 回执并同步 GitHub。

### 0.5 必须等待授权的动作

- 主站 live 上传 smoke 会写入主站测试数据，必须先获得用户明确批准。
- 第三方 live smoke 需要 operator 管理的 inbound bearer 和目标连接信息；没有凭据只能跑 self-test。
- 视频号/登录态 handoff 的 live pass 需要当前代码部署到 8 服务器后复跑；部署必须单独批准。
- 任何录屏都必须有授权记录；没有 `approval_id`、批准人、来源、用途、时长、音频策略、保留期和 handoff 目标时不得执行 live capture。
- 8 服务器内部录屏默认不启用；要评审依赖、隔离浏览器 profile、时长/容量/并发限制、清理策略和回滚窗口。

### 0.6 下一阶段完整可执行方案

本节保留上一版详细方案和历史 runbook。当前实际入口已经收口到第 0.7 节；这里的内容只作为背景、历史证据和命令细节补充，不再作为新的优先级入口，避免把本地验证、live smoke、发版和授权录屏混在一起。

#### 0.6.1 总体目标与不变量

目标：

1. 把视频中已经播放的 PPT、幻灯片、课件画面抽取成截图型 PPTX、`video_slides.md`、notes、manifest 和质量报告。
2. 证明入口覆盖主站直链、主站上传、第三方登记和公开页面 resolver。
3. 对微信视频号、登录态网页、客户私有平台等无法匿名下载视频文件的来源，给出可执行 handoff 或授权录屏兜底。
4. 用合成样例、公开视频课程、客户授权样例三类输入复核抽取质量，而不是只靠一个公开视频直链样例。

不变量：

- 不把普通视频“创作成 PPT”；只抽取视频里已经出现的课件/幻灯片页面。
- 不绕过登录、DRM、平台权限或客户授权；不要求用户交 cookie、扫码截图、账号密码、浏览器 storage 或 HAR。
- 不把 self-test、synthetic fixture 或后端 workflow smoke 说成 live 主站验收。
- 不提交原始视频、生成的 PPTX/帧图、客户文件、私有 object path、source URL、token、cookie、provider payload 或数据库 URL。
- GitHub 同步可以做；8 服务器发版、pull、build、restart 必须单独得到用户明确批准。

#### 0.6.2 默认执行顺序

如果用户没有额外授权，默认先走不写生产数据、不部署的路线；一旦拿到授权，再进入对应 live gate。

| 顺序 | 执行包 | 是否现在可做 | 是否写生产数据 | 是否需要 8 服务器部署 | 完成后得到什么 |
| ---: | --- | --- | --- | --- | --- |
| S1 | 公开视频课程候选准备与访问探测 | 已完成首个候选 | 否 | 否 | media.ccc/NixCon public slides candidate，匿名可下载、可 Range、抽样帧有多页 slide 状态 |
| S2A | public candidate deliverable contract 修复与复跑 | 已完成 | 否 | 否 | public manifest redaction 修复后 validator 通过，quality matrix 结论为 `needs_manual_review` |
| S2B | public candidate 质量增强分流 | 部分完成；人工复核待做 | 否 | 否 | `--public-course-deliverables` 已把样例归入 public category；下一步按 crop/dedupe/sharpness/subtitle/OCR 风险决定继续窄修复，或正式标为需人工复核 |
| S3 | P1-3B 主站上传 controlled smoke | 待用户批准 | 是，写一条非客户 smoke 上传/文档/run | 否，除非现网缺修复 | 上传视频入口能否生成并下载 PPTX/Markdown/manifests 的 live 回执 |
| S4 | P1-3C 第三方登记 controlled smoke | 待 bearer/context | 是，写第三方 smoke event/run | 否，除非现网缺修复 | 第三方“登记视频素材”和“提取视频里的 PPT”特殊触发的 live 回执 |
| S5 | P1-3D 视频号/登录态 handoff live pass | 待部署窗口 | 只写 lightweight smoke，不抓视频 | 是，需批准后部署当前修复 | 主站/第三方返回 handoff 卡片，不走 provider、不生成 PPT |
| S6 | P2-1C 授权录屏样例 | 待 operator 授权 | 录屏文件按授权策略保留 | 默认否；8 内部录屏另批 | 一个普通 MP4 输入，随后复用主站上传或第三方登记抽取 |
| S7 | P2-2E 客户授权质量矩阵 | 待客户/operator 授权 | 视授权输入而定 | 否，除非质量修复需部署 | 客户样例脱敏质量结论：可交付、需复核或不可交付 |

#### 0.6.3 公开视频课程样例执行计划

目的：补齐 P2-2E 真实 public course 样例，验证真实课件视频中的黑边、讲师小窗、转场、弱字幕、低清晰度和重复页，而不是继续只依赖 synthetic fixture。

当前候选状态：

- S1 已完成一次候选探测，结果记录在 `docs/validation/video-ppt-deliverable-smoke.md` 的 `2026-06-08 Public Slides Video Candidate Access Probe`。
- 已排除或降级的候选包括 GCU edShare lecturer-camera 视频、media.ccc 静态单页 slides feed 和一个 404 的 MIT 镜像候选。
- 当前可用 public candidate 是 media.ccc / NixCon 2023 的 `How to teach Nix in 5 minutes!` slides MP4：匿名可下载、支持 Range、页面有 `Slides -> Download mp4`，抽样帧确认有多页 slide 状态。
- S2A 已复跑离线抽取：`frame_count=96`，`deliverable_state=final_pptx_ready`，PPTX、`video_slides.md`、slide notes、selected slides manifest、rectangle manifest、quality report 和发布 manifests 均生成；`selected_count=5`，PPTX/Markdown 均为 5 页。
- public deliverables contract 已通过：`validate-video-deliverables` 接受 PPTX、final/published/version/extraction manifests、rectangle manifest、notes、Markdown 和 quality report；`subtitle_page_map.json` 缺失按无 transcript/subtitle evidence 的条件性交付处理。
- quality matrix 已将该产物归到 `public_course_video` 并判为 `needs_manual_review`：`quality_score=56`，风险包括 `full_frame_rectangle_fallback`、`missing_transcript_alignment`、`selected_slide_duplicates_removed`、`frame_sharpness_review_required` 和 `manual_review_required`；矩阵 summary 为 synthetic deliverable 1、public needs manual review 1、customer pending 1。

候选标准：

- 来源公开、非客户、非登录态、非 DRM。
- 可以匿名下载或 Range 读取；优先 `.mp4`、`.webm`、`.m4v` 等直接视频 URL。
- 视频内容明确包含正在播放的 PPT、幻灯片、白板课件或课程页面。
- 文件大小和时长适合受控 smoke；优先短视频或可截取片段，不把大文件提交到 Git。
- 记录只写 host、content type、size bucket、range support、duration、是否包含课件画面、为什么适合测试；不写敏感 query、私有 object path 或本地下载路径。

访问探测建议：

```bash
curl -fsSI -L "<candidate-video-url>"
curl -fsSIL -H "Range: bytes=0-1048575" "<candidate-video-url>"
ffprobe -hide_banner -v error -show_format -show_streams "<candidate-video-url>"
```

如果必须下载，下载目录只能在 `target/` 下，例如 `target/video-ppt-public-course-fixtures/`；下载文件、抽帧、PPTX、contact sheet 和 matrix report 都不提交。

抽取后复核：

1. 用现有视频抽取流程生成 `generated_artifacts/`。
2. 运行 `node tools/validate-video-deliverables.mjs <generated_artifacts>`。
3. 运行 `npm run smoke:video-ppt-quality-matrix -- --public-course-deliverables <generated_artifacts> --pretty --output-dir target/video-ppt-public-candidate-quality-matrix`；`--synthetic-deliverables` 只用于合成 PPT 视频或合成 fixture，不用于真实公开视频课程样例。
4. 人工抽样检查 PPTX 页、`video_slides.md` 页数、`selected_slides_manifest`、`slide_rectangles_manifest`、`slide_quality_report.json`。
5. 在 `docs/validation/video-ppt-deliverable-smoke.md` 追加脱敏回执；不能因为 public course 通过就标记 customer 样例完成。

当前 S2B 分流顺序：

1. 人工检查 7 页 PPTX/Markdown 与 selected slides manifest，确认 2 个视觉重复去重是否合理。
2. 复核 1 个 full-frame fallback 是否确实需要继续 crop detector 修复；如关键页仍可读，可保持 `需人工复核`。
3. 缺字幕/OCR 属于样例证据缺失，不阻断截图型 PPTX，但不能声称已完成逐页讲稿或字幕页映射。
4. 当前 3 页 high sharpness/readability 风险需要人工确认是否影响客户可读性；若影响，再追加 P2-2F 窄修复或选第二个 public course 样例对照。
5. selected manifest 语义已完成窄修复：`requested_selected_candidate_indices` 保留去重前请求，`selected_candidate_indices` 只保留最终进入 PPTX/Markdown 的候选，避免人工复核误读。
6. 稀疏文字/build 状态自动选页已完成窄修复：visual signature 升级到 32x32，并增加 changed-sample guard；public candidate 复跑为 `requested_selected_count=9`、`selected_count=7`，比旧结果多覆盖 `Know your Audience` 和后续 build 内容。
7. 稳定段 best-sharpness 代表帧选择已完成窄修复：同一稳定段内优先选 sharpness score 更高的帧，分数相同时回退 midpoint；public candidate 当前 quality score 为 61，仍保留弱 fade 页和 1 个 full-frame fallback。
8. 对 final selected frames 做人工页面级复核：确认 7 页是否可读，是否仍保留较弱的 fade/transition 页，是否需要继续 fade filter、crop 或 sharpness/readability 窄修复。
9. 若弱 fade/低对比页与相邻页是同一版式但普通 near-duplicate 未识别，下一步只允许做 contrast-normalized visual-shape duplicate 窄修复：保留 content fingerprint 和 visual near-duplicate 既有顺序，在 final accept 前增加 `visual_shape_duplicate` 去重原因，并把 `visual_shape_duplicate_count` 同步到 selected manifest、rectangle manifest 和 quality report summary。
10. visual-shape 去重必须有信号下限，不能只按低对比空白相似就删页；若 public candidate 复跑后 build 状态明显减少、关键页面缺失或 `requested_selected_candidate_indices`/`selected_candidate_indices` 契约回退，判定该窄修复失败。
11. 复跑 public candidate 时使用新的 `target/video-ppt-public-candidate-extraction-shape-dedupe/` 输出目录；只提交代码、测试和脱敏回执，不提交视频、帧图、PPTX、Markdown 或 `target/` 产物。
12. P2-2E-2B-Next 已完成保守 visual-shape duplicate 切片：新增 shape duplicate fixture，并把 `visual_shape_duplicate_count` 同步到 selected manifest、rectangle manifest 和 quality report summary；looser containment 曾误删 sparse text/build fixture，因此最终 signal-balance 门槛保持保守。
13. shape-dedupe public candidate 最终复跑：`deliverable_state=final_pptx_ready`、`frame_count=96`、`requested_selected_count=9`、`selected_count=7`、`visual_duplicate_count=2`、`visual_shape_duplicate_count=0`、`quality_score=61`，validator 通过，public-course matrix 仍为 `needs_manual_review`。
14. 第二 public probe `Nix in Space` 可匿名下载并通过交付契约，但只选出 1 页浏览器/Google Slides 画面，降级为弱对照样例。
15. 第三 public probe `Layered Nix Stores` 暴露 bright template build 被 shape duplicate 误删：修复前 4 requested 被压到 1 selected；新增 `visual_shape_duplicate_max_avg_luma` 后复跑为 4 requested、3 selected、1 visual near duplicate、0 shape duplicates、quality score 68，validator 和 public-course matrix 通过。
16. 当前 public candidates 只能证明真实公开视频样例可进入交付包并通过 contract；完整 P2-2E 仍缺客户授权样例。

验收结论必须落到三类之一：

- `可交付`：主要课件页完整可读，PPTX/Markdown/manifest 下载或本地包校验通过，质量报告无阻断风险。
- `需人工复核`：PPTX 生成但存在较多 crop fallback、遮挡、重复页、低清晰度、缺字幕映射或页数偏差。
- `不可交付`：视频不可匿名访问、内容没有 PPT/课件、抽帧失败、PPTX 无效、关键页缺失或产物无法安全发布。

#### 0.6.4 微信视频号和登录态来源处理计划

当前策略保持保守：`weixin.qq.com/sph/...`、`channels.weixin.qq.com/sph/...`、二维码登录页、私有播放页都不进入自动下载、抽帧或 OCR。原因是这些链接通常是播放入口，不是匿名可下载视频文件；公开文档和生态说明只支持打开/跳转到视频号视频，参数形态是 `finderUserName` 与 `feedId`，不是提供可下载媒体文件 URL。

产品行为：

1. 主站和第三方都返回 `login_gated_video_source_not_supported`。
2. 回复只给三条路：上传视频文件、提供匿名直接视频 URL、申请授权录屏处理。
3. 回复必须明确“当前没有拿到视频文件，因此还不能抽帧、OCR、转写或生成 PPT”。
4. 不要求 cookie、扫码、账号密码、浏览器登录态或平台内部接口 payload。
5. 如果用户之后上传同一视频文件或提供公开视频直链，则按普通视频输入重新触发“提取 PPT/幻灯片/课件”。

live 验收只验证 handoff，不验证抽取：

```bash
npm run smoke:video-ppt-handoff -- --self-test
npm run smoke:video-ppt-handoff -- \
  --mode main \
  --base-url https://v3.elepcloud.com \
  --local-thread-id video-ppt-handoff-YYYYMMDD-01 \
  --timeout-ms 60000 \
  --output-dir target/video-ppt-handoff-main-smoke
```

执行门槛：

- self-test 可随时跑。
- main/external live pass 需要当前 deterministic handoff 修复已经部署到 8 服务器；部署必须单独批准。
- 第三方 live pass 还需要 inbound bearer、`connection_id` 和 `source_id`。

#### 0.6.5 授权录屏兜底计划

录屏兜底只解决“operator 已合法播放但拿不到视频文件”的问题。录屏结果是一个普通 `.mp4` 输入，后续完全复用现有 `VideoExtraction`，不新建另一套 PPT 抽取逻辑。

执行分层：

1. 先让用户/客户尽量上传视频文件或给匿名直连 URL。
2. 无直链但已授权播放时，优先在 operator workstation 或 jump-host 做短时录屏。
3. 录完人工检查 MP4 是否确实包含课件/幻灯片画面，再通过 P1-3B 主站上传或 P1-3C 第三方登记触发抽取。
4. 只有当业务明确要求服务器内部录制时，才评审 8 服务器 capture fallback；默认 `CAPTURE_FALLBACK_ENABLED=false`，不进入生产路径。

最小授权记录：

- `approval_id`
- `approved_by`
- `source_host_redacted`
- `purpose`
- `max_duration_seconds`
- `capture_audio_allowed`
- `retention_days`
- `cleanup_policy`
- `handoff_to_datamax`

本地/受控 host 验收命令：

```bash
npm run capture:authorized-video -- --self-test
npm run capture:authorized-video -- \
  --dry-run \
  --ack-authorized \
  --approval-id APPROVAL-YYYYMMDD-001 \
  --approved-by operator-name \
  --url https://example.com/authorized-video-page \
  --purpose "authorized courseware video; upload/direct URL unavailable" \
  --duration-seconds 60 \
  --handoff upload-main
```

8 服务器内部录屏评审项：

- 显式开关，默认关闭。
- 独立临时 browser profile，不持久化登录态。
- Xvfb/Chrome/FFmpeg 或同等组件只用于短时任务。
- 并发 1、最大时长、最大文件大小、目标目录容量、清理定时器全部有硬限制。
- 日志仅记录 redacted host、job id、duration、file size、hash prefix、cleanup result。
- 部署、依赖安装、服务 restart 和 rollback 窗口全部单独审批。

#### 0.6.6 质量复核和完成定义

每条真实样例回执必须包含：

- 输入类别：direct URL、uploaded video、public page resolver、third-party registered video、authorized capture。
- 触发语是否明确包含“提取 PPT/幻灯片/课件”。
- workflow id、assistant run id 或 third-party run id。
- `frame_count`、`selected_count`、PPTX slide count、Markdown slide count。
- 交付文件：PPTX、`video_slides.md`、slide notes、rectangle manifest、selected slides manifest、final/published/version/extraction manifests、可选 quality report 和条件性 subtitle map。
- 质量风险：crop fallback、speaker obstruction、transition frame、duplicate removal、subtitle missing、OCR missing、sharpness/readability risk。
- 结论：可交付、需人工复核、不可交付。
- 失败归因：source access、video has no PPT、frame extraction、crop quality、subtitle alignment、artifact visibility、authorization missing、deployment missing。

阶段完成定义：

- `计划完成`：本文件和桌面副本同步；下一阶段步骤、命令、授权门槛、验收证据和失败归因清楚；GitHub 可同步 doc-only 变更。
- `本地质量完成`：相关 Rust/Node tests 和 `validate-video-deliverables` 通过，validation 台账有脱敏回执。
- `public candidate 完成`：公开视频候选复跑后 `validate-video-deliverables` 通过，quality matrix 不再给出 `deliverable_contract_invalid`；最终结论可为 `可交付` 或 `需人工复核`，但必须有 PPTX/Markdown/manifests/quality report 的脱敏回执。
- `主站入口完成`：P1-3B live controlled smoke 证明上传视频文件可以生成并下载 PPTX/Markdown/manifests。
- `第三方入口完成`：P1-3C live controlled smoke 证明第三方登记视频后，特殊触发能返回可见产物或明确 artifact visibility 缺口。
- `视频号处理完成`：P1-3D live pass 证明只返回 handoff，不抓取、不抽帧、不走 provider。
- `录屏兜底完成`：P2-1C 授权样例生成 MP4，并复用普通视频抽取链路得到可复核结果。
- `三样例质量矩阵完成`：合成 PPT、公开视频课程、客户授权样例三类都有脱敏回执和人工质量结论；任何一类 pending 时不得标记完整完成。

#### 0.6.7 立即可执行清单

这部分是下一次实际开发时的逐项 checklist。除非用户明确批准，否则全部停留在本地和 GitHub，不碰 8 服务器。

1. 收口本轮计划文档。
   - 更新本文件和桌面副本 `/Users/manslive01/Desktop/datamax-active-execution-plan.md`。
   - 只提交 repo 内文档；桌面副本仅同步，不进 Git。
   - 若同步 GitHub，commit message 使用 doc-only 口径，不包含 `target/` 产物或未验证代码。

2. P2-2E-2A 已完成的验证基线。
   - public manifest `uri` 和嵌套路径类字段已走 public redaction；`tools/validate-video-deliverables.mjs` 未放宽。
   - 离线 smoke helper 已修 `selected_count` 摘要来源。
   - 已通过 `cargo fmt --check`、`cargo check -p media-worker --bin video_ppt_offline_smoke`、`CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib`、`npm run test:video-deliverables`、public candidate `validate-video-deliverables` 和 quality matrix。

3. P2-2E-2B-Next 已完成的质量分流。
   - validator 已通过，`--public-course-deliverables` 已把样例归入 `public_course_video`，不再按 redaction/schema 或 public sample missing 处理。
   - 当前结论是 `needs_manual_review`，优先看 `full_frame_rectangle_fallback`、`missing_transcript_alignment`、`selected_slide_duplicates_removed`、`frame_sharpness_review_required`。
   - 先记录人工复核结论，再决定是否做窄修复；不要为了把 public candidate 改成 `可交付` 而放宽 validator 或质量矩阵。
   - selected manifest 语义已修，`selected_count`、最终 `selected_candidate_indices` 和 PPTX/Markdown 页数一致，同时保留去重前 requested indices 供审计；后续窄修复不得回退该契约。
   - 稀疏文字/build 状态选页已修并补回归，public candidate 当前是 7 页；若人工复核确认这些页可读，可把 public candidate 保持为 `需人工复核` 的真实样例回执。
   - best-sharpness 代表帧选择已修并补规则级回归，当前 public candidate quality score 为 61，仍是 `needs_manual_review`。
   - visual-shape duplicate 窄修复已补定向 Rust test；selected manifest 出现 `dedupe_reason=visual_shape_duplicate` 时必须同时有 matched candidate、similarity score、threshold 和 `visual_shape_duplicate_count`；rectangle manifest 和 quality report summary 的 duplicate counts 与 selected manifest 一致。
   - shape-dedupe public candidate 已复跑并通过 validator/quality matrix；真实样例没有新增 shape duplicate，保留 7 页和 `needs_manual_review` 结论，因为更宽的 containment 会误删 sparse text/build 页。
   - 若还要继续优化 public candidate，下一步不能放宽 validator；只能选择更明确的 crop detector、sharpness/readability、第二个公开视频样例，或等待客户授权样例。

4. 记录与同步。
   - 追加 `docs/validation/video-ppt-deliverable-smoke.md` 脱敏回执，包含 frame/selected/PPTX/Markdown/quality/matrix 结论和安全边界。
   - 跑 `git diff --check`。
   - 只把通过验证的代码和文档同步 GitHub；`target/`、原始视频、抽帧、PPTX、客户文件和私有路径不提交。

5. 等授权后再进入 live gate。
   - 主站上传：必须获得“可在主站写一条非客户 smoke 记录”的明确授权。
   - 第三方登记：必须获得 inbound bearer、`connection_id`、`source_id` 和输入来源。
   - 视频号/登录态 handoff live：必须先批准 8 服务器部署窗口；验收只看 handoff，不抽视频。
   - 授权录屏：必须有 approval record；录屏产物作为普通 MP4 输入，不新增绕过平台的抓取逻辑。

### 0.7 收口版完整可执行计划

本节是当前实际执行入口。第 0.6 节保留详细背景和历史 runbook；后续执行时先按本节的执行包推进，遇到需要命令细节或历史证据时再跳回第 1-9 节。

#### 0.7.1 执行目标和边界

目标：

1. 继续围绕“提取视频里已经播放的 PPT/幻灯片/课件”开发，不把普通视频创作成 PPT。
2. 用三个层级验证能力：本地 deterministic/fixture、匿名公开视频课程、客户或 operator 授权样例。
3. 把主站上传、第三方登记、公开页面 resolver、视频号/登录态 handoff 和授权录屏兜底拆成独立 gate。
4. 先完成计划和 GitHub 文档同步；后续代码或 live smoke 只在对应授权齐全时执行。

硬边界：

- 本轮计划编写不改业务代码、不跑 live smoke、不部署、不重启 8 服务器。
- GitHub 同步允许 doc-only；GitHub 同步不等于 8 服务器发版。
- 8 服务器只有在用户明确批准“发版/部署/重启服务”后才允许 pull、build、restart。
- 120 服务器不在本计划范围内。
- 不提交或打印原始视频、抽帧、PPTX、客户文件、私有 object path、source URL、cookie、token、provider payload、数据库 URL。
- 微信视频号、登录态网页、二维码登录页和客户私有平台默认不自动抓取；只能走上传文件、匿名直连 URL 或 operator 授权录屏。

#### 0.7.2 当前完成事实和剩余缺口

| 方向 | 已完成事实 | 仍缺什么 | 下一步 |
| --- | --- | --- | --- |
| 主站 direct URL | `smoke:video-ppt-main-visible` 已证明 assistant-run-bound 可见产物，PPTX/Markdown/manifests 可下载 | 作为发布前回归保留，不代表上传入口 | 仅在新部署后按需复跑 |
| 主站上传视频 | `smoke:video-ppt-upload-main` 已实现，并已补离线 self-test 验证 artifact/download contract、最小 PPTX/Markdown 下载校验和视频扩展识别 | live controlled smoke 待用户批准，因为会写一条非客户 smoke 记录 | 执行包 EP2 |
| 第三方登记视频 | `smoke:external-video-ppt` 已实现，self-test 已覆盖 reply surface 与最小 PPTX/Markdown 下载校验 | live bearer、`connection_id`、`source_id` 和授权输入 | 执行包 EP3 |
| 公开视频课程样例 | media.ccc 三个匿名 slides 样例跑通过；主样例 7 页、quality score 61，第三样例修复后 3 页、quality score 68 | 均仍是 `needs_manual_review`，不能替代客户授权样例 | 执行包 EP1/EP5 |
| 微信视频号和登录态 | 主站 early return、第三方 deterministic card 本地测试通过 | live handoff pass 需要先部署到 8 服务器 | 执行包 EP4 |
| 授权录屏 | runbook 和 isolated helper 已实现，self-test/dry-run/授权门禁通过 | 没有 live capture；8 服务器内部录屏未启用 | 执行包 EP4 |
| 三样例质量矩阵 | synthetic self-test、public-course deliverables 输入已可用 | customer authorized sample pending | 执行包 EP5 |
| 8 服务器 | 本轮不部署；历史部署基线保留在第 2 节 | P1-3D live pass 需要部署当前 handoff 修复后复跑 | 执行包 EP7 |

#### 0.7.3 EP0 计划收口与 GitHub 文档同步

目的：完成当前计划文档，不改代码、不跑 live、不发版。

执行步骤：

1. 更新仓库计划：`docs/plans/datamax-active-execution-plan.md`。
2. 同步桌面副本：`/Users/manslive01/Desktop/datamax-active-execution-plan.md`。
3. 做文档级检查，确认只有计划文档变更。
4. 如工作树干净且 diff 为 doc-only，可提交并推送 GitHub。

建议命令：

```bash
git status --short --branch
cp docs/plans/datamax-active-execution-plan.md /Users/manslive01/Desktop/datamax-active-execution-plan.md
cmp -s docs/plans/datamax-active-execution-plan.md /Users/manslive01/Desktop/datamax-active-execution-plan.md
git diff --check
git diff --stat
git diff -- docs/plans/datamax-active-execution-plan.md
git add docs/plans/datamax-active-execution-plan.md
git commit -m "Refine DataMax executable plan"
git push origin main
```

验收：

- `git diff --check` 通过。
- 桌面副本和仓库计划完全一致。
- Git diff 只包含计划文档；不包含 `target/`、视频、抽帧、PPTX、客户文件。
- 最终回执明确：未改业务代码、未部署 8 服务器、未触碰 120 服务器。

#### 0.7.4 EP1 无授权本地基线和公开视频复核

目的：在没有 live 授权时继续保住质量基线，避免把 synthetic fixture 误当客户验收。

适用条件：

- 没有主站上传授权。
- 没有第三方 bearer/context。
- 没有 8 服务器部署窗口。
- 没有客户/operator 授权样例。

必跑基线：

```bash
cargo fmt --check
npm run test:video-deliverables
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty
cargo check -p media-worker --bin video_ppt_offline_smoke
git diff --check
```

公开视频交付包复核：

```bash
node tools/validate-video-deliverables.mjs <generated_artifacts>
npm run smoke:video-ppt-quality-matrix -- \
  --public-course-deliverables <generated_artifacts> \
  --pretty \
  --output-dir target/video-ppt-public-candidate-quality-matrix
```

当前 public 样例结论：

1. `How to teach Nix in 5 minutes!`：96 帧，9 requested、7 selected、2 visual near duplicates、0 shape duplicates，quality score 61，validator 通过，结论 `needs_manual_review`。
2. `Nix in Space`：20 帧，只选出 1 页，quality score 50，validator 通过但降级为弱对照样例。
3. `Layered Nix Stores`：白底 build 防误删修复后 4 requested、3 selected、1 visual near duplicate、0 shape duplicates，quality score 68，validator 和 public-course matrix 通过，结论仍是 `needs_manual_review`。

人工复核清单：

- PPTX slide count、Markdown slide count、`selected_count` 是否一致。
- 是否保留真实 build 状态，是否误删关键 bullet/page。
- 是否有明显 fade/transition 页被当作最终页。
- crop 模式是否过多 fallback；fallback 页是否仍可读。
- `slide_quality_report.json` 的 sharpness/readability 风险是否影响交付。
- 缺字幕/OCR 是否只是证据缺失；不得声称已完成逐页讲稿或字幕映射。

验收：

- 可把 public 样例标为 `可交付`、`需人工复核` 或 `不可交付`，但必须有脱敏回执。
- 当前已有 public 样例应保持 `需人工复核`，除非人工复核和后续窄修复足以证明关键页完整可读。
- public 样例不能替代 P2-2E customer authorized sample。

#### 0.7.5 EP2 主站上传视频 controlled smoke

目的：证明用户把 `.mp4/.mov/.m4v/.webm/.mkv/.avi` 上传到 V3 主站后，只要明确触发“提取 PPT/幻灯片/课件”，就能生成并下载截图型 PPTX、Markdown 和 manifests。

进入条件：

- 用户明确批准在主站写入一条非客户公开视频 smoke 记录。
- 使用公开、非客户、非登录态、可安全保留或可清理的样例视频。
- 不需要部署，除非 smoke 证明现网缺少已修复代码且用户另批部署。

建议命令：

```bash
npm run smoke:video-ppt-upload-main -- \
  --base-url https://v3.elepcloud.com \
  --fixture-file target/video-ppt-public-course-probe/nix-teach-5min-slides.mp4 \
  --fixture-name nix-teach-5min-slides-upload-smoke.mp4 \
  --local-thread-id video-ppt-upload-main-YYYYMMDD-01 \
  --prompt "请提取刚上传视频里的 PPT/幻灯片/课件，输出 PPTX、video_slides.md 和交付 manifest。" \
  --output-dir target/video-ppt-upload-main-smoke
```

如果使用远程公开样例：

```bash
npm run smoke:video-ppt-upload-main -- \
  --base-url https://v3.elepcloud.com \
  --fixture-url https://v3.elepcloud.com/generated-artifacts/samples/react-in-5-minutes.mp4 \
  --local-thread-id video-ppt-upload-main-YYYYMMDD-01 \
  --output-dir target/video-ppt-upload-main-smoke
```

验收：

- 创建或复用 smoke dataset，不污染客户数据。
- 上传、文档登记、ingest、assistant run、视频 PPT 特殊触发完整跑通。
- `deliverable_status.state=final_pptx_ready`。
- PPTX、`video_slides.md`、final/published/version/extraction manifests 可通过 HTML artifact file API 下载。
- PPTX OOXML entry 存在，PPTX slide count 与 Markdown slide count 一致。
- 回执追加到 `docs/validation/video-ppt-deliverable-smoke.md`，只记录脱敏 ids 和文件 kind，不记录 token/cookie/source URL/private path。

失败分流：

- 上传失败：记录 upload/local-document API 问题。
- ingest 失败：记录 video parse 或 queue 问题。
- assistant run 未触发 `VideoExtraction`：记录特殊触发识别问题。
- 有 PPTX 但无下载：记录 artifact visibility 问题。
- 产物存在但质量差：进入 EP1/EP5 质量复核，不放宽 contract。

#### 0.7.6 EP3 第三方视频登记 special-trigger smoke

目的：证明第三方把视频素材登记给 DataMax 后，只有在用户明确说“提取视频里的 PPT/幻灯片/课件”时才进入 `VideoExtraction`，不是普通视频解析默认动作。

进入条件：

- operator 提供 inbound bearer。
- 明确 `connection_id`、`source_id`。
- 输入为公开视频直链、第三方自有对象存储匿名下载 URL，或已授权上传文件。
- 不打印 bearer，不把请求 payload 原文写入文档。

建议命令：

```bash
npm run smoke:external-video-ppt -- \
  --base-url https://v3.elepcloud.com \
  --connection-id <connection_id> \
  --source-id <source_id> \
  --content-url <anonymous_public_video_url> \
  --external-document-id video-ppt-external-YYYYMMDD-01 \
  --message "请提取这个视频里的 PPT/幻灯片/课件，并返回 PPTX、video_slides.md 和交付 manifest。" \
  --output-dir target/external-video-ppt-smoke
```

本地无凭据时只能跑：

```bash
npm run smoke:external-video-ppt -- --self-test
```

验收：

- 第三方登记视频素材和 special-trigger 消息分开记录。
- 返回 run/reply/card 能说明 `final_pptx_ready` 或明确的失败分流。
- 如第三方 surface 还不能下载 PPTX/Markdown/manifests，记录为 `artifact_visibility_gap`，不把后端成功当成第三方交付完成。
- 回执只写脱敏 ids、状态、file kinds、质量结论。

#### 0.7.7 EP4 微信视频号/登录态来源和授权录屏兜底

目的：处理“客户只给链接，但链接里拿不到匿名视频文件”的真实场景，避免误导用户以为系统已经看过视频。

默认来源分类：

- `.mp4/.mov/.m4v/.webm/.mkv/.avi` 直链：进入普通视频输入。
- 公开网页暴露 `<video>`、`<source>`、OpenGraph、Twitter、JSON-LD video URL：resolver 提取候选后进入普通视频输入。
- `weixin.qq.com/sph/...`、`channels.weixin.qq.com/sph/...`、二维码登录页、私有播放页、需要 cookie 的页面：返回 handoff，不抓取、不抽帧、不 OCR、不生成 PPT。

针对 `https://weixin.qq.com/sph/AhfmOtV8P5` 这类链接的产品回执：

1. 当前没有拿到可匿名下载的视频文件，因此不能声称已解析视频内容。
2. 给三条路：上传视频文件、提供匿名直接视频 URL、申请授权录屏处理。
3. 如果之后上传同一视频文件，则按上传视频重新触发“提取视频中的 PPT/幻灯片/课件”。

handoff live 验收：

```bash
npm run smoke:video-ppt-handoff -- --self-test
npm run smoke:video-ppt-handoff -- \
  --mode main \
  --base-url https://v3.elepcloud.com \
  --local-thread-id video-ppt-handoff-YYYYMMDD-01 \
  --timeout-ms 60000 \
  --output-dir target/video-ppt-handoff-main-smoke
```

进入条件：

- self-test 可随时跑。
- main live pass 需要当前 deterministic handoff 修复已部署到 8 服务器，所以必须先走 EP7 部署批准。
- external live pass 还需要 inbound bearer、`connection_id`、`source_id`。

授权录屏兜底：

- 录屏只在 operator 已合法播放且有授权记录时执行。
- 首选 workstation/jump-host 短时录制，生成普通 `.mp4` 后走 EP2 或 EP3。
- 8 服务器内部录屏不是默认能力；必须单独评审和批准。

授权记录最少包含：

- `approval_id`
- `approved_by`
- `source_host_redacted`
- `purpose`
- `max_duration_seconds`
- `capture_audio_allowed`
- `retention_days`
- `cleanup_policy`
- `handoff_to_datamax`

dry-run 命令：

```bash
npm run capture:authorized-video -- --self-test
npm run capture:authorized-video -- \
  --dry-run \
  --ack-authorized \
  --approval-id APPROVAL-YYYYMMDD-001 \
  --approved-by operator-name \
  --url https://example.com/authorized-video-page \
  --purpose "authorized courseware video; upload/direct URL unavailable" \
  --duration-seconds 60 \
  --handoff upload-main
```

8 服务器内部录屏评审项：

- `CAPTURE_FALLBACK_ENABLED=false` 默认关闭。
- 独立临时 browser profile，不持久化登录态。
- Xvfb/Chrome/FFmpeg 或等价组件只用于短时任务。
- 并发 1、最大时长、最大文件大小、临时目录容量、清理定时器都有硬限制。
- 日志只记录 redacted host、job id、duration、file size、hash prefix、cleanup result。
- 依赖安装、服务 restart、回滚点和保留期全部单独审批。

#### 0.7.8 EP5 客户或 operator 授权质量矩阵

目的：完成 P2-2E 的真实三样例质量矩阵，不能只靠 synthetic 和 public samples。

三类样例：

| 类别 | 当前状态 | 完成条件 |
| --- | --- | --- |
| synthetic PPT playback | self-test 和本地 deliverables 输入已可用 | validator 通过，质量矩阵为 deliverable 或带明确风险 |
| public course video | media.ccc 样例已跑通，当前 `needs_manual_review` | 至少一个 public 样例有脱敏回执和人工质量结论 |
| customer authorized video | pending | 客户/operator 授权输入，产物不进 Git，回执脱敏 |

客户样例执行：

1. 确认授权、输入来源、保留期和清理策略。
2. 如果是上传文件，走 EP2；如果是第三方登记，走 EP3；如果只有授权播放页，先走 EP4 录屏。
3. 生成 `generated_artifacts/` 后运行 validator 和 quality matrix。
4. 人工复核 PPTX/Markdown/manifest/quality report。
5. 结论只能是 `可交付`、`需人工复核`、`不可交付` 三选一。

质量矩阵命令：

```bash
node tools/validate-video-deliverables.mjs <generated_artifacts>
npm run smoke:video-ppt-quality-matrix -- \
  --synthetic-deliverables <synthetic_generated_artifacts> \
  --pretty \
  --output-dir target/video-ppt-quality-matrix-synthetic-smoke
npm run smoke:video-ppt-quality-matrix -- \
  --public-course-deliverables <public_generated_artifacts> \
  --pretty \
  --output-dir target/video-ppt-quality-matrix-public-smoke
npm run smoke:video-ppt-quality-matrix -- \
  --customer-deliverables <customer_authorized_generated_artifacts> \
  --customer-approval-id APPROVAL-YYYYMMDD-001 \
  --pretty \
  --output-dir target/video-ppt-quality-matrix-customer-smoke
npm run smoke:video-ppt-quality-matrix -- \
  --synthetic-deliverables <synthetic_generated_artifacts> \
  --public-course-deliverables <public_generated_artifacts> \
  --customer-deliverables <customer_authorized_generated_artifacts> \
  --customer-approval-id APPROVAL-YYYYMMDD-001 \
  --pretty \
  --output-dir target/video-ppt-quality-matrix-complete-smoke
```

三类 deliverables 输入可以组合；只有 synthetic、public course、customer authorized 三类输入都提供时，报告才允许 `matrix_complete=true`。`--customer-deliverables` 已补齐，但只表示脚本能够复核一个已授权的客户/operator 样例交付包；使用该参数必须同时提供 `--customer-approval-id`，报告只记录授权引用存在并已脱敏，不写入 approval id 原文。没有真实授权输入时，customer gate 仍是 pending，不得用 public 或 synthetic 样例冒充客户样例。

失败归因：

- `source_access`: 无法匿名下载、授权不足、登录态不可用。
- `video_has_no_ppt`: 视频内容没有 PPT/课件画面。
- `frame_extraction`: FFmpeg、输入格式、时长或缓存失败。
- `selection_quality`: 关键页缺失、重复页过多、转场页误选。
- `crop_quality`: full-frame fallback 过多、讲师遮挡、边界错误。
- `subtitle_alignment`: 没有 transcript/subtitle evidence 或页映射失败。
- `artifact_visibility`: 产物生成但用户 surface 下载不可见。

#### 0.7.9 EP6 后续本地质量窄修复

目的：在没有 live/客户授权时，继续降低真实样例风险，但不把本地修复冒充 live 验收。

可选切片：

1. 单页输出风险提示已完成：对真实 public sample 只选出 1 页的情况，`slide_quality_report.json` 增加 `single_slide_output_review_required` 和 `summary.single_slide_output=true`，但不把 `final_pptx_ready` 改成失败；`Nix in Space` 复跑已验证该路径。
2. 更明确的 crop detector：针对 full-frame fallback 仍可见但边界不稳定的 public 页做 fixture。
3. 清晰度/readability 分流：把低清晰、低对比和 fade 页更明确标为人工复核。
4. OCR/subtitle 证据增强：只把 OCR 当作 page notes evidence，不伪装成 transcript/subtitle map。
5. 更多匿名公开视频课程样例：只选公开、非登录态、可下载、包含课件画面的短样例。

验收：

- 每个窄修复必须有 Rust/Node 定向测试。
- 复跑最小 public sample 或 synthetic fixture。
- 不放宽 `validate-video-deliverables` 的 redaction/schema 约束。
- 不上传、不跑 live、不部署。

#### 0.7.10 EP7 8 服务器部署门槛

本计划本轮不部署。后续只有用户明确要求“发 8 服务器/部署/重启服务”时才进入本包。

进入条件：

1. 待部署 commit 已推送 GitHub。
2. 本地必要测试通过，至少覆盖受影响脚本、validator、media-worker/platform-api 定向测试。
3. 明确本次部署目的：例如 P1-3D handoff live pass，而不是泛泛“同步一下”。
4. 明确影响服务、回滚 commit、预计停机/重启窗口。
5. 不触碰 120 服务器；不处理无关未跟踪文件。

部署后验收：

- 服务 active。
- media-worker/API 日志无启动错误。
- 如果目标是 P1-3D，则复跑 `smoke:video-ppt-handoff -- --mode main`，只验收 handoff card。
- 如果目标是主站上传或第三方 smoke，则按 EP2/EP3 的 live 回执记录。
- 失败时先记录错误和回滚点，不继续扩大范围。

#### 0.7.11 统一回执模板

每次执行包完成后，在 `docs/validation/video-ppt-deliverable-smoke.md` 追加脱敏回执：

```text
## YYYY-MM-DD <Execution Package Name>

Scope:
- input_type:
- trigger:
- authorization:
- deployment:

Evidence:
- workflow_id / assistant_run_id / third_party_run_id:
- artifact_id:
- frame_count:
- selected_count:
- pptx_slide_count:
- markdown_slide_count:
- deliverable_state:
- file_kinds:
- validator:
- quality_matrix:

Quality:
- crop:
- dedupe:
- transition/fade:
- sharpness/readability:
- subtitle/transcript:
- ocr:

Verdict:
- deliverable | needs_manual_review | not_deliverable
- reason:
- next_action:

Safety:
- no customer raw files committed
- no private paths/source URLs/tokens/cookies/provider payloads recorded
- no 8-server deploy unless explicitly approved
```

#### 0.7.12 下一步推荐顺序

1. 本轮先完成 EP0：计划文档和桌面副本同步，doc-only GitHub 提交，不发 8 服务器。
2. 如果用户批准写主站非客户 smoke 记录，执行 EP2 主站上传视频 controlled smoke。
3. 如果 operator 提供第三方 bearer/context，执行 EP3 第三方视频登记 special-trigger smoke。
4. 如果用户批准 8 服务器部署窗口，先按 EP7 部署当前 handoff 修复，再执行 EP4 的 main handoff live pass。
5. 如果客户或 operator 提供授权样例，执行 EP5；没有授权时只做 EP1/EP6 的本地质量复核。

#### 0.7.13 功能全量验收闭环

这一节定义“从当前状态继续开发到视频 PPT 能力验收通过”的总控闭环。后续每次执行只选一个执行包，不把缺授权的 live gate 和可本地完成的质量切片混在一起。

当前状态：

| Gate | 状态 | 是否阻塞继续开发 | 下一动作 |
| --- | --- | --- | --- |
| 计划收口 | 本节已收口为 EP0-EP7 执行包 | 不阻塞 | 同步桌面副本；如需要，做 doc-only GitHub 提交 |
| 本地交付契约 | 已有 validator、media-worker、quality matrix 和 public probes 证据 | 不阻塞 | 后续代码变更都复跑最小相关验证 |
| 公开视频质量样例 | 三个 media.ccc slides 样例可作为 public 证据，结论均偏 `needs_manual_review` | 不阻塞，但不能冒充客户样例 | 保留为真实 public 回执；继续窄修复只能降低风险，不能替代 live/customer gate |
| 主站上传入口 | 脚本已实现，离线 self-test 已补并覆盖最小 PPTX/Markdown 校验；live smoke 未跑 | 阻塞“上传入口已验收”结论 | 需要用户批准写一条非客户 smoke 记录 |
| 第三方登记入口 | 脚本和 self-test 已实现，离线校验覆盖 reply surface 与最小 PPTX/Markdown；live bearer/context 缺失 | 阻塞“第三方入口已验收”结论 | 需要 inbound bearer、`connection_id`、`source_id` 和安全输入 |
| 视频号/登录态 handoff | 本地 deterministic 逻辑已过；handoff self-test 已补负向 fixture 与无网络/provider/下载信号报告；live pass 需要当前代码在 8 服务器 | 阻塞“现网 handoff 已验收”结论 | 需要用户单独批准 8 服务器部署窗口；验收只看 handoff，不抽视频 |
| 授权录屏兜底 | runbook/helper/self-test/dry-run 门禁已具备；self-test 已补授权负向用例和 `sharedReceipt` 脱敏检查 | 阻塞“兜底样例已验收”结论 | 需要 operator approval record 和可播放来源；默认 workstation/jump-host，不默认 8 服务器 |
| 客户授权质量矩阵 | 脚本支持 customer input 和 approval id redaction；真实样例缺失 | 阻塞 P2-2E complete | 需要客户/operator 授权样例；产物不进 Git |

执行路线：

1. **无新授权时**：只执行 EP1/EP6。可以继续做本地 fixture、public sample 复核、validator/quality-matrix 风险门、文档台账；不能跑主站上传 live、第三方 live、录屏 live 或 8 服务器部署。
2. **批准主站写 smoke 数据后**：执行 EP2。完成条件是上传、登记、ingest、assistant run、`VideoExtraction`、HTML artifact 下载和 PPTX/Markdown/manifests 校验全部有脱敏回执。
3. **提供第三方 bearer/context 后**：执行 EP3。完成条件是“视频素材登记”和“提取视频里的 PPT”特殊触发分别有回执，第三方 surface 可见产物或明确记录 `artifact_visibility_gap`。
4. **批准 8 服务器部署窗口后**：先执行 EP7，再执行 EP4 handoff live。完成条件是现网只返回 `login_gated_video_source_not_supported` 三选项，不抓视频、不抽帧、不走 provider、不生成 PPT。
5. **提供 operator 授权播放来源后**：执行 EP4 的授权录屏兜底。完成条件是 approval record 完整，dry-run 通过，live capture 只在授权 workstation/jump-host 执行，录制 MP4 人工确认包含课件画面后再走 EP2 或 EP3。
6. **提供客户/operator 授权样例后**：执行 EP5。完成条件是 synthetic、public course、customer authorized 三类 deliverables 同时进入 quality matrix，`matrix_complete=true`，报告只记录授权引用存在且已脱敏，不记录 approval id 原文或客户路径。

最终验收必须同时满足：

- `.mp4`、`.mov`、`.m4v`、`.webm`、`.mkv`、`.avi` 上传或登记后，在“提取 PPT/幻灯片/课件”特殊触发下能进入视频 PPT 抽取；普通视频解析不默认变成 PPT 抽取。
- 主站 direct URL、主站上传、第三方登记三类入口至少各有一条通过或有明确、可复现的失败分流回执。
- 视频号/登录态来源在现网返回 handoff；回执明确“未拿到视频文件，不能声称已抽帧/OCR/生成 PPT”。
- 授权录屏只作为 fallback；录屏文件作为普通 MP4 输入复用现有 `VideoExtraction`，不引入绕过平台权限的新路径。
- PPTX、`video_slides.md`、slide notes、rectangle manifest、selected manifest、final/published/version/extraction manifests、可选 quality report、条件性 subtitle map 的交付契约通过 validator。
- `slide_quality_report.json` 对单页输出、full-frame fallback、字幕缺失、重复页、清晰度/可读性风险有可解释 risk flags；`needs_manual_review` 不被包装成 clean deliverable。
- 所有回执都写入 `docs/validation/video-ppt-deliverable-smoke.md`，且不包含原始视频、帧图、PPTX、source URL、cookie、token、provider payload、数据库 URL、客户文件、私有 object path 或本地绝对 artifact path。
- 通过验证的切片可同步 GitHub；8 服务器部署、pull、build、restart 必须单独批准，并且部署后要有对应 live smoke 回执。

给用户/运营的最小决策请求模板：

```text
需要你确认一个 gate：

选项 A：批准主站上传 controlled smoke
- 写入一条非客户公开视频 smoke 上传/文档/run 记录
- 不部署、不重启 8 服务器
- 输出主站上传入口是否能生成并下载 PPTX/Markdown/manifests 的回执

选项 B：提供第三方 live smoke 信息
- inbound bearer、connection_id、source_id
- 公开视频直链或已授权上传文件
- 输出第三方登记视频后特殊触发“提取 PPT”的回执

选项 C：批准 8 服务器部署窗口
- 只部署已推送 GitHub 的当前 commit
- 目标是验证视频号/登录态 handoff live pass
- 不启用录屏，不触碰 120 服务器

选项 D：提供 operator 授权录屏样例
- approval id、approved by、来源 host 摘要、用途、时长、音频策略、保留期、handoff 目标
- 先 dry-run，再短时录屏，录完作为普通 MP4 进入主站上传或第三方登记

选项 E：提供客户/operator 授权质量样例
- 明确授权和保留策略
- 产物不进 Git，只写脱敏质量结论
- 用于补齐 P2-2E 三样例质量矩阵
```

## 1. 计划原则

- DataMax 继续只有一份 active plan；历史计划、执行回执和测试记录压缩到本文件的事实基线和后续队列中。
- 计划收拢阶段已结束；后续允许按本计划切片改代码、脚本和文档，但每个切片必须有对应验证证据。
- GitHub 同步可以包含通过验证的代码/脚本/文档提交；8 服务器只能在用户明确批准“发版/部署”后再 pull、build、restart。
- 120 服务器不在本计划范围内。
- 不记录或传播密钥、cookie、扫码会话、数据库 URL、provider payload、原始客户文件、原始客户行、私有 object path、内部下载地址。
- 视频 PPT 能力只面向“视频里已经播放 PPT/幻灯片/课件”的场景；不是把普通视频创作成 PPT。

## 2. 当前事实基线

### 2.1 代码与部署基线

- P0-3 执行前本地 `HEAD=origin/main=4e77a5f`，对应 `4e77a5f Refresh DataMax next-phase execution plan`。P1-3D 主站 early-return 修复已同步到 `806be94 Short-circuit WeChat video handoff`，第三方 `/events` deterministic handoff follow-up 已同步到 `ad929ee`，P2-2A/P2-2B/P2-2C/P2-2D 本地质量切片已同步到 `bca7481`、`b26049d`、`d558305`、`4c79f96`、`5d5e309`。
- 本轮已复核上一轮遗留的 P2-2 质量报告草稿，确认改动集中在 `crates/media-worker/src/lib.rs`、`tools/validate-video-deliverables.mjs`、`tools/validate-video-deliverables.test.mjs`、`apps/web/app/lib/html-artifact-manifest.js`、`apps/web/app/components/InsightPanel.js`；已按 P2-2A 跑本地验证矩阵，结果见 `docs/validation/video-ppt-deliverable-smoke.md`。
- P0-3 已修正字幕页映射契约：`crates/media-worker/src/lib.rs` 对没有 transcript/subtitle 映射的样例继续让 `has_subtitle_page_map=false` 且不写 `subtitle_page_map.json`；`tools/validate-video-deliverables.mjs` 现在把 `subtitle_page_map` 作为条件性文件校验。无字幕包可通过 validator；有字幕 map、manifest 声明 map 或文件实际存在时继续严格校验。
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
- `67f9b7a Update DataMax video PPT execution plan`
  追加视频测试、视频号 handoff、公开页面 resolver 和授权录屏兜底的阶段性计划与验证口径。
- `e4558a8 Add WeChat video PPT handoff`
  视频号/登录态来源返回 `wechat_video_login_handoff`，不再误导为已解析视频内容。
- `0d32415 Enhance public page video resolver`
  公开页面 resolver 覆盖 video/source/OpenGraph/Twitter/JSON-LD 字段，并稳定失败原因。
- `852d878 Add main-site video PPT release gate`
  把主站 direct URL 视频/PPT smoke 固化为 `npm run smoke:video-ppt-main-visible` 发布可见性 gate。
- `ca7d722 Add main-site uploaded video PPT smoke`
  把主站上传视频文件入口固化为 `npm run smoke:video-ppt-upload-main`，脚本覆盖上传、文档登记、ingest、特殊触发和 artifact 下载校验；live 主站回执仍需授权后执行。
- `5fd11e6 Add third-party video PPT smoke`
  把第三方视频登记和“提取视频中的 PPT/幻灯片/课件”特殊触发固化为 smoke 脚本；本地语法、help 和 self-test 通过，live 第三方 bearer smoke 待授权凭据。
- P1-3D 本轮新增 `npm run smoke:video-ppt-handoff`
  把视频号/登录态来源的 lightweight handoff 固化为 smoke 脚本；本地语法、help、self-test、Rust handoff 单测和 HTML artifact 渲染测试通过。此前主站 live lightweight smoke 未在 60 秒内看到 `wechat_video_login_handoff` artifact，现已用 early-return 修复当前代码路径；live pass 待部署后复跑。
- P2-1 本轮新增 `docs/operations/video-capture-fallback-runbook.md` 和 `npm run capture:authorized-video`
  把授权录屏兜底固定为 operator runbook 和 isolated capture helper；默认 self-test/dry-run 不打开浏览器或 FFmpeg，live capture 必须显式 `--run-capture --ack-authorized --approval-id ...`。本轮只跑 self-test/dry-run/负向授权门禁，不做真实录屏、不上传、不部署。
- P1-3D 本轮 follow-up 修复
  主站 assistant-run 创建对视频号/登录态 PPT 请求改为 deterministic early return：创建 run 后立即写 `wechat_video_login_handoff` event/artifact、完成 run 并返回，不再等待 ReAct/provider。handler 级测试在 provider 模式和不可达网关下通过，证明不会走模型，同时 `/api/v3/html-artifacts` 可列出 handoff artifact。
- P1-3D 本轮第三方 follow-up 修复
  第三方 `/v1/external/channels/{connection_id}/events` 对视频号/登录态 PPT 请求在创建 run 并记录 external message 后立即返回 `wechat_video_login_handoff` card，`task_status=login_gated_video_source_not_supported`，不触发 action/static/model/provider 分支；幂等重复和 `/assistant-runs/{run_id}/reply` 查询都从事件恢复同一张 card。
- P2-2A 本轮质量报告 contract
  视频/PPT 包新增可选 `slide_quality_report.json`，由 selected slides、slide rectangles、subtitle page map 和 dedupe summary 推导质量分、risk flags、逐页 crop/transcript 风险和人工复核建议；validator 兼容没有质量报告的 legacy 包，但新包若包含该文件必须通过 schema、score、summary、per-slide rows 和 redaction 校验；前端下载动作显示“质量报告”。
- P2-2B 本轮 bright-canvas detector 切片
  media-worker 新增 `bright_canvas_v1` 课件区域检测后备路径，用于低对比背景中嵌入亮色 PPT 画布、但 background contrast/edge projection 不足够稳定的场景；公共 validator 允许 `foreground_component_v1` 和 `bright_canvas_v1` detector mode，避免合法产物被交付校验误拒。
- P2-2B 本轮讲师小窗/外部前景遮挡切片
  新增端到端 fixture：含外部前景条的 PPT 帧会通过 `foreground_component_v1` 写入 `slide_rectangles_manifest`、`selected_slides_manifest`、`slide_quality_report` 和 PPTX `a:srcRect`，证明该类遮挡不退回 full-frame fallback。
- P2-2C 本轮暗场稳定段过滤切片
  自动选页在稳定段 midpoint 为极暗且低亮度变化时，将该稳定段写入 `rejected_clusters`，原因 `dark_low_information_stable_segment`，不再把黑屏/暗场转场作为 PPT 页面候选；已有稳定 PPT 页自动选择不受影响。
- P2-2C 本轮深色主题防误伤切片
  新增 contentful dark-theme slide 回归 fixture：稳定深色背景但包含可见图形/课件内容的页面会进入 `selected_clusters`，不会被 `dark/flat_low_information_stable_segment` guard 写入 `rejected_clusters`。
- P2-2C 本轮短动画转场切片
  新增短动画转场 fixture：两段稳定 PPT 页之间的短暂动画帧写入 `unstable_short_segment` rejected clusters，最终 selected slides 只保留稳定页候选。
- P2-2D 本轮 OCR evidence 对齐切片
  selected slides 现在会把时间窗内 keyframe OCR snippets 写入 `ocr_snippets`，并在 `slide_notes.md` 和 `video_slides.md` 中逐页显示 OCR evidence；`subtitle_page_map.json` 仍保持 transcript-only，不把 OCR 伪装为字幕。
- P2-2D 本轮质量报告 OCR coverage 切片
  `slide_quality_report.json` 增加 `ocr_mapped_count`、`ocr_missing_count`、逐页 `ocr_snippet_count` 和 `ocr_risk`，validator 同步校验这些字段，方便复核 OCR 覆盖情况。
- P2-2F 本轮低对比字迹质量报告切片
  新增低对比文字 slide 端到端 fixture：有效 PNG 帧进入 selected slide 后，`slide_quality_report.json` 会记录 `sharpness_status=measured`、`sharpness_risk=high`、`sharpness_high_count=1` 和 `frame_sharpness_review_required`，且不泄露本地 frame path。

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

### P0-3：字幕页映射交付契约一致性

**状态：已完成本地切片，2026-06-08；live smoke 和 8 服务器部署均未执行。**

**目标：** 把“没有字幕/转写的样例也能交付截图型 PPTX”和“有字幕页映射时必须严格校验”拆清楚，避免 validator、manifest、验证文档和主站质量说明互相矛盾。

完成结果：

- 受控公开视频和主站可见 smoke 都记录 `has_subtitle_page_map=false`，原因是样例没有可对齐字幕/转写证据；这不应阻塞截图型 PPTX、Markdown 和 manifest 交付。
- media-worker 仍只在 `subtitle_page_map.status=mapped` 时写出 `subtitle_page_map.json` 并登记该 artifact。
- `tools/validate-video-deliverables.mjs` 已把 `subtitle_page_map` 从无条件 required 改为条件性文件：实际文件存在、状态位为 true、或任一 manifest 声明该 artifact 时才进入 required 校验。
- media-worker 的 deliverable package、published manifest、published version history 和 durable published version manifest 已改为动态 required kinds；无字幕包不再因为缺 `subtitle_page_map` 被判 incomplete。
- `docs/validation/video-ppt-deliverable-smoke.md` 已追加 P0-3 回执，把 public contract 改成基础必交付、条件性交付 subtitle map、可选质量报告三层。

保留决策：

1. 明确最终契约：`subtitle_page_map.json` 是条件性交付文件。`deliverable_status.has_subtitle_page_map=false` 且 evidence outputs 没有该 artifact 时，包仍可通过 validator，但必须保留 `missing_transcript_alignment` 或等价 warning。
2. 如果 `subtitle_page_map.json` 存在，或 `has_subtitle_page_map=true`，validator 必须继续严格校验 schema、`status=mapped`、page count、assignment rule、transcript segments 和 redaction。
3. published manifest 和 version history 也按同一规则处理：无字幕包不得声明有 subtitle map；有字幕包必须完整覆盖。
4. validation 文档更新为“必交付基础包 + 条件性交付 subtitle map + 可选质量报告”的结构，不再把无字幕包描述成失败。
5. 前端和质量报告说明继续区分 `missing_transcript_alignment`、`ocr_assisted`、`subtitle_aligned`，不得把 OCR snippets 伪装成 transcript/subtitle map。

目标测试：

```bash
node --test tools/validate-video-deliverables.test.mjs
npm run test:video-deliverables
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
git diff --check
```

验收：

- 无字幕样例：PPTX、Markdown、slide notes、rectangle manifest、quality report 和发布 manifest 完整时，validator 通过；`has_subtitle_page_map=false` 被当成质量限制，不当成缺文件失败。
- 有字幕样例：必须生成 `subtitle_page_map.json`，且 validator 对 malformed/unmapped map 继续失败。
- 文档和 smoke 回执统一说明 `subtitle_page_map` 是 transcript/subtitle evidence，不由 OCR-only snippets 填充。
- 该切片只改契约、validator、测试和文档；不做 live smoke、不部署 8 服务器。

完成证据：

- `node --test tools/validate-video-deliverables.test.mjs` 通过，19 tests。
- `npm run test:video-deliverables` 通过，19 tests。
- `CC=clang CXX=clang++ cargo test -p media-worker final_video_deliverables_do_not_require_subtitle_page_map_without_transcript_alignment --lib` 通过。
- `CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib` 通过。
- `CC=clang CXX=clang++ cargo test -p media-worker durable_published_version --lib` 通过，2 tests。
- `CC=clang CXX=clang++ cargo test -p platform-api video_extraction --lib` 通过，3 tests。
- `CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib` 通过，5 tests。
- `cargo fmt --check` 和 `git diff --check` 通过。

### P1-1：公开页面视频资源解析增强

**状态：已完成，2026-06-07。**

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

完成证据：

- resolver fixtures 已覆盖 `<video src>`、`<source src>`、OpenGraph `og:video`、Twitter video、JSON-LD `contentUrl`/`embedUrl`、相对 URL、无视频页面和登录态页面。
- 无视频页面失败原因已统一为 `public_page_no_video_asset`；缺少 source URL 统一为 `direct_video_url_required`；视频号/登录态保持 `login_gated_video_source_not_supported`。
- 候选 URL 仍必须通过公开视频页 URL 校验和直接视频后缀校验，播放器页、私网地址、javascript/data URL 和登录态来源不会进入登记。
- 第三方 API 文档和纯第三方指南已说明公开页面可解析字段与失败状态。

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

**状态：进行中；main-site direct URL release gate 已完成，主站上传视频 smoke 脚本已实现但 live 上传回执待授权，第三方视频登记 special-trigger smoke 已实现但 live 回执待 inbound bearer/授权，视频号/登录态 handoff 的主站和第三方本地确定性路径已补齐。**

**目标：** 确认不同入口的最终产物都能被用户拿到。

覆盖入口：

- 主站上传视频。
- 主站消息直接给视频 URL。
- 第三方上传/登记视频文件后触发。
- 后端 workflow smoke。

执行顺序：

1. **P1-3A direct URL gate 已完成。** 继续保留 `smoke:video-ppt-main-visible` 作为每次发布前的非破坏性回归；优先 reuse 已完成 `assistant_run_id`，只有需要验证新部署时才新建公开样例 run。
2. **P1-3B 主站上传视频 smoke。** `smoke:video-ppt-upload-main` 已实现，用非客户公开视频样例文件走主站上传链路：`/api/v3/local-document-uploads` 保存文件、`/api/v3/documents` 登记视频素材、`/api/v3/documents/{document_id}/ingest` 入队解析，然后在同一 local thread 明确发送“提取这个视频里的 PPT/幻灯片/课件”。目标是证明“上传视频文件”入口和 direct URL 入口一样能生成 assistant-run-bound 下载产物；真实主站运行会写入 smoke 记录，需授权后执行。
3. **P1-3C 第三方视频登记 smoke。** `smoke:external-video-ppt` 已实现。脚本先通过 `/v1/external/channels/{connection_id}/documents/parse` 登记一条公开视频文件或 loopback fixture，再通过 `/v1/external/channels/{connection_id}/events` 发送带 `dataset_external_ids` 和 `available_document_external_ids` 的消息，触发 `extract_video_ppt_transcript`。本地 self-test 已覆盖第三方回复 surface 契约；live 主站运行必须使用 operator 管理的 inbound bearer，且需要授权后执行。
4. **P1-3D unsupported 展示复核。** `smoke:video-ppt-handoff` 已实现。脚本用视频号/登录态链接跑主站和第三方的 lightweight smoke，只验证 `wechat_video_login_handoff` artifact 或等价 unsupported-source 卡片，不抓视频、不抽帧、不生成 PPT，防止产品文案回退为“已看过视频”。本地 self-test、Rust handoff 单测和 HTML artifact 测试已通过；主站 handoff 已改为 deterministic early return，handler 级测试证明 provider/ReAct 不会被调用且 artifact list 可见；第三方 `/events` 已补 deterministic handoff card，endpoint 测试证明首次投递、幂等重复和 reply 查询都不会触发 provider。live pass 待部署后复跑。

验收：

- assistant-run-bound 的产物链接通过 `/api/v3/html-artifacts/{artifact_id}/files/{index}` 或等价公开 surface 暴露。
- 没有 assistant_run_id 的后端 smoke 要标注“仅后端证据，不代表主站聊天可见”。
- 下载链接权限正确：同一 run/thread 可访问，跨 scope 不泄露。
- 上传和第三方 smoke 都必须记录：输入来源、document id 或 external document id、触发语、assistant run id、artifact id、`deliverable_status.state`、下载 file kinds、PPTX slide count、Markdown slide count、warning 解读。
- 所有 smoke 只能用非客户、非登录态、可公开访问样例；生成下载保留在 `target/`，不提交。

已完成证据：

- `npm run smoke:video-ppt-main-visible` 已纳入 package scripts 和 `scripts/README.md`，作为 main-site assistant-run-bound video/PPT release gate。
- reuse-mode 已复核主站 `assistant_run_id=46f57e74-85f9-4088-bad0-99f1ae0a6fea`：`deliverableState=final_pptx_ready`，PPTX/Markdown/manifest 6 类关键产物可通过 HTML artifact file API 下载。
- release gate 验证 PPTX OOXML 必需 entry、PPTX slide count 和 Markdown slide heading count 一致；当前样例均为 30。
- `npm run smoke:video-ppt-upload-main` 已纳入 package scripts 和 `scripts/README.md`；脚本语法、help、离线 self-test、最小 PPTX/Markdown 下载校验、upload classifier、HTML artifact manifest 和 video deliverable validator 均已通过本地验证。
- `npm run smoke:external-video-ppt` 已纳入 package scripts 和 `scripts/README.md`；脚本语法、help 和 `--self-test` 均已通过本地验证，self-test 不访问网络，并覆盖最小 PPTX/Markdown 下载校验。
- `npm run smoke:video-ppt-handoff` 已纳入 package scripts 和 `scripts/README.md`；脚本语法、help、`--self-test`、`cargo test -p platform-api wechat_video_login_handoff --lib`、HTML artifact manifest 测试均已通过。
- P1-3D 主站 live lightweight smoke 已试跑一次：没有抓视频、没有抽帧、没有下载产物，但 60 秒内未发现 `wechat_video_login_handoff` artifact；当前代码已修复为 early return，本地 handler 级测试覆盖 artifact list，可在下次部署后复跑 live pass。
- P1-3D 第三方 handoff 本地 endpoint 测试已补齐：`generic_chat_wechat_video_ppt_handoff_short_circuits_provider` 在 provider 模式和不可达 gateway 下通过，证明第三方 `/events` 首次投递、幂等重复和 `/assistant-runs/{run_id}/reply` 查询都返回 card，且不产生 provider/ReAct/model reply 事件。

仍待补充：

- `scripts/smoke/video-ppt-upload-main.mjs` 已覆盖主站上传视频文件入口；仍需授权后跑主站 live/controlled 回执。
- `scripts/smoke/external-video-ppt.mjs` 已覆盖第三方登记视频素材后用“提取视频中的 PPT”特殊触发；仍需 inbound bearer 和授权后跑 live/controlled 回执。
- `scripts/smoke/video-ppt-handoff.mjs` 已覆盖视频号链接 handoff 展示复核；主站和第三方代码路径均已补 deterministic handoff，本地测试已通过，仍需部署后拿到 main/external live pass 回执，第三方 live 仍需 inbound bearer。

### P2-1：授权录屏兜底 MVP 设计

**状态：runbook + isolated script 已实现；live 授权样例未执行；不默认接入 8 服务器。**

**目标：** 在不破坏主线能力、不绕过平台权限的前提下，支持“拿不到直链但 operator 已批准可播放”的视频源。

推荐方案：

- **优先路径：jump-host/Mac 录制。** 适合需要人工打开微信、扫码或客户端播放的场景，录完生成普通 `.mp4`，再上传到 DataMax。
- **可选路径：8 服务器内部隔离录制。** 只适合服务器能合法访问并播放的网页，必须显式开关启用。

阶段拆分：

1. **P2-1A 方案冻结已完成。** 已输出 `docs/operations/video-capture-fallback-runbook.md`，明确授权记录、输入限制、录制时长、文件大小、保留期、日志脱敏和清理策略。该阶段不部署。
2. **P2-1B 本地/受控 host MVP 已完成。** 已新增 `scripts/capture-authorized-video.mjs` 和 `npm run capture:authorized-video`。脚本默认 self-test/dry-run，不打开浏览器或 FFmpeg；live capture 必须显式 `--run-capture --ack-authorized --approval-id ...`，使用临时浏览器 profile 和 FFmpeg/system capture，报告只记录脱敏 source summary。
3. **P2-1C 授权样例 smoke 待执行。** operator 提供一个已授权、可播放但无直链的视频页面，先人工确认边界，再录制 30-60 秒并抽取 PPT。回执要区分播放失败、录制失败、视频无 PPT、抽帧失败、PPT 质量不足。
4. **P2-1D 8 服务器评审。** 只有用户明确批准后才评估 8 服务器内部录制；默认不启用、不部署、不重启服务。

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
- 8 服务器内部录制未获明确批准前，不能出现在默认生产路径、不能在主站自动触发、不能扩大第三方入口权限。

### P2-2：视频/PPT 质量增强

**状态：P2-2A 已完成本地验证；P2-2B bright-canvas detector 本地切片已完成，讲师小窗/外部前景遮挡端到端 crop fixture 已完成；P2-2C 暗色、亮色和纯色低信息稳定段过滤、深色主题内容页防误伤和短动画转场 fixture 已完成；P2-2D OCR evidence 和质量报告 OCR coverage 本地切片已完成；P2-2F 清晰度/可读性质量信号本地切片已完成，低对比字迹端到端质量报告 fixture 已完成；P2-2E 三样例质量矩阵 self-test scaffold 和本地 `generated_artifacts/` 输入适配已完成；P2-2B/P2-2C 更广泛质量处理、P2-2D 真实字幕/OCR 样例复核、P2-2E 真实 public/customer 样例回执待后续执行。**

**目标：** 提高截图型 PPT 的可读性，减少人工复核成本。

阶段拆分：

1. **P2-2A 质量报告 contract。**
   输出可选 `slide_quality_report.json`，汇总页数、质量分、risk flags、crop fallback、subtitle alignment、dedupe 和人工复核建议。该文件先作为 optional artifact，不阻断旧包校验；如果后续验证稳定，再评审是否升级为 required deliverable。
2. **P2-2B 课件区域检测增强。**
   在现有 `slide_rectangles_manifest` 基础上减少 `full_frame_rectangle_fallback`，优先处理公开课视频中播放器边框、讲师小窗、黑边、转场帧和非课件画面误入选的问题。
3. **P2-2C 稳定区间和重复页控制。**
   强化候选帧选择，减少转场帧、遮挡帧、重复页和相似页面误保留；输出 dedupe 原因和可人工复核的 removed candidates 摘要。
4. **P2-2D OCR/字幕/转写对齐。**
   在有字幕或转写的样例中补齐 `subtitle_page_map`，把逐页讲稿备注从 metadata-only 提升为可读讲稿；没有字幕时必须保留明确 `missing_transcript_alignment` warning。
5. **P2-2F 清晰度/可读性质量信号。**
   已完成本地切片：在 `slide_quality_report.json` 中增加可选 sharpness/readability 字段，用本地 frame 解码结果给出 `sharpness_score`、`sharpness_risk` 和 summary counts；legacy 包兼容，报告不写原始 frame path。
6. **P2-2E 多样例复核与交付判定。**
   用合成 PPT 视频、公开视频课程、真实客户上传视频三类样例分别给出可交付、需人工复核、不可交付结论，避免只用一个公开样例证明全部质量。

质量报告 contract 要求：

- `schema=v3.video_ppt_slide_quality_report.v1`。
- `status` 只能表达 `waiting_for_selection`、`review_required`、`review_ready` 等可解释状态，不得把 warning 包装成成功。
- `quality_score` 取值 0-100，计算依据要能从 manifest 推导，不能依赖 provider 文本。
- `risk_flags` 至少覆盖 `full_frame_rectangle_fallback`、`missing_transcript_alignment`、`selected_slide_duplicates_removed`、`manual_review_required`。
- 每页记录 slide number、candidate index、source frame 文件名摘要、timestamp label、rectangle mode/status、crop risk、subtitle alignment、OCR risk、可选 sharpness/readability risk、review_required 和 page quality score。
- 不记录原始视频 URL、本地 frame 绝对路径、cookie、token、provider payload 或私有 object path。

候选方向：

- 更强的课件区域检测，减少 `full_frame_rectangle_fallback`。
- 画面稳定区间识别，避免转场帧和讲师遮挡帧。
- OCR/字幕/转写对齐，补齐 `subtitle_page_map`。
- 多语言 slide notes 与逐页讲稿备注。
- 质量评分：清晰度、重复度、遮挡、裁剪风险、字幕缺失。
- 清晰度/可读性信号应从图像本身计算，不依赖 provider 文本；低清晰度只提示人工复核，不直接阻断交付。

验收：

- 用 3 个样例集复核：合成 PPT 视频、公开视频课程、真实客户上传视频。
- 每个样例输出 PPTX、Markdown、质量报告和人工复核结论。
- legacy deliverables 没有 `slide_quality_report.json` 时仍能通过现有 validator；新包如果包含该文件，validator 必须校验 schema、score、risk summary、per-slide rows 和 redaction。
- 前端下载列表能显示“质量报告”，但不得让用户误解为原生可编辑 PPT 已重建。
- 验证记录写入 `docs/validation/video-ppt-deliverable-smoke.md`，生成 PPTX、视频、帧图和 smoke 输出仍保留在 `target/`，不提交。

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
npm --prefix apps/web run build
node --test apps/web/app/lib/html-artifact-manifest.test.mjs
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

### 5.3.1 P0-3 字幕页映射契约一致性执行步骤

本节已按 2026-06-08 P0-3 本地切片完成，后续作为契约复现 runbook 使用。它不是质量增强，也不是 live smoke；它只收口 public deliverable contract。

目标文件优先级：

- `tools/validate-video-deliverables.mjs`
- `tools/validate-video-deliverables.test.mjs`
- `crates/media-worker/src/lib.rs`
- `docs/validation/video-ppt-deliverable-smoke.md`
- `docs/plans/datamax-active-execution-plan.md`

开发步骤：

1. 先增加一个无字幕 fixture：删除 `subtitle_page_map.json`，从 final/published/extraction manifest 中移除 `subtitle_page_map` artifact，设置 `deliverable_status.has_subtitle_page_map=false`，保留 `missing_transcript_alignment` warning。
2. 调整 validator，把 `subtitle_page_map` 从无条件 `REQUIRED_FILES` 中移出，改为条件性文件：
   - manifest 状态为 `has_subtitle_page_map=false` 且没有对应 artifact 时允许通过；
   - 文件存在、状态为 true、或 manifest 声明该 artifact 时必须严格校验；
   - manifest 声明不一致时仍失败。
3. 保留 malformed subtitle map 的负向测试：`status=unmapped`、page count 不一致、没有 transcript segments、未脱敏路径都必须失败。
4. 检查 media-worker 的 required kind 和 published version history 逻辑，确保无字幕包不会因为缺 map 被标记 `package_incomplete`，有字幕包仍完整登记。
5. 更新 validation 文档，把 public contract 改成：
   - 基础必交付：PPTX、`video_slides.md`、`slide_notes.md`、`slide_rectangles_manifest.json`、`extraction_artifacts_manifest.json`、`final_deliverables_manifest.json`、`published_deliverable_manifest.json`、`published_version_history.json`；
   - 条件必交付：`subtitle_page_map.json`，仅 transcript/subtitle evidence 可 mapped 时出现；
   - 可选辅助交付：`slide_quality_report.json`。

验证命令：

```bash
node --test tools/validate-video-deliverables.test.mjs
npm run test:video-deliverables
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
git diff --check
```

验收记录：

- 在 `docs/validation/video-ppt-deliverable-smoke.md` 追加 P0-3 回执，记录无字幕包通过、malformed subtitle map 仍失败、有字幕路径仍严格的证据。
- 只提交代码/测试/文档；不提交生成 artifacts，不跑主站 live smoke，不部署 8 服务器。

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
- `scripts/smoke/video-ppt-main-visible.mjs`
- 新增或扩展的 upload / third-party video smoke 脚本

#### 5.6.1 direct URL release gate

当前已完成，后续作为回归 gate 使用：

```bash
npm run smoke:video-ppt-main-visible -- \
  --base-url https://v3.elepcloud.com \
  --assistant-run-id 46f57e74-85f9-4088-bad0-99f1ae0a6fea \
  --local-thread-id video-ppt-main-visible-20260607-01 \
  --timeout-ms 60000 \
  --output-dir target/video-ppt-main-visible-release-gate-smoke
```

通过标准：

- `artifactOk=true`。
- `deliverableState=final_pptx_ready`。
- 下载 file kinds 至少包含 `pptx`、`video_slides_markdown`、`final_deliverables_manifest`、`published_deliverable_manifest`、`published_version_history`、`extraction_artifacts_manifest`。
- PPTX 有 OOXML central directory 必需 entry；PPTX slide count 与 Markdown `### Slide` 数一致。

#### 5.6.2 主站上传视频 smoke

实现方式：

1. 已新增 `scripts/smoke/video-ppt-upload-main.mjs`，保持 direct URL gate 不变，避免 `video-ppt-main-visible.mjs` 变复杂。
2. 脚本从公开样例 URL 下载 `.mp4` 到 `target/video-ppt-upload-main-smoke/fixture/`，或接收 `--fixture-file` 指向本地样例；不提交视频文件。
3. POST `FormData(files=...)` 到 `/api/v3/local-document-uploads`，记录返回的 `object_key`、`content_type`、size，但报告里只写脱敏摘要。
4. POST `/api/v3/documents`，body 按主站 `registerAndIngestUploadedFile` 契约提供 `dataset_id`、`title`、`object_key`、`content_type`、`metadata.initial_classification.media_kind=video`、`metadata.parse_state.stage=queued`。
5. POST `/api/v3/documents/{document_id}/ingest`，等待解析/登记 workflow 完成或进入可触发状态。
6. 在同一 local thread 发送“请提取刚上传视频里的 PPT/幻灯片/课件”，并确保 action 进入 `extract_video_ppt_transcript`，不是只停在普通 `parse_video_media`。
7. 复用 direct URL gate 的 artifact 下载与 PPTX/Markdown validator。

建议命令形态：

```bash
npm run smoke:video-ppt-upload-main -- \
  --base-url https://v3.elepcloud.com \
  --fixture-url https://v3.elepcloud.com/generated-artifacts/samples/react-in-5-minutes.mp4 \
  --local-thread-id video-ppt-upload-main-20260607-01 \
  --timeout-ms 300000 \
  --output-dir target/video-ppt-upload-main-smoke
```

验收记录：

- `local_thread_id`、上传返回摘要、`document_id`、ingest workflow id、assistant run id、artifact id。
- `deliverable_status.state=final_pptx_ready`。
- PPTX/Markdown/manifest 下载通过主站 HTML artifact file API。
- 如果上传可登记但特殊触发失败，标记为 trigger 编排缺口；如果后端完成但主站无下载，标记为 artifact visibility 缺口。

#### 5.6.3 第三方视频登记 special-trigger smoke

实现方式：

1. `scripts/smoke/external-video-ppt.mjs` 已新增，保留 `--allow-missing-bearer` 仅用于本地/loopback 自测；真实主站必须提供 operator 管理的 inbound bearer，且不在日志打印。
2. 通过 loopback fixture server 或公开视频 URL 构造视频文件输入，`content_type` 使用 `video/mp4`、`video/webm` 等已支持类型。
3. POST `/v1/external/channels/{connection_id}/documents/parse`，body 至少包含 `source_id`、`dataset_external_id`、`document_external_id`、`revision_external_id`、`title`、`content_type`、`content_url`、`idempotency_key`；loopback 自测可用 `allow_http_loopback=true`。
4. GET `/v1/external/channels/{connection_id}/documents/{document_external_id}/parse-detail` 轮询，确认视频素材已登记或进入可触发状态；如果普通 parse 对视频没有 chunk/evidence，也不能因此误判 PPT 抽取已完成。
5. POST `/v1/external/channels/{connection_id}/events`，同一 `conversation_external_id` 带 `dataset_external_ids` 或 `available_document_external_ids`，文本明确写“提取这个视频里的 PPT/幻灯片/课件”。
6. GET `/v1/external/channels/{connection_id}/assistant-runs/{run_id}/reply` 轮询，确认返回的视频抽取卡片、task status 或 follow-up artifact。
7. 如第三方 surface 不能直接下载 HTML artifact 文件，至少要记录等价的公开产物 surface 和权限边界；否则标记为第三方发布可见性缺口。

建议命令形态：

```bash
npm run smoke:external-video-ppt -- \
  --base-url https://v3.elepcloud.com \
  --connection-id generic-chat-main \
  --source-id third-party-source-main \
  --fixture-url https://v3.elepcloud.com/generated-artifacts/samples/react-in-5-minutes.mp4 \
  --timeout-ms 300000 \
  --output-dir target/external-video-ppt-smoke
```

验收记录：

- `connection_id`、`source_id`、`dataset_external_id`、`document_external_id`、`conversation_external_id`、assistant run id。
- 登记视频素材成功和特殊触发成功要分别记录，不能混成一条“解析成功”。
- 输出必须是 `final_pptx_ready` 或明确失败分流；不能把普通文本回复当成 PPT 交付。

#### 5.6.4 视频号/登录态 handoff smoke

执行步骤：

1. 使用 `npm run smoke:video-ppt-handoff -- --self-test` 先跑离线 surface 契约，不访问网络。
2. 主站 live 用 `--mode main` 发送视频号链接和“提取 PPT”触发语，只检查 `wechat_video_login_handoff` artifact。
3. 第三方 live 用 `--mode external` 或 `--mode both` 发送同样语义的 `/events`，需要 operator 管理的 inbound bearer。
4. 只检查 `wechat_video_login_handoff` 或同等卡片，不下载视频、不抽帧、不生成 PPT。
5. 确认失败原因稳定为 `login_gated_video_source_not_supported`，三选项为上传视频文件、提供匿名直连视频 URL、申请授权录屏处理。

建议命令形态：

```bash
npm run smoke:video-ppt-handoff -- --self-test

npm run smoke:video-ppt-handoff -- \
  --mode main \
  --base-url https://v3.elepcloud.com \
  --local-thread-id video-ppt-handoff-20260607-01 \
  --timeout-ms 60000 \
  --output-dir target/video-ppt-handoff-main-smoke
```

验收：

- 回复不得声称 DataMax 已看过视频内容、已完成 OCR、已生成 PPT。
- 回复不得要求 cookie、扫码截图、账号密码或浏览器登录态。
- 如主站未返回 handoff artifact，先确认是否已部署 early-return 修复；不要改为抓取视频号页面。

#### 5.6.5 P1-3 测试命令

目标测试：

```bash
node --check scripts/smoke/video-ppt-main-visible.mjs
node --test apps/web/app/lib/html-artifact-manifest.test.mjs
node --check apps/web/app/lib/html-artifact-manifest.js
npm --prefix apps/web run build
npm run test:video-deliverables
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
git diff --check
```

如果新增 upload / third-party smoke 脚本：

```bash
node --check scripts/smoke/video-ppt-upload-main.mjs
node --check scripts/smoke/external-video-ppt.mjs
npm run smoke:external-video-ppt -- --help
npm run smoke:external-video-ppt -- --self-test
node --check scripts/smoke/video-ppt-handoff.mjs
npm run smoke:video-ppt-handoff -- --help
npm run smoke:video-ppt-handoff -- --self-test
```

验收：

- 主站用户可直接拿到 PPTX 和 Markdown。
- 主站上传、直连 URL、第三方登记三类入口的证据在 `docs/validation/video-ppt-deliverable-smoke.md` 中分开记录。
- 后端 workflow smoke、主站可见 smoke、第三方 smoke 在文档中明确区分。

### 5.7 P2-1 授权录屏 MVP 执行步骤

推荐先做 isolated script，不直接塞进核心 worker：

- `scripts/capture-authorized-video.mjs`
- `docs/operations/video-capture-fallback-runbook.md`
- `docs/validation/video-ppt-deliverable-smoke.md`

#### 5.7.1 文档和授权记录

先新增 runbook，定义每次录屏 job 必填字段：

- `approval_id`：operator 批准编号。
- `approved_by`：批准人或审批来源。
- `source_host_redacted`：仅记录 host 或脱敏 URL 摘要。
- `purpose`：为什么需要录屏，而不是直连 URL/上传文件。
- `max_duration_seconds`：默认 60 秒，超过需要单独批准。
- `capture_audio_allowed`：默认 false。
- `retention_days`：默认 7 天以内；客户样例可更短。
- `cleanup_policy`：成功/失败后如何清理 profile、录屏文件和中间产物。
- `handoff_to_datamax`：录完后是上传主站、第三方登记，还是只做离线验证。

#### 5.7.2 第一阶段本地/受控 host MVP

1. 用 Playwright 打开 operator 指定 URL 或本地公开测试页。
2. 使用隔离 browser profile，不加载用户日常浏览器 profile，不复用 DataMax 服务 cookie。
3. 用 FFmpeg 从虚拟显示或系统采集设备录制固定时长；macOS 可评估 `avfoundation`，Linux 可评估 `x11grab`，Windows 可评估 `gdigrab`。
4. 输出 `.mp4` 到 `target/authorized-capture-smoke/<run_id>/` 或 operator 指定临时目录。
5. 将 `.mp4` 作为普通视频素材进入现有抽取流程，复用 P1-3 上传 smoke 或第三方登记 smoke。
6. 清理临时 profile 和录屏文件，或按 operator 配置保留短期文件。
7. 回执只记录 redacted host、文件大小、时长、hash 前缀、抽取状态和质量结论。

第二阶段再评审是否接入 8 服务器：

- 默认 `CAPTURE_FALLBACK_ENABLED=false`。
- 只允许并发 1。
- 每次 job 必须有 explicit approval id。
- 最大录制时长和最大文件大小硬限制。
- 日志脱敏；不存 cookie/storage/HAR。

验证命令建议：

```bash
node --check scripts/capture-authorized-video.mjs
npm run capture:authorized-video -- --help
npm run capture:authorized-video -- --self-test
npm run capture:authorized-video -- \
  --dry-run \
  --ack-authorized \
  --approval-id DRYRUN-20260607-003 \
  --approved-by operator-dryrun \
  --url https://example.com/authorized-video-page \
  --purpose dry-run-authorized-capture-plan \
  --duration-seconds 30 \
  --handoff upload-main
node --check scripts/smoke/video-ppt-upload-main.mjs
npm run test:video-deliverables
CC=clang CXX=clang++ cargo test -p media-worker frame_extraction --lib
git diff --check
```

上线门槛：

- 先有公开视频录屏自测。
- 再有 operator 授权样例。
- 再评审 8 服务器部署窗口。
- 未批准前不部署、不启用。

失败分流：

- 播放失败：页面无法打开、需要登录、无法确认授权或播放控件不可用。
- 录制失败：FFmpeg/虚拟显示/设备权限失败，未产出有效 `.mp4`。
- 输入不适用：视频内容没有稳定 PPT/幻灯片/课件画面。
- 抽取失败：录制文件可用，但 `VideoExtraction` 未生成 `final_pptx_ready`。
- 质量不足：PPTX 可生成，但关键页缺失、重复严重、裁剪不可读或字幕映射缺失影响交付。

### 5.8 P2-2 视频/PPT 质量增强执行步骤

P2-2 必须先处理当前工作区状态，再继续开发：

1. 复核上一轮遗留草稿：

```bash
git status --short --branch
git diff -- crates/media-worker/src/lib.rs tools/validate-video-deliverables.mjs tools/validate-video-deliverables.test.mjs apps/web/app/lib/html-artifact-manifest.js apps/web/app/components/InsightPanel.js
```

2. 确认这批改动只属于 P2-2A 质量报告 contract；如果发现混入 unrelated refactor，先拆开，不带入质量报告提交。
3. 先跑最小验证；通过后再更新 `docs/validation/video-ppt-deliverable-smoke.md`，最后才考虑提交 GitHub。

#### 5.8.1 P2-2A 质量报告 contract

目标文件优先级：

- `crates/media-worker/src/lib.rs`
- `tools/validate-video-deliverables.mjs`
- `tools/validate-video-deliverables.test.mjs`
- `apps/web/app/lib/html-artifact-manifest.js`
- `apps/web/app/components/InsightPanel.js`
- `docs/validation/video-ppt-deliverable-smoke.md`

开发步骤：

1. 在 media-worker 生成 `slide_quality_report.json`，来源只使用已有 selected slides、rectangle manifest、subtitle/page map、dedupe summary 和 warning，不引入 provider 原文依赖。
2. 在 final/extraction manifest 中登记 `slide_quality_report` kind，download label 显示为“质量报告”。
3. validator 允许 legacy package 缺少该文件；如果文件存在，必须校验 schema、status、score、summary counts、risk flags、per-slide rows 和 redaction。
4. 前端下载优先级放在 manifest 之后、slide notes 之前；不影响 PPTX/Markdown 主下载动作。
5. 文档记录该报告是质量复核辅助，不是证明视频已被 OCR/转写完整理解。

目标测试：

```bash
cargo fmt --check
CC=clang CXX=clang++ cargo test -p media-worker selected_slides --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
CC=clang CXX=clang++ cargo test -p media-worker slide_rectangle --lib
CC=clang CXX=clang++ cargo test -p platform-api video_extraction --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
npm run test:video-deliverables
node --test apps/web/app/lib/html-artifact-manifest.test.mjs
node --check apps/web/app/components/InsightPanel.js
node --check apps/web/app/lib/html-artifact-manifest.js
npm --prefix apps/web run build
git diff --check
```

验收：

- 新生成包包含 `slide_quality_report.json`，legacy 包缺少该文件也通过 validator。
- 质量报告不包含本地路径、原始 source URL、token、cookie、provider payload 或私有 object path。
- risk flags 能解释公开视频样例中已经出现的 `full_frame_rectangle_fallback`、`missing_transcript_alignment`、`selected_slide_duplicates_removed`。
- 前端 artifact manifest 和 InsightPanel 下载动作能显示该文件。
- 该切片不做 live smoke、不抓视频、不部署 8 服务器；只提交本地验证通过的代码/文档。

#### 5.8.2 P2-2B/P2-2C 裁剪、稳定区间和重复页增强

开发顺序：

1. 从已知公开视频样例导出候选帧 contact sheet，只记录 redacted file names 和 frame index，不提交图片。
2. 增强课件区域检测前，先固定 regression fixture，覆盖黑边、讲师小窗、播放器边框、非课件画面和转场帧；当前已覆盖暗色/亮色/纯色低信息稳定转场、短动画转场、深色主题内容页不被低信息 guard 误拒，以及讲师小窗/外部前景遮挡的 `foreground_component_v1` 端到端 crop。
3. 对 `slide_rectangles_manifest` 增加 detector mode/status 解释，所有 detector crop 仍保持 `review_required=true`。
4. 对候选帧选择增加稳定区间约束，避免短暂转场、动画过渡和讲师遮挡帧进入最终 PPTX。
5. 对 dedupe 输出增加 removed candidate 摘要，方便人工判断是否误删。

目标测试：

```bash
CC=clang CXX=clang++ cargo test -p media-worker slide_rectangle --lib
CC=clang CXX=clang++ cargo test -p media-worker selected_slides --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
npm run test:video-deliverables
git diff --check
```

验收：

- 新样例中 `full_frame_rectangle_fallback` 数量下降，或质量报告能明确列出仍需人工复核的页。
- PPTX slide count、Markdown slide count、selected slides count 保持一致。
- 不因裁剪增强损坏 DrawingML `a:srcRect` 或旧 full-frame fallback 渲染。

已完成 follow-up：

- 自动选页的 `ordinary_video_guard` 已扩展为拒绝暗色、亮色或纯色低信息稳定段。
- 新增 `bright_low_information_stable_segment` rejection reason，稳定白屏/亮色空白转场不再进入 selected slides。
- 新增 `flat_low_information_stable_segment` rejection reason，稳定灰屏/loading 纯色段不再进入 selected slides。
- 新增 contentful dark-theme slide 正向 fixture，稳定深色课件页有可见图形/内容时会被自动选择，`rejected_clusters` 保持为空。
- 新增讲师小窗/外部前景遮挡端到端 fixture，`foreground_component_v1` crop 会贯穿 rectangle manifest、selected slides、quality report 和 PPTX `a:srcRect`。
- 新增短动画转场 fixture，非低信息的短暂动画帧会写入 `unstable_short_segment`，最终 selected slides 只保留稳定 PPT 页。
- `auto_selects` 回归现在覆盖稳定 PPT 页、暗色低信息稳定段、亮色低信息稳定段、纯色低信息稳定段、深色主题内容页和短动画转场六类场景。

#### 5.8.3 P2-2D 字幕/OCR/转写对齐

开发顺序：

1. 先选择有字幕或可公开转写的样例，不能用无字幕样例证明 `subtitle_page_map`。
2. 优先复用现有 transcript/subtitle artifacts；如新增 OCR，只输出 redacted text summary，不记录原始视频私有路径。
3. `slide_notes.md` 每页至少区分 metadata-only、subtitle-aligned、transcript-aligned、ocr-assisted 四类来源。
4. 没有字幕/转写时继续保留 `missing_transcript_alignment`，不得让报告伪装为已对齐。

目标测试：

```bash
npm run test:video-deliverables
CC=clang CXX=clang++ cargo test -p media-worker transcript --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
git diff --check
```

验收：

- 有字幕样例生成 `subtitle_page_map.json` 且质量报告显示 subtitle mapped count。
- 无字幕样例仍显示 missing alignment warning，不误报为质量通过。
- slide notes 的每页来源可解释，且不泄露 provider payload。

#### 5.8.4 P2-2F 清晰度/可读性质量信号

本节已按 2026-06-08 P2-2F 本地切片完成，后续作为复现 runbook 使用。目标是让 reviewer 能更快发现“抽到了正确页但截图不够清晰/字太糊”的情况，不改变视频来源策略，也不把低分直接升级为交付失败。

目标文件优先级：

- `crates/media-worker/src/lib.rs`
- `tools/validate-video-deliverables.mjs`
- `tools/validate-video-deliverables.test.mjs`
- `docs/validation/video-ppt-deliverable-smoke.md`
- `docs/plans/datamax-active-execution-plan.md`

开发步骤：

1. 在 selected slides 生成质量报告前，读取内存里已经存在的 selected candidate `frame_path`，只用于本地解码和计算，不写进 public report。
2. 增加一个轻量图像指标：解码为 luma，采样边缘/梯度或高分位梯度，输出 0-100 的 `sharpness_score`；白屏或低信息帧应得到高风险或 unavailable。
3. 每页新增可选字段：
   - `sharpness_status`：`measured` 或 `unavailable`；
   - `sharpness_score`：0-100，unavailable 时为 `null`；
   - `sharpness_risk`：`low`、`medium`、`high` 或 `unknown`。
4. summary 增加可选计数：`sharpness_low_count`、`sharpness_medium_count`、`sharpness_high_count`、`sharpness_unknown_count`。
5. risk flags 增加 `frame_sharpness_review_required`，只在 high/unknown 明显存在时提示人工复核；不要因此移除 `final_pptx_ready`。
6. validator 对旧包保持兼容；新包如果出现 sharpness 字段，必须校验范围、枚举、summary/row 一致性和 redaction。
7. 增加 Rust fixture：清晰文字/线条型 slide 应低风险，高度均匀或低信息帧应高风险或 unknown；低对比字迹 slide 应在端到端质量报告里进入 high-risk review。
8. 追加 validation 回执，说明该信号是质量复核辅助，不代表 OCR/字幕对齐完成。

目标测试：

```bash
cargo fmt
cargo fmt --check
node --test tools/validate-video-deliverables.test.mjs
npm run test:video-deliverables
CC=clang CXX=clang++ cargo test -p media-worker selected_slides --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
CC=clang CXX=clang++ cargo test -p media-worker slide_rectangle --lib
CC=clang CXX=clang++ cargo test -p platform-api video_extraction --lib
CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib
git diff --check
```

验收：

- 新生成的 `slide_quality_report.json` 可以显示逐页 sharpness/readability 风险和 summary counts。
- 低对比字迹 slide 会在质量报告中显示 `sharpness_status=measured`、`sharpness_risk=high` 和 `frame_sharpness_review_required`，但不阻断截图型 PPTX 交付。
- report、manifest、Markdown、前端下载 surface 不包含本地 frame 绝对路径、原始 source URL、cookie、token、provider payload 或私有 object path。
- legacy package 没有 sharpness 字段仍能通过 validator。
- 当前截图型 PPTX 交付主链路不被低清晰度直接阻断；低清晰度通过 quality report 和 warning 引导人工复核。
- 不跑 live smoke，不下载新视频，不上传文件，不部署 8 服务器。

完成证据：

- `cargo fmt` 和 `cargo fmt --check` 通过。
- `node --test tools/validate-video-deliverables.test.mjs` 通过，19 tests。
- `npm run test:video-deliverables` 通过，19 tests。
- `CC=clang CXX=clang++ cargo test -p media-worker measures_slide_frame_sharpness_for_quality_report --lib` 通过。
- `CC=clang CXX=clang++ cargo test -p media-worker flags_low_contrast_slide_text_in_quality_report --lib` 通过。
- `CC=clang CXX=clang++ cargo test -p media-worker selected_slides --lib` 通过。
- `CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib` 通过。
- `CC=clang CXX=clang++ cargo test -p media-worker slide_rectangle --lib` 通过，5 tests。
- `CC=clang CXX=clang++ cargo test -p platform-api video_extraction --lib` 通过，3 tests。
- `CC=clang CXX=clang++ cargo test -p platform-api video_ppt --lib` 通过，5 tests。
- `git diff --check` 通过。

#### 5.8.5 P2-2E 三类样例复核

本节的质量矩阵 self-test scaffold 已完成，后续作为 P2-2E 真实样例复核的入口。self-test 只固定三类样例的判断结构，不声称完整 P2-2E 已验收。

样例集：

- 合成 PPT 视频：页数和时间戳可控，用于验证 selected count、dedupe、crop 和报告 schema。
- 公开视频课程：用于验证真实播放环境中的黑边、讲师遮挡、转场、无字幕或弱字幕。
- 真实客户上传视频：必须等用户授权，产物和回执只记录脱敏摘要，不提交客户媒体。

复核记录字段：

- input type、触发方式、workflow id、assistant run id 或第三方 run id。
- frame_count、selected_count、PPTX slide count、Markdown slide count。
- `has_slide_quality_report`、quality score、risk flags、人工复核结论。
- warnings 解释和下一步分流：可交付、需人工复核、不可交付。

验收：

- 三类样例至少各有一条回执，写入 `docs/validation/video-ppt-deliverable-smoke.md`。
- 对每个失败都能归因到 source access、video has no PPT、frame extraction、crop quality、subtitle alignment、artifact visibility 或权限问题。
- 生成产物不进 Git，GitHub 只同步代码、脚本、文档和脱敏回执。

已完成 self-test scaffold：

```bash
node --check scripts/smoke/video-ppt-quality-matrix.mjs
npm run smoke:video-ppt-quality-matrix -- --help
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty
```

self-test 产出 `v3.video_ppt_quality_matrix_smoke.v1` 报告，固定三类样例：

- `synthetic_ppt_playback`：本地合成类可判定为 `deliverable`。
- `public_course_video`：S1 已找到匿名可下载 public slides candidate，S2A 复跑已通过 deliverables validator，quality matrix 判为 `needs_manual_review`。
- `customer_authorized_video`：保持 `pending_authorization`，等待客户/operator 授权。

self-test 验收口径：

- `matrix_complete=false`，不得把本地 self-test 伪装成完整三样例验收。
- `live_smoke_run=false`、`production_write_allowed=false`。
- `source_urls_included=false`、`object_paths_included=false`、`credentials_included=false`、`provider_payloads_included=false`。
- 报告写入 `target/video-ppt-quality-matrix-smoke/`，不提交生成报告。

已完成本地 deliverables 输入适配 P2-2E-1 和 public-course 分类入口：

实现结果：

1. `scripts/smoke/video-ppt-quality-matrix.mjs` 已新增 `--synthetic-deliverables <path>`、`--public-course-deliverables <path>`、环境变量兜底 `VIDEO_PPT_QUALITY_MATRIX_SYNTHETIC_DELIVERABLES` 和 `VIDEO_PPT_QUALITY_MATRIX_PUBLIC_COURSE_DELIVERABLES`。
2. 两类路径都可指向一次视频/PPT 抽取输出的 `generated_artifacts/` 或等价目录；报告不写入原始视频路径、source URL、object path、cookie、token、provider payload。
3. 脚本调用 `validateVideoDeliverables(path)` 作为交付契约入口；根据 validator errors/warnings、`final_deliverables_manifest.json` 和 `slide_quality_report.json` 生成对应类别样例的 `deliverable_status`、`quality_report` 和 verdict。
4. `--public-course-deliverables` 会把样例归到 `public_course_video`，不再把真实公开视频样例伪装成 synthetic case；customer authorized 仍然由同一份报告保留为 pending，`matrix_complete=false`。
5. 完整 deliverables fixture、public candidate matrix 报告都写入 `target/`，均不提交。

验证命令：

```bash
node --check scripts/smoke/video-ppt-quality-matrix.mjs
npm run smoke:video-ppt-quality-matrix -- --help
npm run smoke:video-ppt-quality-matrix -- --self-test --pretty
npm run smoke:video-ppt-quality-matrix -- --synthetic-deliverables target/video-ppt-quality-matrix-fixture/generated_artifacts --pretty --output-dir target/video-ppt-quality-matrix-smoke
npm run smoke:video-ppt-quality-matrix -- --public-course-deliverables target/video-ppt-public-candidate-extraction/<session>/generated_artifacts --pretty --output-dir target/video-ppt-public-candidate-quality-matrix
git diff --check
```

完成证据：

- `node --check scripts/smoke/video-ppt-quality-matrix.mjs` 通过。
- `npm run smoke:video-ppt-quality-matrix -- --help` 通过。
- `npm run smoke:video-ppt-quality-matrix -- --self-test --pretty` 通过，`case_count=3`、`deliverable_count=1`、`pending_count=2`。
- `node tools/validate-video-deliverables.mjs target/video-ppt-quality-matrix-fixture/generated_artifacts` 通过，完整包含 PPTX、final/published/version/extraction manifests、rectangle manifest、slide notes、Markdown 和 quality report；`subtitle_page_map` 缺失按条件性交付处理。
- `npm run smoke:video-ppt-quality-matrix -- --synthetic-deliverables target/video-ppt-quality-matrix-fixture/generated_artifacts --pretty --output-dir target/video-ppt-quality-matrix-smoke` 通过，`deliverable_count=1`、`pending_count=2`、`matrix_complete=false`、`deliverables_input_redacted=true`。
- `npm run smoke:video-ppt-quality-matrix -- --public-course-deliverables ... --pretty --output-dir target/video-ppt-public-candidate-quality-matrix` 通过，`deliverable_count=1`、`needs_manual_review_count=1`、`pending_count=1`、`public_course_sample_required=false`、`public_course_deliverables_reviewed=true`。
- 负向参数门禁已验证：同时传 `--self-test` 和任一 deliverables 输入会失败并提示只能二选一。

P2-2E-2B-Next 执行包与完成结果：

1. 先做工作区和样例预检，只读确认当前 public candidate 产物仍存在，不触发 live、不部署、不上传：

```bash
git status --short --branch
git rev-parse --short HEAD
ls target/video-ppt-public-course-probe/nix-teach-5min-slides.mp4
find target -path '*video-ppt-public-candidate-extraction-bestsharp*' -maxdepth 3 -type d | head
```

2. 做页面级人工复核。最小检查对象是 best-sharpness 复跑产物的 PPTX、`video_slides.md`、`selected_slides_manifest.json`、`slide_rectangles_manifest.json` 和 `slide_quality_report.json`。复核结论必须回答：
   - 最终 7 页是否都是真实课件页；
   - weak fade/低对比页是否只是同一版式重复状态；
   - 1 个 full-frame fallback 是否影响主内容可读；
   - 缺字幕/OCR 是否只是样例证据限制，还是影响交付说明；
   - 该 public candidate 应判为 `可交付`、`需人工复核` 还是 `不可交付`。

3. 如果只需要记录人工复核，不改代码，则只追加 `docs/validation/video-ppt-deliverable-smoke.md` 回执和本计划状态；不新增算法、不复跑大矩阵、不碰服务器。

4. 如果需要处理 weak fade/低对比重复页，实施范围必须限制在 selected-slide 去重：
   - 保留 content fingerprint duplicate 和现有 visual near-duplicate 逻辑；
   - 在 visual signature 已存在时增加 contrast-normalized shape mask/Jaccard 类相似度，只有信号样本数足够时才判 `visual_shape_duplicate`；
   - `rejected_duplicate_candidates[]` 记录 `dedupe_reason=visual_shape_duplicate`、matched candidate、matched file、score 和 threshold；
   - selected manifest、slide rectangles manifest、quality report summary 都记录 `visual_shape_duplicate_count`；
   - `visual_duplicate_count` 继续同时统计 `visual_near_duplicate` 和 `visual_shape_duplicate`；
   - 不改变 `requested_selected_candidate_indices` 与最终 `selected_candidate_indices` 的语义。

5. 定向验证命令：

```bash
cargo fmt
CC=clang CXX=clang++ cargo test -p media-worker dedupes_selected_slide_manifest_by_visual_shape_similarity --lib
cargo fmt --check
CC=clang CXX=clang++ cargo test -p media-worker selected_slide --lib
CC=clang CXX=clang++ cargo test -p media-worker auto_selects --lib
CC=clang CXX=clang++ cargo test -p media-worker controlled_video_sample_deliverable_contract_is_complete --lib
npm run test:video-deliverables
cargo check -p media-worker --bin video_ppt_offline_smoke
```

6. public candidate 复跑命令：

```bash
cargo run -p media-worker --bin video_ppt_offline_smoke -- \
  --input target/video-ppt-public-course-probe/nix-teach-5min-slides.mp4 \
  --output-root target/video-ppt-public-candidate-extraction-shape-dedupe \
  --ffmpeg-bin /opt/homebrew/bin/ffmpeg \
  --interval-seconds 15 \
  --title "Public slides sample: How to teach Nix in 5 minutes" \
  --json-output target/video-ppt-public-candidate-extraction-shape-dedupe/offline-smoke-summary.json
```

7. 复跑后读取新 session 的关键指标，记录到 validation 文档，不把 `target/` 内容提交：

```bash
node tools/validate-video-deliverables.mjs target/video-ppt-public-candidate-extraction-shape-dedupe/<session>/generated_artifacts
npm run smoke:video-ppt-quality-matrix -- \
  --public-course-deliverables target/video-ppt-public-candidate-extraction-shape-dedupe/<session>/generated_artifacts \
  --pretty \
  --output-dir target/video-ppt-public-candidate-quality-matrix-shape-dedupe
```

8. 通过标准：
   - validator 通过，不能通过放宽 validator 通过；
   - PPTX slide count、Markdown slide count、`selected_count` 三者一致；
   - `requested_selected_candidate_indices` 保留去重前请求，`selected_candidate_indices` 只保留最终页；
   - 如果出现 `visual_shape_duplicate_count>0`，对应 rejected candidate 能解释 weak fade/低对比重复页；
   - public-course matrix 仍可判 `needs_manual_review`，但不能出现 `deliverable_contract_invalid`；
   - 页面级人工复核确认没有误删关键 build 页面。

9. 失败/回滚标准：
   - 关键 build 内容被误删；
   - selected/PPTX/Markdown 页数不一致；
   - shape duplicate 对低信息空白页过度敏感；
   - manifest count 不一致或 redaction 回退；
   - 复跑质量低于 best-sharpness 基线且没有清晰收益。

出现上述情况时，回滚该窄修复或提高阈值后重跑定向测试；不要扩大到 live、不要部署、不要用客户样例试错。

P2-2E-2B-Next 完成结果：

- `dedupes_selected_slide_manifest_by_visual_shape_similarity` 已覆盖 selected manifest、slide rectangles manifest 和 slide quality report summary 的 `visual_shape_duplicate_count`；
- shape matching 保持保守：Jaccard 阈值外只接受 strict containment，且要求足够 signal samples 和 `visual_shape_duplicate_min_signal_balance=0.80`；
- 较低 signal-balance 门槛会误删 sparse text/build fixture，已被 `auto_selects_sparse_text_build_states_in_public_course_style_slides` 捕获并收紧；
- shape-dedupe public candidate 复跑仍为 `frame_count=96`、`requested_selected_count=9`、`selected_count=7`、`visual_duplicate_count=2`、`visual_shape_duplicate_count=0`、`quality_score=61`；
- public candidate validator 通过，public-course quality matrix 仍为 `needs_manual_review`；
- 页面级复核结论：最终页都是真实课件页，但仍包含弱 fade/清晰度/字幕缺失等风险，因此保持 `需人工复核`，不强行标记为 clean deliverable。

第二/第三 public probe follow-up：

- `Nix in Space` 可匿名下载并通过离线交付契约，但只选出 1 页浏览器/Google Slides 画面，作为弱对照记录，不作为更强 public-course 样例；
- `Layered Nix Stores` 初次复跑暴露 bright template build 误删问题：shape duplicate 把 4 requested 压到 1 selected；
- 新增 `visual_shape_duplicate_max_avg_luma=140.0` 和 `keeps_bright_template_build_states_out_of_visual_shape_dedupe`，避免白底模板 build 被 shape duplicate 删除；
- `Layered Nix Stores` 修复后复跑为 `frame_count=22`、`requested_selected_count=4`、`selected_count=3`、`visual_duplicate_count=1`、`visual_shape_duplicate_count=0`、`quality_score=68`；
- 修复后 `Layered Nix Stores` validator 通过，public-course quality matrix 仍为 `needs_manual_review`，客户授权样例仍 pending。

### 5.9 部署门槛

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
| 主站可见 smoke | 同一公开视频直链 | 用户在主站看到下载动作 | 已通过并纳入 `smoke:video-ppt-main-visible` release gate，PPTX/Markdown/manifest 可下载 | 作为 P1-3 回归基线保留 |
| 无字幕 deliverable contract | 无 transcript/subtitle 的公开视频样例 | 没有 `subtitle_page_map.json` 时仍可交付截图型 PPTX，但必须标记 `missing_transcript_alignment` | P0-3 已完成本地切片；validator、media-worker package summary、published manifest、version history 和 durable manifest 已改为条件性契约 | 作为回归保留 |
| 公开视频课程候选 | media.ccc/NixCon slides MP4 | 真实公开课件视频能通过 validator 并进入 quality matrix | S1 access probe 通过；S2A 离线抽取已生成 PPTX/Markdown/quality report，deliverables validator 通过，quality matrix 判为 `needs_manual_review`；selected manifest、稀疏文字/build 选页、best-sharpness 代表帧、visual-shape duplicate 保守去重均已完成本地修复；shape-dedupe 最终复跑仍为 7 页、质量分 61、`needs_manual_review`；`Nix in Space` 降级为 1 页弱对照；`Layered Nix Stores` 修复 bright-template build 误删后为 3 页、quality score 68、`needs_manual_review` | 若无授权，可继续 crop/sharpness/字幕公开样例窄修复；完整 P2-2E 仍等待客户授权样例 |
| 主站上传视频 | 用户上传 `.mp4/.mov/.m4v/.webm/.mkv/.avi` | 上传登记后明确触发 PPT 抽取，并通过 artifact file API 下载 | `smoke:video-ppt-upload-main` 已实现并通过语法/help/self-test/最小 PPTX-Markdown 校验；live/controlled 回执待授权 | P1-3B |
| 第三方视频登记 | 第三方 `content_url` 或 attachment | 登记视频素材后，特殊触发进入 `VideoExtraction` 并返回可见产物 | `smoke:external-video-ppt` 已实现并通过 self-test/最小 PPTX-Markdown 校验；live 回执待 bearer/授权 | P1-3C live |
| 公开视频页 | HTML 暴露 video/source/OG/Twitter/JSON-LD video | 解析候选并抽取 PPT | P1-1 resolver fixtures 与失败分流已完成 | 用 P1-3 direct URL/page prompt 回归展示 |
| 微信视频号链接 | `weixin.qq.com/sph/...` | 自动解析拒绝，给上传/直链/授权录屏选项 | `smoke:video-ppt-handoff` 已实现并通过 self-test；主站 early-return 和第三方 deterministic card 均已通过本地 endpoint/handler 测试，live pass 待部署后复跑 | P1-3D live |
| 授权录屏兜底 | operator 已批准可播放页面 | 录制 `.mp4` 后复用现有抽取 | runbook 和 `capture:authorized-video` 已实现；self-test/dry-run/授权门禁通过，live 授权样例未执行 | P2-1C |
| 质量报告 | 已生成 PPTX/Markdown/manifest 的 video/PPT 包 | 输出可选 `slide_quality_report.json`，解释质量分和风险 | P2-2A 已完成本地验证；legacy 包兼容，新包质量报告可校验 | 作为后续样例复核辅助保留 |
| 裁剪/稳定/字幕/清晰度增强 | 合成 PPT、公开视频课程、客户授权视频 | 减少 fallback/重复/转场帧，补齐可用字幕页映射，并提示低清晰度/低可读性页 | P2-2B bright-canvas detector、P2-2C 暗色/亮色/纯色低信息稳定段过滤、P2-2D OCR evidence/quality-report coverage、P2-2F 清晰度/可读性信号本地切片已完成；P2-2E quality-matrix self-test scaffold 和 local deliverables input 已完成；真实 public/customer 样例矩阵和更广泛质量处理仍未完成 | 做 P2-2B/P2-2C follow-up、P2-2D sample、P2-2E public/customer real samples |
| 普通视频转 PPT | 没有 PPT/课件画面的普通视频 | 不触发或提示不适用 | 已明确边界 | 保持 |

## 7. 决策门

进入 P1-3B/P1-3C 前需要确认：

- 是否允许用主站 `https://v3.elepcloud.com` 进行非客户公开视频上传 smoke；该操作会写入一条 smoke 文档/任务记录，但不发版、不重启服务。
- 第三方 live smoke 使用哪个 `connection_id`、`source_id` 和 inbound bearer；没有凭据时只能先做本地/loopback deterministic smoke。
- smoke 数据保留策略：公开样例和 generated artifacts 可留在 `target/` 或测试 workspace，不能提交下载产物。

进入 P2-1C/P2-1D 前需要确认：

- 是否允许在 8 服务器部署 capture fallback；如果允许，是否只针对公开可播放页面，还是允许人工登录后的短时受控 session。
- 录屏产物保留期：默认建议 7 天，是否需要更短或按客户配置。
- 授权记录格式：由谁批准、批准哪条链接、最大录制时长、是否允许音频、清理要求。
- 第三方系统是否愿意优先提供自己的视频文件直链，避免 DataMax 做平台录屏。
- 主站 UI 是否需要新增“申请授权录屏处理”按钮，还是先用人工流程。

没有上述确认前，录屏兜底停留在 runbook/self-test/dry-run，不进入 live capture 或生产实现。

## 8. 下一次执行建议

按风险和收益排序：

1. 授权后运行 P1-3B 主站上传视频 live/controlled smoke，证明“用户上传视频文件”入口可交付 PPTX/Markdown/manifest，并把回执追加到验证记录。
2. 授权后运行 P1-3C 第三方视频登记 live/controlled smoke，证明第三方登记视频素材后能通过“提取视频里的 PPT”触发同一交付链路；如 surface 缺少下载出口，标为第三方发布可见性缺口。
3. 在用户明确批准 8 服务器部署窗口后，复核 P1-3D lightweight handoff live surface：当前代码已补齐主站 early return 和第三方 deterministic card，并通过本地 handler/endpoint 测试；部署后复跑 `smoke:video-ppt-handoff -- --mode main`，拿到 main live pass 回执，再在 inbound bearer/context 获批后跑 external live pass。
4. 执行 P2-1C operator 授权样例 smoke：先 dry-run，再在授权 workstation/jump-host 录制 30-60 秒 MP4，人工复核后走主站上传或第三方登记视频 PPT 抽取；没有明确授权前不录屏。
5. 客户/operator 授权后再跑 P2-2E-3 客户样例质量矩阵；不要把 public candidate 或 synthetic fixture 替代客户样例。
6. 如果暂时没有 live/客户授权，再继续本地低风险切片：第二个公开视频课程样例、crop fallback 针对性 fixture、sharpness/readability 复核或字幕/OCR 公开样例；这些都不能替代 live gate。

本计划完成当前阶段的定义：

- active plan 已收敛为当前可执行方案，并把 P1-3B/P1-3C/P1-3D/P2-1 拆成可执行步骤、命令形态和验收记录格式。
- 后端公开视频 smoke、主站可见 smoke、public candidate validator pass 与 `needs_manual_review` 分流、抽取效果复核、视频号限制、授权录屏兜底都已纳入执行队列和验收矩阵。
- GitHub 同步策略：计划/验证文档可按 doc-only 提交；功能或脚本提交必须绑定对应测试证据；任何 GitHub 同步都不等于 8 服务器发版。
- 8 服务器未发版、未重启、未触碰 `mode`。

## 9. 下一阶段里程碑表

| 里程碑 | 执行内容 | 需要输入/授权 | 必跑验证 | 完成证据 | 8 服务器动作 |
| --- | --- | --- | --- | --- | --- |
| M0 | P0-3 subtitle map contract 修正 | 无；本地开发即可 | 已通过 `node --test tools/validate-video-deliverables.test.mjs`、`npm run test:video-deliverables`、media-worker no-subtitle/controlled/durable tests、platform-api video_extraction/video_ppt、`git diff --check` | 已完成：validator 接受无字幕包、拒绝坏 map；validation 文档口径统一 | 未部署 |
| M1 | P1-3B 主站上传视频 controlled smoke | 用户批准在主站写入一条非客户公开视频 smoke 记录 | `npm run smoke:video-ppt-upload-main`，并下载 PPTX/Markdown/manifest 复核 | `docs/validation/video-ppt-deliverable-smoke.md` 追加上传入口回执 | 不部署，除非 smoke 证明现网缺已修复代码且用户另批 |
| M2 | P1-3C 第三方视频登记 controlled smoke | inbound bearer、`connection_id`、`source_id`、可用公开视频或上传文件 | `npm run smoke:external-video-ppt` live；失败时跑 self-test 对照 | 第三方登记和特殊触发分开记录，产物 surface 明确 | 不部署，除非用户另批 |
| M3 | P1-3D 视频号/登录态 handoff live pass | 用户批准 8 服务器部署窗口；第三方模式还需要 inbound bearer | 部署前本地测试、部署后 `npm run smoke:video-ppt-handoff -- --mode main`，第三方按授权补跑 | 主站/第三方返回 handoff 卡片，不抓视频、不生成 PPT、不走 provider | 仅在批准后 pull/build/restart 受影响服务 |
| M4 | P2-1C 授权录屏样例 | operator 授权记录、可播放来源、最大时长、音频策略、保留期、handoff 目标 | `capture:authorized-video` dry-run，live capture 后复用 P1-3 上传或第三方抽取 smoke | 录制 `.mp4` 可进入现有 `VideoExtraction`，并有质量结论 | 默认不启用；8 内部录制需单独批准 |
| M5 | P2-2F 清晰度/可读性质量信号 | 无 live 授权；本地 fixture 即可 | 已通过 media-worker selected_slides/slide_rectangle/controlled sample/sharpness helper、低对比字迹端到端 fixture、deliverable validator、platform-api video tests、`git diff --check` | 已完成：`slide_quality_report.json` 显示 sharpness/readability risk，低对比字迹会提示 high-risk review，旧包兼容 | 未部署 |
| M6 | P2-2E-1 质量矩阵本地 deliverables 输入 | 一次真实或合成视频/PPT 抽取输出的 `generated_artifacts/`；可用临时 target fixture | 已通过 `node --check scripts/smoke/video-ppt-quality-matrix.mjs`、`--help`、`--self-test --pretty`、`node tools/validate-video-deliverables.mjs target/video-ppt-quality-matrix-fixture/generated_artifacts`、`--synthetic-deliverables target/video-ppt-quality-matrix-fixture/generated_artifacts --pretty`、`--public-course-deliverables <public-candidate-generated_artifacts> --pretty` | 已完成：matrix 可读取本地交付包并复用 validator，synthetic/public 可按类别分类，customer gate 仍 pending | 未部署 |
| M6A | P2-2E-2A public candidate deliverable contract 修复 | 无 live 授权；复用 `target/` 下 public candidate | 已通过 `cargo fmt --check`、`cargo check -p media-worker --bin video_ppt_offline_smoke`、media-worker contract test、`npm run test:video-deliverables`、public candidate `validate-video-deliverables`、quality matrix | 已完成：public manifests redaction 修复，`selected_count=5`，validator 通过，quality matrix 结论 `needs_manual_review` | 未部署 |
| M6B | P2-2E-2B selected manifest 去重语义修复 | 无 live 授权；复用 `target/` 下 public candidate | 已通过 selected-slide/dedupe/auto-select tests、controlled contract、`npm run test:video-deliverables`、public candidate offline smoke、public candidate validator、`--public-course-deliverables` quality matrix | 已完成：`requested_selected_candidate_indices` 记录去重前请求，`selected_candidate_indices` 记录最终入选页；public candidate 为 7 requested、5 selected、2 visual duplicates removed | 未部署 |
| M6C | P2-2E-2B 稀疏文字/build 状态自动选页修复 | 无 live 授权；复用 `target/` 下 public candidate | 已通过 sparse-text build fixture、`auto_selects`、selected-slide tests、controlled contract、`npm run test:video-deliverables`、public candidate offline smoke、public candidate validator、public-course quality matrix | 已完成：visual signature 升级到 32x32，新增 changed-sample guard；public candidate 当前为 9 requested、7 selected、2 duplicates removed，覆盖更多 build 内容但仍需人工复核 | 未部署 |
| M6D | P2-2E-2B 稳定段 best-sharpness 代表帧选择 | 无 live 授权；复用 `target/` 下 public candidate | 已通过 best-sharpness rule test、`auto_selects`、selected-slide tests、controlled contract、`npm run test:video-deliverables`、public candidate offline smoke、public candidate validator、public-course quality matrix | 已完成：稳定段内优先选 sharpness score 更高的代表帧；public candidate 当前 quality score=61、sharpness high=3、仍为 `needs_manual_review` | 未部署 |
| M6E | P2-2E-2B-Next 页面级复核与 visual-shape/fade duplicate 窄修复 | 无 live 授权；复用 `target/` 下 public candidate | 已通过 `cargo fmt --check`、shape duplicate fixture、selected-slide/auto-select/controlled contract、`npm run test:video-deliverables`、`cargo check -p media-worker --bin video_ppt_offline_smoke`、shape-dedupe public candidate offline smoke、validator、public-course quality matrix | 已完成：保守 shape duplicate 计数进入 selected/rectangle/quality report；public candidate 仍为 7 页、quality score 61、`needs_manual_review`，且 build fixture 未被误删 | 未部署 |
| M6F | P2-2E public probe follow-up 与白底 build 防误删 | 无 live 授权；使用额外 media.ccc public slides 样例 | 已通过 bright-template build guard fixture、shape duplicate fixture、selected-slide/auto-select tests、`Layered Nix Stores` fixed offline smoke、validator、public-course quality matrix | 已完成：`Nix in Space` 降级为 1 页弱对照；`Layered Nix Stores` 修复后从 1 页恢复到 3 页，`visual_shape_duplicate_count=0`，quality score=68，仍为 `needs_manual_review` | 未部署 |
| M6G | EP6 单页输出复核风险 | 无 live 授权；复用 `Nix in Space` public slides 样例 | 已通过 `cargo fmt --check`、`flags_single_slide_output_for_quality_review_without_blocking_delivery`、selected-slide regression、controlled contract、`npm run test:video-deliverables`、`cargo check -p media-worker --bin video_ppt_offline_smoke`、`Nix in Space` offline smoke、validator、public-course quality matrix | 已完成：1 页 public deliverable 仍保持 `final_pptx_ready`，但质量报告写入 `single_slide_output=true` 和 `single_slide_output_review_required`，避免误读为完整课程页覆盖 | 未部署 |
| M6H | EP5 customer-authorized deliverables 与组合质量矩阵入口 | 无 live 授权；只用本地非客户 fixture 验证脚本契约 | 已通过 `node --check scripts/smoke/video-ppt-quality-matrix.mjs`、help、自测、synthetic/public/customer 单项输入、synthetic+public+customer 三输入组合、缺 customer approval id 负向门禁、self-test/customer 负向门禁 | 已完成：`--customer-deliverables <generated_artifacts> --customer-approval-id <id>` 可把已授权样例归入 `customer_authorized_video`；三类 deliverables 输入可以组合，三类齐全时报告 `matrix_complete=true`，approval id 不写入报告，但真实客户样例仍必须等待授权输入 | 未部署 |
| M6I | slide quality report risk flag schema gate | 无 live 授权；使用 validator fixture 和当前 `Nix in Space` public deliverables | 已通过 `node --check tools/validate-video-deliverables.mjs`、`npm run test:video-deliverables`、当前 public deliverables validator、三输入 quality matrix | 已完成：validator 会拒绝 malformed `risk_flags`，并校验 `single_slide_output=true` 与 `single_slide_output_review_required` 一致；当前生成的 public deliverables 仍可通过 | 未部署 |
| M7 | P2-2C 低信息/短转场过滤与深色主题防误伤 | 无 live 授权；本地 fixture 即可 | 已通过 `cargo fmt --check`、`cargo test -p media-worker auto_selects --lib`、动画转场/深色主题/暗色/亮色/纯色定向测试、selected slides、controlled sample、slide rectangle、video deliverables validator、`git diff --check` | 已完成：稳定黑屏、白屏、灰屏/纯色 loading 和短动画转场分别写入 rejection，不会被自动选为 PPT 页；深色有内容课件页不会被误拒 | 未部署 |
| M8 | P2-2B 讲师小窗/外部前景 crop 端到端 fixture | 无 live 授权；本地 fixture 即可 | 已通过 `cargo fmt --check`、`cargo test -p media-worker writes_foreground_component_crop_for_speaker_window_obstruction --lib`、slide rectangle、selected slides、controlled sample、video deliverables validator、`git diff --check` | 已完成：`foreground_component_v1` crop 写入 manifest/quality report/PPTX，不退回 full-frame fallback | 未部署 |
| M9 | P2-2E-2/3 真实三样例质量复核 | 合成 PPT 视频、公开视频课程、客户授权视频 | 完整验收还需 media-worker quality tests、deliverable validator、逐样例人工复核 | 每个真实样例给出可交付/需人工复核/不可交付结论；失败能归因 | 不部署，除非质量切片已通过并获批 |
