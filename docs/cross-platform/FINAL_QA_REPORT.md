# 终轮自动化与交付候选验收报告（Round 6 / 2.0.0 候选）

- **测试日期**：2026-11-15
- **被测分支 / 提交**：`t6-1-delivery-docs` @ `606f07b9b568d75404990794169243adad3c3ab0`（含 `T5.1`–`T5.3` 与 `T6.1`；工作区无未提交业务修改）
- **本地环境**：macOS（Apple Silicon `aarch64-apple-darwin`）、Rust / Cargo + `cargo-xwin 0.23.1`（`x86_64-pc-windows-msvc`）、Node.js v24.22.2、Vitest `5.1.0` + jsdom、Playwright `1.65.1`（WebKit `v2388`）、Swift 测试基线、`/Applications/Pen.app`（Pen `1.2.23`）、LLDB + AppKit/WKWebView
- **远端 CI 环境**：GitHub Actions（`macos-latest` 与 `windows-latest` 原生运行器）
- **参考文档**：[DEV_PROGRESS.md](DEV_PROGRESS.md)、[MANUAL_ACCEPTANCE.md](MANUAL_ACCEPTANCE.md)、[UPGRADE_REHEARSAL.md](UPGRADE_REHEARSAL.md)、[TEST_PLAN.md](TEST_PLAN.md)、[ISSUES.md](ISSUES.md)、[TEST_REPORT.md](TEST_REPORT.md)（按约定保留原文不改写）

---

## 1. 总结论

- **总体结论**：**2.0.0 交付候选的本机 macOS 全量检查、双端 GitHub Actions 自动化门禁（含 Windows 原生 CI、Pen `1.2.23` 兼容性探测、2.0.0 草稿打包/签名与 2.0.0 → 2.0.1 升级演练）、以及 `v2.0.0` `WKWebView` 运行态回归全部通过 ✅；[MANUAL_ACCEPTANCE.md](MANUAL_ACCEPTANCE.md) 中的真机/实账号/实机多屏及正式发布项目维持未完成，按约束不计为通过 ⚠️。**

---

## 2. 本机实测验证明细（`t6-1-delivery-docs` @ `606f07b`）

