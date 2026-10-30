# Pencil Account Switcher

macOS 菜单栏小工具，用来在 Pen（Pencil 桌面端）多个账号之间切换。

只要 **Apple 芯片**（M 系列）和 **macOS 13** 或更高。需要先安装 [Pen](https://pencil.dev)。

## 给朋友：从这里安装

1. 打开 [Releases](https://github.com/gaogep/pencil-account-switcher/releases/latest)，下载 `Pencil-Account-Switcher-*-arm64.dmg`
2. 打开 DMG，把 **Pencil Account Switcher** 拖进 **应用程序**
3. **第一次**不要双击。在启动台或应用程序文件夹里 **右键 → 打开 → 仍要打开**  
   （没有 Apple 公证，系统会拦一次；点过一次以后就可以正常打开）
4. 菜单栏出现图标后，左键打开面板，点「添加账号」用网页登录

这是菜单栏应用，Dock 里不会常驻。账号只存在你这台电脑上。

有新版本时，打开面板会看到横幅，点「立即更新」即可；下载过程会显示百分比。

## 给开发者

```bash
./build.sh          # 产出 build/Pencil Account Switcher.app
./Support/make_dmg.sh
./Tests/run.sh
```

只依赖 Xcode Command Line Tools。打 `v*` 标签会触发 GitHub Actions，把 DMG 和 zip 挂到 Release。

凭据存在 `~/Library/Application Support/Switchboard/`，不进仓库。
