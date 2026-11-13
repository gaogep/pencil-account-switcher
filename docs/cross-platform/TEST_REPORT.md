# 测试报告：跨平台重构 P0–P1 阶段（分支 `t1-7-app-model`）

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

