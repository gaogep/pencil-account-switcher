import AppKit

enum Theme {
    static let panelWidth: CGFloat = 410
    static let panelRadius: CGFloat = 16

    // 纸面：冷石墨，不要纯白纯黑
    static let panelBG = NSColor(hex: 0xFAFAFA)
    static let cardWhite = NSColor(hex: 0xFCFCFC)
    static let cardStroke = NSColor(hex: 0xE4E4E7)
    static let currentCardBG = NSColor(hex: 0xEFEFF1)
    static let currentCardStroke = NSColor(hex: 0xE4E4E7)
    static let hairline = NSColor(hex: 0xEBEBED)

    static let titleText = NSColor(hex: 0x1C1C1F)
    static let textPrimary = NSColor(hex: 0x27272A)
    static let textSecondary = NSColor(hex: 0x71717A)
    static let textFaint = NSColor(hex: 0x8B8B93)
    static let settingValue = NSColor(hex: 0x52525B)
    static let linkText = NSColor(hex: 0x27272A)

    static let accent = NSColor(hex: 0x1C1C1F)
    static let knob = NSColor(hex: 0xFAFAFA)
    static let progressFill = accent
    static let checkIcon = accent
    static let tagText = NSColor(hex: 0x3F3F46)
    static let connectedText = NSColor(hex: 0x3F3F46)
    static let switchBtnBG = NSColor(hex: 0x1C1C1F)
    static let switchBtnText = NSColor(hex: 0xFAFAFA)
    static let addBtnStroke = NSColor(hex: 0xD4D4D8)
    static let addBtnHoverFill = NSColor(hex: 0xEFEFF1)
    static let addBtnHoverStroke = NSColor(hex: 0xA1A1AA)
    static let addBtnText = NSColor(hex: 0x1C1C1F)
    static let scrollerKnob = NSColor(hex: 0xA1A1AA)
    static let headerAction = NSColor(hex: 0x52525B)
    static let backBtnBG = NSColor(hex: 0xEFEFF1)
    static let backArrow = NSColor(hex: 0x3F3F46)
    static let removeBtnBG = NSColor(hex: 0xF4F4F5)
    static let removeBtnText = NSColor(hex: 0x52525B)
    static let removeBtnIcon = NSColor(hex: 0xA1A1AA)
    static let inUseText = NSColor(hex: 0x3F3F46)
    static let progressTrackCurrent = NSColor(hex: 0xD4D4D8)
    static let progressTrackOther = NSColor(hex: 0xE4E4E7)
    static let switchOffTrack = NSColor(hex: 0xD4D4D8)
    static let bannerFill = NSColor(hex: 0xEFEFF1)
    static let bannerInk = NSColor(hex: 0x1C1C1F)
    static let bannerMuted = NSColor(hex: 0x71717A)
    static let warnFill = NSColor(hex: 0xF4F4F5)
    static let warnInk = NSColor(hex: 0x3F3F46)
    static let warnTint = NSColor(hex: 0x52525B)
    /// 头像只在石墨里换深浅
    static let avatarPalette: [(bg: NSColor, fg: NSColor)] = [
        (NSColor(hex: 0xE4E4E7), NSColor(hex: 0x3F3F46)),
        (NSColor(hex: 0xD4D4D8), NSColor(hex: 0x27272A)),
        (NSColor(hex: 0xEBEBED), NSColor(hex: 0x52525B)),
    ]

    static func avatarColors(for email: String) -> (bg: NSColor, fg: NSColor) {
        avatarPalette[abs(email.stableHash) % avatarPalette.count]
    }
}

extension String {
    /// 简单稳定哈希（同一邮箱恒定取色，不追求密码学强度）
    var stableHash: Int {
        var hash = 5381
        for byte in utf8 { hash = (hash &* 33 &+ Int(byte)) & 0x7FFFFFFF }
        return hash
    }
}

extension NSColor {
    convenience init(hex: UInt32, alpha: CGFloat = 1) {
        self.init(srgbRed: CGFloat((hex >> 16) & 0xFF) / 255,
                  green: CGFloat((hex >> 8) & 0xFF) / 255,
                  blue: CGFloat(hex & 0xFF) / 255, alpha: alpha)
    }
}

/// flipped 容器（自上而下布局）
class FlippedView: NSView {
    override var isFlipped: Bool { true }
}

