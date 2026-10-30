import AppKit

// 品牌视觉取自 switch_prototype.pen：
// 「应用图标 · 独立版」(veF64) 用于面板标题；「macOS 顶栏图标」用于菜单栏 template。

/// lucide panels-left-bottom（24 网格）：rect(3,3,18,18,r2) + 竖线(9,3)-(9,21) + 横线(9,15)-(21,15)
func panelSymbolPath(in rect: CGRect) -> (outline: NSBezierPath, lines: NSBezierPath) {
    let s = rect.width / 24
    func pt(_ x: CGFloat, _ y: CGFloat) -> NSPoint {
        NSPoint(x: rect.origin.x + x * s, y: rect.origin.y + (24 - y) * s)
    }
    let origin = pt(3, 21)
    let outline = NSBezierPath(roundedRect: NSRect(x: origin.x, y: origin.y,
                                                   width: 18 * s, height: 18 * s),
                               xRadius: 2 * s, yRadius: 2 * s)
    let lines = NSBezierPath()
    lines.move(to: pt(9, 3)); lines.line(to: pt(9, 21))
    lines.move(to: pt(9, 15)); lines.line(to: pt(21, 15))
    lines.lineCapStyle = .round
    lines.lineJoinStyle = .round
    return (outline, lines)
}

private func brandGradient(_ stops: [(UInt32, CGFloat)]) -> NSGradient {
    NSGradient(colors: stops.map { NSColor(hex: $0.0) },
               atLocations: stops.map(\.1),
               colorSpace: .sRGB)!
}

private func brandStroked(_ path: NSBezierPath, width: CGFloat) -> NSBezierPath {
    NSBezierPath(cgPath: path.cgPath.copy(strokingWithWidth: width, lineCap: .round, lineJoin: .round, miterLimit: 10))
}

