import { $, browser, expect } from "@wdio/globals";

type Sample = { domMs: number; frameMs: number };
type PerfSamples = {
  expand: Sample[];
  collapse: Sample[];
  typingFrameMs: number[];
  typingEvents: number;
};
function percentile(values: number[], quantile: number): number {
  if (!values.length) return Number.NaN;
  const ordered = [...values].sort((a, b) => a - b);
  return ordered[Math.ceil(ordered.length * quantile) - 1];
}

describe("Issue #19: visible Tasknotes responsiveness diagnostic loop", () => {
  it("measures actual WKWebView event→DOM/paint, typing, and save responsiveness", async () => {
    await browser.switchToWindow("main");
    // Explicit QA-only permissions make it possible to profile an actually
    // rendered native window instead of an offscreen WKWebView.
    await browser.execute(async () => {
      const appWindow = (
        window as typeof window & {
          __TAURI__: {
            window: {
              getCurrentWindow: () => {
                show: () => Promise<void>;
                setFocus: () => Promise<void>;
              };
            };
          };
        }
      ).__TAURI__.window.getCurrentWindow();
      await appWindow.show();
      await appWindow.setFocus();
    });
    await browser.waitUntil(
      async () => {
        return await browser.execute(
          () => document.visibilityState === "visible",
        );
      },
      {
        timeout: 10000,
        timeoutMsg:
          "QA WKWebView remained hidden; cannot assess painted frames",
      },
    );
    await $('input[aria-label="Task title"]').waitForExist({ timeout: 15000 });
    // Rust/Tauri creates the synthetic fixture directly so random focus loss
    // in WebDriver's text-entry setup cannot masquerade as a performance bug.
    await browser.execute(async () => {
      const core = (
        window as typeof window & {
          __TAURI__: {
            core: {
              invoke: (command: string, args: unknown) => Promise<unknown>;
            };
          };
        }
      ).__TAURI__.core;
      await core.invoke("create_task", {
        input: { title: "QA-PERFORMANCE-ONE", description: "" },
      });
      const sort = document.querySelector<HTMLSelectElement>(
        'select[aria-label="Sort tasks"]',
      );
      if (!sort)
        throw new Error("No sort selector to refresh synthetic fixture");
      sort.value = "newest";
      sort.dispatchEvent(new Event("input", { bubbles: true }));
      sort.dispatchEvent(new Event("change", { bubbles: true }));
    });
    const row = await $('article[aria-label="QA-PERFORMANCE-ONE"]');
    await row.waitForExist({ timeout: 15000 });

    const injectedJank = Number(process.env.QA_INJECT_JANK_MS || 0);
    await browser.execute((delayMs) => {
      type Collector = {
        expand: Array<{ domMs: number; frameMs: number }>;
        collapse: Array<{ domMs: number; frameMs: number }>;
        typingFrameMs: number[];
        typingEvents: number;
      };
      const results: Collector = {
        expand: [],
        collapse: [],
        typingFrameMs: [],
        typingEvents: 0,
      };
      (
        window as typeof window & { __tasknotesPerf?: Collector }
      ).__tasknotesPerf = results;
      document.addEventListener(
        "click",
        (event) => {
          const origin = event.target;
          if (!(origin instanceof HTMLElement)) return;
          const button = origin.closest<HTMLButtonElement>("button.task-open");
          if (!button) return;
          const expanding = button.getAttribute("aria-expanded") !== "true";
          const started = performance.now();
          // The optional delay is a red-capability CONTROL, never a claimed real bug.
          if (delayMs > 0) {
            while (performance.now() - started < delayMs) {
              /* synthetic main-thread stall */
            }
          }
          let collected = false;
          const check = () => {
            const visible = Boolean(
              document.querySelector("section[aria-label='Task details']"),
            );
            if (collected || visible !== expanding) return;
            collected = true;
            observer.disconnect();
            const domMs = performance.now() - started;
            const sample = { domMs, frameMs: -1 };
            results[expanding ? "expand" : "collapse"].push(sample);
            requestAnimationFrame(() => {
              sample.frameMs = performance.now() - started;
            });
          };
          const observer = new MutationObserver(check);
          observer.observe(document.getElementById("root")!, {
            childList: true,
            subtree: true,
          });
          queueMicrotask(check);
        },
        true,
      );
      document.addEventListener(
        "input",
        (event) => {
          const target = event.target;
          if (
            !(target instanceof HTMLInputElement) ||
            target.getAttribute("aria-label") !== "Edit title"
          )
            return;
          const started = performance.now();
          results.typingEvents += 1;
          requestAnimationFrame(() => {
            results.typingFrameMs.push(performance.now() - started);
          });
        },
        true,
      );
    }, injectedJank);

    const webkitState = await browser.execute(() => ({
      visible: document.visibilityState,
      hasFocus: document.hasFocus(),
      rafPresent: typeof requestAnimationFrame === "function",
      title: document.title,
    }));
    console.log("[TASKNOTES-PERF-WEBVIEW]", JSON.stringify(webkitState));
    const startedOverall = performance.now();
    for (let i = 0; i < 15; i++) {
      await $("button.task-open").click();
      await $('section[aria-label="Task details"]').waitForExist({
        timeout: 10000,
      });
      await $("button.task-open").click();
      await $('section[aria-label="Task details"]').waitForExist({
        timeout: 10000,
        reverse: true,
      });
    }
    const toggleDriverMs = (performance.now() - startedOverall) / 30;
    await $("button.task-open").click();
    await $('input[aria-label="Edit title"]').waitForExist({ timeout: 10000 });

    const typingStart = performance.now();
    await $('input[aria-label="Edit title"]').setValue(
      "QA-PERFORMANCE-UPDATED-WITH-A-LONGER-TITLE-123456789",
    );
    const typingDriverMs = performance.now() - typingStart;
    const saveStart = performance.now();
    await $("button=Save changes").click();
    await $(
      'article[aria-label="QA-PERFORMANCE-UPDATED-WITH-A-LONGER-TITLE-123456789"]',
    ).waitForExist({ timeout: 10000 });
    const saveDriverMs = performance.now() - saveStart;
    // Give the last requestAnimationFrame a chance to report its paint sample.
    await browser.pause(150);

    const values = await browser.execute(() => {
      return (window as typeof window & { __tasknotesPerf?: PerfSamples })
        .__tasknotesPerf;
    });
    if (!values)
      throw new Error("Native WKWebView instrumentation was not installed");
    const expandDom = values.expand.map((v) => v.domMs);
    const expandFrame = values.expand
      .filter((v) => v.frameMs >= 0)
      .map((v) => v.frameMs);
    const collapseDom = values.collapse.map((v) => v.domMs);
    const collapseFrame = values.collapse
      .filter((v) => v.frameMs >= 0)
      .map((v) => v.frameMs);
    const p95 = {
      expandDomMs: percentile(expandDom, 0.95),
      expandFrameMs: percentile(expandFrame, 0.95),
      collapseDomMs: percentile(collapseDom, 0.95),
      collapseFrameMs: percentile(collapseFrame, 0.95),
      typingFrameMs: percentile(values.typingFrameMs, 0.95),
    };
    console.log(
      "[TASKNOTES-PERF]",
      JSON.stringify({
        mode: injectedJank ? "synthetic-delay-control" : "unmodified-baseline",
        injectedJankMs: injectedJank,
        cases: {
          expand: expandDom.length,
          collapse: collapseDom.length,
          inputEvents: values.typingEvents,
          paintedTypingFrames: values.typingFrameMs.length,
          expandPaintedFrames: expandFrame.length,
          collapsePaintedFrames: collapseFrame.length,
        },
        p95,
        toggleDriverMs,
        typingDriverMs,
        saveDriverMs,
        thresholdsMs: { expandDom: 150, typingFrame: 100, saveDriver: 800 },
      }),
    );
    // A single in-WebView probe avoids WDIO's per-command window switching
    // while measuring true native requestAnimationFrame callbacks.
    const directPaint = await browser.execute(async () => {
      const appWindow = (
        window as typeof window & {
          __TAURI__: {
            window: {
              getCurrentWindow: () => {
                show: () => Promise<void>;
                setFocus: () => Promise<void>;
              };
            };
          };
        }
      ).__TAURI__.window.getCurrentWindow();
      await appWindow.show();
      await appWindow.setFocus();
      const visibilityAtStart = document.visibilityState;
      const nextFrame = async (): Promise<number | null> =>
        await Promise.race([
          new Promise<number>((resolve) =>
            requestAnimationFrame(() => resolve(performance.now())),
          ),
          new Promise<null>((resolve) => setTimeout(() => resolve(null), 600)),
        ]);
      const firstFrame = await nextFrame();
      const idleIntervals: number[] = [];
      let previousFrame = firstFrame;
      if (previousFrame !== null) {
        for (let i = 0; i < 12; i++) {
          const next = await nextFrame();
          if (next === null) break;
          idleIntervals.push(next - previousFrame);
          previousFrame = next;
        }
      }
      const samples: number[] = [];
      if (firstFrame !== null) {
        for (let i = 0; i < 12; i++) {
          const openButton =
            document.querySelector<HTMLButtonElement>("button.task-open");
          if (!openButton) break;
          const started = performance.now();
          openButton.click();
          const painted = await nextFrame();
          if (painted === null) break;
          samples.push(painted - started);
        }
      }
      const visibilityBeforeBlank = document.visibilityState;
      document.body.replaceChildren();
      const blankBodyIntervals: number[] = [];
      let prevBlank = await nextFrame();
      if (prevBlank !== null) {
        for (let i = 0; i < 12; i++) {
          const next = await nextFrame();
          if (next === null) break;
          blankBodyIntervals.push(next - prevBlank);
          prevBlank = next;
        }
      }
      for (const stylesheet of document.querySelectorAll(
        'style,link[rel="stylesheet"]',
      )) {
        stylesheet.remove();
      }
      const blankWithoutCssIntervals: number[] = [];
      let prevBlankCss = await nextFrame();
      if (prevBlankCss !== null) {
        for (let i = 0; i < 12; i++) {
          const next = await nextFrame();
          if (next === null) break;
          blankWithoutCssIntervals.push(next - prevBlankCss);
          prevBlankCss = next;
        }
      }
      const p95 = (samples: number[]) =>
        samples.length
          ? [...samples].sort((a, b) => a - b)[
              Math.ceil(samples.length * 0.95) - 1
            ]
          : null;
      const minimalTimerIntervals: number[] = [];
      for (let i = 0; i < 16; i++) {
        const started = performance.now();
        await new Promise<void>((resolve) => setTimeout(resolve, 0));
        minimalTimerIntervals.push(performance.now() - started);
      }
      let nativeWindowStates: unknown = null;
      try {
        nativeWindowStates = await (
          window as typeof window & {
            __TAURI__: {
              core: { invoke: (command: string) => Promise<unknown> };
            };
          }
        ).__TAURI__.core.invoke("plugin:wdio|get_window_states");
      } catch (error) {
        nativeWindowStates = String(error);
      }
      return {
        minimalTimerP95Ms: p95(minimalTimerIntervals),
        nativeWindowStates,
        nativeDocumentFocused: document.hasFocus(),
        visibilityAtStart,
        visibilityBeforeBlank,
        visibilityAtEnd: document.visibilityState,
        frameAvailable: firstFrame !== null,
        blankBodyFrames: blankBodyIntervals.length,
        blankWithoutCssFrames: blankWithoutCssIntervals.length,
        blankBodyP95Ms: p95(blankBodyIntervals),
        blankWithoutCssP95Ms: p95(blankWithoutCssIntervals),
        count: samples.length,
        idleFrames: idleIntervals.length,
        idleFrameP95Ms: idleIntervals.length
          ? [...idleIntervals].sort((a, b) => a - b)[
              Math.ceil(idleIntervals.length * 0.95) - 1
            ]
          : null,
        paintP95Ms: samples.length
          ? [...samples].sort((a, b) => a - b)[
              Math.ceil(samples.length * 0.95) - 1
            ]
          : null,
      };
    });
    console.log("[TASKNOTES-PERF-DIRECT-PAINT]", JSON.stringify(directPaint));
    const nativeWindow = Array.isArray(directPaint.nativeWindowStates)
      ? (
          directPaint.nativeWindowStates as Array<{
            label: string;
            is_focused: boolean;
          }>
        ).find((item) => item.label === "main")
      : null;
    if (!nativeWindow?.is_focused || !directPaint.nativeDocumentFocused) {
      console.warn(
        "[TASKNOTES-PERF-INCONCLUSIVE] Native Tasknotes window lost foreground focus; test-only rAF throttling is not a verified user-visible Tasknotes defect.",
      );
    }
    // Diagnostic responsiveness gate: three dropped 60 Hz frames are already
    // visually objectionable. This is NOT the final product performance SLA.
    expect(directPaint.visibilityAtStart).toBe("visible");
    expect(directPaint.count).toBeGreaterThanOrEqual(10);
    expect(directPaint.paintP95Ms).toBeLessThan(50);
    expect(values.expand.length).toBeGreaterThanOrEqual(12);
    expect(values.collapse.length).toBeGreaterThanOrEqual(12);
    // These are DIAGNOSTIC thresholds, not a previously approved release SLA.
    expect(p95.expandDomMs).toBeLessThan(150);
    if (values.typingFrameMs.length > 0)
      expect(p95.typingFrameMs).toBeLessThan(100);
    expect(saveDriverMs).toBeLessThan(800);
  });
});