/// drawRect 绘制的圆角底（fill + 可选描边）。
/// 不用 layer.backgroundColor：纯 layer 背景在离屏 cacheDisplay 时不渲染。
class RoundedBoxView: FlippedView {
    var fillColor: NSColor? { didSet { needsDisplay = true } }
    var strokeColor: NSColor? { didSet { needsDisplay = true } }
    var boxRadius: CGFloat = 10 { didSet { needsDisplay = true } }

    init(fill: NSColor?, stroke: NSColor? = nil, radius: CGFloat = 10) {
        self.fillColor = fill
        self.strokeColor = stroke
        self.boxRadius = radius
        super.init(frame: .zero)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func draw(_ dirtyRect: NSRect) {
        let path = NSBezierPath(roundedRect: bounds, xRadius: boxRadius, yRadius: boxRadius)
        fillColor?.setFill()
        path.fill()
        if let strokeColor {
            strokeColor.setStroke()
            path.lineWidth = 1
            path.stroke()
        }
    }
}

func makeLabel(_ text: String, size: CGFloat, weight: NSFont.Weight = .regular,
               color: NSColor = Theme.textPrimary, wrap: Bool = false) -> NSTextField {
    let label = wrap ? NSTextField(wrappingLabelWithString: text) : NSTextField(labelWithString: text)
    label.font = .systemFont(ofSize: size, weight: weight)
    label.textColor = color
    label.lineBreakMode = wrap ? .byWordWrapping : .byTruncatingMiddle
    label.cell?.truncatesLastVisibleLine = !wrap
    return label
}

func makeSymbol(_ name: String, size: CGFloat, weight: NSFont.Weight = .regular,
                color: NSColor) -> NSImageView {
    let view = NSImageView(image: NSImage(systemSymbolName: name, accessibilityDescription: nil) ?? NSImage())
    view.symbolConfiguration = NSImage.SymbolConfiguration(pointSize: size, weight: weight)
    view.contentTintColor = color
    return view
}

func makeImageButton(_ symbol: String, size: CGFloat, weight: NSFont.Weight = .regular,
                     color: NSColor, action: Selector,
                     target: Any, tooltip: String) -> NSButton {
    let image = NSImage(systemSymbolName: symbol, accessibilityDescription: tooltip) ?? NSImage()
    image.isTemplate = true
    let button = NSButton(image: image, target: target, action: action)
    button.isBordered = false
    button.contentTintColor = color
    button.toolTip = tooltip
    button.setAccessibilityLabel(tooltip)
    button.focusRingType = .exterior
    return button
}

func makeLinkButton(_ title: String, action: Selector, target: Any,
                    color: NSColor = Theme.linkText) -> NSButton {
    let button = NSButton(title: title, target: target, action: action)
    button.isBordered = false
    button.font = .systemFont(ofSize: 11, weight: .semibold)
    button.contentTintColor = color
    button.focusRingType = .exterior
    return button
}

/// 账号 token 状态：文字标签，不用圆点。
func makeAuthBadge(_ state: AuthState) -> NSView {
    let fill: NSColor
    let ink: NSColor
    switch state {
    case .ok:
        fill = Theme.backBtnBG
        ink = Theme.connectedText
    case .expired:
        fill = Theme.warnFill
        ink = Theme.warnTint
    case .unknown:
        fill = Theme.removeBtnBG
        ink = Theme.textSecondary
    }
    let label = makeLabel(state.label, size: 10, weight: .semibold, color: ink)
    let wrap = RoundedBoxView(fill: fill, radius: 4)
    wrap.addSubview(label)
    NSLayoutConstraint.pin(label, to: wrap, insets: NSEdgeInsets(top: 3, left: 7, bottom: 3, right: 7))
    wrap.setContentHuggingPriority(.required, for: .horizontal)
    wrap.setContentCompressionResistancePriority(.required, for: .horizontal)
    wrap.setAccessibilityLabel(state.label)
    return wrap
}

enum ButtonID {
    static let switchAccount = "op-switch"
    static let addAccount = "op-add"
    static let reimport = "op-reimport"
    static let cancelLogin = "op-cancel-login"
    static let removeAccount = "op-remove"
    static let importLive = "op-import-live"
    static let importCurrent = "op-import-current"
}

/// 切换和移除按钮把账号 id 放在属性里，identifier 留给操作类别。
final class AccountActionButton: NSButton {
    var accountID: UUID?
}

/// 轻量小按钮：浅底圆角 + 文字 + 可选图标（图标尺寸随字号）
func makePillButton(_ title: String, symbol: String? = nil, iconLeading: Bool = false,
                    bg: NSColor, textColor: NSColor,
                    action: Selector, target: Any, font: NSFont = .systemFont(ofSize: 11, weight: .semibold),
                    insets: NSEdgeInsets = NSEdgeInsets(top: 6, left: 8, bottom: 6, right: 8),
                    radius: CGFloat = 6, identifier: String? = nil, accountID: UUID? = nil) -> NSView {
    let button: NSButton
    if accountID != nil {
        let accountButton = AccountActionButton(title: title, target: target, action: action)
        accountButton.accountID = accountID
        button = accountButton
    } else {
        button = NSButton(title: title, target: target, action: action)
    }
    if let identifier {
        button.identifier = NSUserInterfaceItemIdentifier(identifier)
    }
    button.isBordered = false
    button.font = font
    button.contentTintColor = textColor
    if let symbol {
        let config = NSImage.SymbolConfiguration(pointSize: font.pointSize + 1, weight: .medium)
        button.image = NSImage(systemSymbolName: symbol, accessibilityDescription: nil)?
            .withSymbolConfiguration(config)
        button.imagePosition = iconLeading ? .imageLeading : .imageTrailing
        button.imageScaling = .scaleProportionallyDown
    }
    let wrap = RoundedBoxView(fill: bg, radius: radius)
    wrap.addSubview(button)
    NSLayoutConstraint.pin(button, to: wrap, insets: insets)
    button.setAccessibilityLabel(title)
    button.focusRingType = .exterior
    return wrap
}

/// 次要操作：白底细描边小按钮，和墨色主按钮成对出现。
func makeQuietActionButton(_ title: String, symbol: String, action: Selector, target: Any,
                           font: NSFont = .systemFont(ofSize: 10, weight: .semibold)) -> NSView {
    let button = NSButton(title: title, target: target, action: action)
    button.isBordered = false
    button.font = font
    button.contentTintColor = Theme.addBtnText
    button.imageHugsTitle = true
    let config = NSImage.SymbolConfiguration(pointSize: font.pointSize + 1, weight: .medium)
    button.image = NSImage(systemSymbolName: symbol, accessibilityDescription: nil)?
        .withSymbolConfiguration(config)
    button.imagePosition = .imageLeading
    button.imageScaling = .scaleProportionallyDown
    button.setAccessibilityLabel(title)
    button.focusRingType = .exterior
    let wrap = HoverRoundedBox(fill: Theme.cardWhite, stroke: Theme.addBtnStroke, radius: 6,
                               hoverFill: Theme.addBtnHoverFill, hoverStroke: Theme.addBtnHoverStroke)
    wrap.addSubview(button)
    NSLayoutConstraint.pin(button, to: wrap, insets: NSEdgeInsets(top: 6, left: 8, bottom: 6, right: 8))
    wrap.setContentHuggingPriority(.required, for: .horizontal)
    wrap.setContentCompressionResistancePriority(.required, for: .horizontal)
    return wrap
}

/// 白底描边主行动按钮。NSButton 铺满整块区域，因此任意位置都能点。
func makeOutlineActionButton(_ title: String, symbol: String, action: Selector, target: Any,
                             width: CGFloat = Theme.panelWidth - 40) -> NSView {
    let button = FullHitButton(title: title, target: target, action: action)
    button.isBordered = false
    button.bezelStyle = .regularSquare
    button.font = .systemFont(ofSize: 12, weight: .semibold)
    button.contentTintColor = Theme.addBtnText
    button.imageHugsTitle = true
    button.imagePosition = .imageLeading
    button.imageScaling = .scaleProportionallyDown
    button.image = NSImage(systemSymbolName: symbol, accessibilityDescription: nil)?
        .withSymbolConfiguration(NSImage.SymbolConfiguration(pointSize: 14, weight: .semibold))
    button.setAccessibilityLabel(title)
    button.focusRingType = .exterior
    let wrap = HoverRoundedBox(fill: Theme.cardWhite, stroke: Theme.addBtnStroke, radius: 8,
                               hoverFill: Theme.addBtnHoverFill, hoverStroke: Theme.addBtnHoverStroke)
    wrap.addSubview(button)
    NSLayoutConstraint.pin(button, to: wrap, insets: NSEdgeInsets())
    wrap.widthAnchor.constraint(equalToConstant: width).isActive = true
    wrap.heightAnchor.constraint(equalToConstant: 38).isActive = true
    return wrap
}

/// 不走 NSButtonCell 的标题热区，整块 bounds 按下并在内部松开都会触发。
final class FullHitButton: NSButton {
    override func hitTest(_ point: NSPoint) -> NSView? {
        if frame.contains(point) { return self }
        let local = convert(point, from: superview)
        return bounds.contains(local) ? self : nil
    }

