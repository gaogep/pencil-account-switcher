# 测试计划：Tauri 2 双端重写

> 对应开发任务见 [DEV_PLAN.md](DEV_PLAN.md)，设计见 [DESIGN.md](DESIGN.md)。
> 本文给执行测试的 Agent 用。每条用例有编号，PR 和测试报告里按编号引用。

---

## 1. 目标与原则

- **新版本的行为和 1.0.1 一致**：现有 Swift 测试里的每一条断言，在新代码里都要有对应用例，数量只能多不能少。
- **界面和基线一致**：`Tests/baseline/macos/` 的 22 张图是标准答案。
- **Windows 上没有实机**：靠三层补位——Pen 源码静态检查、GitHub Actions Windows 机器上的自动化、Windows 11 ARM 虚拟机手工验收（DESIGN 第 9 节）。
- **老用户能升级**：1.0.1 → 2.0 的自动更新必须在真实 Release 上演练通过才能发布。

## 2. 测试分层与入口

| 层 | 范围 | 工具 | 运行平台 | 入口 |
|---|---|---|---|---|
| L1 core 单测 | `switchboard-core` 全部逻辑 | `cargo test` | mac + Windows CI | `test.yml` 的 `rust` job |
| L2 平台集成 | `switchboard-platform` 的文件、进程、注册表、自启 | `cargo test --features platform-it` | mac + Windows CI | 同上 |
| L3 前端组件 | 组件在各种 `ViewState` 下的渲染、禁用、文案 | Vitest | ubuntu | `test.yml` 的 `ui` job |
| L4 视觉回归 | 22 个场景和基线逐张比对 | Playwright（WebKit） | macos-14 | `test.yml` 的 `visual` job |
| L5 Pen 兼容性 | Pen 源码关键点 + Windows 上真实运行 Pen | `check_pen_compat.mjs` + `pen-probe` | Windows CI + mac CI | `pen-compat.yml` |
| L6 手工验收 | 真实账号的端到端流程 | 检查清单 | 真机 Mac + Windows 11 ARM 虚拟机 | 本文第 6 节 |
| L7 升级演练 | 1.0.1 → 2.0、2.0 → 2.0.x | 草稿 Release | 真机 Mac + 虚拟机 | 本文第 7 节 |

旧的 Swift 测试（`Tests/run.sh`）在 P6 之前继续在 CI 里跑，作为对照。

---

## 3. L1 core 单测：从 Swift 测试移植

### 3.1 测试替身（先移植这些）

| Swift | Rust | 作用 |
|---|---|---|
| `MemoryCredentials` | `MemoryCredentialStore` | 内存凭据 |
| `FakeTarget` | `FakeTarget` | 可控的会话、运行状态、退出结果、写入失败、启动失败 |
| `FakeQuota` | `FakeQuota` | 可控返回值和延迟 |
| `FakeLogin` | `FakeWebLogin` | 返回指定快照或报错 |
| `FakeFeed` / `FakeTransport` / `FakeInstaller` | 同名 | 更新链路 |

### 3.2 用例清单

编号规则：`C-<模块>-<序号>`。"来源"列是 `Tests/RegressionTests.swift`（R）或 `Tests/UpdateLogicTests.swift`（U）里的断言标签，移植时搜这个标签找原始代码。

**身份同步**

| 编号 | 场景 | 期望 | 来源 |
|---|---|---|---|
| C-ID-01 | 会话文件消失后同步 | 当前账号清空 | R `logout clears current account` |
| C-ID-02 | 会话变成未登记邮箱 | 当前账号清空，`unregisteredLiveEmail` 有值 | R `unregistered identity clears previous current account` |
| C-ID-03 | 邮箱大小写不同 | 视为同一账号 | Swift `emailsMatch` |

**切换**

