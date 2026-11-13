# 开发计划：Tauri 2 双端重写

> 设计依据：[DESIGN.md](DESIGN.md)（架构、模块对照、平台差异）。测试与验收：[TEST_PLAN.md](TEST_PLAN.md)。
> 本文给执行开发的 Agent 用。每个任务写明依赖、输入、产出和完成标准，按编号顺序领取；同一阶段内没有依赖关系的任务可以并行。

---

## 0. 总规则（所有任务都适用）

1. **行为以现有 Swift 代码为准。** 移植时逐个函数对照 `Sources/Switchboard/`，不要按自己的理解"改进"。发现原代码有问题，记进 `docs/cross-platform/ISSUES.md`，不要顺手改。
2. **界面以基线截图为准。** `Tests/baseline/macos/` 里的 22 张图是验收标准，色值取自 `Sources/Switchboard/UI/AppKitCommon.swift` 的 `Theme`，不新增颜色。只有第 9 节列出的三处允许和基线不同。
3. **文案一字不改。** 所有中文提示、错误信息、按钮文字直接从 Swift 源码复制。
4. **macOS 老用户无感升级。** 数据目录、`store.json` / `credentials/*.json` 格式、Bundle ID `dev.switchboard.Switchboard`、更新签名公钥和资产命名都不能变（见 DESIGN 5.5、5.8）。
5. **不强杀 Pen。** 任何平台都只做"礼貌退出"，超时就报"请先退出 Pen"。
6. **旧 Swift 代码保留到 P6。** 新代码放在新目录里，两套并存，方便对照和截基线。
7. 每个任务一个分支、一个 PR。PR 描述里写清：对应任务编号、对照了哪些 Swift 文件、跑了哪些测试。
8. 依赖版本全部锁定（`Cargo.lock`、`package-lock.json` 入库）。

### 目录约定（新建）

```
Cargo.toml                       # workspace
crates/switchboard-core/         # 平台无关逻辑
crates/switchboard-platform/     # macos.rs / windows.rs
src-tauri/                       # Tauri 应用壳
ui/                              # Vite + Preact + TypeScript
tools/pen-probe/                 # Windows 兼容性探测（P0）
scripts/                         # 签名、Pen 兼容性检查
docs/cross-platform/             # 本文、DESIGN、TEST_PLAN、ISSUES
```

---

## 1. 任务总览

| 阶段 | 任务 | 依赖 | 估时 |
|---|---|---|---|
| P0 | T0.1 视觉基线 ✅ 已完成 | — | — |
| P0 | T0.2 Pen 静态兼容检查脚本 | — | 0.5 天 |
| P0 | T0.3 Windows 探测工具 `pen-probe` | — | 1 天 |
| P0 | T0.4 `pen-compat.yml` 工作流 | T0.2、T0.3 | 0.5 天 |
| P0 | T0.5 Windows 11 ARM 虚拟机（人工） | — | 0.5 天 |
| P1 | T1.1 workspace 与 CI 骨架 | — | 0.5 天 |
| P1 | T1.2 数据模型与存储 | T1.1 | 0.5 天 |
| P1 | T1.3 Pen 适配（core 部分） | T1.2 | 0.5 天 |
| P1 | T1.4 额度查询 | T1.2 | 0.5 天 |
| P1 | T1.5 网页登录（core 部分） | T1.1 | 0.5 天 |
| P1 | T1.6 更新逻辑 | T1.1 | 1 天 |
| P1 | T1.7 AppModel 状态机 | T1.2–T1.6 | 1.5 天 |
| P2 | T2.1 平台层 macOS | T1.3 | 1 天 |
| P2 | T2.2 Tauri 壳：托盘与面板（mac） | T1.7、T2.1 | 1.5 天 |
| P2 | T2.3 IPC 层 | T1.7 | 0.5 天 |
| P3 | T3.1 前端骨架与主题 | T2.3 | 0.5 天 |
| P3 | T3.2 组件库 | T3.1 | 1.5 天 |
| P3 | T3.3 三个页面与确认框 | T3.2 | 1.5 天 |
| P3 | T3.4 视觉回归接入 | T3.3 | 1 天 |
| P4 | T4.1 平台层 Windows | T0.3、T1.3 | 1.5 天 |
| P4 | T4.2 Tauri 壳：托盘与面板（Win） | T2.2、T4.1 | 1 天 |
| P4 | T4.3 Windows 网页登录 | T1.5、T4.2 | 0.5 天 |
| P5 | T5.1 更新安装器（双端） | T1.6、T2.1、T4.1 | 1 天 |
| P5 | T5.2 发布流水线 | T5.1 | 1 天 |
| P5 | T5.3 升级演练 | T5.2 | 0.5 天 |
| P6 | T6.1 文档、清理与发布 | 全部 | 1 天 |

