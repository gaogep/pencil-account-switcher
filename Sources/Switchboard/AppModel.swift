import AppKit
import Combine
import Foundation
import ServiceManagement

enum PanelRoute: Equatable {
    case main
    case settings
    case manage
}

enum FeedbackKind: Equatable {
    case success
    case loading
    case warning
}

enum SwitchOutcome: Equatable {
    case completed
    case skipped
    case failed(String)
}

@MainActor
final class AppModel: ObservableObject {
    // MARK: - 状态

    @Published var accounts: [AccountRecord] = [] {
        didSet { persist() }
    }
    @Published var settings = AppSettings()
    @Published var route: PanelRoute = .main
    /// 正在切换到的账号 id；同一时刻只允许一个切换任务（需求 04 章）
    @Published var switchingTo: UUID?
    /// 手动刷新执行中
    @Published var refreshing = false
    @Published var addingAccount = false
    private var loginTask: Task<Void, Never>?
    private let webLogin: WebLoginProvider
    /// 标题下方的短暂操作反馈。样式跟 kind 走，不靠文案前缀判断。
    @Published var feedback: String?
    @Published var feedbackKind: FeedbackKind = .warning
    @Published var feedbackCaption: String?
    /// 目标软件实时身份（来自会话文件，不来自缓存记录）
    @Published var liveEmail: String?
    @Published var updateState: AppUpdateState = .idle

    let adapter: TargetAdapter
    let quota: QuotaProvider
    private let keychain: CredentialStore
    private let storeURL: URL
    private var feedbackTask: Task<Void, Never>?
    private var didSyncPersistence = false
    /// store.json 损坏或版本过新时禁止落盘，避免用空列表覆盖原文件。
    private var persistenceBlocked = false
    private var pendingNotice: String?
    private var persistGeneration = 0
    private var verifyTask: Task<Void, Never>?
    private var updateTask: Task<Void, Never>?
    private let softVerifyNanoseconds: UInt64
    private let updateFeed: ReleaseFeed?
    private let updateInstaller: UpdateInstaller
    private let currentVersion: String
    private let defaults: UserDefaults

    private var demoMode = false

    init(adapter: TargetAdapter = PenTargetAdapter(), quota: QuotaProvider = PenQuotaProvider(),
         keychain: CredentialStore = FileCredentialStore(), storeURL: URL? = nil, webLogin: WebLoginProvider? = nil,
         softVerifyNanoseconds: UInt64 = 4_000_000_000,
         updateFeed: ReleaseFeed? = nil, updateInstaller: UpdateInstaller? = nil,
         currentVersion: String? = nil, defaults: UserDefaults = .standard) {
        self.webLogin = webLogin ?? PenWebLogin()
        self.adapter = adapter
        self.quota = quota
        self.keychain = keychain
        self.softVerifyNanoseconds = softVerifyNanoseconds
        self.updateFeed = updateFeed ?? GitHubReleaseFeed(
            repo: Bundle.main.object(forInfoDictionaryKey: "SBGitHubRepo") as? String ?? "")
        self.updateInstaller = updateInstaller ?? FileUpdateInstaller()
        self.currentVersion = currentVersion
            ?? (Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String)
            ?? "1.0.0"
        self.defaults = defaults
        self.storeURL = storeURL ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Switchboard", isDirectory: true)
            .appendingPathComponent("store.json")
        demoMode = ProcessInfo.processInfo.environment["SWITCHBOARD_DEMO"] == "1"
        if demoMode {
            loadDemoAccounts()
        } else {
            load()
            // load() 期间的赋值不触发落盘；结束后才允许持久化
            didSyncPersistence = true
            if let pendingNotice {
                showFeedback(pendingNotice, kind: .warning, persistent: true)
            }
            settings.launchAtLogin = SMAppService.mainApp.status == .enabled
            syncCurrentIdentity()
        }
    }

