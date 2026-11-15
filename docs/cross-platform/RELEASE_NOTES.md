# 2.0.0

Pencil Account Switcher 改为 Tauri 2 + Rust + Preact，macOS 沿用原来的账号数据和更新签名。新增 Windows x64 测试版。

- 两端支持导入 Pen 当前登录、网页登录添加账号、切换账号、查询额度和移除账号。
- macOS 菜单栏面板和 Windows 托盘面板；Windows 支持浅/深色图标、工作区/DPI 定位与单实例。
- 更新包沿用 Ed25519 验签；macOS 原子替换并重启，Windows 按用户静默安装并重启。
- macOS 1.0.1 的数据目录、Bundle ID、账号文件格式及开机自启方式保留。

macOS 需要 Apple 芯片和 macOS 13+。Windows 为 x64 beta，需要 Windows 10/11 与 WebView2；ARM Windows 通过 x64 模拟运行仍需实机验收。安装包未做 Apple 公证或 Windows 代码签名，首次打开方法见 README。

发布前需完成 `UPGRADE_REHEARSAL.md` 的实机项目和验收记录。草稿 Release 用于验证打包和签名，不代表这些项目已通过。
