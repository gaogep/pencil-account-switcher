# 问题与发现记录

格式见 [TEST_PLAN.md](TEST_PLAN.md) 第 9 节。

| 编号 | 发现阶段 | 相关 | 描述 | 状态 |
|---|---|---|---|---|
| I-001 | P0 基线 | 基线 `panel-main-unregistered-live.png` | 主页"检测到未登记的登录身份"提示里的「导入」按钮文字用了 `Theme.switchBtnText`（白色），在浅色底上几乎看不见（`Screens.swift` `makeUnregisteredHint`）。1.0.1 现有问题 | 已确认：新版改用 `Theme.linkText`，旧版不改（DEV_PLAN 第 9 节） |
| I-002 | P0 基线 | 基线 `panel-manage-confirm-remove.png` | 确认框截图只有内容视图，没有系统 sheet 背景，只能用来核对文案和按钮顺序 | 已知限制 |
| I-003 | P0 调研 | Pen `desktop-app-host.js` | 会话文件不存在时，Pen 会退回读取旧的 `~/.pencil/license-token.json`。切号器总是写会话文件，所以不受影响；但如果用户机器上残留这个旧文件，"Pen 退出登录"后身份判断可能和 Pen 不一致。1.0.1 也是如此 | 记录，暂不处理 |
| I-004 | P0 调研 | Pen `app-window.js` | Windows 上 Pen 的主窗口和弹出文档窗口标题、类名相同。给弹出窗口发 `WM_CLOSE` 只会关一个标签。主窗口判定规则见 DEV_PLAN T0.3 | 待 T0.4 验证 |
| I-005 | P0/P1 测试 | T0.3 / T1.1 / `test.yml` | `cargo clippy --workspace --all-targets --locked -- -D warnings` 在 macOS（`store.rs:39`、`update.rs:180,184`）和 Windows `pen-probe`（`windows.rs:289,290,295`）各报 3 个 error，阻塞 CI | 已修复（复测通过）：macOS 工作区及 Windows `pen-probe` 目标 Clippy 均 0 警告通过；`switchboard-core` Win 目标需待 Windows CI 验证 |
| I-006 | P1 测试 | TEST_PLAN §3（`C-*`） | `crates/switchboard-core/tests/` 缺失，`cargo test` 仅运行 0 个测试，缺少 5 个测试替身与 `v1.0.1` 夹具（未满足 ≥ Swift 版 172 条断言门槛） | 已修复（macOS 复测通过）：已补齐测试替身、`v1.0.1` 夹具与 26 个单测（202 条静态断言 ≥ 172），`cargo test --workspace --locked` 全绿；仍待 Windows CI 跑通 |
| I-007 | P1 测试 | `C-AC-03` / T1.5 `web_login.rs` | `WebLogin::sign_in_inner` 先 `self.window.open(url).await?` 再 `listener.accept().await`：串行等待会导致等待关窗时死锁，或立即返回时用户关窗无法触发取消 | 已修复（复测通过）：`WebLogin::with_api_base` 离线端到端测试（`c_wl_04_05_and_i007_full_offline_web_login_flow`）验证正常登录回调、令牌交换完成、关窗立即取消及退出后关闭监听 |
| I-008 | P1 测试 | `C-UP-04/05/08` / T1.6–T1.7 | `AppModel` 未调用 `update.rs` 的 `UpdateSchedule`、验签与下载编排，而是全权委托给 `AppServices`，core 层无法约束与测试“未验签不解压”和“1 小时退避” | 已修复（core 层复测通过）：已补验 `C-UP-07`（测试公钥验签通过后 `prepare_update` 仅执行一次并进入 `Ready`、触发 `InstallUpdate`）与 `C-UP-09/10`（`parse_release` Mac/Win 资产筛选）；`C-UP-06/14/15` 属 P5 安装器 |
| I-009 | P1 测试 | `C-AC-05` / `app_model.rs:645-657` | 导入当前登录时 `save_login(..., false)` 先设置成功提示，紧接着同步调用 `begin_refresh()` 覆写为「正在刷新额度…」，前一条成功提示在 `publish()` 前被吞掉 | 已修复（复测通过）：成功提示先发布保留 1 秒（期间 `refreshing=true` 保持互斥），随后再切换为「正在刷新额度…」 |
| I-010 | P1 测试 | `C-ST-08` / `store.rs:152-163` | Rust `f64` 将整秒时间戳输出为 `816229440.0` 且字段顺序与 Swift `JSONEncoder` 不同，1.0.1 `store.json` 往返语义等价但字节不完全一致 | 已复测确认：无修改往返保留原文件字节，修改重编码时整秒日期已输出为整数（无 `.0`）；键序不保证逐字节相同属预期限制（Swift 默认编码不承诺固定键序） |
| I-011 | P0 测试 | `P-PR-02` / `tools/pen-probe/src/windows.rs:92-119` | `processes()` 遍历快照时若某个 `Pen.exe` 恰好退出导致 `OpenProcess` 返回 `NULL`，会中断整个枚举并导致 `quit` 轮询提前报错 | 代码复测通过，待 Windows 实测：已跳过 `ERROR_INVALID_PARAMETER` 与非 `STILL_ACTIVE` 退出进程，Win Clippy 通过 |
| I-012 | P0 仓库卫生 | `p0-baseline-compat` (`8b8ef32`) | `p0-baseline-compat` 分支误提交了 `node_modules/` 与 `target/` 共 146 个文件（`origin/main` @ `874dc34` 未包含该提交） | 已修复（复测通过）：`origin/p0-baseline-compat` @ `57519d5` 已移除全部 146 个缓存文件 |
| I-013 | P1 复测 | `C-AC-04` / `C-WL-04/05` / `C-UP-07/09/10` | L1 可测性与细节缺口：(1) `update::PUBLIC_KEY` 为常量且 `fetch_latest` 硬编码 GitHub URL（资产选择逻辑未独立导出），导致 `C-UP-07` 与 `C-UP-09/10` 无法在 L1 离线测；(2) `WebLogin` 硬编码 `api.pencil.dev`、首条有效回调即丢弃监听（`C-WL-05` 返回连接拒绝而非 400）、超 16KB 请求直接 `continue` 未回 400（`C-WL-04`）；(3) `begin_login` 300s 超时硬编码且 `Cargo.toml` 未启用 `tokio/test-util`（`C-AC-04`） | 已修复（复测通过）：`c_ac_04_web_login_300s_timeout`、`c_wl_04_05_and_i007_full_offline_web_login_flow`（>16KB 回 400、令牌交换期重复回调回 400）、`c_up_07_09_10_signed_ready_install_and_asset_selection` 全部在 `cargo test --workspace --locked` 通过 |