    /// 演示数据（原型同款三账号）：仅内存，不落盘、不碰凭据文件
    private func loadDemoAccounts() {
        didSyncPersistence = false
        accounts = [
            AccountRecord(email: "alex.chen@gmail.com", plan: "Pro 方案", isCurrent: true,
                          usedDays: 1, totalDays: 5, lastQuotaUpdate: Date(),
                          authState: .ok),
            AccountRecord(email: "jamie.work@outlook.com", plan: "免费方案",
                          usedDays: 2, totalDays: 5, lastQuotaUpdate: Date(),
                          authState: .ok),
            AccountRecord(email: "alex.creative@icloud.com", plan: "团队方案",
                          usedDays: 4, totalDays: 5, lastQuotaUpdate: Date(),
                          authState: .ok),
        ]
    }

    // MARK: - 持久化（~/Library/Application Support/Switchboard/store.json）

    private func load() {
        guard FileManager.default.fileExists(atPath: storeURL.path) else { return }
        guard let data = try? Data(contentsOf: storeURL) else {
            blockPersistence("账号列表无法读取，已停止写入以免覆盖原文件")
            return
        }
        do {
            let file = try JSONDecoder().decode(StoreFile.self, from: data)
            guard file.storeVersion <= StoreFile.currentVersion else {
                quarantine(data)
                return
            }
            accounts = file.accounts
        } catch {
            quarantine(data)
        }
    }

    private func quarantine(_ data: Data) {
        let stamp = ISO8601DateFormatter().string(from: Date()).replacingOccurrences(of: ":", with: "-")
        let backup = storeURL.deletingPathExtension().appendingPathExtension("corrupt-\(stamp).json")
        try? data.write(to: backup, options: .atomic)
        blockPersistence("账号列表已损坏，已备份且不会覆盖原文件")
    }

    private func blockPersistence(_ notice: String) {
        persistenceBlocked = true
        pendingNotice = notice
    }

    private func persist() {
        guard didSyncPersistence, !persistenceBlocked else { return }
        persistGeneration += 1
        let generation = persistGeneration
        Task { @MainActor in
            await Task.yield()
            guard generation == persistGeneration else { return }
            writeStoreNow()
        }
    }

    /// 取消未完成的合并写入并立刻落盘。退出进程或改账号列表时调用。
    func flushPersist() {
        persistGeneration += 1
        writeStoreNow()
    }

    private func writeStoreNow() {
        guard didSyncPersistence, !persistenceBlocked else { return }
        do {
            let dir = storeURL.deletingLastPathComponent()
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
            let data = try JSONEncoder().encode(StoreFile(accounts: accounts))
            try data.write(to: storeURL, options: .atomic)
        } catch {
            if feedback == nil {
                showFeedback("保存设置失败：\(error.localizedDescription)", kind: .warning)
            }
        }
    }

    private func mutateAccounts(_ body: (inout [AccountRecord]) -> Void) {
        var next = accounts
        body(&next)
        if next != accounts { accounts = next }
    }

    // MARK: - 身份同步

    /// 应用启动或面板重开时校验：当前身份以目标软件实际登录为准（验收 02/11）
    func syncCurrentIdentity() {
        guard !demoMode else { return }
        liveEmail = adapter.liveIdentity()
        mutateAccounts { list in
            for idx in list.indices {
                list[idx].isCurrent = liveEmail.map { emailsMatch(list[idx].email, $0) } ?? false
            }
        }
    }

    /// 未登记的实时身份（登录了但还没导入的账号）
    var unregisteredLiveEmail: String? {
        guard let live = liveEmail, !accounts.contains(where: { emailsMatch($0.email, live) }) else { return nil }
        return live
    }

    var currentAccount: AccountRecord? { accounts.first(where: \.isCurrent) }

    // MARK: - 添加账号