| 编号 | 场景 | 期望 | 来源 |
|---|---|---|---|
| C-SW-01 | 目标账号凭据缺失 | 不退出 Pen，报"凭据缺失" | R `missing credentials do not quit Pen` |
| C-SW-02 | 写会话失败 | 恢复原会话文件 | R `failed write restores original session` |
| C-SW-03 | 切换失败且 Pen 原本在运行 | 重新拉起 Pen | R `failed switch restarts previously running Pen` |
| C-SW-04 | 切换失败 | 当前账号保持真实身份 | R `failed switch keeps actual current identity` |
| C-SW-05 | 原来没有会话文件，切换失败回滚 | 回滚时能创建文件 | R `restore creates missing session file` |
| C-SW-06 | 快照里的邮箱和记录不符 | 写入前拒绝 | R `mismatched snapshot rejected before write` |
| C-SW-07 | Pen 退出时回写了新 token | 用退出后重新捕获的会话作为回滚源和保存源 | R `renewed-token` 相关断言 |
| C-SW-08 | Pen 不肯退出 | 报"未能退出"，不写文件 | R `未能退出` |
| C-SW-09 | 切换成功但 Pen 启动失败 | 切换算成功，提示含"请手动打开" | R `launch failure still completes the switch`、`launch failure keeps the switched account current` |
| C-SW-10 | 切换成功后 4 秒复查发现身份被改 | 只提示，不回滚 | R `soft verify starts from a successful switch`、`hijacked@example.test` |
| C-SW-11 | 刷新额度进行中发起切换 | 拒绝并提示"正在刷新额度" | R `refresh is in progress` |
| C-SW-12 | 两个切换同时发起 | 只执行一个 | AppModel `switchingTo` 守卫 |

**添加 / 导入 / 移除**

| 编号 | 场景 | 期望 | 来源 |
|---|---|---|---|
| C-AC-01 | 网页登录完成 | 保存凭据、写入账号、切换过去；切换期间 `addingAccount` 保持 true | R `login operation finishes` |
| C-AC-02 | 已保存但切换失败 | 提示保持警告样式 | R `saved-but-failed stays a warning` |
| C-AC-03 | 网页登录取消或超时 | 提示"网页登录已取消或超时，原账号保持不变" | AppModel |
| C-AC-04 | 添加 300 秒超时 | 自动取消 | AppModel |
| C-AC-05 | 导入当前登录（已存在的账号） | 提示"登录状态已更新" | AppModel |
| C-AC-06 | 移除界面上过期的"当前账号"记录 | 不能移除真实的当前账号 | R `stale UI record cannot remove live current account` |
| C-AC-07 | 有操作进行中时移除 | 拒绝，提示"有操作正在进行" | R `busy remove does not delete the account` |

**存储与损坏保护**

| 编号 | 场景 | 期望 | 来源 |
|---|---|---|---|
| C-ST-01 | `store.json` 损坏 | 备份为 `store.corrupt-<时间>.json`，禁止写入 | R `corrupt store blocks persistence`、`corrupt-` |
| C-ST-02 | 禁止写入时导入 | 不写凭据文件 | R `locked import writes no credential file` |
| C-ST-03 | 禁止写入时添加 | 不启动网页登录、不写凭据 | R `locked add does not start web login`、`locked add writes no credential file` |
| C-ST-04 | 禁止写入时移除 | 账号保留 | R `locked remove keeps the account` |
| C-ST-05 | 禁止写入时界面状态 | 导入按钮禁用 | R `locked store disables import` |
| C-ST-06 | `storeVersion` 比当前高 | 同 C-ST-01 的只读处理 | AppModel `load` |
| C-ST-07 | 连续多次修改账号 | 合并成一次写入；退出时立即写入 | AppModel `persist` / `flushPersist` |
| C-ST-08 | 读写 1.0.1 生成的真实 `store.json` | 往返后字节一致 | 新增（夹具见 3.3） |
| C-ST-09 | `AuthState` 遇到未知字符串 | 解码为 `unknown` | Models.swift |

**凭据文件**

