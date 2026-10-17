// Test target for measure-budgets-macos.sh: a window titled
// "RightPeekTestTarget" with a Persian sentence selected in a text view,
// brought to the front; SIGUSR1 brings it to the front again. Quits after
// the given seconds (default 60).
//
// Usage: swiftc -o target scripts/macos-test-target.swift && ./target [seconds]
import AppKit

let seconds = Double(CommandLine.arguments.dropFirst().first ?? "") ?? 60
let app = NSApplication.shared
app.setActivationPolicy(.regular)

let window = NSWindow(
    contentRect: NSRect(x: 200, y: 200, width: 480, height: 160),
    styleMask: [.titled], backing: .buffered, defer: false)
window.title = "RightPeekTestTarget"
let text = NSTextView(frame: window.contentView!.bounds)
text.string = "این یک متن آزمایشی است که RightPeek باید آن را بخواند."
text.isEditable = false
window.contentView = text
window.makeKeyAndOrderFront(nil)
window.makeFirstResponder(text)
text.selectAll(nil)
app.activate(ignoringOtherApps: true)

signal(SIGUSR1, SIG_IGN)
let refocus = DispatchSource.makeSignalSource(signal: SIGUSR1, queue: .main)
refocus.setEventHandler {
    window.makeKeyAndOrderFront(nil)
    app.activate(ignoringOtherApps: true)
}
refocus.resume()

DispatchQueue.main.asyncAfter(deadline: .now() + seconds) { app.terminate(nil) }
app.run()
