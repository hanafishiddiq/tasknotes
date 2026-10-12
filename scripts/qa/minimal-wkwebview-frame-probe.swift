// Independent Apple WKWebView/AppKit frame-timing control for Issue #19.
//
// NO Tauri, React, SQLite, WebDriver, frame capture, or private WebKit APIs.
// On-screen display activation is performed through the public NSApp API.
// Never run against user Task data; source only creates synthetic HTML.
// Build: swiftc -O -framework AppKit -framework WebKit -framework QuartzCore
//        scripts/qa/minimal-wkwebview-frame-probe.swift
//        -o src-tauri/target/tasknotes_minimal_wkwebview_probe
// Runs ~12-15 seconds, prints only numeric timings and terminates.
//
// Optional test-only public ProcessInfo activity A/B (not a product feature):
// TASKNOTES_QA_ACTIVE_POWER=1 src-tauri/target/tasknotes_minimal_wkwebview_probe

import AppKit
import WebKit
import QuartzCore

final class Probe: NSObject, NSApplicationDelegate, WKNavigationDelegate, WKScriptMessageHandler {
    var window: NSWindow!
    var webView: WKWebView!
    var begun = false
    var finished = false
    var appNapToken: NSObjectProtocol?
    var displayLink: CADisplayLink?
    var displayTimestamps: [CFTimeInterval] = []

    @objc func sampleDisplay(_ sender: CADisplayLink) {
        displayTimestamps.append(CACurrentMediaTime())
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        window = NSWindow(
            contentRect: NSRect(x: 160, y: 130, width: 950, height: 650),
            styleMask: [.titled, .closable, .resizable, .miniaturizable],
            backing: .buffered, defer: false)
        window.title = "WKWebView Scroll QA (synthetic, not Tasknotes)"
        let config = WKWebViewConfiguration()
        config.userContentController.add(self, name: "probe")
        webView = WKWebView(frame: window.contentView!.bounds, configuration: config)
        webView.navigationDelegate = self
        webView.autoresizingMask = [.width, .height]
        window.contentView?.addSubview(webView)
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        if ProcessInfo.processInfo.environment["TASKNOTES_QA_ACTIVE_POWER"] == "1" {
            appNapToken = ProcessInfo.processInfo.beginActivity(
                options: [.userInitiated],
                reason: "Synthetic WKWebView diagnosis only")
            print("[WK-POWER-ASSERTION] userInitiated=true, testOnly=true")
        }
        if let display = window.screen {
            displayLink = display.displayLink(target: self, selector: #selector(sampleDisplay(_:)))
            displayLink?.add(to: .main, forMode: .common)
        }
        // Hard wall-clock timeout: test must never hang due to WebKit timer throttling.
        DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(30)) { [weak self] in
            guard let self, !self.finished else { return }
            print("[WK-EMERGENCY-TIMEOUT] 30s; incomplete animation benchmark")
            fflush(stdout)
            NSApp.terminate(nil)
        }
        print("[WK-MINIMAL-READY] appActive=\(NSApp.isActive), windowKey=\(window.isKeyWindow), windowVisible=\(window.isVisible)", terminator: "\n")
        fflush(stdout)
        var html = "<!doctype html><html><head><meta name='viewport' content='width=device-width,initial-scale=1'></head><body style='font-family:system-ui;margin:0'><div style='position:sticky;top:0;background:#fff;padding:12px'>Independent Apple WKWebView benchmark</div>"
        for i in 0..<100 {
            let shade = i % 2 == 0 ? "#c7e8ff" : "#ffdbbb"
            html += "<p style='height:50px;margin:0;padding:10px;background:\(shade)'>Synthetic row \(i)</p>"
        }
        if ProcessInfo.processInfo.environment["TASKNOTES_QA_VISIBLE_CSS_ANIMATION"] == "1" {
            html += "<style>@keyframes qaPulse{0%{transform:translateX(0px)}100%{transform:translateX(12px)}} .wkqa-heartbeat{position:fixed;top:4px;right:12px;width:14px;height:14px;background:#fd45ac;animation:qaPulse .27s linear infinite alternate;z-index:100}</style><div class='wkqa-heartbeat' aria-hidden='true'></div>"
            print("[WK-CSS-HEARTBEAT] testOnly=true")
        }
        html += "</body></html>"
        webView.loadHTMLString(html, baseURL: nil)
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        guard !begun else { return }
        begun = true
        print("[WK-MINIMAL-LOADED] appActive=\(NSApp.isActive), windowKey=\(window.isKeyWindow), windowVisible=\(window.isVisible)")
        fflush(stdout)
        runJavaScriptBenchmark()
    }

