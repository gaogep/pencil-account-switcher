# 测试报告：跨平台重构（Round 5 复测：`I-020` 修复验收 & 全量回归）

- **测试日期**：2026-11-14
- **被测分支 / 提交**：`t4-3-windows-login` @ `aa46556` + 本地工作区 `I-020` 修复（`ui/src/components/{accounts.tsx,banners.tsx,components.css,primitives.tsx}`、`ui/src/{screens.css,screens.tsx}`、`ui/src/tests/{app.test.tsx,components.test.tsx,setup.ts}`、`ui/tests/visual/screens.spec.ts`；`Tests/baseline/` 0 改动）
- **测试环境**：macOS（Apple Silicon `aarch64-apple-darwin`，主屏 `1512×982`、`visibleFrame={{0, 75}, {1512, 874}}`）、Rust / Cargo + `cargo-xwin 0.23.1`（`x86_64-pc-windows-msvc` 交叉编译静态检查）、Node.js v24.22.2、Vitest `5.1.0` + jsdom `31.0.1`、Playwright `1.65.1`（WebKit `v2388`）、Swift 测试基线、LLDB + AppKit/WKWebView
- **参考文档**：[DEV_PROGRESS.md](DEV_PROGRESS.md)、[TEST_PLAN.md](TEST_PLAN.md)、[DEV_PLAN.md](DEV_PLAN.md)、[DESIGN.md](DESIGN.md)、[ISSUES.md](ISSUES.md)

---

## Round 5 复测总结论

- **总体结论**：**macOS 范围及跨端构建 / L1 / L3 / L4 全部通过 ✅；余 `I-017` L2 `platform-it` 与 Windows 真机运行/基线待测 ⚠️**
  1. **`I-020` 修复验收通过（L4 WebKit 22/22 全绿 & L3 Vitest 29/29 全绿）**：
     - **L4 Playwright WebKit（`v2388`，`410×1000 @2x`）**：**22/22 passed**（`10.3s`）。除 `manage-confirm-remove` 按 `I-002` 仅核对文案与按钮顺序外，**全部 21 个对比场景均实现 0 尺寸差异**，`pixelmatch`（阈值 `0.1`，允许上限 `1.00%`）差异率位于 **`0.27%`–`1.00%`**：最低为 `main-single` `0.27%`（`1522 / 565800 = 0.2690%`），最高为 `manage-expired` **`1.00%`**（`8151 / 820000 = 0.9940%` 原始比值，严格通过但紧贴 1% 阈值边界，建议后续改版持续关注余量）。
     - **人工视觉抽检（`ui/visual-report/macos/*.png`）**：已核验 `main`、`main-empty`、`main-feedback-long`、`main-unregistered-live`、`manage`、`manage-expired`、`manage-confirm-remove`、`settings` 及对应 `-diff.png`：`36×36` / `34×34` 头像圆圈与字号恢复正常、`AccountEmail` 中段省略（如 `alex.cr…ud.com`）对齐 Swift、透明背景截图一致、`.manage-card .hairline { width: 0 }` 与 `panel-manage*.png` 原图一致，`Tests/baseline/macos/` 未改动。
     - **L3 Vitest + jsdom**：**2 文件 29/29 passed**（`components.test.tsx` 24 + `app.test.tsx` 5），新增 `AccountEmail` 窄幅截断/恢复及 `Avatar` `34px`/`36px` 样式与字号断言。
  2. **macOS `switchboard-app` 真机 `WKWebView` 运行态复测通过（隔离 `HOME=/tmp/sb-qa-home SWITCHBOARD_DEMO=1 SWITCHBOARD_AUTO_OPEN=1` + `vite`）**：
     - 实际运行 `MainScreen`（`h=547`，`topGap=12.0`，3 账号头像实测 `36px/12px` 与 `34px/11px`）→ `SettingsScreen`（`h=345`）→ `ManageScreen`（`h=500`，3 条账号行、3 头像 `34px/11px`），高度与 Swift `panel-{main,settings,manage}.png` 逻辑尺寸逐点一致。
     - `ConfirmDialog`（`inert=true`，按钮 `["移除 jamie.work@outlook.com", "取消"]`）、`UI-PN-03` Esc 行为、`UI-PN-04` 切至 Finder 保持 `VIS=1 KEY=0`、`UI-PN-05` `Reopen` 重置回 `MainScreen`（`h=547`）以及 `quit_app`（退出码 `0`）均通过。
  3. **Rust、Windows 交叉编译、生成物与 Swift 基线全绿**：
     - `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo xwin clippy --workspace --all-targets --target x86_64-pc-windows-msvc --locked -- -D warnings`（Rust 0 警告）、`cargo test --workspace --locked`（`switchboard-core` **26/26 passed**）、`cargo build --locked -p switchboard-app`、`npm run build:ui`、`npm run types:ipc && npm run scenarios:ui`（0 差异）与 `./Tests/run.sh`（Swift `16/16 + 156/156 = 172/172`）全部通过。
  4. **未闭合边界（非 macOS 阻断，待后续阶段/Windows 真机）**：
     - **`I-017`（L2 缺口）**：`crates/switchboard-platform` 尚未定义 `platform-it` feature 与 L2 集成测试（仍 `0 tests`）。
     - **Windows 真机与视觉基线**：`T4.1`–`T4.3` 尚待 Windows 实机（或 VM / GitHub Actions `windows-latest`）执行 `cargo test`、`pen-probe` 实跑、托盘/DPI/四边任务栏/InPrivate 登录验收及人工确认后生成 `Tests/baseline/windows/`。

### Round 5 复测执行明细

| 检查项 | 命令 / 对象 | 结果 | 详情 |
|---|---|---|---|
| 前端 TS + Vite 构建 | `npm run build:ui` | ✅ PASS | `tsc --noEmit` 与 `vite build`（`153ms`）成功 |
| L3 前端单测（Vitest） | `npm test`（`ui/src/tests/*.test.tsx`） | ✅ PASS | **2 文件 29/29 passed**（`components.test.tsx` 24 + `app.test.tsx` 5；含 `Avatar` `34/36px` 样式及 `AccountEmail` 截断/恢复回归） |
| L4 WebKit 视觉回归（`T3.4`） | `npm run test:visual`（WebKit `v2388`，`410×1000 @2x`） | ✅ PASS | **22/22 passed**（`10.3s`）：21 场景 0 尺寸差异，像素差异 `0.27%`–`1.00%`（`manage-expired` 为 `8151/820000 = 0.9940%`，紧贴 1% 阈值）；`manage-confirm-remove` 文案与按钮顺序通过；基线未修改 |
| Rust 格式与双端 Clippy | `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo xwin clippy --workspace --all-targets --target x86_64-pc-windows-msvc --locked -- -D warnings` | ✅ PASS | macOS 与 Windows x64 目标 Rust Clippy 均 **0 警告** |
| Rust 工作区单测与壳构建 | `cargo test --workspace --locked`、`cargo build --locked -p switchboard-app` | ✅ PASS（部分 0 用例） | `switchboard-core` **26/26 passed**；`switchboard-platform`/`switchboard-app`/`pen-probe` 仍 **0 tests**（`I-017`） |
| 生成物与 Swift 基线 | `npm run types:ipc && npm run scenarios:ui`、`./Tests/run.sh` | ✅ PASS | `ui/src/generated/` 与 `ui/public/scenarios/` 0 差异；Swift `16/16 + 156/156 = 172/172` |
| macOS `WKWebView` 实机复测 | `HOME=/tmp/sb-qa-home SWITCHBOARD_DEMO=1 SWITCHBOARD_AUTO_OPEN=1 ./target/debug/switchboard-app` + `vite` + `lldb` | ✅ PASS | `Main`(`h=547`, 头像 `36px/12px` & `34px/11px`) → `Settings`(`h=345`) → `Manage`(`h=500`, 3 头像 `34px/11px`)、`ConfirmDialog`(`inert=true`)、`UI-PN-03..05`、`quit_app` 退出码 `0` |

---

# 历史归档：Round 5 初测（P3 `T3.3–T3.4` + P4 `T4.1–T4.3` 初次验收）