private func brandInnerShadow(path: NSBezierPath, color: NSColor, offset: NSSize, blur: CGFloat) {
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

private func brandFill(_ path: NSBezierPath, gradient: NSGradient, angle: CGFloat) {
    NSGraphicsContext.current?.saveGraphicsState()
    path.addClip()
    gradient.draw(in: path.bounds, angle: angle)
    NSGraphicsContext.current?.restoreGraphicsState()
}

/// 面板标题栏的品牌图标：应用图标 · 独立版的缩小版。
final class BrandIconView: FlippedView {
    override func draw(_ dirtyRect: NSRect) {
        let size = bounds.width
        let scale = size / 256
        // Pencil 0° 向上；flipped view 里 90° 向下、180° 向左。
        let angle: (CGFloat) -> CGFloat = { 270 - $0 }

        let outer = NSBezierPath(roundedRect: bounds, xRadius: 72 * scale, yRadius: 72 * scale)
        brandGradient([
            (0x55595B, 0), (0xD9DDDE, 0.08), (0xFFFFFF, 0.22), (0xA1A5A6, 0.49),
            (0xF4F5F4, 0.73), (0x787D7E, 0.94), (0xE7E8E7, 1),
        ]).draw(in: outer, angle: angle(90))
        NSColor(hex: 0x858A8B).setStroke()
        outer.lineWidth = max(0.6, 1 * scale)
        outer.stroke()

        let innerRect = bounds.insetBy(dx: 4 * scale, dy: 4 * scale)
        let inner = NSBezierPath(roundedRect: innerRect, xRadius: 68 * scale, yRadius: 68 * scale)
        brandGradient([
            (0xF8F8F7, 0), (0xEEEEED, 0.38), (0xDBDDDC, 0.82), (0xC9CBCA, 1),
        ]).draw(in: inner, angle: angle(180))
        brandInnerShadow(path: inner, color: .white,
                         offset: NSSize(width: 0, height: 2 * scale), blur: 3 * scale)
        brandInnerShadow(path: inner, color: NSColor(hex: 0x6B6E6E, alpha: 0x55 / 255),
                         offset: NSSize(width: 0, height: -2 * scale), blur: 4 * scale)
        let rim = NSBezierPath(roundedRect: innerRect, xRadius: 68 * scale, yRadius: 68 * scale)
        rim.lineWidth = max(0.6, 1 * scale)
        brandFill(brandStroked(rim, width: rim.lineWidth),
                  gradient: brandGradient([(0xFFFFFF, 0), (0xFFFFFF, 0.3), (0xB7C1C3, 1)]),
                  angle: angle(145))

        let symbolSize = 144 * scale
        let symbolRect = NSRect(x: (size - symbolSize) / 2, y: (size - symbolSize) / 2,
                                width: symbolSize, height: symbolSize)
        let s = symbolSize / 24
        func pt(_ x: CGFloat, _ y: CGFloat) -> NSPoint {
            NSPoint(x: symbolRect.origin.x + x * s, y: symbolRect.origin.y + y * s)
        }
        let glyph = NSBezierPath(roundedRect: NSRect(x: pt(3, 3).x, y: pt(3, 3).y,
                                                     width: 18 * s, height: 18 * s),
                                 xRadius: 2 * s, yRadius: 2 * s)
        glyph.move(to: pt(9, 3)); glyph.line(to: pt(9, 21))
        glyph.move(to: pt(9, 15)); glyph.line(to: pt(21, 15))
        glyph.lineCapStyle = .round
        glyph.lineJoinStyle = .round
        let lineWidth = symbolSize / 12
        let symbolGradient = brandGradient([
            (0x1C1F1F, 0), (0x4A4E4F, 0.12), (0xC3C6C5, 0.28), (0xEAECEB, 0.42),
            (0x898D8D, 0.57), (0xD1D4D3, 0.76), (0x424647, 0.94), (0x1A1C1D, 1),
        ])
        if let ctx = NSGraphicsContext.current?.cgContext {
            ctx.saveGState()
            ctx.setShadow(offset: CGSize(width: 1 * scale, height: 3 * scale),
                          blur: 3 * scale,
                          color: NSColor(hex: 0x1C2020, alpha: 0x66 / 255).cgColor)
            ctx.setStrokeColor(NSColor(hex: 0x1C1F1F).cgColor)
            ctx.setLineWidth(lineWidth)
            ctx.setLineCap(.round)
            ctx.setLineJoin(.round)
            ctx.addPath(glyph.cgPath)
            ctx.strokePath()
            ctx.restoreGState()
            ctx.saveGState()
            ctx.addPath(glyph.cgPath)
            ctx.setLineWidth(lineWidth)
            ctx.setLineCap(.round)
            ctx.setLineJoin(.round)
            ctx.replacePathWithStrokedPath()
            ctx.clip(using: .winding)
            symbolGradient.draw(in: symbolRect, angle: angle(90))
            ctx.restoreGState()
        }
    }
}

/// 菜单栏单色 template 图（18pt，系统随深浅菜单栏自动取黑/白；原型「顶栏图标」画板）。
/// 直接固化为 2x 位图：NSCustomImageRep（惰性 drawingHandler）在菜单栏会渲染成空白。
func makeMenuBarIcon() -> NSImage {
    let points: CGFloat = 18
    let pixels = 36
    guard let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels,
                                     bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
                                     isPlanar: false, colorSpaceName: .deviceRGB,
                                     bytesPerRow: 0, bitsPerPixel: 0) else {
        return NSImage()
    }
    rep.size = NSSize(width: points, height: points)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let rect = NSRect(x: 0, y: 0, width: points, height: points)
    let (outline, lines) = panelSymbolPath(in: rect)
    outline.lineWidth = 1.5
    lines.lineWidth = 1.5
    NSColor.black.setStroke()
    outline.stroke()
    lines.stroke()
    NSGraphicsContext.restoreGraphicsState()

    let image = NSImage(size: NSSize(width: points, height: points))
    image.addRepresentation(rep)
    image.isTemplate = true
    return image
}
