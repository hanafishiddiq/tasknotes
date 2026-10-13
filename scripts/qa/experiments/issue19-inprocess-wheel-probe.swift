// Issue19: public, strictly in-process AppKit wheel test seam.
// Sends events directly to this process's synthetic WKWebView responder only.
// Never posts to the OS input stream; no event tap, Safari or external app control.
// Simulated native events are NOT physical hardware evidence, even if isTrusted.
import AppKit
import WebKit
import QuartzCore

final class Probe: NSObject, NSApplicationDelegate, WKNavigationDelegate {
    var window: NSWindow!
    var webView: WKWebView!
    var timer: Timer?
    var count = 0
    var lostFocus = false
    var dispatchMs: [Double] = []
    var inputBeganAt: CFTimeInterval = 0
    var inputEndedAt: CFTimeInterval = 0
    var responderName = ""
    var started = false
    var startButton: NSButton!

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        window = NSWindow(contentRect: NSRect(x: 250, y: 180, width: 950, height: 650),
            styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.title = "Issue19 In-Process Wheel Seam (synthetic)"
        let content = window.contentView!
        webView = WKWebView(frame: NSRect(x: 0, y: 0, width: 950, height: 606))
        webView.navigationDelegate = self
        webView.autoresizingMask = [.width, .height]
        content.addSubview(webView)
        startButton = NSButton(title: "Start native seam", target: self, action: #selector(startTest))
        startButton.frame = NSRect(x: 16, y: 612, width: 180, height: 30)
        startButton.isEnabled = false
        content.addSubview(startButton)
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(webView)
        NSApp.activate(ignoringOtherApps: true)
        let rows = (0..<100).map { "<p style='height:48px;margin:0;padding:12px;background:\($0 % 2 == 0 ? "#ddd" : "#fff")'>Synthetic row \($0)</p>" }.joined()
        webView.loadHTMLString("""
          <!doctype html><html><body style='margin:0;font:16px system-ui'>\(rows)
          <script>
          window.qa={wheel:0,trusted:0,scrollEvents:0,travel:0,focusLost:false,raf:[]};
          let y=0,last=null;
          addEventListener('wheel',e=>{qa.wheel++;if(e.isTrusted)qa.trusted++;},{passive:true});
          addEventListener('scroll',()=>{qa.travel+=Math.abs(scrollY-y);y=scrollY;qa.scrollEvents++;},{passive:true});
          function frame(t){if(last!==null)qa.raf.push(t-last);last=t;requestAnimationFrame(frame)}
          requestAnimationFrame(frame);
          </script></body></html>
          """, baseURL: nil)
        DispatchQueue.main.asyncAfter(deadline: .now()+120) {
            print("INCONCLUSIVE: native seam timeout"); fflush(stdout); exit(2)
        }
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        startButton.isEnabled = true
        print("READY: Start native seam button"); fflush(stdout)
    }

    @objc func startTest() {
        guard !started else { return }; started = true
        startButton.isEnabled = false
        DispatchQueue.main.asyncAfter(deadline: .now()+0.25) { self.begin() }
    }

    func begin() {
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(webView)
        NSApp.activate(ignoringOtherApps: true)
        inputBeganAt = CACurrentMediaTime()
        timer = Timer(timeInterval: 1.0/60.0, repeats: true) { _ in self.tick() }
        RunLoop.main.add(timer!, forMode: .common)
    }

    func tick() {
        guard NSApp.isActive && window.isKeyWindow else {
            print("[FOCUS-LOST] frontmost=\(NSWorkspace.shared.frontmostApplication?.bundleIdentifier ?? "unknown") active=\(NSApp.isActive) key=\(window.isKeyWindow) visible=\(window.occlusionState.contains(.visible)) uptime=\(CACurrentMediaTime())")
            fflush(stdout)
            lostFocus = true; timer?.invalidate(); finish(); return
        }
        guard count < 240 else { timer?.invalidate(); finish(); return }
        let point = NSPoint(x: webView.bounds.midX, y: webView.bounds.midY)
        guard let target = webView.hitTest(point), let cg = CGEvent(
            scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 1,
            wheel1: count < 120 ? -16 : 16, wheel2: 0, wheel3: 0) else {
            print("INCONCLUSIVE: missing own-view responder/event"); exit(2)
        }
        // Assign coordinates for hit testing only; this never moves the OS cursor.
        let screenPoint = window.convertPoint(toScreen: webView.convert(point, to: nil))
        cg.location = CGPoint(x: screenPoint.x, y: (NSScreen.screens.first?.frame.height ?? 0)-screenPoint.y)
        cg.setIntegerValueField(.scrollWheelEventIsContinuous, value: 1)
        guard let event = NSEvent(cgEvent: cg) else { exit(2) }
        responderName = target.className
        let start = CACurrentMediaTime()
        target.scrollWheel(with: event)
        dispatchMs.append((CACurrentMediaTime()-start)*1000)
        count += 1
    }

    func finish() {
        inputEndedAt = CACurrentMediaTime()
        DispatchQueue.main.asyncAfter(deadline: .now()+0.8) {
            self.webView.evaluateJavaScript("JSON.stringify({...qa,documentFocus:document.hasFocus(),visibility:document.visibilityState})") { value, error in
                guard let text = value as? String, let data = text.data(using:.utf8),
                      var report = (try? JSONSerialization.jsonObject(with:data)) as? [String:Any] else {
                    print("INCONCLUSIVE: missing own-page metrics"); exit(2)
                }
                let gaps = (report.removeValue(forKey:"raf") as? [Double] ?? []).sorted()
                report["rAFSamples"] = gaps.count
                report["rAFP95Ms"] = gaps.isEmpty ? -1 : gaps[Int(ceil(Double(gaps.count)*0.95))-1]
                let elapsed = self.inputEndedAt - self.inputBeganAt
                let costs = self.dispatchMs.sorted()
                report["inputElapsedSeconds"] = elapsed
                report["nativeDispatchTotalMs"] = costs.reduce(0,+)
                report["nativeDispatchP95Ms"] = costs.isEmpty ? -1 : costs[Int(ceil(Double(costs.count)*0.95))-1]
                report["inputPacingValid"] = !self.lostFocus && self.count == 240 && elapsed >= 3.5 && elapsed <= 6.0
                report["nativeEventsSent"] = self.count
                report["nativeFocusLost"] = self.lostFocus
                report["ownResponderClass"] = self.responderName
                report["inputMethod"] = "appkit-synthetic-in-process"
                report["physicalHardware"] = false
                let valid = !self.lostFocus && self.count == 240 &&
                    (report["trusted"] as? Int ?? 0) > 100 &&
                    (report["travel"] as? Double ?? 0) >= 600
                report["nativeInputSeamValid"] = valid
                print(String(data:try! JSONSerialization.data(withJSONObject:report,options:[.sortedKeys]),encoding:.utf8)!)
                print(valid ? "PASS: native input seam only; not a scrolling-performance verdict" : "INCONCLUSIVE: native input did not reach expected seam")
                fflush(stdout); exit(valid ? 0 : 2)
            }
        }
    }
}
let app = NSApplication.shared
let delegate = Probe()
app.delegate = delegate
app.run()
