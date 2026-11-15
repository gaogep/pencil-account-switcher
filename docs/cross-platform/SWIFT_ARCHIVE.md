# Swift 存档与清理范围

`swift-final` 指向 `f9d27103a0f37c47d36fb40830615d69daada6fa`。该提交保存最终 Swift 实现、旧打包脚本、Swift 回归以及 macOS 视觉基线。Tauri 后续开发没有改写这些内容。

当前保留以下目录和入口，以便完成 Swift 1.0.1 → Tauri 的真实升级验收：

- `Sources/`：旧 Swift 应用与测试状态清单。
- `Support/`：原 Info.plist、公钥、图标、打包和签名脚本；其中图标、公钥和签名兼容性基准仍被新实现或测试使用，不能整目录删除。
- `Tests/` 与 `build.sh`：旧回归和基线生成入口；`Tests/baseline/macos/` 继续作为新前端的视觉验收基线。

旧代码删除前需先列出具体文件，并同步调整 `test.yml` 的 Swift 作业、发布工具的签名兼容性测试及场景导出入口。不能只删 `Sources/` 后把仍有依赖的回归留在失败状态。

本轮不删除以上内容。`Tests/baseline/macos/`、`scripts/check_pen_compat.mjs`、原更新公钥、现有用户数据和测试报告不在未来清理范围内。正式发布和旧代码清理在升级验收记录齐备后进行。