| 编号 | 场景 | 期望 | 来源 |
|---|---|---|---|
| C-CR-01 | 写入后读出 | 内容一致 | R `file credentials round-trip` |
| C-CR-02 | 目录和文件权限（mac） | 目录 0700，文件 0600 | R `credentials directory is 0700`、`credential file is 0600` |
| C-CR-03 | 账号名不是 UUID（含 `../`） | 拒绝 | R `path traversal is rejected` |
| C-CR-04 | 删除后读取 / 重复删除 | 报不存在 / 不报错 | R `deleted credentials are missing`、`repeat delete is idempotent` |
| C-CR-05 | 会话文件写入后权限（mac） | 0600 | R `session file is 0600 after applySnapshot` |

**额度**

| 编号 | 场景 | 期望 | 来源 |
|---|---|---|---|
| C-QU-01 | HTTP 401 | 账号标为失效 | R `401 marks the login expired`、`expired response updates authentication state` |
| C-QU-02 | HTTP 403 | 不标失效，提示没有权限 | R `403 does not expire the account` |
| C-QU-03 | `isMetered=false` | 不计量 | R `unmetered quota body is unsupported` |
| C-QU-04 | 缺少 used / limit | 不计量 | R `missing quota fields are unsupported` |
| C-QU-05 | 正常返回 | 解析出已用和总数 | R `metered quota body is parsed` |
| C-QU-06 | body 不是合法 JSON | 失败 | R `invalid quota body is a failure` |
| C-QU-07 | 多账号刷新 | 最多 3 个并发；按失败数给出三种提示文案 | AppModel `performQuotaRefresh` |
| C-QU-08 | deviceId（mac） | 和 Swift 版 `PenQuotaProvider.deviceId()` 结果相同 | 新增 |
| C-QU-09 | deviceId（Windows） | 和同机 Node 计算的 `md5(os.hostname()+"win32"+"x64")` 相同 | 新增，在 L5 里跑 |

**网页登录**

| 编号 | 场景 | 期望 | 来源 |
|---|---|---|---|
| C-WL-01 | 正确的 state 和 code | 返回 code | R `GET /callback?code=test&state=expected` |
| C-WL-02 | state 不匹配 | 拒绝 | R `...state=wrong` |
| C-WL-03 | state 出现两次 | 拒绝 | R `...state=expected&state=wrong` |
| C-WL-04 | 路径不是 `/callback`、方法不是 GET、请求超过 16KB | 拒绝，回 400 | WebLogin.swift |
| C-WL-05 | 第二次回调 | 拒绝 | WebLogin.swift `receivedCode` |

**更新**

| 编号 | 场景 | 期望 | 来源 |
|---|---|---|---|
| C-UP-01 | 有更新的版本 | 显示更新横幅 | R `newer release becomes a visible update` |
| C-UP-02 | 版本相同 | 保持空闲 | R `same version stays idle` |
| C-UP-03 | 下载进度 | 显示百分比和已下载 / 总大小 | R `42%`、`MB` |
| C-UP-04 | 签名通过后才解压 | 未验签不解压 | R `unpack only runs after a signature is accepted`、`rejected update does not unpack` |
| C-UP-05 | 缺签名文件 / 签名错误 | 不进入可安装状态，文案"发布缺少签名文件" / "签名校验失败" | R `missing signature ...`、`bad signature ...` |
| C-UP-06 | 包内 bundle id 或版本不符 | 不进入可安装状态，文案"更新包内容与发布版本不符" | R `wrong bundle id ...`、`wrong bundle version ...` |
| C-UP-07 | 签名和内容都正确 | 进入可安装状态 | R `signed matching bundle becomes ready` |
| C-UP-08 | 自动检查 | 记录尝试时间；失败后 1 小时内不再自动检查；手动检查不受限 | R `automatic check records an attempt`、`failed update check backs off for an hour`、`forced check ignores the backoff` |
| C-UP-09 | 资产选择（mac） | 只选 arm64 zip；arm64 zip 优先于普通 zip；必须有 `.sig` | U `arm64 zip is preferred`、`non-arm64 zip is rejected`、`arm64 zip wins over a generic zip`、`latest release requires the arm64 zip and its signature`、`a release without a signature asset is rejected` |
| C-UP-10 | 资产选择（Windows） | 只选 `*-windows-x64-setup.exe` + `.sig`；mac 资产不会被选中 | 新增 |
| C-UP-11 | 交叉防误选 | 包含 Windows 资产的 Release 上，旧版 Swift 选择逻辑依然只选中 mac zip | 新增（用 Swift 旧测试代码跑一次新的 Release 资产列表） |
| C-UP-12 | 下载 HTTP 404 / 无响应 / 200 | 报错带状态码 / 报错 / 保留内容 | U `HTTP 404 ...`、`missing HTTP response is rejected`、`HTTP 200 download keeps the body` |
| C-UP-13 | 取消下载 | 停止 | U `cancelling a download stops it` |
| C-UP-14 | zip 内没有 .app / 有两个 .app / 恰好一个 | 拒绝 / 拒绝 / 接受 | U `a package without an app ...`、`two apps ...`、`one app ...` |
| C-UP-15 | 安装位置可写 / App Translocation / 不可写 | 接受 / 拒绝 / 拒绝 | U `writable install location ...`、`App Translocation ...`、`non-writable ...` |

