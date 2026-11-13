# 问题与发现记录

格式见 [TEST_PLAN.md](TEST_PLAN.md) 第 9 节。

| 编号 | 发现阶段 | 相关 | 描述 | 状态 |
|---|---|---|---|---|
| I-001 | P0 基线 | 基线 `panel-main-unregistered-live.png` | 主页"检测到未登记的登录身份"提示里的「导入」按钮文字用了 `Theme.switchBtnText`（白色），在浅色底上几乎看不见（`Screens.swift` `makeUnregisteredHint`）。1.0.1 现有问题 | 已确认：新版改用 `Theme.linkText`，旧版不改（DEV_PLAN 第 9 节） |
| I-002 | P0 基线 | 基线 `panel-manage-confirm-remove.png` | 确认框截图只有内容视图，没有系统 sheet 背景，只能用来核对文案和按钮顺序 | 已知限制 |
| I-003 | P0 调研 | Pen `desktop-app-host.js` | 会话文件不存在时，Pen 会退回读取旧的 `~/.pencil/license-token.json`。切号器总是写会话文件，所以不受影响；但如果用户机器上残留这个旧文件，"Pen 退出登录"后身份判断可能和 Pen 不一致。1.0.1 也是如此 | 记录，暂不处理 |
| I-004 | P0 调研 | Pen `app-window.js` | Windows 上 Pen 的主窗口和弹出文档窗口标题、类名相同。给弹出窗口发 `WM_CLOSE` 只会关一个标签。主窗口判定规则见 DEV_PLAN T0.3 | 待 T0.4 验证 |
