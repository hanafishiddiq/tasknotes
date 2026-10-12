/* Tasknotes Issue #19: QA-only scroll benchmark on the REAL built Tasknotes UI.
 * No WebDriver, no network, no application source changes, no user Tasks.
 * This file is copied exclusively to ignored, synthetic QA frontend assets.
 * Bench output is persisted with the existing Rust TaskStore update_task IPC.
 */
(() => {
  "use strict";
  const PREFIX = "__ISSUE19_QA_AB__";
  const METRICS_TITLE = PREFIX + "METRICS";
  const COUNT = 30;
  const VERSION = 1;
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const p = (arr, q) => {
    if (!arr.length) return null;
    const sorted = [...arr].sort((a, b) => a - b);
    return Math.round(sorted[Math.ceil(sorted.length * q) - 1] * 100) / 100;
  };
  function check(condition, message) {
    if (!condition) throw new Error("QA safety/check: " + message);
  }
  async function waitFor(checker, ms = 12000) {
    const until = performance.now() + ms;
    while (performance.now() < until) {
      const value = await checker();
      if (value) return value;
      await sleep(100);
    }
    throw new Error("QA timed out waiting for real Tasknotes UI");
  }
  const native = window.__TAURI__;
  if (!native?.core?.invoke || !native?.window?.getCurrentWindow) {
    document.title = "Issue19 QA ERROR: Tauri global unavailable";
    return;
  }
  const invoke = (command, args) => native.core.invoke(command, args);
  const windowAPI = native.window.getCurrentWindow();
  // This QA-only script is IDENTICAL for both native placements. Webview label
  // provides evidence of which native path served the same app resources.
  const viewLabel = native.webview?.getCurrentWebview?.()?.label || "unavailable";
  let metricsRecord = null;
  let runIndex = -1;
  async function saveResult(report) {
    if (!metricsRecord) {
      console.error("Issue19 QA: metrics row unavailable", report.error);
      document.title = "Issue19 QA ERROR: metrics row unavailable";
      return;
    }
    const earlier = JSON.parse(metricsRecord.description || '{"runs":[]}');
    const runs = [...(earlier.runs || []), report];
    check(runs.length <= 10, "too many persisted synthetic QA runs");
    metricsRecord = await invoke("update_task", {
      input: {
        id: metricsRecord.id,
        title: METRICS_TITLE,
        description: JSON.stringify({schema: VERSION, runs}),
        expectedRevision: metricsRecord.revision
      }
    });
    document.title = report.valid
      ? "Issue19 QA RESULT " + runs.length
      : "Issue19 QA INCONCLUSIVE " + runs.length;
  }
  async function focused() {
    try { return await windowAPI.isFocused(); }
    catch (error) { return false; }
  }
  async function refocus() {
    await windowAPI.show();
    for (let i = 0; i < 5; i++) {
      await windowAPI.setFocus();
      window.focus();
      await sleep(180);
      if (document.visibilityState === "visible" && document.hasFocus() && await focused()) return true;
    }
    return false;
  }
  async function measure(label, scroll) {
    const gaps = [];
    const times = [];
    const positions = [];
    let rafHandle = null;
    let last = null;
    let active = true;
    let focusLost = false;
    const onScroll = () => { times.push(performance.now()); positions.push(window.scrollY); };
    window.addEventListener("scroll", onScroll, {passive:true});
    const frame = (stamp) => {
      if (!active) return;
      if (last !== null) gaps.push(stamp - last);
      last = stamp;
      rafHandle = requestAnimationFrame(frame);
    };
    rafHandle = requestAnimationFrame(frame);
    const before = performance.now();
    const focusAtStart = document.hasFocus();
    let ticks = 0;
    // rAF ticker and independently paced wheel-equivalent scroll events.
    await new Promise((resolve) => {
      const timer = setInterval(() => {
        if (document.visibilityState !== "visible" || !document.hasFocus()) focusLost = true;
        if (scroll) window.scrollBy(0, ticks < 75 ? 24 : -24);
        ticks++;
        if (ticks >= 150) {
          clearInterval(timer);
          resolve();
        }
      }, 16);
    });
    await sleep(160);
    active = false;
    if (rafHandle != null) cancelAnimationFrame(rafHandle);
    window.removeEventListener("scroll", onScroll);
    const spread = positions.length ? Math.max(...positions) - Math.min(...positions) : 0;
    return {
      label, frameSamples: gaps.length, frameP50Ms: p(gaps, 0.5),
      frameP95Ms: p(gaps, 0.95),
      scrollEvents: times.length, scrollDistancePx: spread,
      timerTicks: ticks, elapsedMs: Math.round(performance.now()-before),
      focusLost,focusAtStart
    };
  }

  async function run() {
    await waitFor(() => document.querySelector('select[aria-label="Sort tasks"]'));
    const list = await invoke("list_tasks", {limit:100,cursor:null,sort:"priority"});
    check(!list.nextCursor, "QA database has over 100 Tasks");
    check(list.items.every(t => t.title.startsWith(PREFIX)),
      "refusing to operate on non-synthetic task data");

    if (!list.items.length) {
      for (let i=0; i<COUNT; i++) {
        await invoke("create_task", {input:{
          title: PREFIX + "TASK-" + String(i).padStart(2,"0"),
          description: "Synthetic Tasknotes scroll comparison row " + i
        }});
      }
      metricsRecord = await invoke("create_task", {
        input:{title:METRICS_TITLE,description:JSON.stringify({schema:VERSION,runs:[]})}
      });
    } else {
      check(list.items.length === COUNT+1, "unexpected synthetic fixture count");
      metricsRecord = list.items.find(t=>t.title===METRICS_TITLE);
      check(metricsRecord, "metrics record missing");
    }

    runIndex = JSON.parse(metricsRecord.description || '{"runs":[]}').runs.length;
    const sort = document.querySelector('select[aria-label="Sort tasks"]');
    sort.value = sort.value === "priority" ? "newest" : "priority";
    sort.dispatchEvent(new Event("input",{bubbles:true}));
    sort.dispatchEvent(new Event("change",{bubbles:true}));
    await waitFor(()=>document.querySelectorAll('article.task-card').length===COUNT+1);
    const scroller = document.scrollingElement;
    check(scroller && scroller.scrollHeight > innerHeight + 250, "not enough scrollable UI height");
    const windowFocused = await refocus();
    if (!windowFocused) {
      await saveResult({schema:VERSION,valid:false,why:"native window not focused",
        runIndex, visibility:document.visibilityState});
      return;
    }

    window.scrollTo({top:0,behavior:"instant"});
    await sleep(200);
    const idle = await measure("idle", false);
    window.scrollTo({top:0,behavior:"instant"});
    await sleep(140);
    const scroll = await measure("scroll", true);
    const focusAtEnd = await focused();
    const valid = !idle.focusLost && !scroll.focusLost && focusAtEnd &&
      idle.frameSamples >= 12 && scroll.frameSamples >= 12 &&
      scroll.scrollDistancePx >= 200;
    const report = {
      schema:VERSION,
      valid, runIndex, reason:valid ? null : "focus/sample/scroll preconditions",
      createdAt:Date.now(),
      visibility:document.visibilityState,
      windowFocusedAtEnd:focusAtEnd,
      viewport:[innerWidth,innerHeight],
      scrollHeight:scroller.scrollHeight,
      frontendLabel:"EXACT_SAME_QA_RESOURCES",
      webviewLabel:viewLabel,
      idle,scroll,
    };
    await saveResult(report);
  }

  run().catch(async (error) => {
    try {
      await saveResult({schema:VERSION,valid:false,runIndex,
        error:String(error?.message||error),createdAt:Date.now()});
    } catch {}
    document.title = "Issue19 QA ERROR";
    console.error("Issue19 QA failed",error);
  });
})();
