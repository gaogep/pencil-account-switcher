import AppKit

// 按 switch_prototype.pen「应用图标 · 独立版」(veF64, 256) 渲染 AppIcon。
// 外层 r72 金属环 + 内层倒角表面 + lucide panels-left-bottom 金属斜面符号。
// 不画原型里的外投影：macOS 会给应用图标加系统阴影。
// 用法: generate_icon <输出iconset目录>

extension NSColor {
    convenience init(hex: UInt32, alpha: CGFloat = 1) {
        self.init(srgbRed: CGFloat((hex >> 16) & 0xFF) / 255,
                  green: CGFloat((hex >> 8) & 0xFF) / 255,
                  blue: CGFloat(hex & 0xFF) / 255, alpha: alpha)
    }
}

func metalGradient(_ stops: [(UInt32, CGFloat)]) -> NSGradient {
    NSGradient(colors: stops.map { NSColor(hex: $0.0) },
               atLocations: stops.map(\.1),
               colorSpace: .sRGB)!
}

/// Pencil 旋转（0° 向上、逆时针）转到 AppKit 未翻转坐标系（0° 向右）。
func appKitAngle(_ pencilDegrees: CGFloat) -> CGFloat { pencilDegrees + 90 }

/// lucide panels-left-bottom（24 网格）：rect(3,3,18,18,r2) + 竖线(9,3)-(9,21) + 横线(9,15)-(21,15)
func panelSymbolPath(in rect: CGRect) -> NSBezierPath {
    let s = rect.width / 24
    func pt(_ x: CGFloat, _ y: CGFloat) -> NSPoint {
        NSPoint(x: rect.origin.x + x * s, y: rect.origin.y + (24 - y) * s)
    }
    let origin = pt(3, 21)
    let path = NSBezierPath(roundedRect: NSRect(x: origin.x, y: origin.y,
                                                width: 18 * s, height: 18 * s),
                            xRadius: 2 * s, yRadius: 2 * s)
    path.move(to: pt(9, 3)); path.line(to: pt(9, 21))
    path.move(to: pt(9, 15)); path.line(to: pt(21, 15))
    path.lineCapStyle = .round
    path.lineJoinStyle = .round
    return path
}

func stroked(from path: NSBezierPath, width: CGFloat) -> NSBezierPath {
    let cg = path.cgPath.copy(strokingWithWidth: width, lineCap: .round, lineJoin: .round, miterLimit: 10)
    return NSBezierPath(cgPath: cg)
}

func paintInnerShadow(path: NSBezierPath, color: NSColor, offset: NSSize, blur: CGFloat) {
    guard let ctx = NSGraphicsContext.current?.cgContext else { return }
    ctx.saveGState()
    path.addClip()
    ctx.setShadow(offset: CGSize(width: offset.width, height: offset.height),
                  blur: blur, color: color.cgColor)
    ctx.setFillColor(NSColor.black.cgColor)
    ctx.addRect(CGRect(x: -4096, y: -4096, width: 8192, height: 8192))
    ctx.addPath(path.cgPath)
    ctx.fillPath(using: .evenOdd)
    ctx.restoreGState()
}

func fill(_ path: NSBezierPath, with gradient: NSGradient, angle: CGFloat) {
    NSGraphicsContext.current?.saveGraphicsState()
    path.windingRule = .nonZero
    path.addClip()
    gradient.draw(in: path.bounds, angle: angle)
    NSGraphicsContext.current?.restoreGraphicsState()
}

func stroke(_ path: NSBezierPath, width: CGFloat, with gradient: NSGradient, angle: CGFloat, in bounds: CGRect) {
    guard let ctx = NSGraphicsContext.current?.cgContext else { return }
    ctx.saveGState()
    ctx.addPath(path.cgPath)
    ctx.setLineWidth(width)
    ctx.setLineCap(.round)
    ctx.setLineJoin(.round)
    ctx.replacePathWithStrokedPath()
    ctx.clip(using: .winding)
    gradient.draw(in: bounds, angle: angle)
    ctx.restoreGState()
}

