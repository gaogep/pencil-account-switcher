# 升级演练

正式发布前先运行草稿流水线，再在隔离环境验证升级。工作流不会把演练的下一版本发布到 Release，也不会修改本地工作区版本。

## 生成候选包

```sh
gh workflow run release.yml --ref t5-2-release-pipeline -f tag=v2.0.0
```

`tauri.conf.json` 和应用 `Cargo.toml` 必须与 tag 匹配。手动触发创建草稿；已公开的同名 Release 会被拒绝覆盖。流水线使用现有 `UPDATE_SIGNING_KEY`，核对旧公钥、完整资产名和签名。mac 更新 ZIP 保留 `-arm64.zip`；Windows 只有 `-windows-x64-setup.exe`，各自有 `.sig`。

## 自动化升级验证

`t5-3-upgrade-rehearsal` 分支推送会自动执行默认的 2.0.0 → 2.0.1 演练。工作流进入默认分支后，也可在候选草稿生成后手动运行：

```sh
gh workflow run upgrade-rehearsal.yml --ref t5-3-upgrade-rehearsal \
  -f previous_version=2.0.0 -f next_version=2.0.1
```

- macOS：从草稿下载已签名旧 ZIP；在 runner 内构建并签名下一版；使用 core 原公钥验证两个包，解包并校验 ID/版本，在临时目录实际调用原子替换 API，再检查新包签名及原账号夹具文件字节不变。不会启动真实候选应用。
- Windows：验证原公钥签名，静默安装旧 NSIS 到带空格的临时目录；放入三个假账号和假凭据；通过生产辅助脚本覆盖安装下一版并重启。运行演示模式，检查 exe 版本、进程路径、替换结果、账号文件 SHA256 和暂存清理。只在隔离 CI runner 执行，结束后移除测试安装。
- `set_rehearsal_version.mjs` 只在 Actions 的临时 checkout 中修改配置、Cargo 及锁文件的应用版本；不改源分支的 2.0.0。
- 产物包括下一版签名包和 Windows JSON 结果。它们是演练产物，不是公开版本。

普通检查：`cargo test --workspace --features platform-it --locked`、`npm test`。mac 真实签名包测试默认 `ignored`，只有该工作流提供两个包后才执行；不能把未执行记为通过。

## 仍需实机验收

自动化夹具不能代替 TEST_PLAN 第 7 节完整的用户路径：

| 项目 | 自动化覆盖 | 发布前仍需验证 |
|---|---|---|
| UP-01 Swift 1.0.1 → Tauri | 数据格式、旧签名格式和资产选择兼容性回归 | 旧客户端从测试 feed 看到横幅、下载、安装/重启，三个真实测试账号及自启状态保留 |
| UP-02 Tauri → Tauri（mac） | 真实签名包解包、校验、原子替换、数据夹具不变 | 面板操作、退出重启和 SMAppService 的真实状态保留 |
| UP-03 Tauri → Tauri（Win） | 真实 NSIS 覆盖、生产辅助脚本、重启和三个账号夹具 | 从更新横幅触发全流程；真实测试账号、系统禁用自启和用户权限场景 |
| UP-04 同时含 mac/Win 资产 | core 测试验证旧规则只选择 arm64 ZIP | 在旧 Swift 客户端从测试 feed 实际下载一次 |
| UP-05 篡改签名 | core 和 Node 回归均拒绝篡改，安装准备之前终止 | 两端 UI 的提示与保留旧应用/数据 |

使用专门测试账号和备份。T0.5 Windows 11 ARM 虚拟机与 TEST_PLAN 的多屏、高 DPI、真实登录、切号等人工项目仍需有可用设备后执行。原 Swift 源码、旧测试和 mac 视觉基线保留至验收完成；正式 `v2.0.0` 发布和删旧代码应在这些记录具备后进行。
