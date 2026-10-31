# Pencil Account Switcher

macOS 菜单栏小工具，用来在 Pen（Pencil 桌面端）多个账号之间切换。需要 Apple 芯片（M 系列）、macOS 13 或更高，以及已安装的 [Pen](https://pencil.dev)。

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

这是菜单栏应用，Dock 里不会常驻。账号只存在你这台电脑上。

有新版本时，打开面板会看到横幅，点「立即更新」即可；下载过程会显示百分比。
