# Issue 19 — normal app versus diagnostic app, native input parity

Date: 2026-10-13. Base: `c8dc03f8f6f57eed75e9834798e574c7f27dcb84`. Branch: `test/issue19-native-input-parity`.

## Define the problem

The owner now explicitly reports: ordinary **Tasknotes is smooth**, but **Issue19 QA still lags**. This is new user acceptance evidence for the ordinary app, not an agent-measured frame rate and not proof of the remaining QA bottleneck. It supersedes the assumption that every installed Tasknotes variant is currently slow. Do not turn a diagnostic-app failure into a production regression without reproducing it there.

Verified running processes: ordinary Tasknotes PID 67613, binary built 2026-10-12 18:22 +07; Hover A/B QA PID 67019, built 2026-10-13 11:19 +07. Ordinary binary SHA256 `6055b42deec803fe3ae34a67c145a0bf58c4a21690fe0f8a105f2a865bbb54ae`; QA binary `f0b688980cdd355def39e65cd3153939f70c9066cea652be4852ee4e97c8fc17`. These PIDs are observations, never reuse without checking identity.

**Direct embedded-asset evidence:** searched generated compressed Tauri asset blobs inside both actual Mach-O executables and decompressed matching blobs using Node zlib. Both embed exactly the same application JavaScript (243233 bytes, SHA256 `78edd264cde7dca18000d0cad0d36048e85d41687089de73096186587524bcb9`) and CSS (14188 bytes, SHA256 `a282ac798bf2a78cfdfacb729288c3e4320b43644fb070c047c42d978397c839`). Both also match current dist assets. Only QA embeds `issue19-hover-ab.mjs` (15335 bytes, SHA256 `8e61fe93134c1f4657a97a492622625d3c44f7211b3a3fe5d9353e8552992155`). HTML differs by its QA module load; QA also enables the global Tauri API and QA window capabilities. Data/Inspector state is not matched by this asset proof.

## Breakdown and ordered execution

1. Preserve the ordinary working bundle and personal data. Record artifact provenance and use only synthetic temporary fixtures. Do not rebuild into or overwrite the known-smooth ordinary bundle.
2. First establish whether a **strictly in-process native test** can deliver scroll to its own synthetic WKWebView through public AppKit `NSResponder.scrollWheel(with:)`, instead of JavaScript `scrollBy`. No system-wide event posting, event taps, global input injection, Safari automation, private APIs, or permission bypass. This is a new native test seam, NOT a retry of the rejected scripted WheelEvent experiment. If the permitted tooling refuses this method, stop that path, do not obfuscate or reroute it.
3. Only after the new seam actually produces scrolling, compare clean application content versus QA instrumentation under matched input, fixture, focus, build and Inspector state. Keep event provenance labelled **AppKit synthetic**, never physical hardware. Browser `isTrusted` alone cannot prove hardware. rAF remains secondary and cannot decide user-visible acceptance.
4. Test a single demonstrated difference at a time. Use red/green tests for any lasting change. Keep diagnostic builds isolated and prevent them from overwriting the normal app. Verify functional native create/edit/restart using synthetic data when changes justify it.
5. Review Standards and Spec against the fixed point, persist results and a Draft PR, and leave main/releases untouched. The permitted independent `agy` CLI was rechecked and is not on PATH; do not label self-review independent.

## Ranked hypotheses (not findings)

1. **Profiler/diagnostic perturbation:** same application assets, different QA script/Inspector. Prediction: with matched native input and fixture, clean/uninspected content has better delivered visual updates than instrumented content. Native event or rAF counts alone do not establish that.
2. **Workload difference:** QA has 32 synthetic Tasks, including a metrics description; ordinary current data were not inspected. Prediction: clean application with the same QA fixture reproduces the poor behavior, even without the QA module.
3. **Native config/session difference:** QA adds global API/capabilities and may have an attached Inspector. Prediction: after holding frontend/data/input constant, one specific config or fresh-session change makes a reproducible difference.
4. **Old rAF feedback loop measures the wrong symptom:** WebKit can scroll independently of main-thread callbacks. Prediction: native scroll visual updates remain healthy while rAF is sparse. Do not classify that as production jank.