合计约 3–4 周（1 人当量）。

---

## 2. P0：基线与兼容性探测

### T0.1 视觉基线 ✅ 已完成

- 现有 Swift 版加了截图状态清单：`Sources/Switchboard/SnapshotScenarios.swift`。
- 运行 `./Tests/snapshot.sh` 生成 22 张图到 `Tests/baseline/macos/`，连续两次运行逐像素一致。
- 状态清单：`main`、`main-empty`、`main-single`、`main-unregistered-live`、`main-expired`、`main-unknown-quota`、`main-switching`、`main-refreshing`、`main-adding`、`main-feedback-success/warning/long`、`main-persistence-blocked`、`main-update-available/downloading/ready`、`settings`、`settings-launch-on`、`settings-update-available`、`manage`、`manage-expired`、`manage-confirm-remove`。
- 已知限制：`manage-confirm-remove` 只截了确认框内容视图，没有系统 sheet 的背景。

### T0.2 Pen 静态兼容检查脚本

- **产出**：`scripts/check_pen_compat.mjs`（Node 20+，依赖 `@electron/asar`，锁定版本）。
- **输入**：一个 `app.asar` 路径（mac：`/Applications/Pen.app/Contents/Resources/app.asar`；win：解压后的 `resources/app.asar`）。
- **检查项**（任何一项不符合就非零退出，并打印实际代码片段）：
  1. `out/constants.js`：`CONFIG_FOLDER = path.join(os.homedir(), ".pencil")`
  2. `out/desktop-app-host.js`：会话文件名 `session-desktop${IS_DEV ? "-dev" : ""}.json`
  3. `out/desktop-app-host.js`：`getDesktopDeviceId` 为 `md5(os.hostname() + os.platform() + os.arch())`
  4. `out/desktop-app-host.js`：`clientHost: os.platform()`
  5. `out/app-window.js`：`close` 事件里非 mac 平台调用 `app.quit()`
  6. 全部 `out/*.js` 中不存在 `new electron_1.Tray(`（Pen 没有托盘）
  7. `out/main.js`：存在 `requestSingleInstanceLock()`
- 输出同时打印 Pen 版本（`package.json`）。
- **完成标准**：对本机 Pen 1.2.23 运行通过；手动改坏一处后能报错。

### T0.3 Windows 探测工具 `pen-probe`

- **产出**：`tools/pen-probe/`（Rust 可执行文件，依赖 `windows-sys`，只在 Windows 编译；其他平台编译出一个直接报错退出的空壳）。
- **子命令**（输出一律为 JSON，方便 CI 断言）：

| 子命令 | 作用 | 实现要点 |
|---|---|---|
| `hostname` | 输出 `GetHostNameW` 结果和 `md5(hostname + "win32" + "x64")` | 必须用 `GetHostNameW`（需先 `WSAStartup`），不要用 `COMPUTERNAME` 环境变量 |
| `processes` | 列出所有 `Pen.exe` 的 pid 和完整路径 | `CreateToolhelp32Snapshot` + `QueryFullProcessImageNameW` |
| `windows` | 列出 Pen 进程的顶层窗口：hwnd、标题、类名、可见性、矩形、DPI | `EnumWindows` + `GetWindowThreadProcessId` + `GetDpiForWindow` |
| `main-window` | 按下面的规则选出主窗口，输出选中的 hwnd 或"无法确定"的原因 | 见下 |
| `quit --timeout <秒>` | 给主窗口发 `WM_CLOSE`，等所有 `Pen.exe` 退出 | 每 200ms 轮询；超时返回非零，**不强杀** |
| `launch <路径>` | 拉起 Pen | `CreateProcessW` + `DETACHED_PROCESS`，不继承句柄 |
| `autostart get/set/clear --name <值名>` | 读写 `HKCU\...\Run`，读取时同时检查 `StartupApproved\Run` | 测试时用专门的值名，避免影响真实配置 |

