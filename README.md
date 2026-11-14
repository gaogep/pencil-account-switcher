# Pencil Account Switcher

macOS 菜单栏 / Windows 托盘工具，用来在 [Pen（Pencil 桌面端）](https://pencil.dev) 多个账号之间切换，支持网页登录、导入当前登录、额度查询、账号管理和签名更新。

2.0.0 为待验收候选，Windows x64 标为 beta。当前公开稳定版 1.0.1 仅支持 macOS；2.0 候选包通过草稿 Release 和 CI 产物交付，正式发布前的验证范围见 [开发进度](docs/cross-platform/DEV_PROGRESS.md) 与 [升级演练](docs/cross-platform/UPGRADE_REHEARSAL.md)。

## macOS 安装

需要 Apple 芯片（M 系列）、macOS 13 或更高，以及已安装的 Pen。

1. 打开 [Releases](https://github.com/gaogep/pencil-account-switcher/releases/latest)，下载 `Pencil-Account-Switcher-*-arm64.dmg`
2. 打开 DMG，把 **Pencil Account Switcher** 拖进 **应用程序**
3. **第一次**不要双击。没有 Apple 公证，系统会写「可能有危害 / 无法验证开发者」，按下面做一次即可：
   - 在启动台或应用程序里 **右键 → 打开**。如果有 **仍要打开**，点它
   - 如果只有「移到废纸篓」或「取消」：打开 **系统设置 → 隐私与安全性**，拉到下面，点 **仍要打开**，再输入开机密码
   - 还不行就打开「终端」，整行粘贴后回车，然后再右键打开一次：

     ```
     xattr -cr "/Applications/Pencil Account Switcher.app"
     ```
4. 菜单栏出现图标后，左键打开面板，点「添加账号」用网页登录

这是菜单栏应用，Dock 里不会常驻。切换器的账号列表和登录快照保存在本机。

有新版本时，打开面板会看到横幅，点「立即更新」即可；下载过程会显示百分比。


## Windows 安装（x64 beta）

需要 Windows 10/11 x64、WebView2 Runtime 和 Pen。Windows 11 ARM 上的 x64 模拟运行仍需实机验收。

1. 从本仓库已验收的 Release 或指定候选产物下载 `Pencil-Account-Switcher-<版本>-windows-x64-setup.exe`。
2. 运行安装包，按当前用户安装，无需管理员权限。未签名包可能显示 SmartScreen；确认来源后点「更多信息 → 仍要运行」。
3. 从开始菜单启动，在任务栏通知区域找到图标；若图标被隐藏，展开通知区域查看。
4. 左键打开面板，使用「添加账号」或导入 Pen 当前登录。右键菜单可以打开面板或退出。

Windows 面板不常驻任务栏。更新包验签通过后，辅助进程等待旧应用退出、静默覆盖安装并重启；安装失败会在下一次启动显示原因。

## 数据与兼容性

| 内容 | macOS | Windows |
|---|---|---|
| 切换器数据 | `~/Library/Application Support/Switchboard/` | `%LOCALAPPDATA%\Switchboard\` |
| Pen 登录快照 | `~/.pencil/session-desktop.json` | `%USERPROFILE%\.pencil\session-desktop.json` |
| 开机自启 | `SMAppService` | 当前用户 `Run`，同时读取系统禁用状态 |

macOS 1.0.1 的数据格式、Bundle ID、更新公钥和 ZIP 命名保持兼容。切换时只请求 Pen 正常退出；若 Pen 未退出，切换器提示先退出 Pen。

## 开发与验证

使用 Rust stable、Node.js 24；原生构建还需要 macOS Xcode 命令行工具或 Windows MSVC/Windows SDK。

```sh
npm ci
npm test
npm run build:ui
cargo fmt --all --check
cargo clippy --workspace --all-targets --features platform-it --locked -- -D warnings
cargo test --workspace --features platform-it --locked
```

模拟界面用 `npm run dev:ui`，原生开发用 `npm run tauri -- dev`。`SWITCHBOARD_DEMO=1` 启用演示账号；视觉回归先执行 `npx playwright install webkit`，再运行 `npm run test:visual`。原始 mac 基线保留在 `Tests/baseline/macos/`。

构建安装包：

```sh
npm run tauri -- build --ci --target aarch64-apple-darwin -- --locked
# Windows 主机：
npm run tauri -- build --ci --target x86_64-pc-windows-msvc -- --locked
```

`release.yml` 手动运行创建草稿；通过验收后，匹配应用版本的 `v*` tag 触发正式发布。更新签名沿用仓库 `UPDATE_SIGNING_KEY`，无需为重写换钥。Windows 资产不能使用 `arm64.zip` 命名。完整发布与演练步骤见 [UPGRADE_REHEARSAL.md](docs/cross-platform/UPGRADE_REHEARSAL.md)。

旧 Swift 实现与测试暂保留，用于升级对照与签名兼容性测试；清理前的存档和范围见 [SWIFT_ARCHIVE.md](docs/cross-platform/SWIFT_ARCHIVE.md)。
