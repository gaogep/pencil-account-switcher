import AppKit

/// 无边框面板也需要能成为 key window：否则 Esc 收不到、无法用键盘操作。
final class SwitchboardPanel: NSPanel {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem?
    private var panelWindow: SwitchboardPanel?
    private var globalClick: Any?
    private var localClick: Any?
    private var escapeMonitor: Any?
    private var shownAt = Date.distantPast
    private let model = AppModel()

    func applicationDidFinishLaunching(_ notification: Notification) {
        let panel = PanelController(model: model)
        let window = SwitchboardPanel(contentRect: NSRect(x: 0, y: 0, width: Theme.panelWidth, height: 400),
                                      styleMask: [.borderless, .nonactivatingPanel],
                                      backing: .buffered, defer: false)
        window.contentViewController = panel
        window.isFloatingPanel = true
        window.level = .statusBar
        window.hasShadow = true
        window.isOpaque = false
        window.backgroundColor = .clear
        window.hidesOnDeactivate = false
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        window.contentMinSize = NSSize(width: Theme.panelWidth, height: 200)
        window.contentMaxSize = NSSize(width: Theme.panelWidth, height: 900)
        panelWindow = window
        if Log.isEnabled {
            NotificationCenter.default.addObserver(forName: NSWindow.didResizeNotification,
                                                   object: window, queue: .main) { _ in
                Task { @MainActor in
                    Log.debug("didResize frame=\(window.frame)")
                }
            }
        }

        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        if let button = item.button {
            button.image = makeMenuBarIcon()
            button.action = #selector(statusItemClicked(_:))
            button.target = self
            button.sendAction(on: [.leftMouseUp, .rightMouseUp])
        }
        statusItem = item

        // 调试辅助：SWITCHBOARD_AUTO_OPEN=1 启动时自动弹出面板（截图走查用）
        if ProcessInfo.processInfo.environment["SWITCHBOARD_AUTO_OPEN"] == "1" {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { [weak self] in
                self?.togglePanel(nil)
            }
        }
        // 调试辅助：SWITCHBOARD_SNAPSHOT=/tmp/sw 离屏渲染三个页面为 PNG 后退出
        if ProcessInfo.processInfo.environment["SWITCHBOARD_SNAPSHOT"] != nil {
            captureSnapshots()
        }
    }