    func runJavaScriptBenchmark() {
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        webView.evaluateJavaScript("""
        (async () => {
          const p95 = values => values.length
            ? [...values].sort((a,b)=>a-b)[Math.ceil(values.length*.95)-1] : null;
          async function runTrial(name, scroll) {
            window.webkit.messageHandlers.probe.postMessage("[WK-PHASE-BEGIN]"+name);
            await new Promise(resolve=>setTimeout(resolve,95));
            const focusAtStart=document.hasFocus() && document.visibilityState==='visible';
            let lostFocus=false;
            let last = null;
            let frames = [], scrollEvents = 0;
            const onScroll = () => { scrollEvents++; };
            window.addEventListener('scroll', onScroll, {passive:true});
            const start = performance.now();
            for (let i=0;i<40;i++) {
              if (!document.hasFocus() || document.visibilityState!=='visible') {
                lostFocus=true;
                break;
              }
              const now = await Promise.race([
                new Promise(resolve => requestAnimationFrame(resolve)),
                new Promise(resolve => setTimeout(()=>resolve(null), 650))
              ]);
              if (now === null) break;
              if (last!==null) frames.push(now-last);
              last = now;
              if (scroll) window.scrollBy(0, i<20?32:-32);
            }
            window.removeEventListener('scroll', onScroll);
            window.webkit.messageHandlers.probe.postMessage("[WK-PHASE-END]"+name);
            return { name, focusAtStart, lostFocus,
              sampleCount:frames.length, p95Ms:p95(frames),
              medianMs:frames.length?p95(frames.slice().sort((a,b)=>a-b).slice(0,Math.ceil(frames.length/2))):null,
              scrollEvents, durationMs:Math.round(performance.now()-start),
              visibility:document.visibilityState, documentHasFocus:document.hasFocus(),
              scrollY:window.scrollY };
          }
          // Fixed-frequency JS timer drives scroll, independent of rAF.
          // If timer callbacks are fast but captured content remains 10Hz,
          // the WebKit presentation path is the focus of diagnosis.
          async function timerScrollTrial(name) {
            window.webkit.messageHandlers.probe.postMessage("[WK-PHASE-BEGIN]"+name);
            let timerTimes=[], rafTimes=[], scrollEvents=0, maximumY=0;
            let lostFocus=false;
            const onScroll=()=>{scrollEvents++; maximumY=Math.max(maximumY,window.scrollY);};
            window.addEventListener('scroll',onScroll,{passive:true});
            let running=true, previous=null;
            const onFrame=now=>{
              if(!running)return;
              if(previous!==null)rafTimes.push(now-previous);
              previous=now;
              requestAnimationFrame(onFrame);
            };
            requestAnimationFrame(onFrame);
            const started=performance.now();
            await new Promise(resolve=>{
              let i=0;
              const timer=setInterval(()=>{
                timerTimes.push(performance.now());
                if((document.visibilityState!=='visible'||!document.hasFocus()) &&
                   performance.now()-started>1000){
                  lostFocus=true;
                  clearInterval(timer);
                  resolve();
                  return;
                }
                window.scrollBy(0,i<5 ? 16 : -16);
                i++;
                if(i%2===0) {
                  window.webkit.messageHandlers.probe.postMessage(
                    "[WK-TIMER-PROGRESS]"+name+" tick="+i+
                    " elapsedMs="+Math.round(performance.now()-started)+
                    " visibility="+document.visibilityState+
                    " documentFocus="+document.hasFocus());
                }
                if(i>=10){
                  clearInterval(timer);
                  setTimeout(resolve,120);
                }
              },16);
            });
            running=false;
            window.removeEventListener('scroll',onScroll);
            window.webkit.messageHandlers.probe.postMessage("[WK-PHASE-END]"+name);
            const timerGaps=timerTimes.slice(1).map((v,i)=>v-timerTimes[i]);
            return {name, timerCount:timerTimes.length, timerGapP95Ms:p95(timerGaps),
                rAFSamples:rafTimes.length, rAFGapP95Ms:p95(rafTimes),
                scrollEvents,maximumY,lostFocus,
                durationMs:performance.now()-started,
                visibility:document.visibilityState,documentHasFocus:document.hasFocus()};
          }
          let results = [];
          results.push(await runTrial('idle', false));
          results.push(await timerScrollTrial('timer-scroll'));
          document.body.replaceChildren();
          const blank = document.createElement('div');
          blank.style.height='6000px';
          blank.style.background='repeating-linear-gradient(to bottom,#a3e2f0 0px,#a3e2f0 30px,#f1c5bb 30px,#f1c5bb 60px)';
          document.body.append(blank);
          results.push(await timerScrollTrial('timer-blank-scroll'));
          window.webkit.messageHandlers.probe.postMessage(JSON.stringify({
             results, platform:navigator.userAgent, screen:[screen.width,screen.height],
             reducedMotion:matchMedia('(prefers-reduced-motion: reduce)').matches,
             reducedTransparency:matchMedia('(prefers-reduced-transparency: reduce)').matches,
             time:Date.now()
          }));
        })().catch(e=> window.webkit.messageHandlers.probe.postMessage(
          JSON.stringify({error:String(e)})))
        """)
    }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        let body = message.body
        if let label = body as? String,
           label.hasPrefix("[WK-PHASE-") || label.hasPrefix("[WK-TIMER-PROGRESS]") {
            if label.hasPrefix("[WK-PHASE-BEGIN]") {
                window.makeKeyAndOrderFront(nil)
                NSApp.activate(ignoringOtherApps: true)
            }
            print("\(label) uptime=\(CACurrentMediaTime()) appActive=\(NSApp.isActive) windowKey=\(window.isKeyWindow)")
            fflush(stdout)
            return
        }
        finished = true
        displayLink?.invalidate()
        let intervals = zip(displayTimestamps.dropFirst(), displayTimestamps).map { ($0.0 - $0.1) * 1000.0 }
        let ordered = intervals.sorted()
        let p95 = ordered.isEmpty ? -1 : ordered[Int(Double(ordered.count - 1) * 0.95)]
        print("[WK-MINIMAL-RESULT] \(body)")
        print("[WK-NATIVE-DISPLAY-LINK] count=\(intervals.count),p95Ms=\(p95),screenHz=\(window.screen?.maximumFramesPerSecond ?? -1)")
        print("[WK-MINIMAL-WINDOW] appActive=\(NSApp.isActive), windowKey=\(window.isKeyWindow), windowVisible=\(window.isVisible)")
        fflush(stdout)
        NSApp.terminate(nil)
    }
}

let app = NSApplication.shared
let delegate = Probe()
app.delegate = delegate
app.run()