- **主窗口判定规则**（Pen 的主窗口和弹出文档窗口标题都是 `pen.dev`，类名都是 `Chrome_WidgetWin_1`，无法直接区分）：
  1. 候选 = 属于 `Pen.exe`、可见、标题为 `pen.dev` 的顶层窗口
  2. 只有一个候选 → 就是主窗口
  3. 多个候选 → 读 `%APPDATA%\Pen\config.json` 的 `windowBounds`（DIP 坐标），按各窗口 DPI 换算后和窗口矩形比对，误差 ≤ 2px 且唯一匹配的就是主窗口
  4. 仍无法确定 → 不发 `WM_CLOSE`，返回"无法确定主窗口"，上层提示用户手动退出 Pen
- **完成标准**：`cargo build --target x86_64-pc-windows-msvc` 在 Windows CI 上通过；T0.4 的工作流全部步骤通过。
- P4 的 T4.1 会把这里的函数挪进 `switchboard-platform`，所以函数按库的方式写（`lib.rs` + 薄的 `main.rs`）。

### T0.4 `pen-compat.yml` 工作流

- **触发**：`workflow_dispatch`（可填 Pen 版本，默认最新）+ 每周一次定时。
- **Job 1 `resolve`**（ubuntu）：从 `https://api.github.com/repos/highagency/pen-desktop-releases/releases/latest` 取版本号和资产地址。
- **Job 2 `windows`**（`windows-latest`）：
  1. 下载 `Pen-<ver>-win-x64.zip` 并解压（免安装版，约 570MB，加缓存）
  2. 对 `resources/app.asar` 运行 T0.2 的脚本
  3. 编译 `pen-probe`
  4. 用 Node 计算 `md5(os.hostname() + os.platform() + os.arch())`，和 `pen-probe hostname` 的结果比对，必须完全一致
  5. 启动 Pen，等它的窗口出现（最多 60 秒），截一张屏幕截图
  6. `pen-probe processes`：至少找到一个进程，路径等于解压目录下的 `Pen.exe`
  7. `pen-probe main-window`：能选出主窗口
  8. `pen-probe quit --timeout 15`：所有 `Pen.exe` 退出
  9. 写入一份假的 `%USERPROFILE%\.pencil\session-desktop.json`（假邮箱、假 token），启动 Pen 等 20 秒再退出，记录文件是否被删除或改写（**这一步只记录不断言**，结果写进报告）
  10. `pen-probe launch` 再启动一次，然后用同一个可执行文件再启动一次，确认只有一个主窗口（单实例）
  11. `pen-probe autostart set/get/clear`，值名用 `SwitchboardCI`
  12. 把所有 JSON 输出、截图和 Pen 日志（`%APPDATA%\Pen\logs\main.log`）作为构建产物上传
- **Job 3 `macos`**（`macos-14`）：下载 mac arm64 zip，只跑 T0.2 的脚本。
- **完成标准**：手动触发一次全部通过；把第 9 步的结论写进 `docs/cross-platform/ISSUES.md`。

### T0.5 Windows 11 ARM 虚拟机（人工）