    func addAccount(window: NSWindow) {
        guard !demoMode else { showFeedback("演示模式不操作真实账号", kind: .warning); return }
        if addingAccount { cancelAddingAccount(); return }
        guard switchingTo == nil, !refreshing else { return }
        addingAccount = true
        showFeedback("请在网页登录要添加的账号", kind: .loading, persistent: true)
        loginTask = Task { [self] in
            let timeout = Task { [weak self] in
                try? await Task.sleep(nanoseconds: 300_000_000_000)
                guard !Task.isCancelled else { return }
                self?.loginTask?.cancel()
            }
            defer { timeout.cancel(); addingAccount = false; loginTask = nil }
            do {
                let snapshot = try await webLogin.signIn(window: window)
                try Task.checkCancellation()
                let record: AccountRecord
                if let existing = accounts.first(where: { emailsMatch($0.email, snapshot.email) }) {
                    record = existing
                } else {
                    record = AccountRecord(email: snapshot.email, authState: .ok)
                }
                try saveSnapshot(snapshot, for: record.id)
                if let idx = accounts.firstIndex(where: { $0.id == record.id }) {
                    accounts[idx].authState = .ok
                    accounts[idx].lastError = nil
                } else {
                    accounts.append(record)
                }
                flushPersist()
                // 登录已完成，超时不再取消；切号期间仍保持 addingAccount，避免点外把面板收掉。
                timeout.cancel()
                let outcome = await switchTo(record, activateNewLogin: true)
                addingAccount = false
                switch outcome {
                case .completed:
                    await refreshAllQuotas()
                case .failed(let message):
                    showFeedback("账号已保存，但切换失败：\(message)", kind: .warning)
                case .skipped:
                    showFeedback("账号已保存，但暂未启用，可稍后切换", kind: .warning)
                }
            } catch is CancellationError {
                showFeedback("网页登录已取消或超时，原账号保持不变", kind: .warning)
            } catch {
                showFeedback("添加失败：\(error.localizedDescription)", kind: .warning)
            }
        }
    }

    func cancelAddingAccount() {
        guard addingAccount else { return }
        loginTask?.cancel()
        webLogin.cancel()
    }

    func importCurrentLogin() {
        guard !addingAccount, switchingTo == nil, !refreshing else { return }
        guard !demoMode else { showFeedback("演示模式不操作真实账号", kind: .warning); return }
        do {
            let snapshot = try adapter.captureSnapshot()
            if let idx = accounts.firstIndex(where: { emailsMatch($0.email, snapshot.email) }) {
                // 重复添加：更新凭据，不新增记录（需求 10 章）
                try saveSnapshot(snapshot, for: accounts[idx].id)
                mutateAccounts { list in
                    guard let idx = list.firstIndex(where: { emailsMatch($0.email, snapshot.email) }) else { return }
                    list[idx].authState = .ok
                    list[idx].lastError = nil
                }
                showFeedback("账号 \(snapshot.email) 的登录状态已更新", kind: .success)
            } else {
                let record = AccountRecord(email: snapshot.email, authState: .ok)
                try saveSnapshot(snapshot, for: record.id)
                accounts.append(record)
                showFeedback("已添加账号 \(snapshot.email)", kind: .success)
            }
            syncCurrentIdentity()
            flushPersist()
            requestQuotaRefresh()
        } catch {
            showFeedback("添加失败：\(error.localizedDescription)", kind: .warning)
        }
    }

    // MARK: - 切换账号