### 3.3 测试夹具

- 用当前 1.0.1（Swift 版）生成一份 `store.json` 和对应的凭据文件，放到 `crates/switchboard-core/tests/fixtures/v1.0.1/`。邮箱和 token 用假数据。
- 一份 Pen 会话文件样例（含 `lastOnlineAt`、`currentWorkspace` 等额外字段），验证移植后只读写需要的字段、其他字段原样保留。

### 3.4 通过标准

- 上面全部用例通过，两平台 CI 都绿。
- 断言总数不少于 Swift 版（当前 `RegressionTests` + `UpdateLogicTests` 的输出里有 "N/M checks passed"，以 M 为下限）。

---

## 4. L2 平台集成测试

| 编号 | 平台 | 场景 | 期望 |
|---|---|---|---|
| P-FS-01 | 双端 | 原子写被中断（写临时文件后模拟失败） | 原文件不变，没有残留的半个文件 |
| P-FS-02 | Windows | 另一个句柄以不共享方式打开目标文件 | 重试约 1 秒后报错；对方及时关闭则写入成功 |
| P-FS-03 | mac | 新建凭据目录和文件 | 0700 / 0600 |
| P-AS-01 | mac | `SMAppService` 状态读取 | 和系统设置里登录项一致（手工核对一次） |
| P-AS-02 | Windows | 写入、读取、删除 `Run` 值（值名 `SwitchboardCI`） | 读到的状态和写入一致 |
| P-AS-03 | Windows | `StartupApproved\Run` 标为禁用 | 读取结果为"未开启" |
| P-PR-01 | Windows | 无 Pen 进程时查询 | 返回空，退出操作直接成功 |
| P-PR-02 | Windows | 用一个假进程（CI 里编一个名为 `Pen.exe` 的小程序，收到 `WM_CLOSE` 就退出） | 能找到、能关闭、能拉起 |
| P-PR-03 | Windows | 假进程忽略 `WM_CLOSE` | 超时返回失败，进程仍在（没有强杀） |
| P-PR-04 | Windows | 假进程开两个标题为 `pen.dev` 的窗口，`config.json` 的 `windowBounds` 指向其中一个 | 只给匹配的那个发 `WM_CLOSE` |
| P-PR-05 | Windows | 两个窗口都不匹配 | 不发任何消息，返回"无法确定主窗口" |

---

## 5. L3 / L4 界面测试

### 5.1 组件测试（Vitest）

- 每个组件至少覆盖：正常、禁用、长文本截断、缺数据（额度显示"— / — 天"）。
- 操作锁规则表（对照 `PanelController.syncOperationLock`）逐行写成用例：