- 在开发用的 Mac 上用 UTM / Parallels / VMware Fusion 装 Windows 11 ARM，装 Pen 官方 x64 安装包，用测试账号登录一次。
- 记录：Pen 实际安装路径、`%USERPROFILE%\.pencil\` 的文件列表、任务栏位置。写进 `ISSUES.md`。
- 这台虚拟机用于 TEST_PLAN 第 6 节的手工验收。

---

## 3. P1：switchboard-core

所有 core 代码不能依赖 Tauri、AppKit 或 Win32。异步用 tokio，HTTP 用 `reqwest`（rustls），JSON 用 serde。

### T1.1 workspace 与 CI 骨架
- 根目录 `Cargo.toml`（workspace），建 `crates/switchboard-core`、`crates/switchboard-platform`（先放空的 trait）。
- 修改 `.github/workflows/test.yml`：保留现有 Swift 测试 job；新增 `rust` job，矩阵 `macos-14` + `windows-latest`，跑 `cargo fmt --check`、`cargo clippy -D warnings`、`cargo test`。
- **完成标准**：空测试在两个平台都通过。

### T1.2 数据模型与存储
- 对照：`Models.swift`、`CredentialStore.swift`、`AppModel.swift` 的 `load / quarantine / persist / writeStoreNow`。
- `AccountRecord`、`AuthState`（未知值解码为 `unknown`）、`StoreFile`（`storeVersion = 1`），JSON 键名和 Swift `Codable` 输出逐字一致（日期格式要和 Swift `JSONEncoder` 默认的一致：自 2001-01-01 起的秒数）。
- `FileCredentialStore`：文件名只接受 UUID；原子写；mac 上文件 0600、目录 0700；Windows 上遇到 `ERROR_SHARING_VIOLATION` 重试，最多约 1 秒。
- 存储目录由 `Platform::data_dir()` 提供，core 不写死路径。
- **完成标准**：用真实的 1.0.1 版 `store.json` 和凭据文件做读写往返，字节级一致（测试夹具见 TEST_PLAN）。

### T1.3 Pen 适配（core 部分）
- 对照：`PenTargetAdapter.swift`。
- 会话 JSON 解析、`liveIdentity`、`captureSnapshot`、`applySnapshot`（写前校验邮箱和 token、原子写、写后再读回校验）、`emailsMatch` 忽略大小写。
- 进程查找、退出、启动通过 `Platform` 调用。
- `TargetAdapter` trait 和 DESIGN 5.1 一致，测试里要能换成假实现。

### T1.4 额度查询
- 对照：`QuotaProvider.swift`。
- 请求格式、header、body、HTTP 状态映射（401 → 失效；403 → 失败但不标失效；其他非 2xx → 失败）、body 解析规则逐条一致。
- deviceId：`md5(hostname + os_tag + arch)`。mac 用 `gethostname` 和 Swift 当前做法一致；Windows 用 T0.3 验证过的 `GetHostNameW`，`os_tag = "win32"`，`arch` 固定 `"x64"`。
- 结果缓存，只算一次。

### T1.5 网页登录（core 部分）
- 对照：`WebLogin.swift`。
- PKCE verifier 生成、127.0.0.1 随机端口监听、`callbackCode` 校验（只接受 `GET /callback`，`state` 和 `code` 都必须恰好出现一次且 state 匹配）、请求大小上限 16KB、回包文案、换 token 的请求。
- 授权窗口抽象成 trait（`open(url) / cancel()`），由平台层实现。
- 300 秒超时由 AppModel 控制，不放这里。

### T1.6 更新逻辑
- 对照：`UpdateChecker.swift`、`UpdateVerifier.swift`。
- 版本比较、GitHub Release 解析、资产选择（按平台：mac 选 `*-arm64.zip` + `.sig`；Windows 选 `*-windows-x64-setup.exe` + `.sig`）、Ed25519 验签（`ed25519-dalek`，公钥沿用 `Support/Info.plist` 里的 `SBUpdatePublicKey`）、下载进度、取消、HTTP 非 2xx 报错带状态码。
- 检查节流：自动检查记录尝试时间，失败后退避 1 小时，手动检查忽略退避。时间记录存在 `data_dir/state.json`。
- 安装动作抽象成 `UpdateInstaller` trait，具体实现在 T5.1。

### T1.7 AppModel 状态机
- 对照：`AppModel.swift`，**逐个方法移植**。
- 结构：一个 tokio 任务持有全部状态，通过 `mpsc` 接收命令；每次状态变化发出完整的 `ViewState`（字段见 DESIGN 5.9）。
- 必须保留的行为清单见 DESIGN 5.1，逐条实现，每条都要有 TEST_PLAN 里对应编号的测试。
- 演示模式 `SWITCHBOARD_DEMO=1`：加载和 Swift 版相同的三个演示账号，所有真实操作返回"演示模式不操作真实账号"等原文案。
- 同时实现截图用的状态注入（对应 `SnapshotScenarios.swift`），供 T3.4 使用：给定场景名，输出对应的 `ViewState` JSON。
- **完成标准**：TEST_PLAN 第 3 节的 core 用例全部通过，且通过的断言数量不少于 Swift 版。

---

## 4. P2：macOS 壳

### T2.1 平台层 macOS
- `crates/switchboard-platform/src/macos.rs`，实现 DESIGN 5.2 的 `Platform` trait。
- 进程：用 `objc2-app-kit` 调 `NSRunningApplication.runningApplicationsWithBundleIdentifier("dev.pencil.desktop")`，`terminate()`；启动用 `NSWorkspace.openApplicationAtURL`；找安装位置先按 bundle id，再试 `/Applications/Pen.app`。
- 开机自启：`objc2-service-management` 调 `SMAppService.mainApp`，**不要用 `tauri-plugin-autostart`**。
- 授权窗口：`ASWebAuthenticationSession` + `prefersEphemeralWebBrowserSession = true`。
- 权限：`restrict_permissions` 设 0600 / 0700。

### T2.2 Tauri 壳：托盘与面板（mac）
- 对照：`AppMain.swift`，行为清单见 DESIGN 5.6。
- 托盘：模板图标；左键开关面板；右键菜单三项（打开面板 / 灰显版本号 / 退出）。
- 面板：`tauri-nspanel` 转 NSPanel，level、collectionBehavior、styleMask 和 Swift 版一致；宽 410；高度由前端上报，夹在 200–900；贴在图标正下方 6pt、水平居中、离屏幕边 8pt；图标不可见时放右上角距边 12pt。
- 收起：失焦（0.3 秒宽限，排除点在托盘图标上的情况）、Esc、打开后 0.35 秒内重复点击忽略；`shouldKeepPanelOpen` 时不收。
- 不进 Dock（`LSUIElement`）；再次打开 .app 时显示面板；每次显示面板发 `panel-shown`。
- 退出时先让 core 立即落盘（对应 `flushPersist`）。

### T2.3 IPC 层
- 实现 DESIGN 5.9 的全部 command 和 event，参数和返回值写成 TypeScript 类型（用 `ts-rs` 或 `specta` 从 Rust 生成，保证两边一致）。
- 前端一律通过 `state` 事件拿状态，不在前端保存业务状态。

---

## 5. P3：界面复刻

### T3.1 前端骨架与主题
- `ui/`：Vite + Preact + TypeScript，严格模式。
- `theme.css`：把 `Theme` 的常量逐个转成 CSS 变量，名字一一对应。
- 字体栈见 DESIGN 5.7。
- 本地开发支持"模拟 IPC"模式：不启动 Tauri，直接从 JSON 读取 `ViewState`，用于组件开发和 Playwright。

### T3.2 组件库
- 对照 `AppKitCommon.swift`、`Screens.swift` 里的 `make*` 函数，一个函数一个组件：品牌头部、导航头部、当前账号卡、其他账号行、进度条、认证标签、状态横幅、更新横幅、开关（`role="switch"`）、刷新按钮（旋转动画，尊重 `prefers-reduced-motion`）、添加账号按钮、链接按钮、药丸按钮、头像（颜色用和 Swift 相同的 `stableHash`）。
- 图标：按 DESIGN 5.7 的对照表换成 Lucide，尺寸和线宽对齐。
- 反馈条自动消失时长由 core 控制，前端不计时。

### T3.3 三个页面与确认框
- `MainScreen`、`SettingsScreen`、`ManageScreen`，结构和顺序跟 Swift 一致；内容区最大高度 430，超出滚动（Swift 里固定是 430）。
- 移除确认框做成面板内模态，文案和按钮顺序同 `ManageScreen.makeRemoveAlert`；Esc 只关确认框。
- 未登记身份提示里的「导入」按钮用 `Theme.linkText`，不照抄原来的白色（见第 9 节）。
- 每次收到 `panel-shown` 回到主页。
- 用 `ResizeObserver` 上报高度。
- 操作锁：根据 `ViewState` 统一计算按钮禁用，规则同 `PanelController.syncOperationLock`。

### T3.4 视觉回归接入
- Playwright（WebKit）在模拟 IPC 模式下渲染 T1.7 输出的 22 个场景，按 2 倍像素比截图，和 `Tests/baseline/macos/` 比对。
- 阈值和例外规则见 TEST_PLAN 第 5 节。

---

## 6. P4：Windows

### T4.1 平台层 Windows
- `crates/switchboard-platform/src/windows.rs`：把 T0.3 的探测函数改成正式实现。
- 数据目录 `%LOCALAPPDATA%\Switchboard`。
- 退出 Pen：先记下所有 `Pen.exe` 的完整路径，再按 T0.3 的主窗口规则发 `WM_CLOSE`，等所有进程退出。
- 定位 Pen：① 刚才记下的路径；② `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\*` 中 `DisplayName` 为 Pen 的 `InstallLocation`；③ `%LOCALAPPDATA%\Programs\{Pen,pen}\Pen.exe`。
- 开机自启：`HKCU\...\Run`，读取时结合 `StartupApproved\Run` 判断是否被用户禁用。

### T4.2 Tauri 壳：托盘与面板（Win）
- 托盘：浅色和深色两套 ICO，监听 `HKCU\...\Themes\Personalize\SystemUsesLightTheme` 切换。
- 面板：无边框、透明、置顶、不进任务栏；按任务栏位置（比较显示器区域和工作区）放到图标的上 / 下 / 左 / 右，同样留 6pt 和 8pt；拿不到图标位置时放到工作区右下角距边 12pt；按 DPI 换算。
- 阴影：透明窗口没有系统阴影，用 CSS 在窗口留白区画。
- 单实例：`tauri-plugin-single-instance`，第二次启动时显示面板。

### T4.3 Windows 网页登录
- 弹出 Tauri `WebviewWindow`，`incognito(true)`；关窗视为取消；登录窗口抢焦点时面板不能收起（靠 `shouldKeepPanelOpen`）。

---

## 7. P5：更新与发布

### T5.1 更新安装器
- mac：解压 → 验签 → 校验 zip 根目录只有一个 `.app` 且 `CFBundleIdentifier`、版本号正确 → 检查安装位置可写、不在 App Translocation → `replaceItemAt` → 等当前进程退出后 `open` 新版本。和 Swift 版逐条一致。
- Windows：下载 → 验签 → `<setup.exe> /S` 静默安装（NSIS，`currentUser` 模式）→ 安装包结束后拉起新版本 → 当前进程退出。

### T5.2 发布流水线
- `release.yml` 改为矩阵：mac 产出 `.dmg` 和 `Pencil-Account-Switcher-<ver>-arm64.zip`（用 `ditto -c -k --keepParent` 打包）；Windows 产出 `Pencil-Account-Switcher-<ver>-windows-x64-setup.exe`。
- 签名：新写 `scripts/sign_update.rs`（或 Node 脚本），使用同一个 `UPDATE_SIGNING_KEY`，输出格式和 `Support/sign_update.swift` 完全一致；先用同一个文件分别跑新旧两个脚本，对比输出。
- 校验 `tauri.conf.json` 的 version 和 tag 一致。
- **Windows 资产名里不能出现 `arm64` 和 `.zip`。**

### T5.3 升级演练
- 在测试仓库或草稿 Release 上走通 TEST_PLAN 第 7 节的全部升级路径。

---

## 8. P6：收尾

- README 增加 Windows 安装说明（含 SmartScreen「更多信息 → 仍要运行」）。
- 删除旧 Swift 代码前打 tag `swift-final` 存档；`Tests/baseline/macos/` 和 `scripts/check_pen_compat.mjs` 保留。
- 发 2.0.0：mac 正式版，Windows 标为 beta（DESIGN 9.5）。

---

## 9. 已确认的界面变更（2026-11-13）

新版和 1.0.1 相比，只有下面三处界面允许不同，视觉回归里按例外处理（TEST_PLAN 5.2）：

1. **图标**：SF Symbols 换成 Lucide（DESIGN 5.7）。
2. **未登记身份提示里的「导入」按钮改成可读的颜色**：1.0.1 用了白色文字（`Theme.switchBtnText`），在浅色底上几乎看不见（ISSUES I-001）。新版改用 `Theme.linkText`（#27272A），和其他链接按钮一致；字号、字重、位置不变。旧 Swift 代码和基线图不改。
3. **移除确认框**：mac 上原来是系统 sheet，新版改成面板内的模态弹层，文案、按钮顺序和 Esc 取消的行为不变。