    @discardableResult
    func switchTo(_ target: AccountRecord, activateNewLogin: Bool = false) async -> SwitchOutcome {
        verifyTask?.cancel()
        guard !addingAccount || activateNewLogin else { return .skipped }
        guard !demoMode else { showFeedback("演示模式不操作真实账号", kind: .warning); return .skipped }
        guard switchingTo == nil else { return .skipped }
        guard !refreshing else {
            showFeedback("正在刷新额度，请稍后再切换", kind: .warning)
            return .skipped
        }
        syncCurrentIdentity()
        guard let target = accounts.first(where: { $0.id == target.id }),
              !target.isCurrent || activateNewLogin else { return .skipped }
        switchingTo = target.id
        defer { switchingTo = nil }

        do {
            // 凭据缺失时直接报错，避免先退出 Pen 才发现无法切换。
            let snapshot = try loadSnapshot(for: target.id)
            guard emailsMatch(snapshot.email, target.email) else {
                throw TargetAdapterError.verifyFailed(expected: target.email, actual: snapshot.email)
            }
            // 1. 回存当前活跃状态（凭据可能已被目标软件刷新，旧快照不可信）
            var previous = try? adapter.captureSnapshot()
            if let liveSnapshot = previous,
               let holder = accounts.firstIndex(where: { emailsMatch($0.email, liveSnapshot.email) }) {
                try saveSnapshot(liveSnapshot, for: accounts[holder].id)
            }
            // 2. 退出目标软件（运行中无法安全写会话文件）
            let wasRunning = adapter.isRunning()
            let willRelaunch = wasRunning || activateNewLogin
            if willRelaunch, !adapter.canLaunch() {
                throw TargetAdapterError.applicationMissing
            }
            if wasRunning {
                guard await adapter.terminateAndWait(timeout: 15) else {
                    throw TargetAdapterError.targetDidNotQuit
                }
            }
            // 3. 落盘目标账号快照；失败则回滚为切换前状态
            do {
                // Pen 退出时可能回写更新后的 token，保留最终会话用于恢复。
                if let finalSnapshot = try? adapter.captureSnapshot() {
                    previous = finalSnapshot
                    if let holder = accounts.first(where: { emailsMatch($0.email, finalSnapshot.email) }) {
                        try saveSnapshot(finalSnapshot, for: holder.id)
                    }
                }
                try adapter.applySnapshot(snapshot)
                // 重新登录当前账号时，上面的回存会用旧 token 盖掉新快照，这里必须再写回。
                if activateNewLogin { try saveSnapshot(snapshot, for: target.id) }
            } catch {
                let switchError = error
                if let previous {
                    do {
                        try adapter.applySnapshot(previous)
                    } catch {
                        throw TargetAdapterError.rollbackFailed
                    }
                }
                if wasRunning {
                    do { try await adapter.launch() } catch {
                        throw TargetAdapterError.relaunchFailed(original: switchError.localizedDescription,
                                                                launch: error.localizedDescription)
                    }
                }
                throw switchError
            }
            // 4. 之前在运行就重新拉起。文件身份已在 applySnapshot 里对过。
            if willRelaunch { try await adapter.launch() }
            syncCurrentIdentity()
            guard let liveEmail, emailsMatch(liveEmail, target.email) else {
                throw TargetAdapterError.verifyFailed(expected: target.email, actual: liveEmail)
            }
            mutateAccounts { list in
                if let idx = list.firstIndex(where: { $0.id == target.id }) {
                    list[idx].authState = .ok
                    list[idx].lastError = nil
                }
            }
            showFeedback("已切换到 \(target.email)", kind: .success)
            if willRelaunch { scheduleSoftVerify(expected: target.email) }
            return .completed
        } catch {
            syncCurrentIdentity()
            showFeedback("切换失败：\(error.localizedDescription)", kind: .warning)
            return .failed(error.localizedDescription)
        }
    }

    // MARK: - 移除账号

    func remove(_ record: AccountRecord) {
        guard !addingAccount, switchingTo == nil, !refreshing else { return }
        guard !demoMode else { showFeedback("演示模式不操作真实账号", kind: .warning); return }
        syncCurrentIdentity()
        guard let record = accounts.first(where: { $0.id == record.id }), !record.isCurrent else { return }
        do {
            try keychain.delete(account: record.id.uuidString)
            accounts.removeAll { $0.id == record.id }
            flushPersist()
            showFeedback("已移除 \(record.email)", kind: .success)
        } catch {
            showFeedback("移除失败：\(error.localizedDescription)", kind: .warning)
        }
    }

    // MARK: - 额度刷新（手动）

    /// 立刻占住刷新锁再异步执行，避免导入后用户抢先点切换把刷新丢掉。
    func requestQuotaRefresh() {
        guard beginQuotaRefresh() else { return }
        Task { await performQuotaRefresh() }
    }

    /// 手动刷新：读取各账号额度，执行中禁止重复请求
    func refreshAllQuotas() async {
        guard beginQuotaRefresh() else { return }
        await performQuotaRefresh()
    }

    private func beginQuotaRefresh() -> Bool {
        guard !demoMode else { showFeedback("演示模式不请求真实额度", kind: .warning); return false }
        guard !refreshing, switchingTo == nil, !addingAccount else { return false }
        refreshing = true
        return true
    }