    override func mouseDown(with event: NSEvent) {
        guard isEnabled else { return }
        isHighlighted = true
    }

    override func mouseDragged(with event: NSEvent) {
        let local = convert(event.locationInWindow, from: nil)
        isHighlighted = bounds.contains(local)
    }

    override func mouseUp(with event: NSEvent) {
        isHighlighted = false
        guard isEnabled else { return }
        let local = convert(event.locationInWindow, from: nil)
        if bounds.contains(local) {
            _ = sendAction(action, to: target)
        }
    }
}

/// 悬停时换成更浅的石墨底，不改按钮的点击逻辑。
final class HoverRoundedBox: RoundedBoxView {
    private let restFill: NSColor?
    private let restStroke: NSColor?
    private let hoverFill: NSColor
    private let hoverStroke: NSColor

    init(fill: NSColor?, stroke: NSColor?, radius: CGFloat, hoverFill: NSColor, hoverStroke: NSColor) {
        restFill = fill
        restStroke = stroke
        self.hoverFill = hoverFill
        self.hoverStroke = hoverStroke
        super.init(fill: fill, stroke: stroke, radius: radius)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        trackingAreas.filter { $0.owner as AnyObject? === self }.forEach(removeTrackingArea)
        addTrackingArea(NSTrackingArea(rect: bounds,
                                       options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect],
                                       owner: self, userInfo: nil))
    }

