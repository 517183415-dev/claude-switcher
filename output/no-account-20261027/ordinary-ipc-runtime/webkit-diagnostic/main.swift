import AppKit
import WebKit
final class Probe: NSObject, NSApplicationDelegate, WKNavigationDelegate {
 var window: NSWindow!
 var view: WKWebView!
 func applicationDidFinishLaunching(_ notification: Notification) {
  window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
  view = WKWebView(frame: window.contentView!.bounds)
  view.navigationDelegate = self
  window.contentView = view
  window.makeKeyAndOrderFront(nil)
  view.load(URLRequest(url: URL(string: CommandLine.arguments[1])!))
  DispatchQueue.main.asyncAfter(deadline: .now() + 12) { print("DIAGNOSTIC_TIMEOUT"); exit(2) }
 }
 func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) { print("DIAGNOSTIC_LOADED"); exit(0) }
 func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) { print("DIAGNOSTIC_ERROR \(error as NSError)"); exit(1) }
 func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) { print("DIAGNOSTIC_ERROR \(error as NSError)"); exit(1) }
}
let app = NSApplication.shared
let probe = Probe()
app.delegate = probe
app.run()