| 检查项 | 命令 / 对象 | 结果 | 详情 |
|---|---|---|---|
| Rust 格式检查 | `cargo fmt --all --check` | ✅ PASS | 退出码 `0` |
| macOS 工作区 Clippy | `cargo clippy --workspace --all-targets --locked -- -D warnings` | ✅ PASS | 0 警告 / 0 错误（含 `--features platform-it`） |
| Windows x64 交叉 Clippy | `cargo xwin clippy --workspace --all-targets --target x86_64-pc-windows-msvc --locked -- -D warnings` | ✅ PASS | 0 Rust 警告（含 `--features platform-it`） |
| L1 + L2 工作区单测（macOS） | `cargo test --workspace --features platform-it --locked -- --nocapture` | ✅ PASS | `switchboard-core` **26/26 passed**；`switchboard-platform` lib **5 passed, 1 ignored**（`signed_release_replacement`）；`platform-it` 通过（含 `chflags uchg` 不可变目标原子写失败保留与临时文件清理） |
| macOS 签名包原子替换测试 | `SWITCHBOARD_PREVIOUS_ARCHIVE=.../2.0.0-arm64.zip SWITCHBOARD_NEXT_ARCHIVE=.../2.0.1-arm64.zip cargo test -p switchboard-platform --locked update::macos::tests::signed_release_replacement -- --ignored --exact --nocapture` | ✅ PASS | **1 passed**：原 Ed25519 公钥验签 `2.0.0` 与 `2.0.1` ZIP、校验包结构与版本、临时目录原子替换并校验保护夹具字节不变 |
| 前端 TS + Vite 构建 | `npm run build:ui` | ✅ PASS | `tsc --noEmit` 与 `vite build`（`129ms`）成功 |
| L3 前端与发布工具单测 | `npm test`（`npm run test:ui && npm run test:release`） | ✅ PASS | Vitest **2 文件 29/29 passed**；Node 发布/签名回归 **4/4 passed**（含 Node Ed25519 与 Swift `Support/sign_update.swift` 双向交叉验签） |
| L4 WebKit 视觉回归（macOS） | `npm run test:visual`（Playwright WebKit `v2388`） | ✅ PASS | **22/22 passed**（`10.3s`），`Tests/baseline/macos/` 未改动 |
| 生成物一致性核验 | `npm run types:ipc && npm run scenarios:ui` + `git status --short` | ✅ PASS | `ui/src/generated/` 与 `ui/public/scenarios/` 0 差异 |
| Swift 1.0.1 基线对照 | `./Tests/run.sh` | ✅ PASS | `UpdateLogicTests` **16/16** + `RegressionTests` **156/156 = 172/172** |
| L5 Pen 静态兼容（本机） | `node scripts/check_pen_compat.mjs /Applications/Pen.app/Contents/Resources/app.asar` | ✅ PASS | Pen `1.2.23` 全部 7 项通过 |
| 本地候选发布资产校验 | `node scripts/release_assets.mjs verify build/release v2.0.0` + `codesign` / `Info.plist` / `file` | ✅ PASS | 5 个资产齐全且 Ed25519 签名全部通过；解包 `Pencil Account Switcher.app` 为 `arm64` Mach-O，`CFBundleIdentifier=dev.switchboard.Switchboard`、`CFBundleShortVersionString=2.0.0`、`codesign --verify --deep --strict` 通过 |
| macOS `v2.0.0` 壳与 `WKWebView` 运行态复测 | `cargo build --locked -p switchboard-app` + `HOME=/tmp/sb-qa-home SWITCHBOARD_DEMO=1 SWITCHBOARD_AUTO_OPEN=1` + `vite` + `lldb` | ✅ PASS | `MainScreen`(`h=547`, 3 账号, `topGap=12.0`) → `SettingsScreen`(`h=345`, 页脚实测显示 `版本 2.0.0`) → `ManageScreen`(`h=500`, 3 行) → `ConfirmDialog`(`inert=true`, `["移除 jamie.work@outlook.com","取消"]`)、Esc 分层关闭、`Reopen` 复位主页及 `quit_app`（退出码 `0`）均通过 |

---

## 3. 远端 GitHub Actions CI 与工件复核明细