    override func mouseEntered(with event: NSEvent) {
        fillColor = hoverFill
        strokeColor = hoverStroke
    }

    override func mouseExited(with event: NSEvent) {
        fillColor = restFill
        strokeColor = restStroke
    }
}

/// 墨色开关，替代系统 NSSwitch 的蓝色。
final class InkSwitch: NSControl {
    var isOn = false {
        didSet {
            needsDisplay = true
            setAccessibilityValue((isOn ? 1 : 0) as NSNumber)
        }
    }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setAccessibilityRole(.checkBox)
        focusRingType = .exterior
        translatesAutoresizingMaskIntoConstraints = false
        widthAnchor.constraint(equalToConstant: 36).isActive = true
        heightAnchor.constraint(equalToConstant: 22).isActive = true
    }

    convenience init(label: String) {
        self.init(frame: .zero)
        setAccessibilityLabel(label)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override var intrinsicContentSize: NSSize { NSSize(width: 36, height: 22) }
    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }

    override func draw(_ dirtyRect: NSRect) {
        let track = NSBezierPath(roundedRect: bounds, xRadius: bounds.height / 2, yRadius: bounds.height / 2)
        (isOn ? Theme.accent : Theme.switchOffTrack).setFill()
        track.fill()
        let d = bounds.height - 4
        let x = isOn ? bounds.width - d - 2 : 2
        Theme.knob.setFill()
        NSBezierPath(ovalIn: NSRect(x: x, y: 2, width: d, height: d)).fill()
    }

    override func mouseDown(with event: NSEvent) {
        guard isEnabled else { return }
        isOn.toggle()
        sendAction(action, to: target)
    }

    override func keyDown(with event: NSEvent) {
        guard isEnabled else { super.keyDown(with: event); return }
        if event.keyCode == 49 {
            isOn.toggle()
            sendAction(action, to: target)
        } else {
            super.keyDown(with: event)
        }
    }
}

/// 滚动时才出现的细石墨条，无轨道，避开面板圆角。
final class PanelScroller: NSScroller {
    override class var isCompatibleWithOverlayScrollers: Bool { true }

    override class func scrollerWidth(for controlSize: NSControl.ControlSize,
                                      scrollerStyle: NSScroller.Style) -> CGFloat { 8 }

    override func draw(_ dirtyRect: NSRect) {
        drawKnob()
    }

    override func drawKnobSlot(in slotRect: NSRect, highlight flag: Bool) {}

    override func drawKnob() {
        var knob = rect(for: .knob).insetBy(dx: 2.5, dy: 4)
        knob.size.width = 3
        guard knob.height > 8 else { return }
        Theme.scrollerKnob.withAlphaComponent(0.38).setFill()
        NSBezierPath(roundedRect: knob, xRadius: 1.5, yRadius: 1.5).fill()
    }
}

