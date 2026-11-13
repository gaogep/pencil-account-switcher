# 开发交接：停止于 T3.2

当前停止点由用户确认：T3.2 组件库写完即停，不进入 T3.3。按 DEV_PLAN 的 26 个任务等权估算，已有基线成果或代码的任务为 16 个，约 62%；这不是验收通过率。

## 本轮本地提交

- T2.1 `4ebb443`：macOS 原生平台服务、独立 Cookie 认证窗口；保留旧数据目录和 bundle 标识。
- T2.2 `142dbaf`：Tauri 壳、模板托盘、NSPanel 定位与收起、退出落盘。
- T2.3 `678ceb7`：命令和事件桥接、Rust 生成的 TypeScript 类型、确认弹层的面板保留接口。Tauri 自动生成的 schema 已移出版本控制。
- T3.1 `f0c7fc3`：Vite/Preact/TypeScript、Theme CSS、真实/模拟 IPC、22 份 core 场景 JSON。
- T3.2：14 类组件、原品牌 PNG、开发组件目录，见当前 `t3-2-component-library` 分支。

以上提交按顺序依赖，当前分支包含全部实现。尚未推送或合并本轮开发。

## 可用开发入口

- `npm ci` 后用 `npm run dev:ui` 开启模拟 IPC 模式；`/?mock=main&gallery` 显示组件目录，可将 main 换为 ui/public/scenarios/ 中的场景名。组件目录只展示输入状态，按钮不模拟业务操作。
- `npm run build:ui` 做 TypeScript 检查和前端打包。
- `npm run dev:tauri --workspace ui` 启动真实 IPC 前端开发服务；另一个终端运行 `cargo run -p switchboard-app` 启动开发壳。macOS 原生接口要求应用主事件循环运行。
- `npm run types:ipc` 重新生成 ui/src/generated/；`npm run scenarios:ui` 重新导出 core 场景数据，需要 cargo 在 PATH 中。

## 检查与边界

本轮只执行编译、Clippy、前端打包和开发资产导出，没有执行测试用例，没有启动应用做真实账号操作，没有浏览器或视觉回归检查。

- T3.3 的 MainScreen / SettingsScreen / ManageScreen、移除确认框和完整路由尚未组装；默认应用内容目前只有品牌头部。
- 自动更新的原生安装器留待 P5；P2 开发壳暂时不启用自动更新检查，避免把未接入的安装操作暴露为可用功能。
- Windows 平台实现和 Windows 应用壳留待 P4；本轮提供的原生实现只适用于 macOS。
- 需要后续验证：托盘/面板焦点和屏幕定位、独立会话登录、自启系统授权、退出落盘、IPC 权限与事件、组件视觉与禁用状态、减少动态效果。
- 本轮新任务的 PR 与功能验收均未完成；这里的“写完”仅表示开发交付状态。