| 状态 | 切换按钮 | 添加 / 重新导入 | 取消网页登录 | 移除 / 导入 |
|---|---|---|---|---|
| 刷新中 | 禁用 | 禁用 | 禁用 | 可用 |
| 切换中 | 禁用 | 禁用 | 禁用 | 可用 |
| 下载或准备安装更新 | 禁用 | 禁用 | 可用 | 可用 |
| 添加账号中 | 禁用 | 禁用 | 可用 | 可用 |
| 存储只读 | 不变 | 禁用 | 不变 | 禁用 |

- 头像颜色：给定邮箱，`stableHash` 选中的颜色和 Swift 一致（取 5 个邮箱对照）。
- 反馈条：前端不自己计时；收到清除事件才消失。

### 5.2 视觉回归（Playwright）

- **场景**：和 `SnapshotScenarios.swift` 同名的 22 个场景，`ViewState` 由 core 的场景注入生成（DEV_PLAN T1.7），不要在前端手写。
- **截图**：WebKit，设备像素比 2，宽 410，高度取内容高度。
- **比对**：和 `Tests/baseline/macos/panel-<场景>.png` 比较。
  - 尺寸必须完全一致（高度差 0）
  - 像素差异用 `pixelmatch`，阈值 0.1；差异像素占比 ≤ 1%
  - 允许的例外（对应 DEV_PLAN 第 9 节）：
    - 图标区域（SF Symbols → Lucide）单独圈出，不计入
    - `main-unregistered-live` 场景里「导入」按钮区域不计入，另外断言该按钮文字颜色为 `#27272A`
    - `manage-confirm-remove` 场景只比较文案和按钮顺序，不比像素
- **失败时**：上传差异图作为 CI 产物。
- **人工复核**：P3 结束时，把 22 组"基线 / 新版 / 差异"拼成一页给人看一遍，确认图标替换的观感。
- **Windows**：没有基线。在 Windows CI 上截同样的 22 张图作为产物，人工看一次字体和布局有没有溢出，然后把它们存为 `Tests/baseline/windows/`，之后 Windows 用自己的基线做回归。

### 5.3 面板行为（mac，可自动化的部分）

| 编号 | 场景 | 期望 |
|---|---|---|
| UI-PN-01 | 内容变化 | 面板高度跟着变，夹在 200–900，顶部锚点不动 |
| UI-PN-02 | 打开 0.35 秒内再次点击图标 | 忽略 |
| UI-PN-03 | Esc | 收起；确认框打开时只关确认框 |
| UI-PN-04 | 切换中 / 添加中 / 下载更新中点面板外 | 不收起 |
| UI-PN-05 | 每次打开面板 | 回到主页 |
| UI-PN-06 | 刷新按钮动画 | 开始和结束时按钮位置不变；重复渲染不重启动画；开启"减少动态效果"时不旋转 |

---

## 6. L5 Pen 兼容性 与 L6 手工验收

### 6.1 自动（`pen-compat.yml`）

步骤和断言见 DEV_PLAN T0.4。每周跑一次；Pen 发新版后手动触发一次。任何一步失败都要开 issue，并在修复前暂停发布。

### 6.2 手工验收清单

在真机 Mac 和 Windows 11 ARM 虚拟机上各跑一遍。准备两个测试用 Pen 账号（A、B）。每项记录：通过 / 失败 / 截图。