    private func performQuotaRefresh() async {
        syncCurrentIdentity()
        showFeedback("正在刷新额度…", kind: .loading, persistent: true)
        defer {
            refreshing = false
            flushPersist()
        }
        var failures = 0
        for record in accounts {
            let result: QuotaResult
            do {
                // 当前账号直接读实时会话（token 可能已轮换），并回存快照；
                // 其他账号用本地凭据文件里的快照
                let snapshot: TargetSnapshot
                if let live = try? adapter.captureSnapshot(), emailsMatch(live.email, record.email) {
                    try? saveSnapshot(live, for: record.id)
                    snapshot = live
                } else {
                    snapshot = try loadSnapshot(for: record.id)
                }
                result = await quota.fetchQuota(snapshot: snapshot)
            } catch TargetAdapterError.snapshotNotFound {
                result = .expired
            } catch {
                result = .failure(error.localizedDescription)
            }
            // 网络请求期间允许移除账号，返回后必须按 id 重新查找位置。
            mutateAccounts { list in
                guard let idx = list.firstIndex(where: { $0.id == record.id }) else { return }
                switch result {
                case .quota(let used, let total, let plan):
                    list[idx].usedDays = used
                    list[idx].totalDays = total
                    if let plan { list[idx].plan = plan }
                    list[idx].lastQuotaUpdate = Date()
                    list[idx].lastError = nil
                    list[idx].authState = .ok
                case .unsupported:
                    list[idx].usedDays = nil
                    list[idx].totalDays = nil
                    list[idx].lastQuotaUpdate = Date()
                    list[idx].lastError = nil
                    list[idx].authState = .ok
                case .expired:
                    list[idx].authState = .expired
                    list[idx].lastError = "登录已过期，请重新登录该账号"
                case .failure(let message):
                    list[idx].lastError = message
                }
            }
            switch result {
            case .quota, .unsupported: break
            case .expired, .failure: failures += 1
            }
        }
        if accounts.isEmpty {
            showFeedback("还没有可刷新的账号", kind: .warning)
        } else if failures > 0 && failures < accounts.count {
            showFeedback("额度已刷新，\(failures) 个账号更新失败", kind: .warning)
        } else if failures >= accounts.count {
            showFeedback("额度更新失败", kind: .warning)
        } else {
            showFeedback("额度已刷新", kind: .success, caption: "刚刚更新")
        }
    }

    // MARK: - 设置：开机自启

    func applyLaunchAtLogin(_ enabled: Bool) {
        guard !demoMode else { settings.launchAtLogin = enabled; return }
        do {
            if enabled {
                try SMAppService.mainApp.register()
            } else {
                try SMAppService.mainApp.unregister()
            }
            settings.launchAtLogin = enabled
        } catch {
            // 失败恢复原值并反馈原因（需求 07 章）
            settings.launchAtLogin = SMAppService.mainApp.status == .enabled
            showFeedback("设置开机启动失败：\(error.localizedDescription)", kind: .warning)
        }
    }

    // MARK: - 快照读写

    private func saveSnapshot(_ snapshot: TargetSnapshot, for id: UUID) throws {
        let data = try JSONEncoder().encode(snapshot)
        try keychain.save(data, account: id.uuidString)
    }

    private func loadSnapshot(for id: UUID) throws -> TargetSnapshot {
        let data: Data
        do {
            data = try keychain.load(account: id.uuidString)
        } catch let error as CredentialError {
            switch error {
            case .notFound, .invalidAccount:
                throw TargetAdapterError.snapshotNotFound
            }
        }
        do {
            return try JSONDecoder().decode(TargetSnapshot.self, from: data)
        } catch {
            throw TargetAdapterError.snapshotNotFound
        }
    }

    // MARK: - 反馈