## Primary-source research before executing the native seam

- Apple public NSResponder scroll handler: https://developer.apple.com/documentation/appkit/nsresponder/scrollwheel(with:)
- Apple public local scroll-event construction: https://developer.apple.com/documentation/coregraphics/cgevent/init(scrollwheelevent2source:units:wheelcount:wheel1:wheel2:wheel3:)
- WebKit scrolling architecture aims to keep scrolling independent of main-thread work: https://trac.webkit.org/wiki/Scrolling
- WebKit says Media & Animations, Memory and JavaScript Allocations timelines can affect performance; Screenshots captures after composites: https://webkit.org/web-inspector/timelines-tab/
- DOM isTrusted semantics: https://dom.spec.whatwg.org/#dom-event-istrusted

These sources motivate controlled tests, not a conclusion that Inspector or the QA overlay caused this user's lag. Previously rejected release-only, main/child, background throttling, blank-DOM rAF and uncontrolled capture experiments remain rejected; repeat only if a genuinely different seam/controlled variable warrants it.

## Completion meaning

Preserve the owner's ordinary-app smoothness report. Distinguish product acceptance, QA correctness, simulated native-input evidence and actual physical display evidence. No invented hardware provenance, no claim that a regression test proves physical FPS, no changes to agreed product features or personal data. If a physical gate truly remains inaccessible, record the exact boundary and all completed safe work rather than requesting the previous capture again by default.

## One environment-qualified seam retry

After the isolated bundle build, the Mac observation newly reported both Tasknotes windows on-screen, and named QA activation produced a focused, fully rendered Ready screenshot. Earlier native pilots had off-screen/hidden windows. This changed observable condition justifies one retry of the already compiled native-Start helper without changing its source, visibility policy, input dispatch or thresholds. Use only the normal scoped native activation and semantic button; do not apply the blocked floating/all-spaces adjustment. A new failure still remains INCONCLUSIVE.

## Next gate after the native seam passes

The environment-qualified retry completed 240/240 native synthetic wheel events, 3840 px travel, focus retained and a visible document. It validates event delivery, not physical FPS. Before implementing any new visual sampler, check whether own-window ScreenCaptureKit capture is already authorized via Apple's read-only `CGPreflightScreenCaptureAccess()`. Do not request a new permission or capture another application. If the preflight is false or unavailable, stop that measurement path and preserve the exact boundary. If available, any proposed sampler must be synthetic-window-only, keep pixels in memory, report delivered visual changes rather than physical hardware FPS, and first demonstrate that capture does not invalidate the native workload. Do not repeat the old JS-timer capture experiment.

## Bounded visual-sampling experiment

Read-only screen-capture preflight returned true. Extend only the archived synthetic helper with optional own-window ScreenCaptureKit sampling, no pixel/image persistence, cursor excluded, plus native dispatch duration and actual input-phase elapsed time. First add/report only the already collected native dispatch costs and phase elapsed time, and run the no-capture pacing control. Implement the optional sampler only if that input-pacing prerequisite passes; this avoids building a visual sampler on an invalid input stream. Then run one capture condition with the same 240 in-process events. Require focus throughout and roughly the intended input pacing (240 events at 60 Hz; elapsed 3.5–6 seconds) before treating captured visual cadence as representative of this input. If direct responder delivery itself is materially slower, report that as a synthetic-input limitation and do not promote it to a product-performance test. Capture can only measure delivered own-window samples, not prove physical monitor FPS. Stop if capture changes the workload validity or permission state; no automatic permission request.

## Execution refinement: reproduced output collision

The live raw `src-tauri/target/debug/tasknotes` hash equalled the QA bundle, not the ordinary owner-accepted app. This directly demonstrates the build-isolation defect anticipated in step 1; it does not establish the physical-lag root cause. The lasting fix stays at the actual runner's external build-command seam: explicit dedicated Cargo target, reject missing isolated bundle, and no fallback to legacy output. Regression tests use disposable build-tool stubs and never open an app or database. A native build in the dedicated target verifies packaging separately. Repeated focus-invalid native pilots were stopped; no new performance threshold or product workaround was introduced.
