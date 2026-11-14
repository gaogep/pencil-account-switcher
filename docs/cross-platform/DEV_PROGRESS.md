# 开发交接：停止于 T4.3

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