func drawAppIcon(in rect: CGRect) {
    let size = rect.width
    let scale = size / 256

    let outer = NSBezierPath(roundedRect: rect, xRadius: 72 * scale, yRadius: 72 * scale)
    metalGradient([
        (0x55595B, 0), (0xD9DDDE, 0.08), (0xFFFFFF, 0.22), (0xA1A5A6, 0.49),
        (0xF4F5F4, 0.73), (0x787D7E, 0.94), (0xE7E8E7, 1),
    ]).draw(in: outer, angle: appKitAngle(90))
    NSColor(hex: 0x858A8B).setStroke()
    outer.lineWidth = max(0.6, 1 * scale)
    outer.stroke()

    let pad = 4 * scale
    let innerRect = rect.insetBy(dx: pad, dy: pad)
    let inner = NSBezierPath(roundedRect: innerRect, xRadius: 68 * scale, yRadius: 68 * scale)
    metalGradient([
        (0xF8F8F7, 0), (0xEEEEED, 0.38), (0xDBDDDC, 0.82), (0xC9CBCA, 1),
    ]).draw(in: inner, angle: appKitAngle(180))
    // Pencil y 向下；未翻转 AppKit 里取反。
    paintInnerShadow(path: inner, color: NSColor.white,
                     offset: NSSize(width: 0, height: -2 * scale), blur: 3 * scale)
    paintInnerShadow(path: inner, color: NSColor(hex: 0x6B6E6E, alpha: 0x55 / 255),
                     offset: NSSize(width: 0, height: 2 * scale), blur: 4 * scale)

    let rim = NSBezierPath(roundedRect: innerRect, xRadius: 68 * scale, yRadius: 68 * scale)
    rim.lineWidth = max(0.6, 1 * scale)
    fill(stroked(from: rim, width: rim.lineWidth),
         with: metalGradient([(0xFFFFFF, 0), (0xFFFFFF, 0.3), (0xB7C1C3, 1)]),
         angle: appKitAngle(145))

    let symbolSize = 144 * scale
    let symbolRect = NSRect(x: (size - symbolSize) / 2, y: (size - symbolSize) / 2,
                            width: symbolSize, height: symbolSize)
    let glyph = panelSymbolPath(in: symbolRect)
    let lineWidth = symbolSize / 12
    let symbolGradient = metalGradient([
        (0x1C1F1F, 0), (0x4A4E4F, 0.12), (0xC3C6C5, 0.28), (0xEAECEB, 0.42),
        (0x898D8D, 0.57), (0xD1D4D3, 0.76), (0x424647, 0.94), (0x1A1C1D, 1),
    ])
    if let ctx = NSGraphicsContext.current?.cgContext {
        ctx.saveGState()
        ctx.setShadow(offset: CGSize(width: 1 * scale, height: -3 * scale),
                      blur: 3 * scale,
                      color: NSColor(hex: 0x1C2020, alpha: 0x66 / 255).cgColor)
        ctx.setStrokeColor(NSColor(hex: 0x1C1F1F).cgColor)
        ctx.setLineWidth(lineWidth)
        ctx.setLineCap(.round)
        ctx.setLineJoin(.round)
        ctx.addPath(glyph.cgPath)
        ctx.strokePath()
        ctx.restoreGState()
    }
    stroke(glyph, width: lineWidth, with: symbolGradient, angle: appKitAngle(90), in: symbolRect)
}

func renderPNG(size: CGFloat) -> Data {
    let image = NSImage(size: NSSize(width: size, height: size), flipped: false) { rect in
        drawAppIcon(in: rect)
        return true
    }
    guard let tiff = image.tiffRepresentation,
          let rep = NSBitmapImageRep(data: tiff),
          let png = rep.representation(using: .png, properties: [:]) else {
        fatalError("PNG 渲染失败")
    }
    return png
}

let outDir = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "AppIcon.iconset"
try? FileManager.default.createDirectory(atPath: outDir, withIntermediateDirectories: true)

let sizes: [(String, CGFloat)] = [
    ("icon_16x16.png", 16), ("icon_16x16@2x.png", 32),
    ("icon_32x32.png", 32), ("icon_32x32@2x.png", 64),
    ("icon_128x128.png", 128), ("icon_128x128@2x.png", 256),
    ("icon_256x256.png", 256), ("icon_256x256@2x.png", 512),
    ("icon_512x512.png", 512), ("icon_512x512@2x.png", 1024),
]
for (name, px) in sizes {
    try! renderPNG(size: px).write(to: URL(fileURLWithPath: "\(outDir)/\(name)"))
    print("wrote \(outDir)/\(name) (\(px)px)")
}
