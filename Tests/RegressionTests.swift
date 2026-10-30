import AppKit
import Foundation

final class MemoryCredentials: CredentialStore {
    var items: [String: Data] = [:]
    func save(_ data: Data, account: String) throws { items[account] = data }
    func load(account: String) throws -> Data {
        guard let data = items[account] else { throw CredentialError.notFound }
        return data
    }
    func delete(account: String) throws { items.removeValue(forKey: account) }
}

func snapshot(_ email: String) -> TargetSnapshot {
    TargetSnapshot(sessionFile: "{\"email\":\"\(email)\",\"token\":\"test-token\"}",
                   email: email, token: "test-token")
}

final class FakeTarget: TargetAdapter {
    var session: TargetSnapshot? = snapshot("a@example.test")
    var running = false
    var quitCount = 0
    var launchCount = 0
    var failEmail: String?
    var refuseQuit = false
    func liveIdentity() -> String? { session?.email }
    func captureSnapshot() throws -> TargetSnapshot {
        guard let session else { throw TargetAdapterError.notLoggedIn }
        return session
    }
    func applySnapshot(_ snapshot: TargetSnapshot) throws {
        session = snapshot
        if snapshot.email == failEmail {
            throw TargetAdapterError.verifyFailed(expected: snapshot.email, actual: nil)
        }
    }
    func isRunning() -> Bool { running }
    func canLaunch() -> Bool { true }
    func terminateAndWait(timeout: TimeInterval) async -> Bool {
        quitCount += 1
        if refuseQuit { return false }
        running = false
        return true
    }
    func launch() async throws {
        launchCount += 1
        running = true
    }
}

final class QAWindow: NSPanel {
    override var canBecomeKey: Bool { true }
}

@MainActor
final class FakeQuota: QuotaProvider {
    var result: QuotaResult = .quota(used: 2, total: 5, plan: nil)
    var onFetch: ((TargetSnapshot) -> Void)?
    var fetchedEmails: [String] = []
    var delayNanoseconds: UInt64 = 0
    func fetchQuota(snapshot: TargetSnapshot) async -> QuotaResult {
        fetchedEmails.append(snapshot.email)
        onFetch?(snapshot)
        if delayNanoseconds > 0 { try? await Task.sleep(nanoseconds: delayNanoseconds) }
        await Task.yield()
        return result
    }
}

@MainActor
final class FakeWebLogin: WebLoginProvider {
    var result = snapshot("new@example.test")
    var failure: Error?
    var delay: UInt64 = 0
    func signIn(window: NSWindow) async throws -> TargetSnapshot {
        if delay > 0 { try await Task.sleep(nanoseconds: delay) }
        if let failure { throw failure }
        return result
    }
    func cancel() {}
}

final class FakeFeed: ReleaseFeed {
    var release: AppRelease?
    var error: Error?
    func fetchLatest() async throws -> AppRelease {
        if let error { throw error }
        guard let release else { throw URLError(.cannotParseResponse) }
        return release
    }
}

final class FakeInstaller: UpdateInstaller {
    var replaced = false
    var relaunched = false
    func unpackZip(_ zipURL: URL) throws -> URL { zipURL }
    func replace(current: URL, with newApp: URL) throws { replaced = true }
    func relaunch(at url: URL) { relaunched = true }
}

@main
struct RegressionTests {
    @MainActor static var failures = 0
    @MainActor static var checks = 0
    @MainActor static func check(_ condition: Bool, _ label: String) {
        checks += 1
        if !condition { failures += 1 }
        print("\(condition ? "PASS" : "FAIL") \(label)")
    }

