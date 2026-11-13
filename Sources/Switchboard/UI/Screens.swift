import AppKit
import Combine

// MARK: - 页面基类
// 结构只构建一次（rootStack/scrollView/header/footer 槽位常驻），
// rebuild() 原地刷新内容——避免弹出面板因整树替换/尺寸抖动跳位。

class BaseScreen: NSViewController {
    let model: AppModel
    let contentStack = NSStackView()
    let scrollView = NSScrollView()
    private var rootStack: NSStackView?
    private var headerWrap = FlippedView()
    private var statusWrap = FlippedView()
    private var footerWrap = FlippedView()
    private var scrollHeight: NSLayoutConstraint?
    private var statusHeight: NSLayoutConstraint?
    private var cachedMaxScrollHeight: CGFloat = 430
    private var lastBannerToken: String?

    init(model: AppModel) {
        self.model = model
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func loadView() {
        view = FlippedView()
        view.widthAnchor.constraint(equalToConstant: Theme.panelWidth).isActive = true
        contentStack.orientation = .vertical
        contentStack.spacing = 16
        contentStack.alignment = .leading
        contentStack.translatesAutoresizingMaskIntoConstraints = false
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        rebuild()
    }

    /// 子类实现：清空并重建 contentStack，最后调用 finalize
    func rebuild() {}

    func resetContent() {
        contentStack.arrangedSubviews.forEach {
            contentStack.removeArrangedSubview($0)
            $0.removeFromSuperview()
        }
    }

    func makeAddButton(width: CGFloat) -> NSView {
        let cancelling = model.addingAccount
        let view = makeOutlineActionButton(cancelling ? "取消网页登录" : "添加账号",
                                           symbol: cancelling ? "xmark" : "plus",
                                           action: #selector(addAccount), target: self, width: width)
        if let button = view.subviews.first as? NSButton {
            button.identifier = NSUserInterfaceItemIdentifier(cancelling ? ButtonID.cancelLogin : ButtonID.addAccount)
            if !cancelling && model.persistenceBlocked {
                button.isEnabled = false
            }
        }
        return view
    }

    @objc func addAccount() {
        if let window = view.window { model.addAccount(window: window) }
    }

    func makeReimportButton(for record: AccountRecord) -> NSView {
        let view = makeQuietActionButton("重新导入", symbol: "arrow.triangle.2.circlepath",
                                         action: #selector(addAccount), target: self)
        if let button = view.subviews.first as? NSButton {
            button.identifier = NSUserInterfaceItemIdentifier(ButtonID.reimport)
            let busy = model.refreshing || model.switchingTo != nil || model.addingAccount || model.persistenceBlocked
            button.isEnabled = !busy
            button.toolTip = "登录已失效，打开网页重新登录 \(record.email)，无需先移除"
            button.setAccessibilityLabel("重新导入 \(record.email)")
        }
        return view
    }

    /// 子类实现：各页自身的滚动区内边距
    var contentInsets: NSEdgeInsets {
        NSEdgeInsets(top: 2, left: 18, bottom: 14, right: 18)
    }

    func finalize(header: NSView, footer: NSView) {
        contentStack.edgeInsets = contentInsets
        cachedMaxScrollHeight = 430

        if rootStack == nil {
            let clip = FlippedView()
            clip.translatesAutoresizingMaskIntoConstraints = false
            clip.addSubview(contentStack)
            scrollView.translatesAutoresizingMaskIntoConstraints = false
            scrollView.hasVerticalScroller = true
            scrollView.autohidesScrollers = true
            scrollView.scrollerStyle = .overlay
            scrollView.drawsBackground = false
            scrollView.borderType = .noBorder
            scrollView.scrollerInsets = NSEdgeInsets(top: 8, left: 0, bottom: 8, right: 3)
            let scroller = PanelScroller()
            scroller.controlSize = .mini
            scroller.scrollerStyle = .overlay
            scrollView.verticalScroller = scroller
            scrollView.documentView = clip

            let stack = NSStackView(views: [headerWrap, statusWrap, scrollView, footerWrap])
            stack.orientation = .vertical
            stack.spacing = 0
            stack.alignment = .leading
            stack.distribution = .fill
            stack.detachesHiddenViews = true
            stack.translatesAutoresizingMaskIntoConstraints = false
            headerWrap.setContentHuggingPriority(.required, for: .vertical)
            footerWrap.setContentHuggingPriority(.required, for: .vertical)
            statusWrap.setContentHuggingPriority(.required, for: .vertical)
            statusWrap.isHidden = true
            view.addSubview(stack)
            let bannerHeight = statusWrap.heightAnchor.constraint(equalToConstant: 0)
            NSLayoutConstraint.activate([
                stack.topAnchor.constraint(equalTo: view.topAnchor),
                stack.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                stack.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                stack.bottomAnchor.constraint(equalTo: view.bottomAnchor),
                stack.widthAnchor.constraint(equalTo: view.widthAnchor),
                headerWrap.widthAnchor.constraint(equalTo: stack.widthAnchor),
                statusWrap.widthAnchor.constraint(equalTo: stack.widthAnchor),
                footerWrap.widthAnchor.constraint(equalTo: stack.widthAnchor),
                scrollView.widthAnchor.constraint(equalTo: stack.widthAnchor),
                bannerHeight,
                contentStack.topAnchor.constraint(equalTo: clip.topAnchor),
                contentStack.leadingAnchor.constraint(equalTo: clip.leadingAnchor),
                contentStack.trailingAnchor.constraint(equalTo: clip.trailingAnchor),
                clip.widthAnchor.constraint(equalTo: scrollView.contentView.widthAnchor),
                clip.heightAnchor.constraint(greaterThanOrEqualTo: contentStack.heightAnchor),
            ])
            let h = scrollView.heightAnchor.constraint(equalToConstant: 0)
            h.isActive = true
            scrollHeight = h
            statusHeight = bannerHeight
            rootStack = stack
        }

        // 原地替换头部/底部内容
        headerWrap.subviews.forEach { $0.removeFromSuperview() }
        headerWrap.addSubview(header)
        NSLayoutConstraint.pin(header, to: headerWrap, insets: NSEdgeInsets())
        footerWrap.subviews.forEach { $0.removeFromSuperview() }
        footerWrap.addSubview(footer)
        NSLayoutConstraint.pin(footer, to: footerWrap, insets: NSEdgeInsets())
        updateStatusBanner(force: true)
    }

    /// 标题下方状态条：更新横幅优先于普通反馈。
    func updateStatusBanner(force: Bool = false) {
        let token = bannerToken()
        if !force && token == lastBannerToken { return }
        lastBannerToken = token
        statusWrap.subviews.forEach { $0.removeFromSuperview() }
        if model.hasVisibleUpdate, let banner = makeUpdateBanner() {
            statusWrap.addSubview(banner)
            NSLayoutConstraint.pin(banner, to: statusWrap, insets: NSEdgeInsets())
            statusHeight?.constant = 72
            statusWrap.isHidden = false
        } else if let text = model.feedback {
            let banner = makeStatusBanner(text, kind: model.feedbackKind)
            statusWrap.addSubview(banner)
            NSLayoutConstraint.pin(banner, to: statusWrap, insets: NSEdgeInsets())
            statusHeight?.constant = 47
            statusWrap.isHidden = false
        } else {
            statusHeight?.constant = 0
            statusWrap.isHidden = true
        }
        refreshFittingSize()
    }

    private func bannerToken() -> String {
        switch model.updateState {
        case .downloading(_, let percent, let received, let total) where model.hasVisibleUpdate:
            return "update-dl:\(percent):\(received):\(total ?? -1)"
        case .available(let release, false):
            return "update-available:\(release.version)"
        case .ready(let release, _):
            return "update-ready:\(release.version)"
        default:
            break
        }
        return model.feedback.map { "\($0)|\(model.feedbackKind)|\(model.feedbackCaption ?? "")" } ?? ""
    }

    private func makeUpdateBanner() -> NSView? {
        let title: String
        let caption: String?
        var actions: [(String, Selector, Bool)] = []
        var fraction: Double?
        switch model.updateState {
        case .available(let release, false):
            title = "新版本 \(release.version) 可用"
            caption = "下载完成后会重启应用"
            actions = [("稍后", #selector(postponeUpdate), true),
                       ("立即更新", #selector(startUpdate), false)]
        case .downloading(_, let percent, let received, let total):
            title = "正在下载更新… \(percent)%"
            if let total {
                caption = "\(UpdateVersion.formatBytes(received)) / \(UpdateVersion.formatBytes(total))"
                fraction = Double(percent) / 100
            } else {
                caption = UpdateVersion.formatBytes(received)
            }
            actions = [("稍后", #selector(postponeUpdate), true),
                       ("立即更新", #selector(startUpdate), false)]
        case .ready(let release, _):
            title = "已下载，将重启以完成更新"
            caption = "会替换应用程序文件夹里的副本"
            actions = [("安装并重启", #selector(installUpdate), false)]
            _ = release
        default:
            return nil
        }
        let iconName: String
        switch model.updateState {
        case .ready: iconName = "checkmark.circle"
        case .downloading: iconName = "arrow.clockwise"
        default: iconName = "arrow.down.circle"
        }
        let icon = makeSymbol(iconName, size: 15, color: Theme.accent)
        var textViews: [NSView] = [makeLabel(title, size: 12, weight: .semibold, color: Theme.bannerInk)]
        if let caption {
            textViews.append(makeLabel(caption, size: 11, color: Theme.bannerMuted))
        }
        if let fraction {
            let bar = QuotaProgressBar(width: 160, height: 4,
                                       track: Theme.progressTrackOther, fillColor: Theme.accent)
            bar.update(fraction: fraction)
            textViews.append(bar)
        }
        let text = makeVStack(textViews, spacing: 3)
        text.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        var views: [NSView] = [icon, text, makeSpacer()]
        let downloading: Bool
        if case .downloading = model.updateState { downloading = true } else { downloading = false }
        for (label, action, ghost) in actions {
            if ghost {
                let button = makeLinkButton(label, action: action, target: self, color: Theme.bannerMuted)
                button.isEnabled = !downloading
                views.append(button)
            } else {
                let wrap = makePillButton(label, bg: downloading ? Theme.switchOffTrack : Theme.switchBtnBG,
                                          textColor: downloading ? Theme.textSecondary : Theme.switchBtnText,
                                          action: action, target: self)
                if let button = wrap.subviews.first as? NSButton {
                    button.isEnabled = !downloading || label == "安装并重启"
                }
                views.append(wrap)
            }
        }
        let row = makeHStack(views, spacing: 8)
        let card = RoundedBoxView(fill: Theme.bannerFill, radius: 8)
        card.identifier = NSUserInterfaceItemIdentifier("refresh-status")
        card.setAccessibilityLabel(title)
        card.addSubview(row)
        NSLayoutConstraint.pin(row, to: card, insets: NSEdgeInsets(top: 10, left: 12, bottom: 10, right: 12))
        let container = FlippedView()
        container.addSubview(card)
        NSLayoutConstraint.pin(card, to: container, insets: NSEdgeInsets(top: 0, left: 18, bottom: 12, right: 18))
        container.widthAnchor.constraint(equalToConstant: Theme.panelWidth).isActive = true
        container.heightAnchor.constraint(equalToConstant: 72).isActive = true
        return container
    }

    @objc func startUpdate() { model.startUpdate() }
    @objc func postponeUpdate() { model.postponeUpdate() }
    @objc func installUpdate() { model.installUpdate() }
    @objc func checkUpdates() { model.checkForUpdates(force: true) }

    private func refreshFittingSize() {
        guard rootStack != nil else { return }
        view.layoutSubtreeIfNeeded()
        let contentHeight = contentStack.fittingSize.height
        let headerH = headerWrap.fittingSize.height
        let footerH = footerWrap.fittingSize.height
        let bannerH = statusWrap.isHidden ? 0 : (statusHeight?.constant ?? 0)
        let available = (view.window?.screen ?? NSScreen.main)?.visibleFrame.height ?? 800
        let scrollLimit = max(100, available - headerH - bannerH - footerH - 24)
        scrollHeight?.constant = min(contentHeight, cachedMaxScrollHeight, scrollLimit)
        view.layoutSubtreeIfNeeded()
        let newSize = NSSize(width: Theme.panelWidth, height: headerH + bannerH + (scrollHeight?.constant ?? 0) + footerH)
        Log.debug("finalize content=\(contentHeight) scroll=\(scrollHeight?.constant ?? -1) size=\(newSize) items=\(contentStack.arrangedSubviews.count) old=\(preferredContentSize)")
        if preferredContentSize != newSize {
            preferredContentSize = newSize
        }
    }

    /// 标题栏：品牌图标 + 标题 + 右侧操作（主面板用）
    func makeBrandHeader(actions: [NSView]) -> NSView {
        let icon = BrandIconView()
        icon.widthAnchor.constraint(equalToConstant: 32).isActive = true
        icon.heightAnchor.constraint(equalToConstant: 32).isActive = true

        let title = makeLabel("Pencil Account Switcher", size: 14, weight: .bold, color: Theme.titleText)
        title.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        let titleStack = makeVStack([title,
                                     makeLabel("账号切换器", size: 11, color: Theme.textSecondary)],
                                    spacing: 2)
        titleStack.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        let actionStack = NSStackView(views: actions)
        actionStack.orientation = .horizontal
        actionStack.spacing = 14
        actionStack.translatesAutoresizingMaskIntoConstraints = false

        let header = makeHStack([icon, titleStack, makeSpacer(), actionStack], spacing: 10)
        return wrapHeader(header, insets: NSEdgeInsets(top: 20, left: 22, bottom: 16, right: 22))
    }

    /// 标题栏：返回圆角块 + 标题（设置/管理页用）
    func makeNavHeader(backAction: Selector, backLabel: String,
                       title: String, subtitle: String, trailing: NSView?) -> NSView {
        let back = makeImageButton("chevron.left", size: 14, weight: .semibold,
                                   color: Theme.backArrow, action: backAction, target: self,
                                   tooltip: "返回\(backLabel)")
        let backBlock = RoundedBoxView(fill: Theme.backBtnBG, radius: 8)
        backBlock.addSubview(back)
        NSLayoutConstraint.pin(back, to: backBlock, insets: NSEdgeInsets(top: 6, left: 6, bottom: 6, right: 6))
        backBlock.widthAnchor.constraint(equalToConstant: 30).isActive = true
        backBlock.heightAnchor.constraint(equalToConstant: 30).isActive = true
        backBlock.setAccessibilityLabel("返回\(backLabel)")

        let titleStack = makeVStack([makeLabel(title, size: 15, weight: .bold, color: Theme.titleText),
                                     makeLabel(subtitle, size: 11, color: Theme.textSecondary)], spacing: 3)
        var views: [NSView] = [backBlock, titleStack, makeSpacer()]
        if let trailing { views.append(trailing) }
        let header = makeHStack(views, spacing: 12)
        return wrapHeader(header, insets: NSEdgeInsets(top: 20, left: 22, bottom: 18, right: 22))
    }

    private func wrapHeader(_ header: NSStackView, insets: NSEdgeInsets) -> NSView {
        let container = FlippedView()
        container.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(header)
        NSLayoutConstraint.pin(header, to: container, insets: insets)
        header.widthAnchor.constraint(equalToConstant: Theme.panelWidth - insets.left - insets.right).isActive = true
        return container
    }

    func makeStatusBanner(_ text: String, kind: FeedbackKind) -> NSView {
        let success = kind == .success
        let loading = kind == .loading
        let warning = kind == .warning
        let tint = warning ? Theme.warnTint : Theme.accent
        let titleColor = warning ? Theme.warnInk : Theme.bannerInk
        let icon = makeSymbol(success ? "checkmark.circle" : (loading ? "arrow.clockwise" : "exclamationmark.circle"),
                              size: 15, color: tint)
        let title = makeLabel(text, size: 12, weight: .semibold, color: titleColor)
        title.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        var views: [NSView] = [icon, title]
        if let caption = model.feedbackCaption, !caption.isEmpty {
            views.append(makeLabel(caption, size: 11, color: Theme.bannerMuted))
        }
        views.append(makeSpacer())
        let row = makeHStack(views, spacing: 8)
        let card = RoundedBoxView(fill: warning ? Theme.warnFill : Theme.bannerFill, radius: 8)
        card.identifier = NSUserInterfaceItemIdentifier("refresh-status")
        card.toolTip = text
        card.setAccessibilityLabel(captionLabel(text: text, caption: model.feedbackCaption))
        card.addSubview(row)
        NSLayoutConstraint.pin(row, to: card, insets: NSEdgeInsets(top: 10, left: 12, bottom: 10, right: 12))
        let container = FlippedView()
        container.addSubview(card)
        NSLayoutConstraint.pin(card, to: container, insets: NSEdgeInsets(top: 0, left: 18, bottom: 12, right: 18))
        container.widthAnchor.constraint(equalToConstant: Theme.panelWidth).isActive = true
        container.heightAnchor.constraint(equalToConstant: 47).isActive = true
        return container
    }

    private func captionLabel(text: String, caption: String?) -> String {
        if let caption, !caption.isEmpty { return "\(text)，\(caption)" }
        return text
    }
}

// MARK: - 主面板

final class MainScreen: BaseScreen {
    override func rebuild() {
        resetContent()
        let others = model.accounts.filter { !$0.isCurrent }
        let anySwitching = model.switchingTo != nil || model.addingAccount
        Log.debug("rebuild accounts=\(model.accounts.count) current=\(model.currentAccount?.email ?? "nil") others=\(others.count) live=\(model.liveEmail ?? "nil")")

        if model.accounts.isEmpty {
            contentStack.addArrangedSubview(makeEmptyState())
        } else {
            if let current = model.currentAccount {
                contentStack.addArrangedSubview(makeCurrentCard(current))
            }
            if let unregistered = model.unregisteredLiveEmail {
                contentStack.addArrangedSubview(makeUnregisteredHint(unregistered))
            }
            if !others.isEmpty {
                contentStack.addArrangedSubview(makeOthersTitle(count: others.count))
                for record in others {
                    contentStack.addArrangedSubview(makeOtherRow(record, anySwitching: anySwitching))
                }
            }
            contentStack.addArrangedSubview(makeAddButton(width: Theme.panelWidth - 36))
        }
        let spacer = NSView()
        spacer.widthAnchor.constraint(equalToConstant: Theme.panelWidth - 36).isActive = true
        contentStack.addArrangedSubview(spacer)

        let refresh = RefreshButton(action: #selector(refreshQuotas), target: self)
        refresh.identifier = NSUserInterfaceItemIdentifier("quota-refresh")
        let settings = makeImageButton("gearshape", size: 15, color: Theme.headerAction,
                                       action: #selector(openSettings), target: self, tooltip: "设置")
        let brand = makeBrandHeader(actions: [refresh, settings])
        finalize(header: brand, footer: makeFooter())
    }

    private static let quotaClock: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "HH:mm"
        return formatter
    }()

    private func footerQuotaText() -> String {
        if let latest = model.accounts.compactMap(\.lastQuotaUpdate).max() {
            return "额度更新于 \(Self.quotaClock.string(from: latest))"
        }
        return "额度更新于 —"
    }

    private func makeEmptyState() -> NSView {
        let symbol = makeSymbol("person.crop.circle.badge.questionmark", size: 32,
                                color: Theme.textSecondary)
        let title = makeLabel("还没有添加账号", size: 13, weight: .medium, color: Theme.titleText)
        let hint = makeLabel("点击添加账号，在 Pen 官方网页完成登录即可开始使用", size: 11, color: Theme.textSecondary, wrap: true)
        hint.alignment = .center
        hint.preferredMaxLayoutWidth = Theme.panelWidth - 64
        let add = makeAddButton(width: Theme.panelWidth - 64)
        let stack = makeVStack([symbol, title, hint, add], spacing: 10, alignment: .centerX)
        let card = makeCard(background: .clear)
        card.addSubview(stack)
        NSLayoutConstraint.pin(stack, to: card, insets: NSEdgeInsets(top: 24, left: 14, bottom: 24, right: 14))
        stack.widthAnchor.constraint(equalToConstant: Theme.panelWidth - 64).isActive = true
        return card
    }

    /// 当前账号卡片：石墨底、三段式（标签行/信息行/额度行）
    private func makeCurrentCard(_ record: AccountRecord) -> NSView {
        let card = makeCard(background: Theme.currentCardBG, stroke: Theme.currentCardStroke, radius: 11)
        card.translatesAutoresizingMaskIntoConstraints = false

        let tag = makeLabel("当前使用", size: 11, weight: .semibold, color: Theme.tagText)
        let topRow = makeHStack([tag, makeSpacer(), makeAuthBadge(record.authState)], spacing: 6)

        let avatar = AvatarView.make(email: record.email, letters: record.avatarLetters, size: 36)
        var infoViews: [NSView] = [makeLabel(record.email, size: 14, weight: .semibold, color: Theme.titleText)]
        if let plan = record.plan {
            infoViews.append(makeLabel(plan, size: 11, color: Theme.settingValue))
        }
        let info = makeVStack(infoViews, spacing: 3)
        info.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        var trailing: [NSView] = []
        if record.showsReimport { trailing.append(makeReimportButton(for: record)) }
        trailing.append(makeSymbol("checkmark.circle.fill", size: 16, color: Theme.checkIcon))
        let infoRow = makeHStack([avatar, info, makeSpacer()] + trailing, spacing: 8)

        let quotaTitle = makeLabel("MCP 使用天数", size: 11, color: Theme.textSecondary)
        let quotaValue = makeLabel(quotaText(record.usedDays, record.totalDays), size: 11,
                                   weight: .semibold, color: Theme.accent)
        quotaValue.toolTip = record.lastError
        let quotaRow = makeHStack([quotaTitle, makeSpacer(), quotaValue], spacing: 8)
        let bar = QuotaProgressBar(width: Theme.panelWidth - 36 - 32, height: 5,
                                   track: Theme.progressTrackCurrent, fillColor: Theme.progressFill)
        bar.update(used: record.usedDays, total: record.totalDays)
        let quotaStack = makeVStack([quotaRow, bar], spacing: 8)

        let stack = makeVStack([topRow, infoRow, quotaStack], spacing: 14)
        card.addSubview(stack)
        NSLayoutConstraint.pin(stack, to: card, insets: NSEdgeInsets(top: 16, left: 16, bottom: 16, right: 16))
        stack.widthAnchor.constraint(equalToConstant: Theme.panelWidth - 36 - 32).isActive = true
        card.setAccessibilityLabel("当前账号 \(record.email)，\(record.authState.label)")
        return card
    }

    /// 目标软件登录了但还没导入切号器的身份（验收 11：不误标当前账号）
    private func makeUnregisteredHint(_ email: String) -> NSView {
        let card = makeCard(background: Theme.warnFill, stroke: Theme.cardStroke)
        let icon = makeSymbol("exclamationmark.triangle", size: 12, color: Theme.warnTint)
        let text = makeLabel("检测到未登记的登录身份：\(email)", size: 11, color: Theme.textSecondary)
        let importButton = makeLinkButton("导入", action: #selector(importLive), target: self,
                                          color: Theme.switchBtnText)
        importButton.identifier = NSUserInterfaceItemIdentifier(ButtonID.importLive)
        importButton.isEnabled = !model.persistenceBlocked
        let row = makeHStack([icon, text, makeSpacer(), importButton], spacing: 8)
        card.addSubview(row)
        NSLayoutConstraint.pin(row, to: card, insets: NSEdgeInsets(top: 10, left: 12, bottom: 10, right: 12))
        row.widthAnchor.constraint(equalToConstant: Theme.panelWidth - 36 - 24).isActive = true
        return card
    }

    private func makeOthersTitle(count: Int) -> NSView {
        makeHStack([makeLabel("其他账号", size: 12, weight: .bold, color: Theme.titleText),
                    makeSpacer(),
                    makeLabel("\(count) 个账号", size: 11, color: Theme.textSecondary)], spacing: 8)
    }

    /// 其他账号行：无卡片背景两行式（上行信息+按钮，下行额度+进度条）
    private func makeOtherRow(_ record: AccountRecord, anySwitching: Bool) -> NSView {
        let isSwitching = model.switchingTo == record.id

        let avatar = AvatarView.make(email: record.email, letters: record.avatarLetters, size: 34)
        var infoViews: [NSView] = [makeLabel(record.email, size: 12, weight: .semibold, color: Theme.textPrimary)]
        if let plan = record.plan {
            infoViews.append(makeLabel(plan, size: 10, color: Theme.textSecondary))
        }
        let info = makeVStack(infoViews, spacing: 3)
        info.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let switchButton = makePillButton(isSwitching ? "切换中…" : "切换",
                                          bg: Theme.switchBtnBG, textColor: Theme.switchBtnText,
                                          action: #selector(switchAccount(_:)), target: self,
                                          identifier: ButtonID.switchAccount, accountID: record.id)
        if let button = switchButton.subviews.first as? NSButton {
            button.isEnabled = !anySwitching && !model.refreshing
            if record.authState == .expired {
                button.toolTip = "登录已失效，切换后若仍无法使用请点重新导入"
            }
            button.setAccessibilityLabel("切换到 \(record.email)")
        }

        var actions: [NSView] = [makeAuthBadge(record.authState)]
        if record.showsReimport { actions.append(makeReimportButton(for: record)) }
        actions.append(switchButton)
        let actionRow = makeHStack(actions, spacing: 6)
        actionRow.setContentHuggingPriority(.required, for: .horizontal)
        let line1 = makeHStack([avatar, info, makeSpacer(), actionRow], spacing: 10)
        let quotaLabel = makeLabel("MCP 使用天数", size: 10, color: Theme.textSecondary)
        let quotaValue = makeLabel(quotaText(record.usedDays, record.totalDays), size: 10,
                                   weight: .semibold, color: Theme.accent)
        quotaValue.toolTip = record.lastError
        let bar = QuotaProgressBar(width: 140, height: 4,
                                   track: Theme.progressTrackOther, fillColor: Theme.progressFill)
        bar.update(used: record.usedDays, total: record.totalDays)
        let line2 = makeHStack([quotaLabel, quotaValue, makeSpacer(), bar], spacing: 6)

        let stack = makeVStack([line1, line2], spacing: 10)
        stack.translatesAutoresizingMaskIntoConstraints = false
        let wrap = FlippedView()
        wrap.addSubview(stack)
        NSLayoutConstraint.pin(stack, to: wrap, insets: NSEdgeInsets(top: 10, left: 2, bottom: 13, right: 2))
        line1.widthAnchor.constraint(equalToConstant: Theme.panelWidth - 40).isActive = true
        line2.widthAnchor.constraint(equalToConstant: Theme.panelWidth - 40).isActive = true
        return wrap
    }

    /// 底部：额度更新时间
    private func makeFooter() -> NSView {
        let time = makeLabel(footerQuotaText(), size: 10, color: Theme.textFaint)
        let row = makeHStack([time, makeSpacer()], spacing: 8)
        row.setAccessibilityLabel("额度更新信息")
        let container = FlippedView()
        container.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(row)
        NSLayoutConstraint.pin(row, to: container, insets: NSEdgeInsets(top: 17, left: 20, bottom: 18, right: 20))
        row.widthAnchor.constraint(equalToConstant: Theme.panelWidth - 40).isActive = true
        return container
    }

    // MARK: 动作

    @objc private func refreshQuotas() {
        model.requestQuotaRefresh()
    }
    @objc private func openSettings() { model.route = .settings }
    @objc private func importLive() { model.importCurrentLogin() }
    @objc private func switchAccount(_ sender: NSButton) {
        guard let id = (sender as? AccountActionButton)?.accountID,
              let record = model.accounts.first(where: { $0.id == id }) else { return }
        Task { await model.switchTo(record) }
    }
}

// MARK: - 设置页

final class SettingsScreen: BaseScreen {
    override var contentInsets: NSEdgeInsets {
        NSEdgeInsets(top: 2, left: 20, bottom: 14, right: 20)
    }

    override func rebuild() {
        resetContent()
        contentStack.spacing = 20

        contentStack.addArrangedSubview(group("通用", rows: [
            row(title: "开机自动启动", description: "登录 Mac 时自动运行切号器",
                trailing: launchToggle()),
        ]))

        let manageLink = makeLinkButton("\(model.accounts.count) 个账号", action: #selector(openManage), target: self,
                                        color: Theme.settingValue)
        let manageRow = row(title: "管理账号", description: "添加、查看或移除已登录账号",
                            trailing: makeHStack([manageLink, makeSymbol("chevron.right", size: 10, color: Theme.textFaint)], spacing: 2))
        manageRow.setAccessibilityLabel("管理账号，共 \(model.accounts.count) 个账号")
        contentStack.addArrangedSubview(group("账号", rows: [manageRow]))

        finalize(header: makeNavHeader(backAction: #selector(backToMain), backLabel: "账号列表",
                                       title: "设置", subtitle: "管理你的切换体验",
                                       trailing: makeSymbol("gearshape", size: 15, color: Theme.headerAction)),
                 footer: makeFooter())
    }

    private func group(_ title: String, rows: [NSView]) -> NSView {
        let card = makeCard(background: Theme.cardWhite, stroke: Theme.cardStroke, radius: 10)
        let rowsStack = stackedCardRows(rows)
        card.addSubview(rowsStack)
        NSLayoutConstraint.pin(rowsStack, to: card, insets: NSEdgeInsets())
        rowsStack.widthAnchor.constraint(equalToConstant: Theme.panelWidth - 40).isActive = true
        return makeVStack([makeLabel(title, size: 12, weight: .bold, color: Theme.titleText), card], spacing: 8)
    }

    private func row(title: String, description: String, trailing: NSView) -> NSView {
        let info = makeVStack([makeLabel(title, size: 12, weight: .semibold, color: Theme.textPrimary),
                               makeLabel(description, size: 10, color: Theme.textSecondary)], spacing: 4)
        let line = makeHStack([info, makeSpacer(), trailing], spacing: 12)
        let wrap = FlippedView()
        wrap.translatesAutoresizingMaskIntoConstraints = false
        wrap.addSubview(line)
        NSLayoutConstraint.pin(line, to: wrap, insets: NSEdgeInsets(top: 12, left: 13, bottom: 12, right: 13))
        line.widthAnchor.constraint(equalToConstant: Theme.panelWidth - 40 - 26).isActive = true
        return wrap
    }

    private func launchToggle() -> NSView {
        let toggle = InkSwitch(label: "开机自动启动")
        toggle.isOn = model.settings.launchAtLogin
        toggle.target = self
        toggle.action = #selector(launchToggleChanged(_:))
        return toggle
    }

    private func makeFooter() -> NSView {
        let versionText = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "1.0.0"
        let version = makeLinkButton("版本 \(versionText)", action: #selector(checkUpdates), target: self,
                                     color: Theme.textPrimary)
        version.font = .systemFont(ofSize: 10, weight: .semibold)
        version.toolTip = "检查更新"
        let versionRow: NSView
        if model.hasAvailableUpdate {
            versionRow = makeHStack([version, makeLabel("有更新", size: 10, color: Theme.textSecondary)], spacing: 8)
        } else {
            versionRow = version
        }
        let quit = makeLinkButton("退出应用", action: #selector(quitApp), target: self)
        quit.toolTip = "退出切号器；已添加的账号与设置会保留"
        let stack = makeVStack([versionRow, quit], spacing: 8, alignment: .centerX)
        let container = FlippedView()
        container.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(stack)
        NSLayoutConstraint.pin(stack, to: container, insets: NSEdgeInsets(top: 22, left: 22, bottom: 20, right: 22))
        stack.widthAnchor.constraint(equalToConstant: Theme.panelWidth - 44).isActive = true
        return container
    }

    @objc private func launchToggleChanged(_ sender: InkSwitch) {
        model.applyLaunchAtLogin(sender.isOn)
        sender.isOn = model.settings.launchAtLogin
    }
    @objc private func openManage() { model.route = .manage }
    @objc private func backToMain() { model.route = .main }
    @objc private func quitApp() { NSApp.terminate(nil) }
}

// MARK: - 账号管理页

final class ManageScreen: BaseScreen {
    override var contentInsets: NSEdgeInsets {
        NSEdgeInsets(top: 2, left: 20, bottom: 14, right: 20)
    }

    override func rebuild() {
        resetContent()
        contentStack.spacing = 12

        contentStack.addArrangedSubview(
            makeHStack([makeLabel("已添加账号", size: 12, weight: .bold, color: Theme.titleText),
                        makeSpacer(),
                        makeLabel("邮箱与登录状态", size: 10, color: Theme.textSecondary)], spacing: 8))

        let listCard = makeCard(background: Theme.cardWhite, stroke: Theme.cardStroke, radius: 10)
        let rows = model.accounts.map { makeRow($0) }
        let rowsStack = stackedCardRows(rows)
        listCard.addSubview(rowsStack)
        NSLayoutConstraint.pin(rowsStack, to: listCard, insets: NSEdgeInsets())
        rowsStack.widthAnchor.constraint(equalToConstant: Theme.panelWidth - 40).isActive = true
        contentStack.addArrangedSubview(listCard)

        contentStack.addArrangedSubview(makeAddButton(width: Theme.panelWidth - 40))

        let importFallback = makeLinkButton("导入 Pen 当前登录（备用）", action: #selector(importCurrent), target: self)
        importFallback.identifier = NSUserInterfaceItemIdentifier(ButtonID.importCurrent)
        importFallback.isEnabled = !model.persistenceBlocked
        contentStack.addArrangedSubview(importFallback)

        let hintIcon = makeSymbol("info.circle", size: 13, color: Theme.textSecondary)
        let hintText = makeLabel("移除账号前会再次确认。移除只清理切号器本地的记录与凭据，不会注销远端账号。",
                                 size: 10, color: Theme.textSecondary, wrap: true)
        hintText.preferredMaxLayoutWidth = Theme.panelWidth - 61
        hintText.widthAnchor.constraint(equalToConstant: Theme.panelWidth - 61).isActive = true
        contentStack.addArrangedSubview(makeHStack([hintIcon, hintText], spacing: 7))

        let spacer = NSView()
        spacer.widthAnchor.constraint(equalToConstant: Theme.panelWidth - 40).isActive = true
        contentStack.addArrangedSubview(spacer)

        let name = makeLabel("账号管理", size: 10, color: Theme.textSecondary)
        let back = makeLinkButton("返回设置", action: #selector(backToSettings), target: self)
        let footerStack = makeVStack([name, back], spacing: 8, alignment: .centerX)
        let footerWrap = FlippedView()
        footerWrap.addSubview(footerStack)
        NSLayoutConstraint.pin(footerStack, to: footerWrap, insets: NSEdgeInsets(top: 14, left: 22, bottom: 20, right: 22))
        footerStack.widthAnchor.constraint(equalToConstant: Theme.panelWidth - 44).isActive = true

        finalize(header: makeNavHeader(backAction: #selector(backToSettings), backLabel: "设置",
                                       title: "管理账号", subtitle: "添加或移除已登录的账号",
                                       trailing: makeLabel("\(model.accounts.count) 个账号", size: 11,
                                                           color: Theme.textSecondary)),
                 footer: footerWrap)
    }

    private func makeRow(_ record: AccountRecord) -> NSView {
        let avatar = AvatarView.make(email: record.email, letters: record.avatarLetters, size: 34)
        let email = makeLabel(record.email, size: 12, weight: .semibold, color: Theme.textPrimary)
        var detail: [NSView] = []
        if let plan = record.plan {
            detail.append(makeLabel(plan, size: 10, color: Theme.textSecondary))
        }
        if record.isCurrent {
            detail.append(makeLabel("使用中", size: 10, color: Theme.inUseText))
        }
        let info = detail.isEmpty
            ? makeVStack([email], spacing: 4)
            : makeVStack([email, makeHStack(detail, spacing: 6)], spacing: 4)
        info.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        var trailing: [NSView] = [makeAuthBadge(record.authState)]
        if record.showsReimport { trailing.append(makeReimportButton(for: record)) }
        let remove = makePillButton("移除", symbol: "trash", iconLeading: true,
                                    bg: Theme.removeBtnBG, textColor: Theme.removeBtnText,
                                    action: #selector(confirmRemove(_:)), target: self,
                                    font: .systemFont(ofSize: 10),
                                    insets: NSEdgeInsets(top: 7, left: 8, bottom: 7, right: 8),
                                    identifier: ButtonID.removeAccount, accountID: record.id)
        if let button = remove.subviews.first as? NSButton {
            button.isEnabled = !record.isCurrent && model.switchingTo == nil && !model.addingAccount
                && !model.persistenceBlocked
            button.contentTintColor = record.isCurrent ? Theme.removeBtnIcon : Theme.removeBtnText
            button.toolTip = record.isCurrent ? "当前账号不可直接移除，请先切换到其他账号" : "从切号器移除 \(record.email)"
            button.setAccessibilityLabel(record.isCurrent ? "当前账号不可移除" : "移除 \(record.email)")
        }

        let line = makeHStack([avatar, info, makeSpacer()] + trailing + [remove], spacing: 8)
        let wrap = FlippedView()
        wrap.translatesAutoresizingMaskIntoConstraints = false
        wrap.addSubview(line)
        NSLayoutConstraint.pin(line, to: wrap, insets: NSEdgeInsets(top: 14, left: 13, bottom: 14, right: 13))
        line.widthAnchor.constraint(equalToConstant: Theme.panelWidth - 40 - 26).isActive = true
        return wrap
    }

    @objc private func importCurrent() { model.importCurrentLogin() }

    @objc private func confirmRemove(_ sender: NSButton) {
        guard let id = (sender as? AccountActionButton)?.accountID,
              let record = model.accounts.first(where: { $0.id == id }),
              !record.isCurrent else { return }
        let alert = Self.makeRemoveAlert(email: record.email)
        guard let window = view.window else { return }
        alert.beginSheetModal(for: window) { [weak model = model] response in
            if response == .alertFirstButtonReturn {
                model?.remove(record)
            }
        }
    }

    /// 移除确认框。截图基线也用它，保证文案和按钮与实际一致。
    static func makeRemoveAlert(email: String) -> NSAlert {
        let alert = NSAlert()
        alert.messageText = "从切号器移除此账号？"
        alert.informativeText = "将删除 \(email) 在本机的登录凭据与额度缓存；远端账号不受影响。"
        alert.addButton(withTitle: "移除 \(email)")
        alert.addButton(withTitle: "取消")
        alert.buttons.last?.keyEquivalent = "\u{1b}"
        alert.buttons.first?.hasDestructiveAction = true
        return alert
    }

    @objc private func backToSettings() { model.route = .settings }
}

// MARK: - 面板根控制器：路由

final class PanelController: NSViewController {
    private let model: AppModel
    private var cancellables = Set<AnyCancellable>()
    private var currentScreen: BaseScreen?
    private var currentRoute: PanelRoute?
    private var lastSignature = ""
    private let screenHost = FlippedView()

    init(model: AppModel) {
        self.model = model
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func loadView() {
        view = RoundedBoxView(fill: Theme.panelBG, radius: Theme.panelRadius)
        view.wantsLayer = true
        view.layer?.cornerRadius = Theme.panelRadius
        view.layer?.masksToBounds = true
        view.widthAnchor.constraint(equalToConstant: Theme.panelWidth).isActive = true
        view.appearance = NSAppearance(named: .aqua)
        screenHost.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(screenHost)
        NSLayoutConstraint.pin(screenHost, to: view, insets: NSEdgeInsets())
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        model.objectWillChange
            .receive(on: DispatchQueue.main)
            .sink { [weak self] _ in self?.render() }
            .store(in: &cancellables)
        render()
    }

    /// 截图模式用：不等 Combine 的异步回调，立即按当前状态渲染一次。
    func renderNow() { render() }

    private func render() {
        // 内容签名：只有真正影响界面内容的变更才重建。
        // 设置值变化（如自启开关）不重建——避免拆掉正在跟踪鼠标事件的控件导致浮窗塌缩。
        let signature = model.accounts
            .map { "\($0.id)|\($0.email)|\($0.plan ?? "")|\($0.isCurrent)|\($0.usedDays ?? -1)|\($0.totalDays ?? -1)|\($0.lastQuotaUpdate?.timeIntervalSince1970 ?? 0)|\($0.authState.rawValue)|\($0.lastError ?? "")" }
            .joined(separator: ";")
            + "|route:\(model.route)|sw:\(model.switchingTo?.uuidString ?? "")|live:\(model.liveEmail ?? "")|adding:\(model.addingAccount)|upd:\(model.updateState.signature)"
        Log.debug("render route=\(model.route) current=\(String(describing: currentRoute)) changed=\(signature != lastSignature)")
        if signature != lastSignature {
            lastSignature = signature
            if model.route == currentRoute, let screen = currentScreen {
                screen.rebuild()
            } else {
                let screen: BaseScreen
                switch model.route {
                case .main: screen = MainScreen(model: model)
                case .settings: screen = SettingsScreen(model: model)
                case .manage: screen = ManageScreen(model: model)
                }
                currentScreen?.view.removeFromSuperview()
                currentScreen?.removeFromParent()
                addChild(screen)
                screenHost.addSubview(screen.view)
                NSLayoutConstraint.pin(screen.view, to: screenHost, insets: NSEdgeInsets())
                currentScreen = screen
                currentRoute = model.route
            }
        }
        currentScreen?.updateStatusBanner()
        if let size = currentScreen?.preferredContentSize, preferredContentSize != size {
            preferredContentSize = NSSize(width: Theme.panelWidth, height: size.height)
            if let window = view.window, window.isVisible {
                var frame = window.frame
                let delta = size.height - frame.height
                frame.size = NSSize(width: Theme.panelWidth, height: size.height)
                frame.origin.y -= delta
                if let screen = window.screen?.visibleFrame {
                    frame.origin.y = min(max(frame.origin.y, screen.minY + 8), screen.maxY - frame.height - 8)
                }
                window.setFrame(frame, display: true, animate: false)
                if window.frame.width < Theme.panelWidth - 1 {
                    window.setContentSize(NSSize(width: Theme.panelWidth, height: size.height))
                }
            }
        }
        // 刷新中不拆视图树：只转现有按钮，避免点击过程中把按钮拆掉导致浮窗塌缩
        if let refresh = findRefreshButton(in: currentScreen?.view) {
            refresh.setSpinning(model.refreshing)
            refresh.isEnabled = !model.refreshing
        }
        syncOperationLock()
    }

    /// 刷新不重建页面，否则会拆掉正在转的按钮。这里只改切换和添加按钮的可点状态。
    private func syncOperationLock() {
        let locked = model.refreshing || model.switchingTo != nil || model.updateState.isBusy
        func walk(_ view: NSView) {
            if let button = view as? NSButton {
                switch button.identifier?.rawValue {
                case ButtonID.switchAccount:
                    button.isEnabled = !locked && !model.addingAccount
                case ButtonID.addAccount, ButtonID.reimport:
                    button.isEnabled = !locked && !model.addingAccount && !model.persistenceBlocked
                case ButtonID.cancelLogin:
                    button.isEnabled = !model.refreshing && model.switchingTo == nil
                case ButtonID.removeAccount, ButtonID.importLive, ButtonID.importCurrent:
                    if model.persistenceBlocked { button.isEnabled = false }
                default:
                    break
                }
            }
            view.subviews.forEach(walk)
        }
        if let root = currentScreen?.view { walk(root) }
    }

    private func findRefreshButton(in root: NSView?) -> RefreshButton? {
        guard let root else { return nil }
        if let button = root as? RefreshButton,
           button.identifier?.rawValue == "quota-refresh" {
            return button
        }
        for child in root.subviews {
            if let found = findRefreshButton(in: child) { return found }
        }
        return nil
    }
}