/// 只旋转独立图标层，不改变 AppKit 管理的按钮层和点击区域。
final class RefreshButton: NSButton {
    private let iconLayer = CALayer()
    private let tintedIcon: NSImage

    init(action: Selector, target: AnyObject) {
        let icon = NSImage(systemSymbolName: "arrow.clockwise", accessibilityDescription: "刷新额度") ?? NSImage()
        tintedIcon = NSImage(size: icon.size, flipped: false) { rect in
            icon.draw(in: rect)
            Theme.headerAction.setFill()
            rect.fill(using: .sourceIn)
            return true
        }
        super.init(frame: .zero)
        image = icon
        imagePosition = .imageOnly
        isBordered = false
        self.target = target
        self.action = action
        toolTip = "刷新额度"
        setAccessibilityLabel("刷新额度")
        focusRingType = .exterior
        wantsLayer = true
        let scale = NSScreen.main?.backingScaleFactor ?? 2
        iconLayer.contentsScale = scale
        iconLayer.contents = tintedIcon.layerContents(forContentsScale: scale)
        iconLayer.contentsGravity = .resizeAspect
        iconLayer.anchorPoint = CGPoint(x: 0.5, y: 0.5)
        layer?.addSublayer(iconLayer)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    // 图标由独立图层绘制；保留 image 供按钮计算原有布局尺寸。
    override func draw(_ dirtyRect: NSRect) {}

    override func layout() {
        super.layout()
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2
        if iconLayer.contentsScale != scale {
            iconLayer.contentsScale = scale
            iconLayer.contents = tintedIcon.layerContents(forContentsScale: scale)
        }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        iconLayer.bounds = CGRect(origin: .zero, size: bounds.size)
        iconLayer.position = CGPoint(x: bounds.midX, y: bounds.midY)
        CATransaction.commit()
    }

    func setSpinning(_ active: Bool) {
        if NSWorkspace.shared.accessibilityDisplayShouldReduceMotion {
            iconLayer.removeAnimation(forKey: "spin")
            return
        }
        if active {
            guard iconLayer.animation(forKey: "spin") == nil else { return }
            let anim = CABasicAnimation(keyPath: "transform.rotation.z")
            anim.fromValue = 0
            anim.toValue = Double.pi * 2
            anim.duration = 0.7
            anim.timingFunction = CAMediaTimingFunction(name: .linear)
            anim.repeatCount = .infinity
            iconLayer.add(anim, forKey: "spin")
        } else {
            iconLayer.removeAnimation(forKey: "spin")
        }
    }
}

/// 卡片圆角容器（可选描边）
func makeCard(background: NSColor, stroke: NSColor? = nil, radius: CGFloat = 10) -> NSView {
    RoundedBoxView(fill: background, stroke: stroke, radius: radius)
}

/// 设置卡、账号列表用的 1px 分隔线。
func makeHairline(inset: CGFloat = 13) -> NSView {
    let line = RoundedBoxView(fill: Theme.hairline, radius: 0)
    line.heightAnchor.constraint(equalToConstant: 1).isActive = true
    let wrap = FlippedView()
    wrap.translatesAutoresizingMaskIntoConstraints = false
    wrap.addSubview(line)
    NSLayoutConstraint.activate([
        line.leadingAnchor.constraint(equalTo: wrap.leadingAnchor, constant: inset),
        line.trailingAnchor.constraint(equalTo: wrap.trailingAnchor),
        line.topAnchor.constraint(equalTo: wrap.topAnchor),
        line.bottomAnchor.constraint(equalTo: wrap.bottomAnchor),
        wrap.heightAnchor.constraint(equalToConstant: 1),
    ])
    return wrap
}

func stackedCardRows(_ rows: [NSView]) -> NSStackView {
    var views: [NSView] = []
    for (index, row) in rows.enumerated() {
        if index > 0 { views.append(makeHairline()) }
        views.append(row)
    }
    return makeVStack(views, spacing: 0)
}

/// 头像：drawRect 绘制浅色圆角底 + 深色字母（按账号配色）
final class AvatarView: NSView {
    private let letters: String
    private let bg: NSColor
    private let fg: NSColor