    private func captureSnapshots() {
        guard let prefix = ProcessInfo.processInfo.environment["SWITCHBOARD_SNAPSHOT"],
              let panel = panelWindow?.contentViewController else { return }
        // 演示模式下逐个跑完整状态清单；非演示模式只截三个页面（真实数据，仅供本地走查）
        let demo = ProcessInfo.processInfo.environment["SWITCHBOARD_DEMO"] == "1"
        let scenarios: [SnapshotScenario] = demo ? SnapshotScenarios.all : [
            SnapshotScenario(name: "main") { $0.route = .main },
            SnapshotScenario(name: "settings") { $0.route = .settings },
            SnapshotScenario(name: "manage") { $0.route = .manage },
        ]
        let only = ProcessInfo.processInfo.environment["SWITCHBOARD_SNAPSHOT_ONLY"]
            .map { Set($0.split(separator: ",").map(String.init)) }
        var queue = scenarios.filter { only?.contains($0.name) ?? true }
        func next() {
            guard !queue.isEmpty else {
                NSApp.terminate(nil)
                return
            }
            let scenario = queue.removeFirst()
            if demo { model.resetDemoForSnapshot(quotaTime: SnapshotScenarios.quotaTime) }
            scenario.setup(model)
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) { [model] in
                let url = URL(fileURLWithPath: "\(prefix)-\(scenario.name).png")
                if let email = scenario.alertEmail?(model) {
                    let alert = ManageScreen.makeRemoveAlert(email: email)
                    alert.layout()
                    if let view = alert.window.contentView {
                        Self.writePNG(of: view, to: url)
                    }
                } else {
                    (panel as? PanelController)?.renderNow()
                    let view = panel.view
                    let size = NSSize(width: Theme.panelWidth, height: max(view.fittingSize.height, 200))
                    view.setFrameSize(size)
                    view.layoutSubtreeIfNeeded()
                    Self.writePNG(of: view, to: url)
                }
                next()
            }
        }
        next()
    }

    private static func writePNG(of view: NSView, to url: URL) {
        guard let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { return }
        view.cacheDisplay(in: view.bounds, to: rep)
        guard let data = rep.representation(using: .png, properties: [:]) else { return }
        do {
            try data.write(to: url)
            FileHandle.standardError.write("wrote \(url.path)\n".data(using: .utf8)!)
        } catch {
            FileHandle.standardError.write("failed \(url.path): \(error)\n".data(using: .utf8)!)
        }
    }

    func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool { true }

    func applicationWillTerminate(_ notification: Notification) {
        model.flushPersist()
    }

    /// 双击 .app 再次启动时弹出面板（只负责打开；关闭由点图标、点外部或 ESC 完成）
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if panelWindow?.isVisible != true {
            togglePanel(nil)
        }
        return true
    }

    /// 右键：弹退出菜单；左键：开合面板
    @objc private func statusItemClicked(_ sender: Any?) {
        let type = NSApp.currentEvent?.type
        Log.debug("statusItemClicked type=\(type?.rawValue ?? 999)")
        if type == .rightMouseUp || type == .rightMouseDown {
            showContextMenu()
        } else {
            togglePanel(sender)
        }
    }

    private func showContextMenu() {
        Log.debug("showContextMenu entered")
        let menu = NSMenu()

        let openItem = NSMenuItem(title: "打开面板", action: #selector(menuOpenPanel), keyEquivalent: "")
        openItem.target = self
        menu.addItem(openItem)

        let version = NSMenuItem(title: "Pencil Account Switcher \(Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "1.0.0")",
                                 action: nil, keyEquivalent: "")
        version.isEnabled = false
        menu.addItem(version)

        menu.addItem(.separator())

        let quitItem = NSMenuItem(title: "退出 Pencil Account Switcher", action: #selector(menuQuit), keyEquivalent: "q")
        quitItem.target = self
        menu.addItem(quitItem)

        // 直接在按钮下方弹出，不占用 statusItem.menu（否则左键也会变成菜单）
        guard let item = statusItem else { return }
        if panelWindow?.isVisible == true { hidePanel(reason: "context menu") }
        // 标准做法：临时挂上菜单再 performClick，菜单由系统状态栏机制呈现
        item.menu = menu
        item.button?.performClick(nil)
        item.menu = nil
        Log.debug("context menu dismissed")
    }

    @objc private func menuOpenPanel() {
        if panelWindow?.isVisible != true { showPanel() }
    }

    @objc private func menuQuit() {
        NSApp.terminate(nil)
    }

    @objc private func togglePanel(_ sender: Any?) {
        let visible = panelWindow?.isVisible == true
        // 防抖：单次点击偶尔会送达两回动作，刚显示就立即收起会让面板一闪而过
        if visible, Date().timeIntervalSince(shownAt) < 0.35 {
            Log.debug("toggle ignored: debounce")
            return
        }
        Log.debug("toggle: isVisible=\(visible) frame=\(String(describing: panelWindow?.frame)) shown=\(String(describing: panelWindow?.isOnActiveSpace))")
        if visible {
            hidePanel()
        } else {
            showPanel()
        }
    }

    /// 鼠标是否落在菜单栏图标上（点图标由状态项动作负责开关，不算"点外部"）
    private func isOnStatusItem() -> Bool {
        guard let window = statusItem?.button?.window else { return false }
        return window.frame.contains(NSEvent.mouseLocation)
    }

    private func showPanel() {
        guard let button = statusItem?.button, let window = panelWindow else {
            Log.debug("showPanel: missing button/window")
            return
        }
        let anchor = button.window?.convertToScreen(button.convert(button.bounds, to: nil))
        Log.debug("showPanel: anchor=\(String(describing: anchor)) statusWindow=\(String(describing: button.window?.frame))")

        model.route = .main
        model.syncCurrentIdentity()
        model.checkForUpdates(force: false)
        let height = max(window.contentViewController?.preferredContentSize.height ?? 400, 200)
        let width = Theme.panelWidth
        let screen = button.window?.screen?.visibleFrame
            ?? NSScreen.main?.visibleFrame
            ?? NSRect(x: 0, y: 0, width: 1440, height: 900)

        var x: CGFloat
        var y: CGFloat
        // 锚点被菜单栏收纳工具折叠到屏幕外时，退化为贴着屏幕右上角显示（不静默失败）
        if let anchor, NSScreen.screens.contains(where: { $0.frame.intersects(anchor) }) {
            x = anchor.midX - width / 2
            y = anchor.minY - height - 6
        } else {
            Log.debug("showPanel: anchor off-screen, falling back to top-right")
            x = screen.maxX - width - 12
            y = screen.maxY - height - 12
        }
        x = min(max(x, screen.minX + 8), screen.maxX - width - 8)
        y = max(y, screen.minY + 8)
        window.setFrame(NSRect(x: x, y: y, width: width, height: height), display: true)

        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
        shownAt = Date()
        Log.debug("shown: visible=\(window.isVisible) frame=\(window.frame) alpha=\(window.alphaValue)")

        // 延迟注册：否则触发本次显示的这记点击会被当成"点击外部"而立刻收起面板
        DispatchQueue.main.async { [weak self] in
            guard let self, self.panelWindow?.isVisible == true else { return }
            if self.globalClick == nil {
                self.globalClick = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] _ in
                    guard let self, let panel = self.panelWindow, panel.isVisible else { return }
                    if self.shouldKeepPanelOpen() { return }
                    guard panel.attachedSheet == nil else { return }
                    // 宽限期：触发显示的那记点击（含合成的 AX 点击）不算"点击外部"
                    guard Date().timeIntervalSince(self.shownAt) > 0.3 else { return }
                    // 点的是菜单栏图标：交给状态项动作，避免重复事件把刚打开的面板关掉
                    if self.isOnStatusItem() { return }
                    // 再按鼠标位置判定：落在面板范围内就不关（避免事件路由误判把面板关掉）
                    if panel.frame.contains(NSEvent.mouseLocation) { return }
                    self.hidePanel(reason: "global click")
                }
            }
            if self.localClick == nil {
                self.localClick = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] event in
                    guard let self, let panel = self.panelWindow, panel.isVisible else { return event }
                    if self.shouldKeepPanelOpen() { return event }
                    guard panel.attachedSheet == nil else { return event }
                    guard Date().timeIntervalSince(self.shownAt) > 0.3 else { return event }
                    if event.window === panel { return event }
                    if event.window === self.statusItem?.button?.window { return event }
                    self.hidePanel(reason: "local click")
                    return event
                }
            }
            if self.escapeMonitor == nil {
                self.escapeMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
                    guard self?.shouldKeepPanelOpen() != true else { return event }
                    guard self?.panelWindow?.attachedSheet == nil else { return event }
                    if event.keyCode == 53 {
                        self?.hidePanel(reason: "escape")
                        return nil
                    }
                    return event
                }
            }
        }
    }

    private func shouldKeepPanelOpen() -> Bool {
        model.addingAccount || model.switchingTo != nil || model.updateState.isBusy
    }

    private func hidePanel(reason: String = "") {
        Log.debug("hidePanel reason=\(reason) sinceShown=\(Date().timeIntervalSince(shownAt))")
        panelWindow?.orderOut(nil)
        if let globalClick {
            NSEvent.removeMonitor(globalClick)
            self.globalClick = nil
        }
        if let localClick {
            NSEvent.removeMonitor(localClick)
            self.localClick = nil
        }
        if let escapeMonitor {
            NSEvent.removeMonitor(escapeMonitor)
            self.escapeMonitor = nil
        }
    }
}

@main
enum SwitchboardApp {
    @MainActor
    static func main() {
        let app = NSApplication.shared
        let delegate = AppDelegate()
        app.delegate = delegate
        app.setActivationPolicy(.accessory)
        app.run()
    }
}