- **测试日期**：2026-11-14
- **被测分支 / 提交**：`t4-3-windows-login` @ `aa46556`（含 `ba3d1b3` → `fb921d9` `T3.3` → `f7c1312` `T3.4` → `0d7cf7c` `T4.1` → `99ab70f` `T4.2` → `aa46556` `T4.3`）
- **测试环境**：macOS（Apple Silicon `aarch64-apple-darwin`，主屏 `1512×982`、`visibleFrame={{0, 75}, {1512, 874}}`）、Rust / Cargo + `cargo-xwin 0.23.1`（`x86_64-pc-windows-msvc` 交叉编译静态检查）、Node.js v24.22.2、Vitest `5.1.0` + jsdom `31.0.1`、Playwright `1.65.1`（WebKit `v2388`）、Swift 测试基线、LLDB + AppKit/WKWebView
- **参考文档**：[DEV_PROGRESS.md](DEV_PROGRESS.md)、[TEST_PLAN.md](TEST_PLAN.md)、[DEV_PLAN.md](DEV_PLAN.md)、[DESIGN.md](DESIGN.md)、[ISSUES.md](ISSUES.md)

---

## Round 5 初测总结论

- **总体结论**：**不通过（L4 WebKit 视觉回归 21/22 失败且发现 `Avatar` 样式缺陷 `I-020`；L1/L3/macOS 运行态 `T3.3` 与 `cargo xwin clippy` 均绿，余 L2 `platform-it` 与 Windows 真机待测）** ❌⚠️
  1. **构建、静态检查、L1 Core、L3 Vitest、生成物与 Swift 基线全部通过**：
     - Rust：`cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、Windows x64 交叉编译 `cargo xwin clippy --workspace --all-targets --target x86_64-pc-windows-msvc --locked -- -D warnings`（Rust 0 警告）、`cargo build --locked -p switchboard-app` 与 `cargo test --workspace --locked`（`switchboard-core` `26/26 passed`）全部通过。
     - 前端与基线：`npm run build:ui`（`tsc --noEmit && vite build`）、`npm test`（Vitest **2 文件 26/26 通过**）、`npm run types:ipc && npm run scenarios:ui`（0 差异）与 `./Tests/run.sh`（Swift `16/16 + 156/156 = 172/172`）全部通过。
  2. **`T3.3` macOS 实机运行态交互验收通过（隔离 `HOME=/tmp/sb-qa-home SWITCHBOARD_DEMO=1 SWITCHBOARD_AUTO_OPEN=1`）**：
     - 真机验证 `MainScreen`（`h=545`）→ `SettingsScreen`（`h=339`）→ `ManageScreen`（`h=479`，3 条账号行）导航与 `ResizeObserver` 高度联动（`frame.size` 实时贴合且 `topGap=12.0` 不飘移）。
     - 验证 `ConfirmDialog`（`[role="alertdialog"]`，背景 `inert=true`，按钮顺序 `["移除 jamie.work@outlook.com", "取消"]`）、`UI-PN-03` Esc 仅关确认框、再按 Esc 收起面板、`UI-PN-04` 确认框打开期间（`set_confirmation_open(true)`）切到 Finder 保持 `VIS=1 KEY=0`、`UI-PN-05` `Reopen` 重新显示并复位主页（`h=545`），以及页脚「退出应用」触发 `quit_app` 正常退出（状态码 `0`）。
  3. **阻塞问题 `I-020`（🔴 不通过）：L4 WebKit 视觉回归 21/22 尺寸不符且 `Avatar` 头像塌陷**：
     - 实际运行 `npm run test:visual`（Playwright WebKit `v2388`，`410×1000 @2x`）：仅 `manage-confirm-remove`（按 `I-002` 例外跳过尺寸与像素对比）通过，**其余 21/22 场景全部在 `screens.spec.ts:50-52` 的严格尺寸断言失败**，提前返回导致 **0/21 完成 `pixelmatch` 像素比对**，`.github/workflows/test.yml` 的 `macos-14` `visual` job 将直接失败。
     - **根因 A（代码缺陷 — `Avatar` 内联无单位数值在 Preact 11 被丢弃）**：[primitives.tsx:59](../../ui/src/components/primitives.tsx#L59) 向 `style` 传入无单位数字 `{ width: size, height: size, fontSize: size === 36 ? 12 : 11 }`；`preact@11.2.0` 已移除核心对数值样式自动补 `"px"` 的处理，WebKit 实际渲染丢失了 `width`/`height`/`fontSize`，导致所有账号头像从 `36×36` / `34×34` 塌陷成约 `16×14.4px` 小块且 34px 头像字号误用 `12px`。直接造成 `manage` / `manage-expired` 少 `-18pt`（`-36px@2x`）、`main-single` 少 `-6pt`（`-12px@2x`）。
     - **根因 B（CSS 行高与空态间距偏差）**：14 个 `430px` 限高 `main-*` 场景均差 `-2pt`（`-4px@2x`，`brand-header` 68 vs 69、`main-footer` 47 vs 48，源于 `line-height: 1.2` 与 AppKit `NSTextField` 固有行高差）；3 个 `settings*` 场景均差 `-4pt`（`-8px@2x`）；`main-empty` 多出 `+6pt`（`+12px@2x`，空态起点 `y=97` vs 基线 `y=90`）。
  4. **Windows `T4.1–T4.3` 代码走查与未闭合项**：
     - `T4.1`–`T4.3` 静态代码结构、ICO 多尺寸资源（`app.ico` 16/32/48/128/256、`tray-light/dark.ico` 16/20/24/32 均为 32bpp RGBA PNG）、`image-ico` 特性、四边任务栏定位与 DPI 换算、`WM_CLOSE` 多窗口判定及 `.incognito(true)` 独立登录窗口均对齐 [DESIGN.md](DESIGN.md) 与 [DEV_PLAN.md](DEV_PLAN.md)。
     - 两条非阻塞观察：(a) [windows.rs:37](../../crates/switchboard-platform/src/windows.rs#L37) `startup_disabled()` 仅匹配首字节 `3 | 7`（常见任务管理器禁用值），按位判断奇数禁用状态或 `!= 2 && != 6` 会更稳妥；(b) [panel.rs:178-181](../../src-tauri/src/windows/panel.rs#L178-L181) 在 `AUTO_OPEN=1` 首次 `show()` 时若 DOM 尚未挂载，`--panel-max-height` 由随后 `ResizeObserver -> resize -> position` 补写（可正常工作，但显式由 `initialization_script` 或 `resize` 保证更直接）。
     - **仍待后续验证**：`I-017`（L2 `crates/switchboard-platform` `platform-it` 集成测试仍 `0 tests`，L3 `components.test.tsx` 亦待补 `Avatar` 尺寸断言）与全部 Windows 真机运行态（`P-FS-02`、`P-AS-02/03`、`P-PR-01..05`、多 DPI/四边任务栏/托盘切换/InPrivate 登录/`Tests/baseline/windows/`）。

---

## Round 5 初测执行明细

| 检查项 | 命令 / 对象 | 结果 | 详情 |
|---|---|---|---|
| Rust 格式与 Clippy（macOS） | `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings` | ✅ PASS | 0 警告 / 0 错误 |
| Windows x64 交叉编译检查 | `cargo xwin clippy --workspace --all-targets --target x86_64-pc-windows-msvc --locked -- -D warnings` | ✅ PASS | Rust Clippy 0 警告（MSVC SDK/CRT 编译通过；未在 Windows 执行） |
| Rust 工作区单测与构建 | `cargo test --workspace --locked`、`cargo build --locked -p switchboard-app` | ✅ PASS（部分 0 用例） | `switchboard-core` **26/26 passed**；`switchboard-platform`、`switchboard-app`、`pen-probe` 仍为 **0 tests**（`I-017`） |
| 前端 TS + Vite 构建 | `npm run build:ui` | ✅ PASS | `tsc --noEmit` 与 `vite build` 成功 |
| L3 前端单测（Vitest） | `npm test`（`ui/src/tests/*.test.tsx`） | ✅ PASS | **2 文件 26/26 passed**（`app.test.tsx` 5 条、`components.test.tsx` 21 条；覆盖 `TEST_PLAN §5.1` 矩阵，缺 `Avatar` 像素尺寸断言） |
| 生成物一致性核验 | `npm run types:ipc && npm run scenarios:ui && git status --short` | ✅ PASS | `ui/src/generated/` 与 `ui/public/scenarios/`（22 场景）0 差异 |
| Swift 基线对照 | `./Tests/run.sh` | ✅ PASS | `UpdateLogicTests` `16/16` + `RegressionTests` `156/156`（`172/172`） |
| `T3.3` macOS 实机 E2E | `HOME=/tmp/sb-qa-home SWITCHBOARD_DEMO=1 SWITCHBOARD_AUTO_OPEN=1 ./target/debug/switchboard-app` + `lldb` / `WKWebView` | ✅ PASS | 验证 `MainScreen`(`h=545`) → `SettingsScreen`(`h=339`) → `ManageScreen`(`h=479`) 路由与高度跟随、`ConfirmDialog`(`inert=true`)、`UI-PN-03/04/05`、`quit_app`（退出码 `0`） |
| L4 WebKit 视觉回归（`T3.4`） | `npm run test:visual`（Playwright WebKit `v2388`，22 场景） | ❌ FAIL（`I-020`） | **1 passed**（`manage-confirm-remove` 跳过比对）/ **21 failed**（尺寸不符提前返回，未执行 `pixelmatch`）：14 个 `main-*` 差 `-4px`（`-2pt`）、3 个 `settings*` 差 `-8px`（`-4pt`）、`main-empty` 差 `+12px`（`+6pt`）、`main-single` 差 `-12px`（`-6pt`）、`manage`/`manage-expired` 差 `-36px`（`-18pt`）；且发现 `Avatar` 尺寸塌陷 |
| `T4.1–T4.3` Windows 代码与图标走查 | `crates/switchboard-platform/src/windows*`、`src-tauri/src/windows*`、`src-tauri/icons/*.ico`、`.github/workflows/test.yml` | ⚠️ 静态对齐，待真机 | 多尺寸 ICO 与 `image-ico` 有效，4 边任务栏/DPI/`WM_CLOSE`/`incognito(true)` 与设计一致；CI `macos-14` `visual` job 受 `I-020` 阻塞，Windows 真机测试与基线建立待运行 |

### L4 WebKit 视觉尺寸差异明细（`I-020`）

| 场景组 | 场景数 | 基线尺寸（`@2x`） | WebKit 实际（`@2x`） | 差值（物理 / 逻辑） | 根因定位 |
|---|---|---|---|---|---|
| 限高主面板（`main` 等 14 个） | 14 | `820×1094` / `1188` / `1238` | `820×1090` / `1184` / `1234` | `-4px`（`-2.0pt`） | 内容区均夹紧 `430pt`；`brand-header`（`68` vs `69`）与 `main-footer`（`47` vs `48`）受 `line-height: 1.2` 影响各少 `1pt` |
| `settings` / `settings-launch-on` / `settings-update-available` | 3 | `820×690` / `834` | `820×682` / `826` | `-8px`（`-4.0pt`） | `nav-header`、设置项双行标题与 `settings-footer` 行高略紧于 AppKit `NSTextField` |
| `main-single` | 1 | `820×690` | `820×678` | `-12px`（`-6.0pt`） | `Avatar size={36}` 尺寸被 Preact 11 丢弃塌陷（`16×14.4px`）+ `line-height: 1.2` 偏差 |
| `manage` / `manage-expired` | 2 | `820×1000` | `820×964` | `-36px`（`-18.0pt`） | 3 行账号 `Avatar size={34}` 均塌陷为 `16×14.4px`（占主要缩水）+ `line-height: 1.2` 偏差 |
| `main-empty` | 1 | `820×636` | `820×648` | `+12px`（`+6.0pt`） | WebKit 空态符号起止位置比基线低 `7pt`（`y=97` vs `y=90`），抵消页脚 `-1pt` 后净高 `+6pt` |
| `manage-confirm-remove` | 1 | `520×538` | `820×964` | 例外跳过 | `screens.spec.ts:41-44` 仅校验弹窗文案与按钮顺序（`I-002`） |

---

# 历史归档：Round 4（P2 `T2.1–T2.3` 运行态与 `I-019` 修复验收）

- **测试日期**：2026-11-14
- **被测分支 / 提交**：`t3-2-component-library` @ `724adc2` + 本地工作区未提交修复（`src-tauri/src/macos.rs`、`src-tauri/src/macos/panel.rs`、`ui/index.html`、`ui/src/components/components.css`）
- **测试环境**：macOS（Apple Silicon `aarch64-apple-darwin`，主屏 `1512×982`、`visibleFrame={{0, 75}, {1512, 874}}`）、Rust / Cargo、Node.js v24.22.2、Python 3.12 + Playwright Chromium 1254、Swift 测试基线、LLDB + AppKit/WKWebView + `CGWindowListCopyWindowInfo`
- **参考文档**：[DEV_PROGRESS.md](DEV_PROGRESS.md)、[TEST_PLAN.md](TEST_PLAN.md)、[DEV_PLAN.md](DEV_PLAN.md)、[DESIGN.md](DESIGN.md)、[ISSUES.md](ISSUES.md)

---

## Round 4 总结论

- **总体结论**：**P2 macOS 壳运行态与 P3 组件库渲染验收通过（`I-014`、`I-015`、`I-016`、`I-018`、`I-019` 全部修复并实机验证通过；余 `I-017` 待补充自动化单测及受环境限制项）** ✅⚠️
  1. **阻塞问题 `I-019` 已彻底解除**：移除 `panel.to_window()`、改用 `app.get_webview_window("panel")` 注册失焦事件后，`SwitchboardPanel` 在初始化后保持注册，`position` / `show` / `hide` / `hide_for_menu` / `show_if_hidden` / `toggle` / `resize` / `blur` 全部命中并真实生效。
  2. **P2 macOS 运行态核心交互实测全绿（隔离 `HOME=/tmp/sb-qa-home`）**：
     - **冷启动与进程属性（`M-01` / `M-15`）**：默认 `AUTO_OPEN=0` 启动隐藏（`ApplicationType="UIElement"`、`onScreen count=0`）；非 Demo 模式 `get_state` 返回 `{"demo":false,"count":0,"blocked":false,"version":"1.0.1"}`；`quit_app` 以退出码 `0` 正常退出。`SwitchboardPanel` 实测属性为 `level=25`、`collectionBehavior=0x101`（`CanJoinAllSpaces | FullScreenAuxiliary`）、`styleMask=0x80`（`NonactivatingPanel`）、`isFloatingPanel=1`、`hidesOnDeactivate=0`、`hasShadow=1`、`isOpaque=0`。
     - **定位与高度夹取（`UI-PN-01` / `I-016`）**：初始挂载 `set_panel_height(68)` 夹取至最小高度 `200`（`topGap=12.0`）；`set_panel_height(900)` 在可用高 `874px` 屏幕下正确夹取至 `858px`（`frame={{1090, 83}, {410, 858}}`，上下各留 `8.0px`）。
     - **托盘防抖、失焦宽限与右键菜单（`UI-PN-02..04` / `I-015`）**：左键托盘在 `95ms` 内二次点击不收起（350ms 防抖），`590ms` 后再点击正常收起；显示后 `>300ms` 及 `<300ms`（`50ms` 内）切前台应用均触发失焦重试并收起；`set_confirmation_open(true)` 时 `Escape` 与切应用失焦均保持 `VIS=1`，而右键托盘触发 `hide_for_menu` 立即收起（`VIS=0`）；`set_confirmation_open(false)` 后 `Escape` 正常收起。
     - **`Reopen` 不重复重置与 Demo IPC（`I-016` / `M-17` / `M-18`）**：面板隐藏时 `Reopen` 呼出面板并派发 `panel-shown`（计数 `0 -> 1`），已可见时再次 `Reopen` 不再触发 `show`（计数保持 `1`）；Demo 模式 `WKWebView` 收到 `state`（3 个演示账号），触发 `add_account` 收到 `feedback.text = "演示模式不操作真实账号"`。
  3. **仍待补充与环境受限边界**：
     - **`I-017`（未闭合）**：`crates/switchboard-platform` 尚未补充 `platform-it` L2 集成测试（仍 `0 tests`），`ui/` 尚未配置 Vitest 组件单测。
     - **环境限制（未做物理点击/多屏实测）**：测试机当前状态栏图标溢出屏幕左侧（`NSStatusBarWindow` `frame={{-2575, 949}, {34, 33}}`）且为单屏环境，故本次实测了“锚点不可见回退右上角（`x=1090, topGap=12`）”，**可见托盘图标居中对齐**与**多屏拖动（`M-12`）**仅通过代码走查核对；另本终端无 Screen Recording 权限（`screencapture -l` 报错），窗口与 UI 验证通过 `CGWindowListCopyWindowInfo`、`lldb` 断点追踪及 `WKWebView evaluateJavaScript` 完成。
     - **按计划延后项**：`T3.3` 三页面集成、`T3.4` Playwright WebKit 走查、P4 Windows 与 P5 安装器仍按 [DEV_PROGRESS.md](DEV_PROGRESS.md) 属后续阶段。

---

## Round 4 执行明细

| 检查项 | 命令 / 测试路径 | 结果 | 详情 |
|---|---|---|---|
| 静态检查 / 构建 / Core 单测 / Swift 基线 | `cargo fmt --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --locked`、`cargo build --locked -p switchboard-app`、`./Tests/run.sh` | ✅ PASS | `fmt` / `clippy` 0 警告；`switchboard-core` `26/26 passed`；Swift 基线 `172/172`（`platform`/`app`/`pen-probe` 仍 `0 tests`，见 `I-017`） |
| 前端构建 / 生成物 / Chromium 22 场景 | `npm run build:ui`、`npm run types:ipc && npm run scenarios:ui`、Playwright Chromium 410×900 @2x | ✅ PASS | 0 差异、0 控制台/HTTP 错误；`I-018` 实心黑底白勾与禁用按钮 `opacity=1`、`brand.png` favicon 已验证 |
| 隔离非 Demo 启动与退出（`M-01` / `UI-PN-06`） | `HOME=/tmp/sb-qa-home ./target/debug/switchboard-app`（无 `SWITCHBOARD_DEMO`） | ✅ PASS | `AUTO_OPEN=0` 保持隐藏（`ApplicationType="UIElement"`、`onScreen=0`）；`get_state` 返回 `{"demo":false,"count":0,"blocked":false,"version":"1.0.1"}`；`quit_app` 退出码 `0` |
| `I-019` 面板注册与窗口属性（`M-15` / `M-18`） | `HOME=/tmp/sb-qa-home SWITCHBOARD_DEMO=1 SWITCHBOARD_AUTO_OPEN=1` + `lldb` / `WKWebView` | ✅ PASS | `SwitchboardPanel` `visible=1, key=1, level=25, onScreen=1, collectionBehavior=0x101, styleMask=0x80, isFloatingPanel=1, hidesOnDeactivate=0, hasShadow=1, isOpaque=0`；`add_account` 返回 `"演示模式不操作真实账号"` |
| 定位回退与高度夹取（`UI-PN-01` / `I-016`） | `set_panel_height(68)` 与 `set_panel_height(900)`（可用高 `874px`） | ✅ PASS | `68` 夹取为 `200`（`topGap=12.0`）；`900` 夹取为 `858`（`frame={{1090, 83}, {410, 858}}`，上下留白各 `8.0`） |
| 托盘左键切换与 350ms 防抖（`UI-PN-02`） | `NSStatusBarWindow` 合成 `LeftMouseDown`+`LeftMouseUp`（`NSTimer` 主循环追踪） | ✅ PASS | `toggle @ 5.721` -> `show @ 5.732`；`+95ms`（`5.827`）二次点击保持显示；`+590ms` 三次点击收起（`VIS=0 KEY=0`） |
| 失焦宽限、锁保护与右键菜单（`UI-PN-03..04` / `I-015`） | 切换 `TextEdit` / `Microsoft Edge`、`set_confirmation_open(true/false)`、右键托盘 | ✅ PASS | `>300ms` 及 `<300ms`（`50ms` 内）切应用均收起；`confirmation_open=true` 时 `Escape` 与切应用保持 `VIS=1`，右键托盘执行 `hide_for_menu` 立即收起（`VIS=0`）；取消锁后 `Escape` 收起 |
| `Reopen` 去重（`I-016` / `M-17`） | `applicationShouldHandleReopen` + 监听 `panel-shown` | ✅ PASS | 隐藏时触发打开并使 `panel-shown` 计数 `0 -> 1`；已可见时再次触发保持 `VIS=1` 且计数仍为 `1` |
| 自动化单测缺口（`I-017`） | `crates/switchboard-platform`、`ui/` | ❌ 待补充 | L2 `platform-it`（`P-FS-01/03`, `P-AS-01`）与 L3 Vitest 单测仍未编写 |

---

# 历史归档：Round 3（P2 `T2.1–T2.3` + P3 `T3.1–T3.2` 工作区修复初复测）

- **测试日期**：2026-11-14
- **被测分支 / 提交**：`t3-2-component-library` @ `724adc2` + 本地工作区未提交修复（`src-tauri/src/macos.rs`、`src-tauri/src/macos/panel.rs`、`ui/index.html`、`ui/src/components/components.css`）
- **测试环境**：macOS（Apple Silicon `aarch64-apple-darwin`）、Rust / Cargo、Node.js v24.22.2、Python 3.12 + Playwright Chromium 1254（本机未安装 Playwright WebKit 二进制）、Swift 测试基线、LLDB / CoreGraphics 窗口检查
- **参考文档**：[DEV_PROGRESS.md](DEV_PROGRESS.md)、[TEST_PLAN.md](TEST_PLAN.md)、[DEV_PLAN.md](DEV_PLAN.md)、[DESIGN.md](DESIGN.md)、[ISSUES.md](ISSUES.md)

---

## Round 3 总结论

- **总体结论**：**不通过（`I-014` / `I-018` 已修复，新发现阻塞缺陷 `I-019`）** ❌
  1. **构建、静态检查、L1 / Swift 基线与资产生成全绿**：`cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --locked`（`switchboard-core` `26/26`）、Swift `./Tests/run.sh`（`16/16` + `156/156 = 172/172`）、`npm run build:ui`、`npm run types:ipc && npm run scenarios:ui`（0 差异）均通过。
  2. **`I-018` 前端视觉微调已修复并通过复测**：Playwright Chromium（410×900 @2x）验证 `CurrentAccountCard` 勾选图标为实心 `#1C1C1F` 底 + `#FAFAFA` 白勾、`UpdateBanner` 下载中禁用按钮 `opacity = 1`、`ui/index.html` 使用 `/brand.png` favicon，全 22 场景与默认路由 **0** 控制台/HTTP 404 报错。
  3. **`I-014` 启动 `SIGABRT` 已解除，但新发现 `I-019`（🔴 阻塞：`panel.to_window()` 注销面板句柄）**：
     - 在隔离 `HOME=/tmp/sb-qa-home SWITCHBOARD_DEMO=1 SWITCHBOARD_AUTO_OPEN=1` 下，`./target/debug/switchboard-app` 已不再触发 `SIGABRT` 崩溃，进程以 `ActivationPolicy::Accessory`（`lsappinfo`: `ApplicationType="UIElement"`）稳定驻留，WebView 正常加载 `http://127.0.0.1:1420/` 并通过 IPC 调用 `set_panel_height(68)`。
     - **但 `src-tauri/src/macos/panel.rs:63` 为挂载 `on_window_event` 调用了 `panel.to_window()`**：经核对 `tauri-nspanel` 2.1.0 源码（`panel.rs:360-395`）并用 `lldb` + `CGWindowListCopyWindowInfo` 实机验证，`to_window()` 会执行 `remove_webview_panel("panel")` 并通过 `object_setClass` 将其退化回普通 `NSWindow`。结果所有后续 `app.get_webview_panel("panel")` 均返回 `Err(PanelNotFound)` 并静默跳过，`panel::show`（`panel.rs:167`）与 `panel::position`（`panel.rs:148`）从未执行，`NSApp.windows` 中面板始终处于 `visible=0`、`onScreen=nil`、默认居中 frame `{{551, 430}, {410, 400}}`（未显示、未定位、未响应缩放与托盘交互）。
  4. **仍待后续补充/解阻项**：`I-015`、`I-016` 及 P2 真机交互（`UI-PN-01..06`、`M-01/15/17/18`）仍受 `I-019` 阻塞；`I-017`（L2 `switchboard-platform` `platform-it` 与 L3 `ui/` Vitest 单测）尚未补充。

---

## Round 3 执行明细

| 检查项 | 命令 / 对象 | 结果 | 详情 |
|---|---|---|---|
| 代码格式检查 | `cargo fmt --all --check` | ✅ PASS | 退出码 `0` |
| Workspace Clippy | `cargo clippy --workspace --all-targets --locked -- -D warnings` | ✅ PASS | 4 个 member 0 warning / 0 error |
| Workspace 单元测试 | `cargo test --workspace --locked` | ✅ PASS（部分 0 用例） | `switchboard-core` `26/26 passed`；`switchboard-platform`、`switchboard-app`、`pen-probe` 仍为 `0 tests`（`I-017`） |
| Swift 基线对照 | `./Tests/run.sh` | ✅ PASS | `UpdateLogicTests` `16/16` + `RegressionTests` `156/156`（`172/172`） |
| 前端 TS + Vite 构建 | `npm run build:ui` | ✅ PASS | `tsc --noEmit` 与 `vite build`（176ms）成功 |
| 生成物一致性核验 | `npm run types:ipc && npm run scenarios:ui` | ✅ PASS | `ui/src/generated/` 与 `ui/public/scenarios/` 与仓库 0 差异 |
| 前端 22 场景 + `I-018` 复测 | Playwright Chromium（410×900 @2x） | ✅ PASS | 全 22 场景 + 默认页 + `I-018`（`.current-check` `circleFill=rgb(28, 28, 31)` / `pathStroke=rgb(250, 250, 250)`；下载禁用按钮 `opacity=1`；`/brand.png` favicon），**0** 控制台/HTTP 错误 |
| 实机壳启动与 IPC 连通 | `HOME=/tmp/sb-qa-home SWITCHBOARD_DEMO=1 SWITCHBOARD_AUTO_OPEN=1 ./target/debug/switchboard-app` | ⚠️ 部分通过（`I-014` ✅ / `I-019` ❌） | 进程无崩溃、`ApplicationType="UIElement"`、WebView 加载 `127.0.0.1:1420` 并触发 `panel::resize(68 -> 200)`；但因 `panel.to_window()` 注销面板（`I-019`），`position`/`show` 均未命中，`visible=0`、`onScreen=nil`、frame 停留 `{{551, 430}, {410, 400}}` |
| P2 面板定位/失焦/托盘/热键 | `UI-PN-01..06`、`M-01/15/17/18`、`I-015`、`I-016` | ❌ 阻塞（`I-019`） | 面板无法显示且无法通过 `get_webview_panel` 控制，待修复 `I-019` 后继续实机复测 |

---

# 历史归档：Round 2（P2 `T2.1–T2.3` + P3 `T3.1–T3.2` 初测）

- **测试日期**：2026-11-13
- **被测分支 / 提交**：`t3-2-component-library` @ `724adc2`（基于 `origin/main` @ `78c02d2`，顺序包含 `4ebb443` T2.1、`142dbaf` T2.2、`678ceb7` T2.3、`f0c7fc3` T3.1、`724adc2` T3.2）
- **测试环境**：macOS（Apple Silicon `aarch64-apple-darwin`）、Rust / Cargo、Node.js v24.22.2、Python 3.12 + Playwright Chromium 1254（本机未安装 Playwright WebKit 二进制）、Swift 测试基线
- **参考文档**：[DEV_PROGRESS.md](DEV_PROGRESS.md)、[TEST_PLAN.md](TEST_PLAN.md)、[DEV_PLAN.md](DEV_PLAN.md)、[DESIGN.md](DESIGN.md)、[ISSUES.md](ISSUES.md)

---

## Round 2 总结论（`T2.1`–`T3.2`）

- **总体结论**：**不通过（阻塞于 `I-014`）** ❌
  1. **构建、静态检查、L1 与资产导出全部通过**：`cargo fmt --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --locked`（`switchboard-core` 26/26 / 202 条断言）、Swift 基线 `./Tests/run.sh`（172/172）、`npm run build:ui`（TS 严格检查 + Vite 打包）、`npm run types:ipc` 与 `npm run scenarios:ui`（0 差异可重现）均全绿。
  2. **P2 macOS 应用壳（`T2.2`）启动即崩溃（`I-014` 🔴 阻塞）**：实机在隔离 `HOME=/tmp/sb-qa-home` + `SWITCHBOARD_DEMO=1 SWITCHBOARD_AUTO_OPEN=1` 启动 `./target/debug/switchboard-app` 立即以 `SIGABRT`（退出码 `134`）崩溃，原因为 `src-tauri/src/macos/panel.rs` 未在 `with_window` 设置 `.decorations(false)` 即替换 `style_mask`，触发 AppKit `NSInternalInconsistencyException`（`WKWindowVisibilityObserver` / `contentLayoutRect`）。受此阻塞，**`T2.1`/`T2.2`/`T2.3` 的全部真机运行态验收（托盘、`NSPanel`、`UI-PN-01..06`、`M-01/15/17/18`、IPC 权限/事件、独立登录窗口、`SMAppService`）均无法开展**；另静态走查发现失焦/右键菜单/重开/高度夹取问题（`I-015`、`I-016`）。
  3. **P3 前端骨架与组件库（`T3.1`–`T3.2`）静态与浏览器布局走查基本通过，余缺单测与视觉微调**：通过 Chromium（410×900 @2x）实渲 `/?mock=<scenario>&gallery` 全 22 个场景，主题变量、`operationLocks`、22 场景 JSON、图标/尺寸/溢出、`prefers-reduced-motion` 与 `stableHash` 均与 Swift 高度一致；存在空心勾选图标、下载禁用按钮淡化与 dev `/favicon.ico` 404（`I-018`），以及缺少 L2 `platform-it` / L3 Vitest 用例（`I-017`）。
  4. **预期边界（不计为本轮缺陷）**：`T3.3` 三页面路由/确认弹窗与 `P5` 原生更新安装器（`update_repo = ""`）按 [DEV_PROGRESS.md](DEV_PROGRESS.md) 约定暂未接入；`P4` Windows 实现尚未开始。

---

## Round 2 执行明细

### R2.1 构建、Lint、Core 与生成物校验

| 检查项 | 命令 | 结果 | 详情 |
|---|---|---|---|
| 代码格式检查 | `cargo fmt --all --check` | ✅ PASS | 退出码 `0` |
| Workspace Clippy | `cargo clippy --workspace --all-targets --locked -- -D warnings` | ✅ PASS | 4 个 member（含 `switchboard-platform`、`switchboard-app`）0 warning / 0 error |
| Workspace 单元测试 | `cargo test --workspace --locked` | ✅ PASS（部分 0 用例） | `switchboard-core` `26/26 passed`；`switchboard-platform` **0 tests**、`switchboard-app` **0 tests**、`pen-probe` **0 tests** |
| Swift 基线对照 | `./Tests/run.sh` | ✅ PASS | `UpdateLogicTests` `16/16` + `RegressionTests` `156/156` |
| 前端 TS + Vite 构建 | `npm run build:ui` | ✅ PASS | `tsc --noEmit`（strict）与 `vite build` 成功（161ms） |
| 生成物一致性核验 | `npm run types:ipc && npm run scenarios:ui && git status --short` | ✅ PASS | `ui/src/generated/*.ts`（9 个文件）与 `ui/public/scenarios/*.json`（22 个文件）均与仓库 **0 差异** |
| macOS 壳二进制编译 | `cargo build --locked -p switchboard-app` | ✅ PASS | 产物 `./target/debug/switchboard-app` 编译成功 |

### R2.2 P2 macOS 平台层与 Tauri 壳（`T2.1`–`T2.3`）

| 检查项 | 覆盖范围 | 结果 | 详情 |
|---|---|---|---|
| 隔离环境真机冒烟启动 | `HOME=/tmp/sb-qa-home SWITCHBOARD_DEMO=1 SWITCHBOARD_AUTO_OPEN=1 ./target/debug/switchboard-app` | ❌ FAIL（`I-014`） | 进程启动即 panic 并 `SIGABRT`（退出码 `134`）：`AppKit rejected the panel style mask: Cannot remove an observer <WKWindowVisibilityObserver> for the key path "contentLayoutRect" from <SwitchboardPanel> because it is not registered as an observer` |
| 托盘/面板交互与 `UI-PN-01..06` / `M-01,15,17,18` | `src-tauri/src/macos.rs`、`panel.rs` | ❌ 阻塞 / 待修复（`I-014`–`I-016`） | 因启动崩溃无法实测；静态对照 `AppMain.swift` 发现失焦 0.3s 宽限无重试、右键托盘菜单不收起面板、悬停鼠标 Cmd+Tab 不收起（`I-015`），以及 `Reopen` 无条件重置路由、小屏未限制最大高度（`I-016`） |
| IPC 契约与权限 | `core/src/ipc.rs`、`src-tauri/src/macos/ipc.rs`、`capabilities/panel.json`、`ui/src/ipc.ts` | ⚠️ 代码对齐，待运行态验证 | DESIGN §5.9 全部 16 个命令 + `set_confirmation_open` 名称、参数与 `ts-rs` 类型一一对齐；`panel.json` 含 `core:event:allow-listen`/`unlisten`，待 `I-014` 修复后在真机 WebView 验证实际 `invoke`/`listen` 连通 |
| 平台层 `MacPlatform` & `MacAuthWindow` | `crates/switchboard-platform/src/macos.rs`, `macos/auth.rs` | ⚠️ 静态对齐，缺 L2 用例（`I-017`） | 数据目录、bundle id（`dev.pencil.desktop` / `dev.switchboard.Switchboard`）、`SMAppService`、`ASWebAuthenticationSession(prefersEphemeralWebBrowserSession=true)`、`0700`/`0600` 与设计一致；但无 `platform-it` 测试（`P-FS-01`、`P-FS-03`、`P-AS-01`）且依赖修复 `I-014` 后实机测 |

### R2.3 P3 前端骨架与组件库（`T3.1`–`T3.2`）

| 检查项 | 覆盖范围 | 结果 | 详情 |
|---|---|---|---|
| 主题 CSS 与资源 | `ui/src/theme.css`、`ui/public/brand.png`、`src-tauri/icons/tray.png` | ✅ PASS | 46 项色值/字号/字体栈与 `AppKitCommon.swift` `Theme` 及 `DESIGN §5.7` 逐一一致；`brand.png`（128×128）与 `tray.png`（36×36 单色模板）视觉清晰 |
| 全 22 场景浏览渲染（Chromium 410×900 @2x） | `http://127.0.0.1:1420/?mock=<name>&gallery` 与默认 `/?mock=main` | ✅ PASS（含微调建议 `I-018`） | 22 个场景均加载无 JS 异常；`.panel` 宽 410px、无横向溢出（`scrollWidth <= 410`）；`StatusBanner`（47/35px）与 `UpdateBanner`（72/60px）高度贴合无裁切 |
| 操作锁规则表 | `operationLocks`（`main` / `refreshing` / `switching` / `adding` / `downloading` / `ready` / `persistenceBlocked`） | ✅ PASS | 浏览器 DOM 验证与 `PanelController.syncOperationLock` 完全一致；`adding` 时「取消网页登录」仅在刷新/切换中禁用 |
| 可访问性与动效 | `InkSwitch`、`RefreshButton`、`AuthBadge`、`LinkButton` | ✅ PASS | `role="switch"` + `aria-checked` 正确；`prefers-reduced-motion: reduce` 下旋转图标 `animationName === "none"`；`LinkButton` 色值 `#27272A`（`rgb(39, 39, 42)`）已对齐 `I-001` 确认方案 |
| 头像哈希配色 | `stableHash`（`alex.chen@gmail.com` 等） | ✅ PASS | 32-bit 位运算与 Swift `(hash &* 33 &+ byte) & 0x7FFFFFFF` 一致，实渲调色板对齐 |
| 组件自动化单测（L3） | [TEST_PLAN.md §5.1](TEST_PLAN.md#L184-L199) | ❌ 缺失（`I-017`） | `ui/` 尚未配置 Vitest 及组件单元测试用例 |

---

## Round 2 整改与下一步门禁建议

1. **立即修复 `I-014`（阻塞）**：在 [panel.rs](file:///Users/apple/Documents/pen_switch_account/src-tauri/src/macos/panel.rs#L48-L55) 的 `with_window` 中显式设置 `.decorations(false)`（并使用 `StyleMask::empty().nonactivating_panel()`），然后重新验证 `./target/debug/switchboard-app` 在 `SWITCHBOARD_DEMO=1 SWITCHBOARD_AUTO_OPEN=1` 下可正常启动、显示托盘与 `NSPanel`。
2. **修复 `I-015` / 确认 `I-016`**：改用全局/本地事件监听或修正 `blur()` 宽限期与右键菜单收起逻辑，并在修复 `I-014` 后完成 P2 面板实机验证（`UI-PN-01..06`、`M-01/15/17/18`）。
3. **补齐测试与视觉微调（`I-017`、`I-018`）**：补充 `switchboard-platform` L2 测试与 `ui/` Vitest 组件单测；确认 `CurrentAccountCard` 勾选图标实心填充及 `UpdateBanner` 禁用态对比度后进入 `T3.3` / `T3.4`（Playwright WebKit）。

---

# 历史归档：P0–P1 阶段测试报告（分支 `t1-7-app-model`）

- **测试日期**：2026-11-13
- **被测分支 / 提交**：`t1-7-app-model` @ `194577e`（包含 `T0.2–T0.4`、`T1.1–T1.7`；对照基线：`main` @ `0f2ab7d` / `874dc34`）
- **测试环境**：macOS（Apple Silicon `aarch64-apple-darwin`）、Rust / Cargo、Swift、Node.js v20+、本机 `/Applications/Pen.app`（Pen `1.2.23`）
- **参考文档**：[TEST_PLAN.md](TEST_PLAN.md)、[DEV_PLAN.md](DEV_PLAN.md)、[DESIGN.md](DESIGN.md)、[ISSUES.md](ISSUES.md)

---

## 1. 总结论

- **初测状态（`194577e`）**：不通过（已触发修复 `I-005` ~ `I-012`）。
- **复测状态（2026-11-13 本地工作区第二轮修复 + L1 用例全量验证）**：**macOS 侧 P1 核心门禁已通过（`26/26` tests / `202` 条静态断言，超过 Swift 基线 `172` 条；`I-005` ~ `I-010`、`I-012`、`I-013` 已全部在 macOS 验证通过）**，详见[第 5 节复测记录](#5-%E5%A4%8D%E6%B5%8B%E8%AE%B0%E5%BD%95%E5%B7%A5%E4%BD%9C%E5%8C%BA%E4%BF%AE%E5%A4%8D--l1-%E7%94%A8%E4%BE%8B%E8%90%BD%E5%9C%B0)。仅余：
  1. **Windows CI / 实机验证**：`pen-probe` Win 目标 Clippy 已 0 警告通过（`I-005`、`I-011`），`switchboard-core` Win 单测、L5 `pen-probe` 实跑与 `C-QU-09` 需提交后在 GitHub Actions / Win11 ARM VM 验证。
  2. **P5 平台更新器范围**：`C-UP-06`、`C-UP-14`、`C-UP-15` 属 `UpdateInstaller`（P5 `T5.1`）任务范围，将在 P5 阶段实现并补测。

---

## 2. 分层测试执行明细

### 2.1 对照基准：Swift 1.0.1 现有测试

| 检查项 | 命令 | 结果 | 详情 |
|---|---|---|---|
| 更新逻辑单测 | `./Tests/run.sh` (`UpdateLogicTests`) | ✅ PASS | **16/16 checks passed** |
| 回归测试 | `./Tests/run.sh` (`RegressionTests`) | ✅ PASS | **156/156 checks passed** |
| **合计下限** | — | — | **172** 条断言（作为 L1 移植断言下限参考） |

### 2.2 L1 Core 单测 & L2 平台集成（`crates/switchboard-core` / `switchboard-platform`）

| 检查项 | 命令 / 对象 | 结果 | 详情 |
|---|---|---|---|
| 代码格式检查 | `cargo fmt --all --check` | ✅ PASS | 退出码 `0` |
| macOS Clippy 检查 | `cargo clippy --workspace --all-targets --locked -- -D warnings` | ❌ FAIL | 3 个 error：`crates/switchboard-core/src/store.rs:39` (`clippy::never_loop`)、`crates/switchboard-core/src/update.rs:180,184` (`clippy::unnecessary_map_or`) |
| Workspace 单元测试 | `cargo test --workspace --locked` | ❌ FAIL（缺用例） | 编过可运行，但 `switchboard-core`、`switchboard-platform`、`pen-probe` 均 **0 passed; 0 failed** |
| 测试替身与夹具 | [TEST_PLAN.md §3.1, §3.3](TEST_PLAN.md#L33-L41) | ❌ 缺失 | 未创建 `crates/switchboard-core/tests/`，无 `MemoryCredentialStore`、`FakeTarget`、`FakeQuota`、`FakeWebLogin`、`FakeFeed`/`FakeTransport`/`FakeInstaller` 及 `fixtures/v1.0.1/` |
| `deviceId` 一致性（mac） | 用例 `C-QU-08` | ✅ PASS | Swift `PenQuotaProvider.deviceId()` 与 Rust/Node `md5(hostname + "darwin" + "arm64")` 均输出 `d0d90555d378a6c6747bcb047202af72` |
| 22 个快照场景导出 | `cargo run -p switchboard-core --bin snapshot -- <scenario>` | ✅ PASS | `SnapshotScenarios.swift` 同名 22 个场景全部输出合法 `Snapshot` JSON |
| 1.0.1 `store.json` 往返 | 用例 `C-ST-08` | ⚠️ 有差异 | Swift `JSONEncoder` 整秒时间戳输出整数（`816229440`）且键序 `accounts` 在前；Rust `f64` 输出 `816229440.0` 且 `storeVersion` 在前。语义一致但字节不完全一致 |
| Win 交叉编译（本机） | `cargo check --workspace --target x86_64-pc-windows-msvc` | ⚠️ 环境受限 | `ring` 编译缺少 Windows C CRT 头文件（`assert.h`），需在 Windows CI 验证 |

**L1 用例覆盖状态矩阵（对应 TEST_PLAN 第 3.2 节）**：

| 模块 | 用例范围 | 自动化测试状态 | 备注 |
|---|---|---|---|
| 身份同步 | `C-ID-01` ~ `C-ID-03` | ❌ 未编写 | `sync_identity`、`emails_match` 逻辑已实现 |
| 切换 | `C-SW-01` ~ `C-SW-12` | ❌ 未编写 | `switch_account`、`SoftVerify` 逻辑已实现 |
| 添加 / 导入 / 移除 | `C-AC-01` ~ `C-AC-07` | ❌ 未编写 | 发现 `C-AC-03` 取消机制与 `C-AC-05` 反馈覆盖问题（见第 3 节） |
| 存储与损坏保护 | `C-ST-01` ~ `C-ST-09` | ❌ 未编写 | `Store::load` 备份逻辑已实现；`C-ST-08` 缺夹具且存在字节差异 |
| 凭据文件 | `C-CR-01` ~ `C-CR-05` | ❌ 未编写 | `FileCredentialStore` 与 `atomic_write` 已实现 |
| 额度 | `C-QU-01` ~ `C-QU-09` | ❌ 未编写 | `C-QU-08` 手工核对一致；`C-QU-09` 需在 Windows CI 跑 |
| 网页登录 | `C-WL-01` ~ `C-WL-05` | ❌ 未编写 | 发现 `WebLogin::sign_in_inner` 并发缺陷（见第 3 节） |
| 更新 | `C-UP-01` ~ `C-UP-15` | ❌ 未编写 | 发现 `UpdateSchedule` 与验签未接入 `AppModel`（见第 3 节） |

### 2.3 L3 / L4 / L6 / L7（未覆盖）

- **L3 前端组件 / L4 视觉回归 / 面板行为**：`ui/`、`src-tauri/` 尚未建库（属 P2–P3 任务），本次仅验证了 core 侧的 22 个快照 JSON 注入接口。
- **L6 手工验收 / L7 升级演练**：尚未进入 P2–P5，不具备端到端运行条件。

### 2.4 L5 Pen 兼容性（T0.2–T0.4）

| 检查项 | 命令 / 对象 | 结果 | 详情 |
|---|---|---|---|
| Pen 静态兼容检查（正测） | `node scripts/check_pen_compat.mjs /Applications/Pen.app/Contents/Resources/app.asar` | ✅ PASS | Pen `1.2.23` 全部 7 项检查通过（`CONFIG_FOLDER`、`session filename`、`device ID`、`clientHost`、`non-macOS close quits`、`single instance lock`、`Pen tray`） |
| Pen 静态兼容检查（变异） | 篡改 `out/desktop-app-host.js` 中 `clientHost` | ✅ PASS | 退出码 `1`，准确报 `FAIL clientHost` 并打印代码片段 |
| `pen-probe` 非 Win 保护 | `cargo run -p pen-probe -- hostname` | ✅ PASS | 返回 `{"error":"pen-probe only supports Windows"}` 并以非零退出 |
| `pen-probe` Win Clippy | `cargo clippy -p pen-probe --target x86_64-pc-windows-msvc -- -D warnings` | ❌ FAIL | 3 个 error：`tools/pen-probe/src/windows.rs:289,290,295` |
| `pen-compat.yml` 工作流 | [.github/workflows/pen-compat.yml](../../.github/workflows/pen-compat.yml) | ⚠️ 待 CI 实跑 | YAML 逻辑与 `DEV_PLAN T0.4` 对齐，但尚未合入默认分支并在 GitHub Actions 上实跑 |

---

## 3. 发现的问题汇总（同步登记于 `ISSUES.md`）

| 编号 | 严重程度 | 关联用例 / 任务 | 位置 | 问题描述与影响 |
|---|---|---|---|---|
| **I-005** | 🔴 阻塞 | T1.1 / `test.yml` | `crates/switchboard-core/src/store.rs:39`、`update.rs:180,184`、`tools/pen-probe/src/windows.rs:289,290,295` | `cargo clippy -- -D warnings` 在 macOS 与 Windows 目标各报 3 个错，CI 直接失败 |
| **I-006** | 🔴 阻塞 | TEST_PLAN §3（P1 门槛） | `crates/switchboard-core/` | 缺少 L1 全部单元测试（0 tests）、5 个测试替身以及 `tests/fixtures/v1.0.1/`，远低于 Swift 版 172 条断言门槛 |
| **I-007** | 🔴 高危 | `C-AC-03` / T1.5 | `crates/switchboard-core/src/web_login.rs:120-122` | `self.window.open(url).await?` 在 `listener.accept().await` 之前串行执行：若 `open` 等待关窗会直接死锁，若 `open` 立即返回则用户关窗无法取消登录监听（只能等 300s 超时） |
| **I-008** | 🟠 重要 | `C-UP-04/05/08` / T1.6, T1.7 | `crates/switchboard-core/src/app_model.rs:733-779` | `UpdateSchedule`（24h 节流与 1h 失败退避）、`fetch_latest`、`download`、`verify_signature` 未由 `AppModel` 调用，而是整块委托给 `AppServices`，core 层无法验证“未验签不解压”和“自动检查退避” |
| **I-009** | 🟡 一般 | `C-AC-05` / T1.7 | `crates/switchboard-core/src/app_model.rs:534-535` | `importCurrentLogin` 在 `save_login(..., false)` 中先设「已添加账号 / 登录状态已更新」反馈，紧接着同步调用 `begin_refresh()` 立刻覆写为「正在刷新额度…」，前一条成功提示在 `publish()` 前即被吞掉 |
| **I-010** | 🟡 一般 | `C-ST-08` / T1.2 | `crates/switchboard-core/src/model.rs:40` | Rust `f64` 将整数时间戳序列化为 `816229440.0` 且字段顺序与 Swift `JSONEncoder` 不同，导致 1.0.1 `store.json` 读写往返语义一致但**非字节级一致** |
| **I-011** | 🟡 一般 | `P-PR-02` / T0.3 | `tools/pen-probe/src/windows.rs:92-97` | `processes()` 遍历进程快照时若某 `Pen.exe` 刚退出导致 `OpenProcess` 返回 `NULL`，会直接使整个函数报错，进而打断 `quit()` 的轮询等待 |
| **I-012** | 🟡 一般 | Git 仓库卫生 | `p0-baseline-compat` (`8b8ef32`) | `p0-baseline-compat` 分支误提交了 `node_modules/` 与 `target/` 共 146 个文件（`origin/main` @ `874dc34` 未受污染，建议清理远端 `p0-baseline-compat`） |

---

## 4. 阶段门槛评估与整改建议

对照 [TEST_PLAN.md §8](TEST_PLAN.md#L275-L286)：

- **P0 门槛**：T0.1 基线 22 张图已入库 ✅；T0.2 脚本验证通过 ✅；但 **`pen-compat.yml` 尚未在 CI 上全绿跑通** ❌、**`pen-probe` 存在 Windows Clippy 报错** ❌、**Windows 11 ARM 虚拟机（T0.5）尚未记录到 `ISSUES.md`** ⚠️。
- **P1 门槛**：**未通过** ❌（缺 L1 单测、Clippy 失败、存在 `I-007`/`I-008` 架构偏差）。

**建议整改顺序**：
1. 清理 `origin/p0-baseline-compat` 上的 `8b8ef32` 误提交；合入 `pen-compat.yml` 并修复 `pen-probe` Clippy 错误后，在 GitHub Actions 手动触发运行一次 `pen-compat.yml` 完成 P0 闭环。
2. 修复 `store.rs` 与 `update.rs` 的 Clippy 错误。
3. 重构 `web_login.rs` 的 `AuthWindow` 取消协作模型（与 `listener.accept()` 并发 `select!`），并将 `UpdateSchedule` 与验签流程收口到 `AppModel`。
4. 在 `crates/switchboard-core/tests/` 补齐 `v1.0.1` 夹具及 `TEST_PLAN.md §3` 全部 `C-*` 用例，确保 `cargo test --workspace` 全绿且断言总数达到或超过 Swift 版（≥ 172）。

---

## 5. 复测记录（工作区修复 + L1 用例落地）

> 复测时间：2026-11-13；范围：`t1-7-app-model` 本地工作区未提交修复（`store.rs`、`update.rs`、`web_login.rs`、`app_model.rs`、`tools/pen-probe/src/windows.rs`）、远端 `origin/p0-baseline-compat` @ `57519d5`，以及 QA 补齐的 `crates/switchboard-core/tests/`。

### 5.1 工具链与命令复测结果

| 检查项 | 命令 | 复测结果 | 说明 |
|---|---|---|---|
| 代码格式检查 | `cargo fmt --all --check` | ✅ PASS | 0 差异 |
| macOS 全工作区 Clippy | `cargo clippy --workspace --all-targets --locked -- -D warnings` | ✅ PASS | **0 warning / 0 error**（`I-005` 已清） |
| Windows `pen-probe` Clippy | `cargo clippy -p pen-probe --all-targets --locked --target x86_64-pc-windows-msvc -- -D warnings` | ✅ PASS | **0 warning / 0 error**（`I-005` / `I-011` Win 目标编译通过） |
| Workspace 单元测试 | `cargo test --workspace --locked -- --nocapture` | ✅ PASS | `core_tests` **26/26 passed**，静态 `assert*` 宏 **202 条**（含 22 场景、夹具、300s 虚拟时间超时、离线网页登录回调及签名更新全流程，高于 Swift 172 条下限） |
| 远端缓存清理核验 | `git fetch origin p0-baseline-compat` + `git ls-tree` | ✅ PASS | `57519d5` 上 `node_modules/` 与 `target/` 跟踪文件数为 **0**（`I-012` 已清） |

### 5.2 问题单（`I-005` ~ `I-013`）复测结论

| 编号 | 复测结论 | 验证依据 |
|---|---|---|
| `I-005` | ✅ 已修复 | macOS workspace 与 Windows `pen-probe` Clippy 均 `-- -D warnings` 通过 |
| `I-006` | ✅ 已修复（macOS） | 已新增 `tests/common/mod.rs`、`tests/fixtures/v1.0.1/` 与 `tests/core_tests.rs`（**26 tests / 202 条静态断言**），`cargo test --workspace --locked` 全绿 |
| `I-007` | ✅ 已修复 | `c_wl_04_05_and_i007_full_offline_web_login_flow` 离线验证完整登录流程、关窗立即取消及退出关闭监听 |
| `I-008` | ✅ 已修复（core 层） | `c_up_01_to_05_and_08_*` 与 `c_up_07_09_10_*` 验证 `UpdateSchedule`、进度、`C-UP-04/05` 拦截、`C-UP-07` 验签后进入 `Ready` 与 `InstallUpdate`，以及 `C-UP-09/10` 资产筛选 |
| `I-009` | ✅ 已修复 | `c_ac_05_06_07_*` 验证导入当前登录时先发布「登录状态已更新 / 已添加账号」成功提示并保持至少 1 秒（期间 `refreshing == true`），之后再刷新额度 |
| `I-010` | ✅ 已复测确认 | 无修改往返保持 **逐字节一致**；修改重编码后整秒 `lastQuotaUpdate` 已输出为整数（无 `.0`）。键序差异属已知可接受限制 |
| `I-011` | ⚠️ 代码通过，待 Win 实测 | `tools/pen-probe/src/windows.rs` 已跳过 `ERROR_INVALID_PARAMETER` 与非 `STILL_ACTIVE` 进程并过 Win Clippy，待 Windows CI / VM 实跑 |
| `I-012` | ✅ 已修复 | 远端 `origin/p0-baseline-compat` @ `57519d5` 已移除全部 146 个缓存文件 |
| `I-013` | ✅ 已修复 | 已验证 `C-AC-04`（`start_paused = true` 300s 超时）、`C-WL-04`（>16KB 返回 400）、`C-WL-05`（令牌交换期重复回调返回 400）、`C-UP-07`（测试公钥端到端 `Ready`）与 `C-UP-09/10`（`parse_release` Mac/Win 筛选） |

### 5.3 更新后的 L1 用例覆盖矩阵

| 模块 | 已在 `core_tests.rs` 验证 | 跨平台 / 后续阶段项 |
|---|---|---|
| 身份同步 | `C-ID-01` ~ `C-ID-03` ✅ | — |
| 切换 | `C-SW-01` ~ `C-SW-12` ✅ | — |
| 添加 / 导入 / 移除 | `C-AC-01` ~ `C-AC-07` ✅（含 `C-AC-04` 300s 虚拟时间超时） | — |
| 存储与损坏保护 | `C-ST-01` ~ `C-ST-09` ✅（含无修改原字节保留 & 整秒日期整数输出） | 修改重编码键序不保证逐字节一致（`I-010`） |
| 凭据文件 | `C-CR-01` ~ `C-CR-05` ✅ | — |
| 额度 | `C-QU-01` ~ `C-QU-08` ✅ | `C-QU-09`（需在 Windows CI 跑） |
| 网页登录 | `C-WL-01` ~ `C-WL-05` ✅（含 >16KB 400、重复回调 400、关窗取消） | — |
| 更新 | `C-UP-01` ~ `C-UP-05`、`C-UP-07` ~ `C-UP-10`、`C-UP-12` ~ `C-UP-13` ✅ | `C-UP-06`/`14`/`15`（属 P5 `UpdateInstaller`）；`C-UP-11`（Swift 基线对照） |
| 演示 & 视觉快照 | Demo 拦截 + 全 22 个 `snapshot::scenario` + 设置/自启生命周期 ✅ | — |