    init(letters: String, size: CGFloat, bg: NSColor, fg: NSColor) {
        self.letters = letters
        self.bg = bg
        self.fg = fg
        super.init(frame: NSRect(x: 0, y: 0, width: size, height: size))
        widthAnchor.constraint(equalToConstant: size).isActive = true
        heightAnchor.constraint(equalToConstant: size).isActive = true
        setAccessibilityHidden(true)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func draw(_ dirtyRect: NSRect) {
        let radius = bounds.width * 0.27
        bg.setFill()
        NSBezierPath(roundedRect: bounds, xRadius: radius, yRadius: radius).fill()

        let font = NSFont.systemFont(ofSize: bounds.width == 36 ? 12 : 11, weight: .bold)
        let attrs: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: fg]
        let size = letters.size(withAttributes: attrs)
        letters.draw(at: NSPoint(x: (bounds.width - size.width) / 2,
                                 y: (bounds.height - size.height) / 2),
                     withAttributes: attrs)
    }

    static func make(email: String, letters: String, size: CGFloat) -> AvatarView {
        let colors = Theme.avatarColors(for: email)
        return AvatarView(letters: letters, size: size, bg: colors.bg, fg: colors.fg)
    }
}

/// 额度进度条（drawRect：当前卡片 h5/r4，其他账号 h4/r3）
final class QuotaProgressBar: NSView {
    private var fraction: Double?
    private let track: NSColor
    private let fillColor: NSColor

    init(width: CGFloat, height: CGFloat, track: NSColor, fillColor: NSColor) {
        self.track = track
        self.fillColor = fillColor
        super.init(frame: NSRect(x: 0, y: 0, width: width, height: height))
        widthAnchor.constraint(equalToConstant: width).isActive = true
        heightAnchor.constraint(equalToConstant: height).isActive = true
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    func update(used: Int?, total: Int?) {
        if let used, let total, total > 0 {
            fraction = min(1, max(0, Double(used) / Double(total)))
        } else {
            fraction = nil
        }
        needsDisplay = true
    }

    func update(fraction value: Double) {
        fraction = min(1, max(0, value))
        needsDisplay = true
    }

    override func draw(_ dirtyRect: NSRect) {
        let radius = bounds.height / 2
        track.setFill()
        NSBezierPath(roundedRect: bounds, xRadius: radius, yRadius: radius).fill()
        if let fraction {
            fillColor.setFill()
            let fillRect = NSRect(x: 0, y: 0,
                                  width: bounds.width * CGFloat(min(1, max(0, fraction))),
                                  height: bounds.height)
            NSBezierPath(roundedRect: fillRect, xRadius: radius, yRadius: radius).fill()
        }
    }
}

func quotaText(_ used: Int?, _ total: Int?) -> String {
    guard let used, let total else { return "— / — 天" }
    return "\(used) / \(total) 天"
}

func makeVStack(_ views: [NSView], spacing: CGFloat,
                alignment: NSLayoutConstraint.Attribute = .leading) -> NSStackView {
    let stack = NSStackView(views: views)
    stack.orientation = .vertical
    stack.spacing = spacing
    stack.alignment = alignment
    stack.translatesAutoresizingMaskIntoConstraints = false
    return stack
}

func makeHStack(_ views: [NSView], spacing: CGFloat) -> NSStackView {
    let stack = NSStackView(views: views)
    stack.orientation = .horizontal
    stack.spacing = spacing
    stack.alignment = .centerY
    stack.distribution = .fill
    stack.translatesAutoresizingMaskIntoConstraints = false
    return stack
}

/// 可拉伸的水平占位（低拥抱优先级，让两侧内容贴边）
func makeSpacer() -> NSView {
    let view = NSView()
    view.translatesAutoresizingMaskIntoConstraints = false
    view.setContentHuggingPriority(NSLayoutConstraint.Priority(249), for: .horizontal)
    view.setContentCompressionResistancePriority(.required, for: .horizontal)
    return view
}

extension NSLayoutConstraint {
    @discardableResult
    static func pin(_ child: NSView, to parent: NSView, insets: NSEdgeInsets,
                    activate: Bool = true) -> [NSLayoutConstraint] {
        child.translatesAutoresizingMaskIntoConstraints = false
        let constraints = [
            child.topAnchor.constraint(equalTo: parent.topAnchor, constant: insets.top),
            child.leadingAnchor.constraint(equalTo: parent.leadingAnchor, constant: insets.left),
            child.trailingAnchor.constraint(equalTo: parent.trailingAnchor, constant: -insets.right),
            child.bottomAnchor.constraint(equalTo: parent.bottomAnchor, constant: -insets.bottom),
        ]
        if activate { NSLayoutConstraint.activate(constraints) }
        return constraints
    }
}
