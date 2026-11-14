# 开发进度：2.0.0 候选

当前交付分支为 `t6-1-delivery-docs`。业务功能、双端更新安装器、发布流水线和升级演练工具已实现；下表区分代码交付与实机验收，不按代码任务数推算验收通过率。

| 范围 | 当前结果 |
|---|---|
| I-020 与测试文档检查点 | f9d2710 已提交并推送；TEST_REPORT.md 原文保留 |
| T5.1 双端更新安装器 | 已完成；[PR #5](https://github.com/gaogep/pencil-account-switcher/pull/5)；双端 CI 和 Windows 辅助脚本回归通过 |
| T5.2 发布流水线 | 已完成；[PR #6](https://github.com/gaogep/pencil-account-switcher/pull/6)；双端签名包、完整资产与草稿发布成功 |
| T5.3 升级演练 | 工具已实现；[PR #7](https://github.com/gaogep/pencil-account-switcher/pull/7)；mac 签名包实际替换本机通过；Windows 真实 NSIS 升级、重启及三个账号夹具保留已在 CI 通过；mac runner 已通过，双端隔离升级演练完成 |
| T6.1 文档与存档 | README 双端说明、Windows SmartScreen、发布/验收文档和 `swift-final` 存档已完成；旧代码保留至验收后清理 |
| 人工与正式发布 | T0.5 Windows ARM 虚拟机、完整真实账号/自启迁移/多屏测试及正式公开发布尚未执行 |

## 当前验证证据

- [Test 43580088864](https://github.com/gaogep/pencil-account-switcher/actions/runs/43580088864)：双端 Rust/Clippy、platform-it、Windows 辅助脚本、前端、Swift 和双端视觉全部通过。Windows 独占锁回归验证超时保留原文件、短暂锁释放后写入成功。
- [Test 43580415671](https://github.com/gaogep/pencil-account-switcher/actions/runs/43580415671)：发布工具与 2.0.0 配置检查通过。
- [Release 43580428113](https://github.com/gaogep/pencil-account-switcher/actions/runs/43580428113)：mac arm64 app/DMG/ZIP 与 Windows x64 currentUser NSIS 构建、原公钥对应签名、完整资产复核、草稿汇总全部通过。[2.0.0 草稿](https://github.com/gaogep/pencil-account-switcher/releases/tag/untagged-3bc22be2f3bcf683cb59)仍未公开发布。
- 候选资产已下载到 `build/release/`，本机用原公钥复核 ZIP/EXE 签名通过；mac 完整包严格 codesign、Bundle ID、LSUIElement、版本和 arm64 架构检查通过。
- 本机完整 `npm test`、Rust/Clippy 和平台检查已通过；新增演练版本编辑回归同时覆盖 LF/CRLF。平台集成又补充 mac 目标文件不可变时的失败保留和临时文件清理检查。
- 升级演练 43581711837 的 Windows 作业已通过真实 NSIS 覆盖安装、重启、exe 版本与 SHA256 保护检查；mac 真实签名包 2.0.0 → 2.0.1 的原子替换和完整签名检查也已在本机显式执行通过。[Upgrade rehearsal 43581711837](https://github.com/gaogep/pencil-account-switcher/actions/runs/43581711837) 的双端作业均已通过。
- Windows 初次演练的 CRLF 版本编辑问题已修复并补回归；草稿下载权限按 GitHub 要求修正。以修复后最后一次演练结果为准，早期失败不计作通过。

## 验收边界

实现与自动化证据不代替真实用户路径。[UPGRADE_REHEARSAL.md](UPGRADE_REHEARSAL.md) 逐项列出 UP-01..05 自动化覆盖和剩余项目；[ISSUES.md](ISSUES.md) 保留 I-017 的人工核对和 Windows 进程夹具细项缺口。真实账号登录、切换、系统自启状态迁移和多显示器/DPI 仍需测试人员验收。

原 Swift 源码、Support、Tests、mac 视觉基线和测试报告自 f9d2710 后未改写；已推送 `swift-final` 指向该提交。清理范围和依赖见 [SWIFT_ARCHIVE.md](SWIFT_ARCHIVE.md)。本轮没有替换当前用户安装或操作真实账号，也没有正式公开发布。

---

以下保留先前阶段交接记录；其中“尚待”“本轮”等状态以其所属阶段为准，当前结论以上表为准。

# 先前阶段记录：T4.3

本轮按用户要求继续到一个完整功能点后停止。沿用 DEV_PLAN 的 26 项任务等权口径，已有基线成果或代码的任务为 **21/26，约 81%**；这是代码任务覆盖率，**不是测试或验收通过率**。当前分支 `t4-3-windows-login` 按顺序包含 T3.3 至 T4.3 的本地提交，没有推送或创建本轮 PR。

## 本轮内容与提交

- `ba3d1b3`：先提交 I-014–I-019 修复及测试人员的 Round 4 报告，再从当前分支进入 T3.3。
- `fb921d9` / `t3-3-screens`：主页、设置、账号管理和面板内移除确认框；IPC 操作、操作锁、430px 滚动区、`panel-shown` 回主页、Esc 和高度上报。新增 Vitest/jsdom/Testing Library 配置与 26 条用例。
- `f7c1312` / `t3-4-visual-regression`：22 场景 WebKit 视觉回归入口、2 倍截图、严格尺寸检查、pixelmatch 0.1 / 1% 门槛、限定例外、差异图和汇总 HTML、CI 产物。Windows 首次截图需人工确认后建立自己的基线，不自动覆盖基线。
- `0d7cf7c` / `t4-1-windows-platform`：正式 Windows 平台实现。沿用 pen-probe 的主窗口判定；退出前记住所有 Pen 进程路径，`WM_CLOSE` 后等全部进程退出；定位安装路径、LOCALAPPDATA 数据目录、自启注册表及 StartupApproved 读取、x64 deviceId。
- `99ab70f` / `t4-2-windows-shell`：Windows 托盘、浅/深色 ICO、注册表主题监听、无边框面板、工作区与 DPI 定位、阴影、焦点收起和单实例。IPC 文件移到两端共用位置，macOS 的命令行为保持原样。
- `t4-3-windows-login`：InPrivate WebviewWindow 接入 core 的 PKCE/本地回调登录流程；关窗取消、显式取消、排队创建期间取消和成功后关闭窗口。添加期间使用同一面板保留规则。

T4.2 同时依据 Swift 的结构补齐主页/管理页末尾的空白排列项和主页页脚内边距。旧 Swift 源码与既有视觉基线均保留；本轮没有改写 TEST_REPORT.md。

## 开发与检查入口

- `npm ci` 安装锁定依赖。
- `npm run dev:ui` 使用模拟 IPC；`/?mock=main` 显示完整页面，`/?mock=main&gallery` 保留组件目录。场景名取自 `ui/public/scenarios/`。
- `npm test` / `npm run test:ui` 执行 Vitest；`npm run build:ui` 检查前端、测试配置类型并打包。
- `npm run dev:tauri --workspace ui` 配合 `cargo run -p switchboard-app` 运行真实壳；Windows 首发目标为 x64，需 WebView2 Runtime。
- `npm run types:ipc`、`npm run scenarios:ui` 重新导出 core 类型和场景。
- `npx playwright install webkit` 后运行 `npm run test:visual`；只列出用例用 `npm run test:visual -- --list`。
- 视觉产物位于 `ui/visual-report/<platform>/index.html`、`ui/playwright-report/` 和 `ui/test-results/`。像素或尺寸失败不会自动重建基线；图标区域及导入按钮例外单独处理，确认框只核对文案和按钮顺序。
- 原生 Windows CI 可直接执行 `cargo clippy --workspace --all-targets --locked -- -D warnings` 与计划内的测试；工作流已配置前端单测及 macOS/Windows 视觉作业，本轮没有触发远端 CI。

## 本轮实际验证

- **通过**：26 条 Vitest 用例；TypeScript + Vite 构建；macOS 工作区 Clippy（全部 target，`-D warnings`）；格式与 Git 差异检查。
- **通过**：Windows x64 工作区 `cargo xwin check` 和 `cargo xwin clippy`（均 `--workspace --all-targets --locked`，Clippy 加 `-D warnings`）。这是在 Mac 上使用 Windows SDK/CRT 的交叉编译检查，没有执行 Windows 程序。资源编译器有一条未使用 `-L` 参数的 clang 提示，Rust Clippy 无诊断。
- **视觉入口可发现 22 条用例，像素验收未完成**：检查入口时曾因 npm 参数未转发误触发测试；本机缺少匹配的 `webkit-2388` 运行文件，22 条用例均在浏览器启动阶段失败，未生成有效截图。参数转发已修正；没有将这次执行计为通过，也没有安装浏览器或补跑视觉验收。
- 本轮没有运行 Rust/Swift 功能测试，没有启动真实应用操作账号，没有执行 Windows 真机登录、自启或进程操作，没有做安装或升级演练。

为完成 Windows 交叉编译，本机新增 cargo-xwin 0.23.1、Rust llvm-tools 和 Homebrew LLVM；Homebrew 随依赖安装 z3 并将 lz4 从 1.10.0 升至 1.11.1。PATH 仅在检查命令中设置，没有修改 shell 启动配置。

## 尚待测试与后续阶段

- I-017 的 **L3 Vitest 基础已补齐**；L2 `platform-it` feature 与平台集成测试仍未补充，问题单保持部分未闭合。
- 三页面的像素一致性、所有组件状态与视觉人工复核仍待测试人员验收。Windows 截图需人工确认后建立基线。
- Windows 需实测：多屏/多 DPI 与四边任务栏定位、隐藏图标时的回退、主题切换、二次启动、Alt+F4/失焦/右键菜单、便携版路径、Pen 未保存文档退出确认、自启被系统禁用、InPrivate 会话隔离与取消。
- macOS 需复测新页面路由、确认框和共享 IPC 接线；Round 4 的真实面板验收仅覆盖当时的代码。
- 剩余 5 项为 T0.5 Windows 虚拟机人工准备、T5.1 双端更新安装器、T5.2 发布流水线、T5.3 升级演练、T6.1 收尾。自动更新仍不启用，避免暴露尚未接入的安装操作；不进入 P5。

## Round 5 修复补充（I-020）

- 修复 Preact 11 的头像尺寸/字号无单位问题；按 AppKit 的实际行高与控件占位修正页面高度、描边、禁用按钮显示、重新导入按钮和中间截断。邮箱完整值保留在提示与可访问名称中，未改业务数据。
- WebKit 截图使用透明背景，保留全部原基线、严格尺寸断言、原图标例外和 1% 像素阈值；新增 34/36px 头像计算尺寸/字号断言。
- 开发侧验证：`npm test` **29/29**、`npm run build:ui` 通过；`npm run test:visual` **22/22**，其中 21 个场景完成尺寸与像素比对，确认框按既有规则核对文案和按钮顺序。
- TEST_REPORT.md 的 Round 5 原文保持不变，修复结果记在问题单；I-017 的 L2 `platform-it` 与 Windows 真机验收仍待后续完成。本轮未启动真实账号操作，未运行原生功能验收或触发远端 CI。

## P5 开发补充：T5.1

- 双端 `NativeUpdateInstaller` 已接入应用：core 先用原 Ed25519 公钥验签，平台层再准备包；mac 检查唯一 .app、Bundle ID、版本、可写位置与 App Translocation，原子替换后等待当前进程退出并重启。
- Windows 将已验签 exe 复制到独立暂存目录。辅助 PowerShell 等旧进程退出，以 `/S /UPDATE /D=<原安装目录>` 调用 currentUser NSIS，再重启；失败记录写入数据目录并在下次启动显示，暂存文件清理不涉及账号数据。成功安装通过原退出入口落盘。
- 新增 `platform-it`（独立主线程测试入口，避免 Cocoa 调度死锁）：只读原生路径/进程/自启，凭据与文件锁测试全部在临时目录内执行；不改变真实自启或 Pen 会话。另提供 Windows 辅助脚本夹具集成测试。
- 本机验证：mac Clippy `-D warnings`、Rust 26 个 core 测试 + 4 个安装器测试 + platform-it 通过。Windows 本机仅交叉编译检查，辅助脚本实际执行由 Windows CI 验证。
- 前一提交 f9d2710 的 Windows/mac Rust、前端和 Swift CI 通过；mac 视觉作业因 GitHub `macos-14-arm64-us-default is not supported` 在分配前取消，已改为可用的 `macos-latest`，未将取消计作通过。

## P5 开发补充：T5.2

- `release.yml` 改为 macOS arm64 与 Windows x64 矩阵，产出原命名的 mac DMG/ZIP 和 Windows currentUser NSIS EXE；手动运行创建草稿，tag 推送走发布路径，禁止覆盖已公开版本。
- 新增 Node Ed25519 签名器，使用原 `UPDATE_SIGNING_KEY`（仓库已配置），签名前核对派生公钥。发布汇总会再次校验完整资产名、签名和版本，避免老 mac 客户端选错 Windows 包。
- 与 Swift 的同文件/同测试 seed 对比已完成：本机 CryptoKit 连续签名本身字节不稳定，新旧签名格式均为无换行的 base64 64 字节 Ed25519 签名，互相验签均通过；不以签名字节相同作为兼容性条件。
- 应用/配置/前端版本设为 2.0.0，锁定 Tauri CLI 2.12.4。`npm test` 包含 UI 和发布工具测试。已在本机完成 arm64 `.app`、DMG 和兼容 ZIP 打包，LSUIElement/架构检查通过；显式 ad-hoc 签名后，完整包 codesign 严格校验通过，校验也已加入打包入口。
- Windows L2 锁文件测试发现 MoveFileEx 可能返回 ACCESS_DENIED（5），已在 T5.1 补上和 SHARING_VIOLATION（32）相同的有限重试；Windows/mac 视觉 CI 均已执行通过，Windows 锁文件修复正在重跑。

## P5 开发补充：T5.3

- 新增隔离 runner 升级演练：从草稿取原公钥签名包，在 disposable checkout 构建/签名下一数字版本。mac 使用真实签名 ZIP 测试准备、替换与数据文件字节保留；Windows 使用真实 NSIS、生产等待/安装/重启辅助脚本及三个假账号夹具，生成 JSON 证据。
- 该演练不发布下一版，不修改真实用户账号或本地版本。mac 的候选替换测试默认为 ignored，显式提供签名包后才执行；真实客户端横幅/登录/自启迁移仍按 UPGRADE_REHEARSAL.md 人工验收。
- T5.1 的远端 Test 43580088864 已全部通过：双端 Rust/Clippy/平台集成、Windows 辅助脚本夹具、前端、Swift 和双端视觉；Pen compatibility 43580088878 通过。mac 安装器又补充不可写父目录回归，本机通过。

## 完成性审计补充：Windows L2

- 审计发现 P-PR-01..05 与 P-AS-03 的原生夹具仍缺，已补测试实现，等待本次 Windows CI 执行。进程测试复制自身为私有临时 `Pen.exe`，覆盖空进程、发现/正常退出、含空格便携路径重启、拒绝 WM_CLOSE 超时、两个相同标题按配置唯一选择，以及无匹配时不发送关闭。
- 注册表测试使用进程内 `RegOverridePredefKey` 把 HKCU 映射至私有 `Software\SwitchboardCI` 子树，再调用生产自启实现，验证 Run 写/读/删及 Windows 禁用状态 3/7；恢复映射并清理测试子树，不改真实 Run/StartupApproved 值。
- 这些可变 Windows 夹具仅在隔离的 Actions runner 执行；本机只有交叉 Clippy 检查，不将编译通过计为 Windows 执行通过。Mac 的 SMAppService 与系统设置人工一致性核对仍需实机验收。