    @MainActor static func main() {
        let app = NSApplication.shared
        if ProcessInfo.processInfo.processName == "SwitchboardQA" {
            app.setActivationPolicy(.regular)
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
            defer { try? FileManager.default.removeItem(at: directory) }
            let target = FakeTarget()
            let credentials = MemoryCredentials()
            let quota = FakeQuota()
            quota.delayNanoseconds = 2_000_000_000
            let model = AppModel(adapter: target, quota: quota, keychain: credentials,
                                 storeURL: directory.appendingPathComponent("store.json"),
                                 softVerifyNanoseconds: 0)
            model.accounts = [AccountRecord(email: "a@example.test", isCurrent: true, authState: .ok),
                              AccountRecord(email: "b@example.test", authState: .ok),
                              AccountRecord(email: String(repeating: "long-email", count: 9) + "@example.test",
                                            authState: .expired)]
            for account in model.accounts {
                try! credentials.save(JSONEncoder().encode(snapshot(account.email)), account: account.id.uuidString)
            }
            let panel = PanelController(model: model)
            let window = QAWindow(contentRect: NSRect(x: 0, y: 0, width: 410, height: 600),
                                  styleMask: .borderless, backing: .buffered, defer: false)
            window.title = "Switchboard QA — 隔离测试账号"
            window.isReleasedWhenClosed = false
            window.isOpaque = false
            window.backgroundColor = .clear
            window.contentViewController = panel
            window.setContentSize(panel.preferredContentSize)
            window.center()
            window.makeKeyAndOrderFront(nil)
            app.activate(ignoringOtherApps: true)
            app.run()
            return
        }
        app.setActivationPolicy(.prohibited)
        Task { @MainActor in
            await run()
            print("\(checks - failures)/\(checks) checks passed")
            exit(failures == 0 ? 0 : 1)
        }
        app.run()
    }

