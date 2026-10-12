import { $, browser, expect } from "@wdio/globals";

type ScrollMetrics = {
  scrollEvents: number;
  sampledFrames: number;
  frameP95Ms: number | null;
  frameMedianMs: number | null;
  scrollGapP95Ms: number | null;
  distancePx: number;
  visible: string;
  nativeFocused: boolean | null;
};

function percentile(values: number[], p: number): number | null {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.ceil(p * sorted.length) - 1];
}

describe("Issue 19: Tasknotes scroll jank native reproduction", () => {
  it("measures scroll frame cadence at 1 task, 30 tasks and without heavy CSS", async () => {
    await browser.switchToWindow("main");
    await browser.execute(async () => {
      const api = (
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
      await api.show();
      await api.setFocus();
    });
    await $('input[aria-label="Task title"]').waitForExist({ timeout: 15000 });

    async function addTasks(count: number, offset: number) {
      await browser.execute(
        async (number, start) => {
          const core = (
            window as typeof window & {
              __TAURI__: {
                core: {
                  invoke: (command: string, args: unknown) => Promise<unknown>;
                };
              };
            }
          ).__TAURI__.core;
          for (let i = 0; i < number; i++) {
            await core.invoke("create_task", {
              input: {
                title: "SCROLL-QA-" + (start + i),
                description: "synthetic scroll item " + i,
              },
            });
          }
          const sort = document.querySelector<HTMLSelectElement>(
            'select[aria-label="Sort tasks"]',
          );
          if (!sort) throw new Error("Missing sort control to refresh tasks");
          sort.value = sort.value === "priority" ? "newest" : "priority";
          sort.dispatchEvent(new Event("input", { bubbles: true }));
          sort.dispatchEvent(new Event("change", { bubbles: true }));
        },
        count,
        offset,
      );
      await browser.waitUntil(
        async () =>
          (await $$("article.task-card")).length >=
          Math.min(50, count + offset),
        {
          timeout: 20000,
          timeoutMsg: "Synthetic Task records did not render",
        },
      );
    }

    // The embedded macOS W3C wheel handler sends scrollBy to the scoped
    // WKWebView. This is not a physical trackpad or compositor-presentation test.
    async function scrollTrial(label: string): Promise<ScrollMetrics> {
      await browser.execute(() => {
        window.scrollTo({ top: 0, behavior: "instant" });
      });
      const id = "scroll-qa-" + label;
      await browser.execute((key) => {
        type Collector = {
          active: boolean;
          frames: number[];
          scrolls: number[];
          positions: number[];
          lastFrame: number | null;
          visible: string;
        };
        const collector: Collector = {
          active: true,
          frames: [],
          scrolls: [],
          positions: [],
          lastFrame: null,
          visible: document.visibilityState,
        };
        (
          window as typeof window & { __scrollPerf?: Record<string, Collector> }
        ).__scrollPerf ??= {};
        (
          window as typeof window & { __scrollPerf?: Record<string, Collector> }
        ).__scrollPerf![key] = collector;
        const onScroll = () => {
          if (!collector.active) return;
          collector.scrolls.push(performance.now());
          collector.positions.push(window.scrollY);
        };
        addEventListener("scroll", onScroll, { passive: true });
        const tick = (now: number) => {
          if (!collector.active) {
            removeEventListener("scroll", onScroll);
            return;
          }
          if (collector.lastFrame !== null)
            collector.frames.push(now - collector.lastFrame);
          collector.lastFrame = now;
          requestAnimationFrame(tick);
        };
        requestAnimationFrame(tick);
      }, id);

      // Over ~2.6 s a 10 Hz WKWebView should produce enough frame samples
      // for a meaningful p95 rather than the old 5–8-frame approximation.
      // Small wheel steps avoid exhausting a 30-Task scroll range instantly.
      const actions = Array.from({ length: 160 }, (_, i) => ({
        type: "scroll",
        x: 610,
        y: 440,
        deltaX: 0,
        deltaY: i < 80 ? 16 : -16,
        duration: 16,
      }));
      await browser.performActions([
        { type: "wheel", id: "wheel-" + label, actions },
      ]);
      await browser.releaseActions();
      await browser.pause(150);

      return browser.execute(async (key) => {
        const data = (
          window as typeof window & {
            __scrollPerf?: Record<
              string,
              {
                active: boolean;
                frames: number[];
                scrolls: number[];
                positions: number[];
                visible: string;
              }
            >;
          }
        ).__scrollPerf?.[key];
        if (!data) throw new Error("Scroll probe collection lost");
        data.active = false;
        const p = (values: number[], q: number): number | null => {
          if (!values.length) return null;
          const arr = [...values].sort((a, b) => a - b);
          return arr[Math.ceil(q * arr.length) - 1];
        };
        const gaps = data.scrolls.slice(1).map((t, i) => t - data.scrolls[i]);
        const focused = await (
          window as typeof window & {
            __TAURI__: {
              window: {
                getCurrentWindow: () => { isFocused: () => Promise<boolean> };
              };
            };
          }
        ).__TAURI__.window
          .getCurrentWindow()
          .isFocused();
        return {
          scrollEvents: data.scrolls.length,
          sampledFrames: data.frames.length,
          frameP95Ms: p(data.frames, 0.95),
          frameMedianMs: p(data.frames, 0.5),
          scrollGapP95Ms: p(gaps, 0.95),
          distancePx: data.positions.length
            ? Math.max(...data.positions) - Math.min(...data.positions)
            : 0,
          visible: document.visibilityState,
          nativeFocused: focused,
        };
      }, id);
    }

    await addTasks(1, 0);
    const small = await scrollTrial("one-task");
    await addTasks(29, 1);
    const many = await scrollTrial("thirty-tasks");
    await browser.execute(() => {
      // Synthetic QA only, not a product-code change: remove expensive
      // gradients/shadows for a falsifiable compositor cost comparison.
      const style = document.createElement("style");
      style.id = "synthetic-qa-css-ablation";
      style.textContent =
        "* { box-shadow: none !important; filter: none !important; backdrop-filter: none !important; text-shadow: none !important; } .main-view, .sidebar, .brand-mark { background: #ffffff !important; }";
      document.head.append(style);
    });
    const noEffects = await scrollTrial("no-effects");
    await browser.execute(() => {
      // A control with no React, Tasknotes DOM or shipped stylesheet. It is
      // still scrollable so W3C wheel and rAF can be compared at the same seam.
      document.body.replaceChildren();
      for (const sheet of document.querySelectorAll(
        "style,link[rel=stylesheet]",
      ))
        sheet.remove();
      const spacer = document.createElement("div");
      spacer.style.height = "3600px";
      spacer.style.background = "#ffffff";
      document.body.append(spacer);
    });
    const blankNative = await scrollTrial("blank-native");
    console.log(
      "[TASKNOTES-SCROLL-ISSUE19]",
      JSON.stringify({
        oneTask: small,
        thirtyTasks: many,
        noVisualEffects: noEffects,
        blankNativePage: blankNative,
        host: "macOS WKWebView; embedded W3C wheel->window.scrollBy, not physical trackpad",
        diagnosticThresholdFrameP95Ms: 50,
      }),
    );

    expect(many.scrollEvents).toBeGreaterThan(0);
    expect(many.distancePx).toBeGreaterThan(100);
    expect(many.sampledFrames).toBeGreaterThanOrEqual(12);
    expect(blankNative.sampledFrames).toBeGreaterThanOrEqual(12);
    // Diagnostic red reproduction, not an approved release threshold.
    expect(many.frameP95Ms).toBeLessThan(50);
  });
});