| 工作流 / Run | 分支 / 提交 | 状态 | 复核内容与关键结论 |
|---|---|---|---|
| `Test` [43667472129](https://github.com/gaogep/pencil-account-switcher/actions/runs/43667472129) | `t6-1-delivery-docs` @ `606f07b` | ✅ 6/6 success | `rust (windows-latest)`、`rust (macos-latest)`、`test`（Swift 16+156=172）、`ui`（build + 29 Vitest + 4 release）、`visual (macos-latest)`（22 passed）、`visual (windows-latest)`（22 passed, `17.6s`）。日志确认 Windows 原生执行 **26 个 core 单测 + 1 个 Windows `P-AS-02/03 & P-PR-01..05` 夹具测试 + `platform-it` + `scripts/test_windows_update.ps1`** |
| `Pen compatibility` [43667472102](https://github.com/gaogep/pencil-account-switcher/actions/runs/43667472102) | `t6-1-delivery-docs` @ `606f07b` | ✅ 3/3 success | 已下载 `pen-windows-report` 工件实核（Windows 上 `Pen.exe 1.2.23`）：7 项静态检查 PASS；`deviceId` (`c1a76373937eb04838609f5840c2de86`)、`processes.json`（6 进程）、`main-window.json`（`pen.dev`, `Chrome_WidgetWin_1`, `1024×720`）、`quit`/`fake-session-quit`/`final-quit` 均 `exited: true`、`single-instance-windows.json` 单可见窗、`session-observation.json` 会话文件存在且 SHA256 变更、`autostart-set/get/clear` 均 `success: true`、`screen.png` 实测展示 `Pen.exe` 正常启动 |
| `Release` [43580428113](https://github.com/gaogep/pencil-account-switcher/actions/runs/43580428113) | `t5-2-release-pipeline` @ `1976e51` | ✅ 3/3 success | `gh release view v2.0.0` 确认草稿状态 `isDraft: true`（未公开发布），含全部 5 个规范资产（`arm64.dmg` 5,974,084 B、`arm64.zip` 5,315,994 B + `.sig` 88 B、`windows-x64-setup.exe` 3,356,809 B + `.sig` 88 B） |
| `Upgrade rehearsal` [43581711837](https://github.com/gaogep/pencil-account-switcher/actions/runs/43581711837) | `t5-3-upgrade-rehearsal` | ✅ 2/2 success | macOS 与 Windows（`2.0.0 → 2.0.1`）均通过。已下载并核验 `windows-report.json`：`installedVersion="2.0.1"`、`executableReplaced=true`、`restarted=true`、`accountFixtures=3`、`protectedFiles=4`、`protectedBytesUnchanged=true`、`realLoginTested=false`、`startupMigrationTested=false` |

### 3.1 Windows 22 场景渲染人工抽检（`visual-windows-latest` 工件）

已下载 `Test 43667472129` 的 `visual-windows-latest` 全部 22 张实际截图（`@2x` 物理像素）并完成尺寸与视觉抽检：

- **像素维度与排版**：22 张 `@2x` 截图逻辑宽度均为 `410pt`，各场景逻辑高度与 macOS 基线逐一吻合（`main`: `820×1094`、`main-update-*`: `820×1238`、`main-single`: `820×690`、`main-empty`: `820×636`、`manage*`: `820×1000`、`settings`/`settings-launch-on`: `820×690`、`settings-update-available`: `820×834`）。
- **人工抽检观感**：已目视核查 `main-actual`、`manage-actual`、`manage-expired-actual`、`settings-actual`、`main-update-available-actual`、`main-feedback-long-actual` 与 `manage-confirm-remove-actual`：中文与邮箱字体清晰、Settings 文案正确显示 `登录 Windows 时自动运行切号器`、`alex.cr...oud.com` 中段截断正常、移除确认框焦距环与按钮顺序正常、无文本重叠或横向溢出。
- **基线约定**：按 [MANUAL_ACCEPTANCE.md](MANUAL_ACCEPTANCE.md)，CI Playwright 截图仅作跨端回归与初步人工抽检证据，正式的 `Tests/baseline/windows/` 基线需待实体 Windows 环境复核后批准入库。

---

## 4. 仍保留的实机验收边界

下列项目因暂无实体 Windows 设备及本轮不操作真实账号与公开发布，**维持未完成，不计为通过**，统一由 [MANUAL_ACCEPTANCE.md](MANUAL_ACCEPTANCE.md) 跟踪：

- `T0.5` Windows 11 ARM 设备/VM（官方 Pen x64 安装路径、实际任务栏与模拟运行验证）
- 真实账号路径：`M-01..M-10`（真实登录、切号、未保存文档关闭确认、多文档弹窗、额度与移除）、`M-17..M-18`
- 系统级交互与多端外观：`M-11` / `P-AS-01`（真机重启后 `SMAppService` 与 Windows 设置自启一致性）、`M-12..M-16`（多屏、四边任务栏、深浅色切换、高 DPI、macOS 全屏空间）、Windows L4 正式基线批准
- 真实 UI 触发升级：`UP-01..UP-05`（旧 Swift 1.0.1 → Tauri 横幅下载重启迁移、两端 Tauri → 下一版 UI 升级、篡改签名报错与回退）
- `v2.0.0` 草稿保持草稿态；正式公开发布与按 [SWIFT_ARCHIVE.md](SWIFT_ARCHIVE.md) 清理旧 Swift 代码均在实机验收之后执行。