    @MainActor static func run() async {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try! FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let target = FakeTarget()
        let credentials = MemoryCredentials()
        let model = AppModel(adapter: target, quota: UnsupportedQuotaProvider(), keychain: credentials,
                             storeURL: directory.appendingPathComponent("store.json"),
                             softVerifyNanoseconds: 0)
        let a = AccountRecord(email: "a@example.test", isCurrent: true, authState: .ok)
        let b = AccountRecord(email: "b@example.test", authState: .ok)
        model.accounts = [a, b]
        target.session = nil
        model.syncCurrentIdentity()
        check(model.currentAccount == nil, "logout clears current account")
        target.session = snapshot("other@example.test")
        model.accounts = [a, b]
        model.syncCurrentIdentity()
        check(model.currentAccount == nil && model.unregisteredLiveEmail != nil,
              "unregistered identity clears previous current account")

        target.session = snapshot(a.email)
        target.running = true
        model.accounts = [a, b]
        await model.switchTo(b)
        check(target.quitCount == 0 && target.running, "missing credentials do not quit Pen")
        try! credentials.save(JSONEncoder().encode(snapshot(b.email)), account: b.id.uuidString)
        target.running = true
        target.failEmail = b.email
        await model.switchTo(b)
        check(target.session?.email == a.email, "failed write restores original session")
        check(target.running, "failed switch restarts previously running Pen")
        check(model.currentAccount?.email == a.email, "failed switch keeps actual current identity")

        let adapter = PenTargetAdapter(home: directory)
        do {
            try adapter.applySnapshot(snapshot(a.email))
            check(adapter.liveIdentity() == a.email, "restore creates missing session file")
        } catch {
            check(false, "restore creates missing session file")
        }

        target.failEmail = nil
        await model.switchTo(b)
        check(model.currentAccount?.id == b.id && target.session?.email == b.email,
              "successful switch updates actual identity")
        let staleB = b
        model.remove(staleB)
        check(model.accounts.count == 2, "stale UI record cannot remove live current account")

        let quota = FakeQuota()
        let quotaModel = AppModel(adapter: target, quota: quota, keychain: credentials,
                                  storeURL: directory.appendingPathComponent("quota.json"),
                                  softVerifyNanoseconds: 0)
        quotaModel.accounts = [a, b]
        quota.onFetch = { _ in quotaModel.accounts.removeAll { $0.id == a.id } }
        await quotaModel.refreshAllQuotas()
        check(quotaModel.accounts.count == 1 && quotaModel.accounts[0].id == b.id
              && quotaModel.accounts[0].usedDays == 2,
              "refresh safely handles account removal across await")
        quota.onFetch = nil
        quota.result = .expired
        await quotaModel.refreshAllQuotas()
        check(quotaModel.accounts[0].authState == .expired, "expired response updates authentication state")
        quota.result = .quota(used: 3, total: 5, plan: "Test plan")
        await quotaModel.refreshAllQuotas()
        check(quotaModel.accounts[0].authState == .ok && quotaModel.accounts[0].lastError == nil,
              "successful refresh clears expired state")
        quota.result = .unsupported
        await quotaModel.refreshAllQuotas()
        check(quotaModel.accounts[0].usedDays == nil && quotaModel.accounts[0].totalDays == nil,
              "unmetered response clears cached quota")
        target.session = snapshot("other@example.test")
        quota.fetchedEmails = []
        quotaModel.accounts = [a, b]
        await quotaModel.refreshAllQuotas()
        check(!quota.fetchedEmails.contains("other@example.test"),
              "external login cannot overwrite a registered account credential")
        check((try? JSONDecoder().decode(TargetSnapshot.self,
              from: credentials.load(account: b.id.uuidString)))?.email == b.email,
              "registered credential identity remains intact")

        do {
            var invalid = snapshot(b.email)
            invalid.sessionFile = "{\"email\":\"other@example.test\",\"token\":\"test-token\"}"
            try adapter.applySnapshot(invalid)
            check(false, "mismatched snapshot rejected before write")
        } catch {
            check(adapter.liveIdentity() == a.email, "mismatched snapshot rejected before write")
        }

        model.accounts = [a, b]
        let panel = PanelController(model: model)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 410, height: 450),
                              styleMask: .borderless, backing: .buffered, defer: false)
        window.contentViewController = panel
        window.contentView?.layoutSubtreeIfNeeded()
        check((panel.view as? RoundedBoxView)?.boxRadius == 16, "panel uses prototype 16pt corners")
        model.showFeedback("test feedback")
        try? await Task.sleep(nanoseconds: 100_000_000)
        panel.view.layoutSubtreeIfNeeded()
        let settings = buttons(in: panel.view).first { $0.toolTip == "设置" }!
        let point = settings.convert(NSPoint(x: settings.bounds.midX, y: settings.bounds.midY), to: panel.view.superview)
        check(panel.view.hitTest(point) === settings, "feedback overlay does not intercept buttons")
        model.feedback = nil
        try? await Task.sleep(nanoseconds: 400_000_000)
        panel.view.layoutSubtreeIfNeeded()
        let settingsAfter = buttons(in: panel.view).first { $0.toolTip == "设置" }!
        let pointAfter = settingsAfter.convert(NSPoint(x: settingsAfter.bounds.midX, y: settingsAfter.bounds.midY), to: panel.view.superview)
        check(settingsAfter === settings && panel.view.hitTest(pointAfter) === settingsAfter,
              "dismissed feedback does not intercept buttons")
        let refresh = buttons(in: panel.view).first { $0.identifier?.rawValue == "quota-refresh" }! as! RefreshButton
        let refreshFrame = refresh.frame
        refresh.setSpinning(true)
        let rotatingLayer = animatedLayer(in: refresh.layer!)!
        check(rotatingLayer.bounds.size == refresh.bounds.size && rotatingLayer.bounds.height > 0,
              "refresh icon keeps its original display size")
        let anchor = CGPoint(x: rotatingLayer.bounds.minX + rotatingLayer.bounds.width * rotatingLayer.anchorPoint.x,
                             y: rotatingLayer.bounds.minY + rotatingLayer.bounds.height * rotatingLayer.anchorPoint.y)
        check(abs(anchor.x - rotatingLayer.bounds.midX) < 0.01
              && abs(anchor.y - rotatingLayer.bounds.midY) < 0.01,
              "refresh glyph rotates around its own center")
        check(refresh.frame == refreshFrame, "starting refresh does not move the button")
        let existingAnimation = rotatingLayer.animation(forKey: "spin")!.copy() as! CAAnimation
        let startedAt = CACurrentMediaTime() - 0.25
        existingAnimation.beginTime = startedAt
        rotatingLayer.add(existingAnimation, forKey: "spin")
        refresh.setSpinning(true)
        check(rotatingLayer.animation(forKey: "spin")!.beginTime == startedAt,
              "repeated render does not restart the running animation")
        refresh.setSpinning(false)
        check(animatedLayer(in: refresh.layer!) == nil && refresh.frame == refreshFrame,
              "stopping refresh clears animation without moving the button")
        let initialSize = panel.preferredContentSize
        for route in [PanelRoute.main, .settings, .manage] {
            model.route = route
            try? await Task.sleep(nanoseconds: 50_000_000)
            let sizeBefore = panel.preferredContentSize
            let long = "已切换到 " + String(repeating: "very-long-email", count: 12) + "@example.test"
            model.showFeedback(long)
            try? await Task.sleep(nanoseconds: 50_000_000)
            panel.view.layoutSubtreeIfNeeded()
            let banner = viewWithID("refresh-status", in: panel.view)
            check(banner != nil, "status banner appears at top on \(route)")
            check(labels(in: banner!).contains { $0.stringValue == long },
                  "long message remains in the top banner on \(route)")
            check(labels(in: panel.view).filter { $0.stringValue == long }.count == 1,
                  "no bottom feedback on \(route)")
            let titleText = route == .main ? "Pencil Account Switcher" : (route == .settings ? "设置" : "管理账号")
            if let banner, let title = labels(in: panel.view).first(where: { $0.stringValue == titleText }) {
                let bannerFrame = banner.convert(banner.bounds, to: panel.view)
                let titleFrame = title.convert(title.bounds, to: panel.view)
                check(!bannerFrame.intersects(titleFrame), "status banner does not obscure title on \(route)")
            } else { check(false, "status banner does not obscure title on \(route)") }
            model.feedback = nil
            try? await Task.sleep(nanoseconds: 50_000_000)
            check(viewWithID("refresh-status", in: panel.view) == nil, "dismissed banner is removed on \(route)")
            check(panel.preferredContentSize == sizeBefore, "dismissing feedback restores panel size on \(route)")
        }
        model.route = .main
        try? await Task.sleep(nanoseconds: 50_000_000)
        check(panel.preferredContentSize == initialSize, "navigation restores stable main panel size")
        model.showFeedback("额度已刷新", kind: .success)
        try? await Task.sleep(nanoseconds: 80_000_000)
        panel.view.layoutSubtreeIfNeeded()
        let inlineSuccess = viewWithID("refresh-status", in: panel.view)
        check(inlineSuccess != nil, "refresh success uses the prototype inline banner")
        if let inlineSuccess, let title = labels(in: panel.view).first(where: { $0.stringValue == "Pencil Account Switcher" }) {
            let bannerFrame = inlineSuccess.convert(inlineSuccess.bounds, to: panel.view)
            let titleFrame = title.convert(title.bounds, to: panel.view)
            check(!bannerFrame.intersects(titleFrame), "refresh banner does not obscure title")
        } else { check(false, "refresh banner does not obscure title") }
        check(labels(in: panel.view).filter { $0.stringValue == "额度已刷新" }.count == 1,
              "refresh success is not duplicated at the bottom")
        for status in ["正在刷新额度…", "额度已刷新，1 个账号更新失败", "额度更新失败", "还没有可刷新的账号"] {
            model.showFeedback(status, kind: status == "正在刷新额度…" ? .loading : .warning)
            try? await Task.sleep(nanoseconds: 80_000_000)
            panel.view.layoutSubtreeIfNeeded()
            let banner = viewWithID("refresh-status", in: panel.view)
            check(banner != nil && labels(in: banner!).contains { $0.stringValue == status },
                  "refresh status appears in the same banner: \(status)")
            check(labels(in: panel.view).filter { $0.stringValue == status }.count == 1,
                  "refresh status is not duplicated at the bottom: \(status)")
        }
        model.showFeedback("额度已刷新", kind: .success)
        model.route = .settings
        try? await Task.sleep(nanoseconds: 80_000_000)
        check(viewWithID("refresh-status", in: panel.view) != nil, "refresh feedback remains visible on other pages")
        check(labels(in: panel.view).filter { $0.stringValue == "额度已刷新" }.count == 1,
              "other pages do not use the bottom feedback slot")
        model.feedback = nil
        model.route = .main
        try? await Task.sleep(nanoseconds: 80_000_000)
        check(viewWithID("refresh-status", in: panel.view) == nil, "expired refresh banner is removed")
        let oldUpdate = Date(timeIntervalSince1970: 100)
        model.accounts[0].lastQuotaUpdate = oldUpdate
        try? await Task.sleep(nanoseconds: 50_000_000)
        model.accounts[0].lastQuotaUpdate = Date(timeIntervalSince1970: 200)
        model.accounts[0].plan = "Updated plan"
        try? await Task.sleep(nanoseconds: 50_000_000)
        check(labels(in: panel.view).contains { $0.stringValue == "Updated plan" },
              "plan-only change refreshes screen")
        check(!buttons(in: panel.view).contains { $0.title == "重新导入" },
              "live token hides reimport")
        check(labels(in: panel.view).contains { $0.stringValue == "生效中" },
              "live token shows active badge")
        model.accounts[1].authState = .expired
        try? await Task.sleep(nanoseconds: 50_000_000)
        panel.view.layoutSubtreeIfNeeded()
        check(buttons(in: panel.view).contains { $0.title == "重新导入" },
              "expired token shows reimport")
        check(labels(in: panel.view).contains { $0.stringValue == "已失效" },
              "expired token shows inactive badge")
        model.route = .settings
        try? await Task.sleep(nanoseconds: 50_000_000)
        check(buttons(in: panel.view).contains { $0.title == "退出应用" }, "settings route renders")
        model.showFeedback("settings feedback")
        try? await Task.sleep(nanoseconds: 50_000_000)
        check(viewWithID("refresh-status", in: panel.view) != nil, "settings keeps dedicated feedback region")
        model.route = .manage
        try? await Task.sleep(nanoseconds: 50_000_000)
        check(buttons(in: panel.view).contains { $0.title == "移除" }, "manage route renders")
        check(buttons(in: panel.view).contains { $0.title == "重新导入" }, "manage offers reimport without removing")
        check(viewWithID("refresh-status", in: panel.view) != nil, "manage keeps dedicated feedback region")
        let hint = labels(in: panel.view).first { $0.stringValue.hasPrefix("移除账号前") }!
        check(hint.fittingSize.height > 16 && hint.alignmentRect(forFrame: hint.frame).maxX <= hint.superview!.bounds.maxX + 0.5,
              "management help wraps within panel")
        model.accounts = []
        model.route = .main
        try? await Task.sleep(nanoseconds: 50_000_000)
        panel.view.layoutSubtreeIfNeeded()
        check(buttons(in: panel.view).contains { $0.title == "添加账号" }
              && panel.preferredContentSize.width == 410, "empty state fits panel and allows import")
        let add = buttons(in: panel.view).first { $0.title == "添加账号" }!
        panel.view.layoutSubtreeIfNeeded()
        check(add.bounds.width > 200 && add.bounds.height >= 37, "add account button fills the row")
        let edge = add.convert(NSPoint(x: 8, y: add.bounds.midY), to: panel.view.superview)
        check(panel.view.hitTest(edge) === add, "add account is clickable at the edge")
        check(scrollViews(in: panel.view).contains { $0.scrollerStyle == .overlay && $0.verticalScroller is PanelScroller },
              "panel uses an overlay scroller")
        check(PenWebLogin.callbackCode("GET /callback?code=test&state=expected HTTP/1.1\r\n\r\n", state: "expected") == "test",
              "web login accepts matching callback")
        check(PenWebLogin.callbackCode("GET /callback?code=test&state=wrong HTTP/1.1\r\n\r\n", state: "expected") == nil,
              "web login rejects incorrect state")
        check(PenWebLogin.callbackCode("GET /callback?code=test&state=expected&state=wrong HTTP/1.1\r\n\r\n", state: "expected") == nil,
              "web login rejects duplicate callback fields")
        let login = FakeWebLogin()
        let loginTarget = FakeTarget()
        loginTarget.running = true
        let loginCredentials = MemoryCredentials()
        let loginModel = AppModel(adapter: loginTarget, quota: UnsupportedQuotaProvider(),
                                  keychain: loginCredentials, storeURL: directory.appendingPathComponent("login.json"),
                                  webLogin: login, softVerifyNanoseconds: 0)
        loginModel.accounts = [a]
        loginModel.addAccount(window: window)
        await waitForLogin(loginModel)
        check(loginModel.accounts.count == 2 && loginModel.currentAccount?.email == login.result.email,
              "web login automatically saves and activates new account")
        check(loginTarget.quitCount == 1 && loginTarget.launchCount == 1,
              "adding account restarts Pen exactly once")
        var renewed = login.result
        renewed.token = "renewed-token"
        renewed.sessionFile = renewed.sessionFile.replacingOccurrences(of: "test-token", with: "renewed-token")
        login.result = renewed
        loginModel.addAccount(window: window)
        await waitForLogin(loginModel)
        let saved = try! JSONDecoder().decode(TargetSnapshot.self,
            from: loginCredentials.load(account: loginModel.currentAccount!.id.uuidString))
        check(loginModel.accounts.count == 2 && saved.token == "renewed-token",
              "re-login deduplicates account and preserves new token")
        let countBefore = loginTarget.quitCount
        login.failure = PenWebLogin.error("test failure")
        loginModel.addAccount(window: window)
        await waitForLogin(loginModel)
        check(loginModel.accounts.count == 2 && loginTarget.quitCount == countBefore,
              "failed authentication leaves Pen and accounts unchanged")
        login.failure = nil
        login.delay = 10_000_000_000
        loginModel.addAccount(window: window)
        loginModel.cancelAddingAccount()
        await waitForLogin(loginModel)
        check(!loginModel.addingAccount && loginTarget.quitCount == countBefore,
              "cancelled authentication releases state without quitting Pen")
        let corruptURL = directory.appendingPathComponent("broken.json")
        try! Data("{".utf8).write(to: corruptURL)
        let broken = AppModel(adapter: FakeTarget(), quota: UnsupportedQuotaProvider(),
                              keychain: MemoryCredentials(), storeURL: corruptURL,
                              softVerifyNanoseconds: 0)
        broken.accounts = [AccountRecord(email: "kept@example.test")]
        check(try! Data(contentsOf: corruptURL) == Data("{".utf8), "corrupt store is not overwritten")
        let backups = try! FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
        check(backups.contains { $0.lastPathComponent.contains("corrupt-") }, "corrupt store is backed up")
        switch PenQuotaProvider.result(forHTTPStatus: 401) {
        case .expired: check(true, "401 marks the login expired")
        default: check(false, "401 marks the login expired")
        }
        switch PenQuotaProvider.result(forHTTPStatus: 403) {
        case .failure: check(true, "403 does not expire the account")
        default: check(false, "403 does not expire the account")
        }
        let caseTarget = FakeTarget()
        caseTarget.session = snapshot("A@example.test")
        let caseModel = AppModel(adapter: caseTarget, quota: UnsupportedQuotaProvider(),
                                 keychain: MemoryCredentials(),
                                 storeURL: directory.appendingPathComponent("case.json"),
                                 softVerifyNanoseconds: 0)
        caseModel.accounts = [AccountRecord(email: "a@example.test")]
        caseModel.syncCurrentIdentity()
        check(caseModel.currentAccount?.email == "a@example.test" && caseModel.unregisteredLiveEmail == nil,
              "email match ignores case")
        caseModel.showFeedback("账号已保存，但切换失败", kind: .warning)
        check(caseModel.feedbackKind == .warning, "saved-but-failed stays a warning")

        let timeoutTarget = FakeTarget()
        timeoutTarget.session = snapshot(a.email)
        timeoutTarget.running = true
        timeoutTarget.refuseQuit = true
        let timeoutCredentials = MemoryCredentials()
        let timeoutModel = AppModel(adapter: timeoutTarget, quota: UnsupportedQuotaProvider(),
                                    keychain: timeoutCredentials,
                                    storeURL: directory.appendingPathComponent("timeout.json"),
                                    softVerifyNanoseconds: 0)
        timeoutModel.accounts = [a, b]
        try! timeoutCredentials.save(JSONEncoder().encode(snapshot(b.email)), account: b.id.uuidString)
        await timeoutModel.switchTo(b)
        check(timeoutTarget.running && timeoutTarget.session?.email == a.email,
              "quit timeout leaves Pen and original session untouched")
        check(timeoutModel.feedback?.contains("未能退出") == true, "quit timeout explains the save dialog")

        let verifyTarget = FakeTarget()
        verifyTarget.session = snapshot(a.email)
        verifyTarget.running = true
        let verifyCredentials = MemoryCredentials()
        let verifyModel = AppModel(adapter: verifyTarget, quota: UnsupportedQuotaProvider(),
                                   keychain: verifyCredentials,
                                   storeURL: directory.appendingPathComponent("verify.json"),
                                   softVerifyNanoseconds: 80_000_000)
        verifyModel.accounts = [a, b]
        try! verifyCredentials.save(JSONEncoder().encode(snapshot(b.email)), account: b.id.uuidString)
        await verifyModel.switchTo(b)
        check(verifyModel.currentAccount?.email == b.email, "soft verify starts from a successful switch")
        verifyTarget.session = snapshot("hijacked@example.test")
        try? await Task.sleep(nanoseconds: 200_000_000)
        check(verifyModel.feedbackKind == .warning
              && verifyModel.feedback?.contains("登录身份已变化") == true,
              "delayed verify warns without rolling back")
        check(verifyTarget.session?.email == "hijacked@example.test",
              "delayed verify does not overwrite the live session")

        switch PenQuotaProvider.parseBody(Data("{\"isMetered\":false}".utf8)) {
        case .unsupported: check(true, "unmetered quota body is unsupported")
        default: check(false, "unmetered quota body is unsupported")
        }
        switch PenQuotaProvider.parseBody(Data("{}".utf8)) {
        case .unsupported: check(true, "missing quota fields are unsupported")
        default: check(false, "missing quota fields are unsupported")
        }
        switch PenQuotaProvider.parseBody(Data("{\"isMetered\":true,\"used\":1,\"limit\":5}".utf8)) {
        case .quota(let used, let total, _):
            check(used == 1 && total == 5, "metered quota body is parsed")
        default: check(false, "metered quota body is parsed")
        }
        switch PenQuotaProvider.parseBody(Data("not-json".utf8)) {
        case .failure: check(true, "invalid quota body is a failure")
        default: check(false, "invalid quota body is a failure")
        }

        let persistURL = directory.appendingPathComponent("persist.json")
        let persistModel = AppModel(adapter: FakeTarget(), quota: UnsupportedQuotaProvider(),
                                    keychain: MemoryCredentials(), storeURL: persistURL,
                                    softVerifyNanoseconds: 0)
        persistModel.accounts = [a]
        persistModel.accounts[0].plan = "once"
        persistModel.accounts[0].plan = "twice"
        persistModel.flushPersist()
        let persisted = try! JSONSerialization.jsonObject(with: Data(contentsOf: persistURL)) as! [String: Any]
        check(persisted["settings"] == nil, "launch-at-login is not written to store.json")
        let persistedAccounts = persisted["accounts"] as! [[String: Any]]
        check(persistedAccounts.first?["plan"] as? String == "twice", "account writes are coalesced")

        let credDir = directory.appendingPathComponent("creds-\(UUID().uuidString)")
        let fileStore = FileCredentialStore(directoryURL: credDir)
        let credID = UUID().uuidString
        let payload = Data("credential-blob".utf8)
        try! fileStore.save(payload, account: credID)
        check(try! fileStore.load(account: credID) == payload, "file credentials round-trip")
        let dirMode = (try! FileManager.default.attributesOfItem(atPath: credDir.path)[.posixPermissions] as! NSNumber).uint16Value & 0o777
        check(dirMode == 0o700, "credentials directory is 0700")
        let credFile = credDir.appendingPathComponent("\(UUID(uuidString: credID)!.uuidString).json")
        let fileMode = (try! FileManager.default.attributesOfItem(atPath: credFile.path)[.posixPermissions] as! NSNumber).uint16Value & 0o777
        check(fileMode == 0o600, "credential file is 0600")
        do {
            try fileStore.save(payload, account: "../evil")
            check(false, "path traversal is rejected")
        } catch CredentialError.invalidAccount {
            check(true, "path traversal is rejected")
        } catch {
            check(false, "path traversal is rejected")
        }
        try! fileStore.delete(account: credID)
        do {
            _ = try fileStore.load(account: credID)
            check(false, "deleted credentials are missing")
        } catch CredentialError.notFound {
            check(true, "deleted credentials are missing")
        } catch {
            check(false, "deleted credentials are missing")
        }
        try! fileStore.delete(account: credID)
        check(true, "repeat delete is idempotent")

        check(UpdateVersion.compare("1.0.0", "1.1.0") == .orderedAscending, "older version compares as less")
        check(UpdateVersion.compare("1.1.0", "1.1.0") == .orderedSame, "same version compares equal")
        check(UpdateVersion.normalized("v1.2.0") == "1.2.0", "tag prefix is stripped")
        check(UpdateVersion.zipAsset(from: ["notes.txt", "Pencil-Account-Switcher-1.1.0-arm64.zip"])
                == "Pencil-Account-Switcher-1.1.0-arm64.zip",
              "arm64 zip is preferred")
        check(UpdateVersion.formatBytes(1_800_000).contains("MB"), "byte formatter uses megabytes")

        let feed = FakeFeed()
        feed.release = AppRelease(version: "1.1.0",
                                  zipURL: URL(string: "https://example.test/app.zip")!,
                                  pageURL: URL(string: "https://example.test/rel")!)
        let updateDefaults = UserDefaults(suiteName: UUID().uuidString)!
        let installer = FakeInstaller()
        let updateModel = AppModel(adapter: target, quota: UnsupportedQuotaProvider(), keychain: credentials,
                                   storeURL: directory.appendingPathComponent("update.json"),
                                   softVerifyNanoseconds: 0, updateFeed: feed, updateInstaller: installer,
                                   currentVersion: "1.0.0", defaults: updateDefaults)
        updateModel.checkForUpdates(force: true)
        for _ in 0..<50 where updateModel.updateState.release == nil {
            try? await Task.sleep(nanoseconds: 10_000_000)
        }
        check(updateModel.hasVisibleUpdate, "newer release becomes a visible update")
        updateModel.postponeUpdate()
        check(!updateModel.hasVisibleUpdate && updateModel.hasAvailableUpdate,
              "postpone hides banner but keeps available update")
        let sample = AppRelease(version: "1.1.0",
                                zipURL: URL(string: "https://example.test/app.zip")!,
                                pageURL: URL(string: "https://example.test/rel")!)
        model.updateState = .downloading(sample, percent: 42, received: 1_800_000, total: 4_300_000)
        try? await Task.sleep(nanoseconds: 50_000_000)
        panel.view.layoutSubtreeIfNeeded()
        check(labels(in: panel.view).contains { $0.stringValue.contains("42%") },
              "download banner shows percent")
        feed.release = AppRelease(version: "1.0.0",
                                  zipURL: URL(string: "https://example.test/app.zip")!,
                                  pageURL: URL(string: "https://example.test/rel")!)
        let sameModel = AppModel(adapter: target, quota: UnsupportedQuotaProvider(), keychain: credentials,
                                 storeURL: directory.appendingPathComponent("same.json"),
                                 softVerifyNanoseconds: 0, updateFeed: feed, updateInstaller: installer,
                                 currentVersion: "1.0.0", defaults: UserDefaults(suiteName: UUID().uuidString)!)
        sameModel.checkForUpdates(force: true)
        try? await Task.sleep(nanoseconds: 80_000_000)
        if case .idle = sameModel.updateState { check(true, "same version stays idle") }
        else { check(false, "same version stays idle") }

        window.orderOut(nil)
    }

    @MainActor static func waitForLogin(_ model: AppModel) async {
        var sawBusy = false
        for _ in 0..<100 {
            let busy = model.addingAccount || model.switchingTo != nil
            if busy { sawBusy = true }
            if sawBusy && !busy { return }
            try? await Task.sleep(nanoseconds: 10_000_000)
        }
        check(false, "login operation finishes")
    }

    @MainActor static func viewWithID(_ id: String, in view: NSView) -> NSView? {
        if view.identifier?.rawValue == id { return view }
        return view.subviews.compactMap { viewWithID(id, in: $0) }.first
    }

    @MainActor static func labels(in view: NSView) -> [NSTextField] {
        (view as? NSTextField).map { [$0] } ?? view.subviews.flatMap { labels(in: $0) }
    }

    @MainActor static func animatedLayer(in layer: CALayer) -> CALayer? {
        if layer.animation(forKey: "spin") != nil { return layer }
        return layer.sublayers?.compactMap { animatedLayer(in: $0) }.first
    }

    @MainActor static func buttons(in view: NSView) -> [NSButton] {
        (view as? NSButton).map { [$0] } ?? view.subviews.flatMap { buttons(in: $0) }
    }

    @MainActor static func scrollViews(in view: NSView) -> [NSScrollView] {
        (view as? NSScrollView).map { [$0] } ?? view.subviews.flatMap { scrollViews(in: $0) }
    }
}