| 编号 | 步骤 | 期望 | Mac | Win |
|---|---|---|---|---|
| M-01 | 全新安装后首次启动 | 托盘出现图标，不进 Dock / 任务栏 | ☐ | ☐ |
| M-02 | Pen 已登录 A，点"导入 Pen 当前登录" | A 出现在列表并标为当前 | ☐ | ☐ |
| M-03 | 网页添加 B（浏览器里已登录 A 的情况下） | 授权页是全新会话，能登录 B；添加后自动切到 B | ☐ | ☐ |
| M-04 | Pen 运行中，从 B 切回 A | Pen 退出、重启，登录身份为 A | ☐ | ☐ |
| M-05 | Pen 未运行时切换 | 不启动多余进程，切换成功（是否拉起 Pen 与 1.0.1 一致） | ☐ | ☐ |
| M-06 | Pen 里有未保存文档时切换 | Pen 弹保存提示；超时后切换器提示"请先退出 Pen"，会话文件未改动 | ☐ | ☐ |
| M-07 | Win：打开一个弹出的文档窗口后切换 | 能找到主窗口并正常退出，或明确提示手动退出；不会只关掉一个标签 | — | ☐ |
| M-08 | 刷新额度 | 数字和 Pen 里显示的一致 | ☐ | ☐ |
| M-09 | 在 Pen 里退出登录后打开面板 | 当前账号清空 | ☐ | ☐ |
| M-10 | 移除非当前账号 | 出现确认框；确认后消失，凭据文件被删除 | ☐ | ☐ |
| M-11 | 开启 / 关闭开机自启，重启系统 | 实际行为和开关一致；在系统设置 / 任务管理器里改动后，开关状态跟着变 | ☐ | ☐ |
| M-12 | 多显示器、把托盘图标移到副屏 | 面板贴着图标出现，不越出屏幕 | ☐ | ☐ |
| M-13 | Win：任务栏放在上 / 下 / 左 / 右 | 面板都在图标旁边，不被任务栏挡住 | — | ☐ |
| M-14 | Win：切换系统深色 / 浅色 | 托盘图标跟着换 | — | ☐ |
| M-15 | mac：全屏 App 上打开面板 | 面板出现在全屏空间上 | ☐ | — |
| M-16 | 高 DPI（Win 150% 缩放） | 面板清晰、尺寸正确 | — | ☐ |
| M-17 | 双击 App 再次打开 | 显示面板，不启动第二个实例 | ☐ | ☐ |
| M-18 | 演示模式 `SWITCHBOARD_DEMO=1` | 三个演示账号；所有操作提示"演示模式不操作真实账号" | ☐ | ☐ |

---

## 7. L7 升级演练

在测试仓库或草稿 Release 上进行，不要直接在正式仓库发版。

| 编号 | 起点 | 终点 | 期望 |
|---|---|---|---|
| UP-01 | mac 1.0.1（Swift，已有 3 个账号、开了自启） | 2.0.0-rc（Tauri） | 横幅提示更新 → 下载百分比 → 安装并重启 → 账号、当前账号、自启状态全部保留 |
| UP-02 | mac 2.0.0-rc | 2.0.1-rc | 同上 |
| UP-03 | Win 2.0.0-rc | 2.0.1-rc | 静默安装后自动重启，账号保留 |
| UP-04 | mac 1.0.1 | 一个同时含 Windows 资产的 Release | 1.0.1 只下载 mac zip（C-UP-11 的实机版本） |
| UP-05 | 签名被篡改的 Release | — | 两端都拒绝安装，提示"签名校验失败" |

---

## 8. 各阶段的验收门槛

| 阶段结束 | 必须通过 |
|---|---|
| P0 | `pen-compat.yml` 全绿；基线 22 张已入库；虚拟机可用 |
| P1 | 第 3 节全部用例；断言数 ≥ Swift 版 |
| P2 | 5.3 面板行为；M-01、M-04、M-15、M-17（mac） |
| P3 | 5.1、5.2 全部通过；人工复核图标观感 |
| P4 | 第 4 节全部；6.2 的 Windows 列全部通过 |
| P5 | 第 7 节全部 |
| P6（发布） | 以上全部 + 最近一次 `pen-compat.yml` 通过 |

## 9. 缺陷记录

- 发现的问题记在 `docs/cross-platform/ISSUES.md`，格式：编号、发现阶段、用例编号、复现步骤、实际 / 期望、是否阻塞发布。
- 和 1.0.1 行为不一致的，一律按缺陷处理；确实要改行为的，需产品确认后在 DESIGN 里注明。
