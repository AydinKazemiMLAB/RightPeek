// Draws the macOS menu-bar icon: a template image (black + alpha only;
// macOS tints it for light/dark menu bars). A page with text lines and a
// head peeking out from behind it, like the app icon.
//
// Writes raw RGBA (36x36 px = 18 pt at 2x), which the app embeds without an
// image decoder: swift scripts/macos-tray-icon.swift src-tauri/icons/tray-macos.rgba
import CoreGraphics
import Foundation

let size = 36
let output = CommandLine.arguments.count > 1
    ? CommandLine.arguments[1] : "src-tauri/icons/tray-macos.rgba"

let context = CGContext(
    data: nil, width: size, height: size, bitsPerComponent: 8, bytesPerRow: size * 4,
    space: CGColorSpaceCreateDeviceRGB(),
    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
context.setFillColor(CGColor(red: 0, green: 0, blue: 0, alpha: 1))

// Head behind the page, then a gap around the page so the two read apart.
context.fillEllipse(in: CGRect(x: 19.5, y: 16, width: 15, height: 15))
let page = CGRect(x: 3, y: 2, width: 21, height: 32)
context.setBlendMode(.clear)
context.addPath(CGPath(roundedRect: page.insetBy(dx: -2, dy: -2),
                       cornerWidth: 5, cornerHeight: 5, transform: nil))
context.fillPath()
context.setBlendMode(.normal)
context.addPath(CGPath(roundedRect: page, cornerWidth: 3.5, cornerHeight: 3.5, transform: nil))
context.fillPath()

// Text lines cut out of the page.
context.setBlendMode(.clear)
for (y, width) in [(24.0, 13.0), (18.5, 13.0), (13.0, 9.0)] {
    context.addPath(CGPath(roundedRect: CGRect(x: 7, y: y, width: width, height: 3),
                           cornerWidth: 1.5, cornerHeight: 1.5, transform: nil))
    context.fillPath()
}

let bytes = UnsafeBufferPointer(
    start: context.data!.assumingMemoryBound(to: UInt8.self), count: size * size * 4)
try! Data(buffer: bytes).write(to: URL(fileURLWithPath: output))
