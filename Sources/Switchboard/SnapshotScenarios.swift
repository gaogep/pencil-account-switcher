import AppKit

/// 视觉基线的状态清单。用法：
///   SWITCHBOARD_DEMO=1 SWITCHBOARD_SNAPSHOT=<目录>/panel "<App>/Contents/MacOS/Switchboard"
/// 每个状态输出 <前缀>-<name>.png。跨平台重构时，新界面要在同样的状态下和这些图逐张比对，
/// 所以这里的文案都取自 AppModel 里真实出现的提示。
struct SnapshotScenario {
    let name: String
    let setup: @MainActor (AppModel) -> Void
    /// 非 nil 时截确认框而不是面板
    var alertEmail: ((AppModel) -> String?)? = nil
}

@MainActor
enum SnapshotScenarios {
    /// 固定的额度更新时间（本地 10:24），页脚显示「额度更新于 10:24」
    static let quotaTime: Date = {
        var components = DateComponents()
        components.year = 2026
        components.month = 11
        components.day = 13
        components.hour = 10
        components.minute = 24
        return Calendar.current.date(from: components) ?? Date(timeIntervalSince1970: 0)
    }()

    private static let release = AppRelease(
        version: "1.1.0",
        zipURL: URL(string: "https://example.invalid/Pencil-Account-Switcher-1.1.0-arm64.zip")!,
        pageURL: URL(string: "https://example.invalid/releases/v1.1.0")!)

    private static func other(_ model: AppModel, _ index: Int = 0) -> AccountRecord? {
        model.accounts.filter { !$0.isCurrent }.dropFirst(index).first
    }

    static let all: [SnapshotScenario] = [
        // 三个页面的默认状态
        SnapshotScenario(name: "main") { _ in },
        SnapshotScenario(name: "settings") { $0.route = .settings },
        SnapshotScenario(name: "settings-launch-on") { model in
            model.settings.launchAtLogin = true
            model.route = .settings
        },
        SnapshotScenario(name: "manage") { $0.route = .manage },

        // 主页的各种内容
        SnapshotScenario(name: "main-empty") { $0.accounts = [] },
        SnapshotScenario(name: "main-single") { model in
            model.accounts = model.accounts.filter(\.isCurrent)
        },
        SnapshotScenario(name: "main-unregistered-live") { $0.liveEmail = "new.member@example.com" },
        SnapshotScenario(name: "main-expired") { model in
            guard let target = other(model) else { return }
            model.accounts = model.accounts.map { record in
                var record = record
                if record.id == target.id {
                    record.authState = .expired
                    record.lastError = "登录已失效"
                    record.usedDays = nil
                    record.totalDays = nil
                }
                return record
            }
        },
        SnapshotScenario(name: "main-unknown-quota") { model in
            model.accounts = model.accounts.map { record in
                var record = record
                if !record.isCurrent {
                    record.authState = .unknown
                    record.usedDays = nil
                    record.totalDays = nil
                    record.lastQuotaUpdate = nil
                }
                return record
            }
        },

        // 进行中的操作
        SnapshotScenario(name: "main-switching") { model in
            model.switchingTo = other(model)?.id
        },
        SnapshotScenario(name: "main-refreshing") { model in
            model.refreshing = true
            model.showFeedback("正在刷新额度…", kind: .loading, persistent: true)
        },
        SnapshotScenario(name: "main-adding") { model in
            model.addingAccount = true
            model.showFeedback("请在网页登录要添加的账号", kind: .loading, persistent: true)
        },

        // 反馈条
        SnapshotScenario(name: "main-feedback-success") { model in
            model.showFeedback("额度已刷新", kind: .success, persistent: true, caption: "刚刚更新")
        },
        SnapshotScenario(name: "main-feedback-warning") { model in
            model.showFeedback("额度已刷新，1 个账号更新失败", kind: .warning, persistent: true)
        },
        SnapshotScenario(name: "main-feedback-long") { model in
            model.showFeedback(
                "已切换到 jamie.work@outlook.com，但 Pen 启动失败，请手动打开：找不到 Pen，请先安装后再切换",
                kind: .warning, persistent: true)
        },
        SnapshotScenario(name: "main-persistence-blocked") { model in
            model.blockPersistenceForSnapshot("账号列表已损坏，已备份且不会覆盖原文件")
        },

        // 更新横幅
        SnapshotScenario(name: "main-update-available") { model in
            model.updateState = .available(release, silenced: false)
        },
        SnapshotScenario(name: "main-update-downloading") { model in
            model.updateState = .downloading(release, percent: 42, received: 4_404_019, total: 10_485_760)
        },
        SnapshotScenario(name: "main-update-ready") { model in
            model.updateState = .ready(release, unpackedApp: URL(fileURLWithPath: "/tmp/Pencil Account Switcher.app"))
        },
        SnapshotScenario(name: "settings-update-available") { model in
            model.updateState = .available(release, silenced: false)
            model.route = .settings
        },

        // 管理页
        SnapshotScenario(name: "manage-expired") { model in
            guard let target = other(model, 1) else { return }
            model.accounts = model.accounts.map { record in
                var record = record
                if record.id == target.id { record.authState = .expired }
                return record
            }
            model.route = .manage
        },
        SnapshotScenario(name: "manage-confirm-remove",
                         setup: { $0.route = .manage },
                         alertEmail: { other($0)?.email }),
    ]
}