    /// Pen 启动后才会校验 token。几秒后再读一次会话，只提示，不回滚。
    private func scheduleSoftVerify(expected: String) {
        verifyTask?.cancel()
        let delay = softVerifyNanoseconds
        guard delay > 0 else { return }
        verifyTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: delay)
            guard !Task.isCancelled, let self else { return }
            self.syncCurrentIdentity()
            guard let live = self.liveEmail, emailsMatch(live, expected) else {
                self.showFeedback("Pen 启动后登录身份已变化，请确认当前账号", kind: .warning)
                return
            }
        }
    }

    func showFeedback(_ text: String, kind: FeedbackKind = .warning, persistent: Bool = false,
                      caption: String? = nil) {
        feedbackKind = kind
        feedbackCaption = caption
        feedback = text
        feedbackTask?.cancel()
        guard !persistent else { return }
        feedbackTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(min(20, max(6, text.count / 10 + 4))) * 1_000_000_000)
            if !Task.isCancelled {
                self?.feedback = nil
                self?.feedbackCaption = nil
            }
        }
    }

    // MARK: - 应用更新

    var hasVisibleUpdate: Bool {
        switch updateState {
        case .available(_, let silenced): return !silenced
        case .downloading, .ready: return true
        default: return false
        }
    }

    var hasAvailableUpdate: Bool {
        updateState.release != nil
    }

    func checkForUpdates(force: Bool) {
        guard !demoMode else { return }
        guard let feed = updateFeed, !feedRepoMissing else { return }
        if updateState.isBusy { return }
        if !force {
            let last = defaults.object(forKey: "SBLastUpdateCheck") as? Date ?? .distantPast
            if Date().timeIntervalSince(last) < 24 * 60 * 60 { return }
        }
        updateTask?.cancel()
        updateTask = Task { [weak self] in
            guard let self else { return }
            do {
                let release = try await feed.fetchLatest()
                self.defaults.set(Date(), forKey: "SBLastUpdateCheck")
                guard UpdateVersion.compare(self.currentVersion, release.version) == .orderedAscending else {
                    if force { self.showFeedback("已是最新版本 \(self.currentVersion)", kind: .success) }
                    return
                }
                if case .available(let current, let silenced) = self.updateState,
                   current.version == release.version {
                    self.updateState = .available(release, silenced: silenced)
                    return
                }
                self.updateState = .available(release, silenced: false)
            } catch {
                if force {
                    self.showFeedback("检查更新失败：\(error.localizedDescription)", kind: .warning)
                }
            }
        }
    }

    func startUpdate() {
        guard case .available(let release, _) = updateState else { return }
        updateTask?.cancel()
        updateState = .downloading(release, percent: 0, received: 0, total: nil)
        updateTask = Task { [weak self] in
            guard let self else { return }
            let download = DownloadTask()
            download.onProgress = { received, total in
                let percent = total > 0 ? Int((Double(received) / Double(total)) * 100) : 0
                Task { @MainActor in
                    guard case .downloading(let current, let oldPercent, _, _) = self.updateState,
                          current == release else { return }
                    guard percent != oldPercent || total <= 0 else { return }
                    self.updateState = .downloading(release, percent: min(100, percent),
                                                    received: received,
                                                    total: total > 0 ? total : nil)
                }
            }
            do {
                let zip = try await download.download(release.zipURL)
                try Task.checkCancellation()
                let unpacked = try self.updateInstaller.unpackZip(zip)
                self.updateState = .ready(release, unpackedApp: unpacked)
            } catch is CancellationError {
                self.updateState = .available(release, silenced: false)
            } catch {
                self.updateState = .available(release, silenced: false)
                self.showFeedback("下载更新失败：\(error.localizedDescription)", kind: .warning)
            }
        }
    }

    func postponeUpdate() {
        if case .available(let release, _) = updateState {
            updateState = .available(release, silenced: true)
        }
    }

    func installUpdate() {
        guard case .ready(_, let unpacked) = updateState else { return }
        do {
            let current = Bundle.main.bundleURL
            try updateInstaller.replace(current: current, with: unpacked)
            updateInstaller.relaunch(at: current)
            NSApp.terminate(nil)
        } catch {
            showFeedback("安装更新失败：\(error.localizedDescription)", kind: .warning)
        }
    }

    func openUpdatePage() {
        guard let page = updateState.release?.pageURL else { return }
        NSWorkspace.shared.open(page)
    }

    private var feedRepoMissing: Bool {
        if let feed = updateFeed as? GitHubReleaseFeed { return feed.repo.isEmpty }
        return updateFeed == nil
    }
}
