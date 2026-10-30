/**
 * Ticket 17's measurements inside the real window, in a build made for them
 * (VITE_TASKNOTES_PERF_PROBE=1 and the Tauri feature qa-probe). It drives the
 * ordinary interface, times it, sends one report and ends the run.
 *
 * Scrolling is scripted: each frame sets scrollTop, which makes the main
 * thread do the layout. That is the costly path; a trackpad usually scrolls on
 * the compositor, so these frame times are an upper bound, not the feel of a
 * finger on a trackpad.
 */
import { invoke } from "@tauri-apps/api/core";
import { NO_FILTERS, taskApi } from "../lib/task-api";
import { noteApi } from "../lib/note-api";

const SOUND_KEY = "tasknotes.sound";
let firstRowAt = 0;
let commandLogError = "";
/** Every call to the core, for reading where a slow interaction spends its time. */
const commands: { command: string; at_ms: number; took_ms: number }[] = [];
const TASK_WORDS = ["b", "ba", "qua", "quarterly", "roadmap", "zephyrine", "quarterly roadmap", "nomatchanywhere"];
const NOTE_WORDS = ["b", "meeting", "kelvinite", "nomatchanywhere"];
const IPC_RUNS = 60;
const SCROLL_MS = 10_000;
const SCROLL_PX_PER_S = 1800;

/** Before the app starts: keep the Mac quiet during a run. */
export function prepare() {
  // Timed without frames: a window macOS has not drawn yet gets none.
  new MutationObserver((_, observer) => {
    if (document.querySelector("article[aria-label]")) {
      firstRowAt = performance.now();
      observer.disconnect();
    }
  }).observe(document.documentElement, { subtree: true, childList: true });
  // Tauri may freeze this; the run must still go on without the command log.
  try {
    const internals = (window as unknown as { __TAURI_INTERNALS__: { invoke: (...args: unknown[]) => Promise<unknown> } }).__TAURI_INTERNALS__;
    const real = internals.invoke;
    internals.invoke = (...args: unknown[]) => {
      const entry = { command: String(args[0]), at_ms: round(performance.now()), took_ms: -1 };
      commands.push(entry);
      return real.apply(internals, args).finally(() => { entry.took_ms = round(performance.now() - entry.at_ms); });
    };
  } catch (error) {
    commandLogError = String(error);
  }
  window.localStorage.setItem(SOUND_KEY, JSON.stringify({
    interaction: { volume: 0, muted: true }, ambient: { volume: 0, muted: true },
  }));
}

const tick = () => new Promise<void>((resolve) => { window.setTimeout(resolve, 4); });

const frame = () => new Promise<number>((resolve) => requestAnimationFrame(resolve));

/** Whether this window is being drawn: a sleeping or locked screen gives no frames worth timing. */
async function framesFlow() {
  const winner = await Promise.race([
    frame().then(() => true),
    new Promise<boolean>((resolve) => { window.setTimeout(() => resolve(false), 3000); }),
  ]);
  return winner;
}

/** After a change on screen: the next frame when frames flow, else the next turn of the event loop. */
let settled: () => Promise<unknown> = tick;

async function until<T>(check: () => T | null | undefined | false, timeoutMs = 30_000): Promise<T> {
  const deadline = performance.now() + timeoutMs;
  for (;;) {
    const value = check();
    if (value) return value;
    if (performance.now() > deadline) throw new Error(`Timed out waiting for: ${check}`);
    await tick();
  }
}

function percentile(sorted: number[], percent: number) {
  if (sorted.length === 0) return 0;
  return sorted[Math.max(1, Math.ceil((sorted.length * percent) / 100)) - 1];
}

const round = (value: number) => Math.round(value * 100) / 100;

function timing(samples: number[]) {
  const sorted = [...samples].sort((a, b) => a - b);
  return {
    runs: sorted.length,
    p50_ms: round(percentile(sorted, 50)),
    p95_ms: round(percentile(sorted, 95)),
    max_ms: round(sorted[sorted.length - 1] ?? 0),
  };
}

function frames(deltas: number[]) {
  const total = deltas.reduce((sum, delta) => sum + delta, 0);
  return {
    ...timing(deltas),
    fps: round((deltas.length * 1000) / Math.max(total, 1)),
    over_16_7_ms: deltas.filter((delta) => delta > 16.7).length,
    over_33_4_ms: deltas.filter((delta) => delta > 33.4).length,
  };
}

const scroller = () => document.querySelector<HTMLElement>(".main-scroll");
const firstRow = () => document.querySelector<HTMLElement>("article[aria-label]");
const rowCount = () => document.querySelectorAll("article[aria-label]").length;

/** Scroll down and back for `ms`; every frame's length. */
async function scrollFrames(element: HTMLElement, ms: number) {
  const deltas: number[] = [];
  let last = await frame();
  const end = last + ms;
  let direction = 1;
  let maxRows = 0;
  while (last < end) {
    const now = await frame();
    const delta = now - last;
    deltas.push(delta);
    last = now;
    element.scrollTop += direction * SCROLL_PX_PER_S * (delta / 1000);
    const limit = element.scrollHeight - element.clientHeight;
    if (element.scrollTop >= limit - 1) direction = -1;
    if (element.scrollTop <= 1) direction = 1;
    maxRows = Math.max(maxRows, rowCount());
  }
  element.scrollTop = 0;
  await frame();
  return {
    ...frames(deltas),
    scroll_height_px: element.scrollHeight,
    most_rows_in_dom: maxRows,
  };
}

function setInput(input: HTMLInputElement, value: string) {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
  setter.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

/** From typing to the last change on screen, debounce included. */
async function settleAfter(root: Element, act: () => void, quietMs = 500) {
  let last = performance.now();
  const observer = new MutationObserver(() => { last = performance.now(); });
  observer.observe(root, { subtree: true, childList: true, characterData: true, attributes: true });
  const started = performance.now();
  act();
  await until(() => performance.now() - Math.max(last, started) > quietMs);
  observer.disconnect();
  return last - started;
}

async function timed<T>(call: () => Promise<T>, runs: number) {
  await call();
  const samples: number[] = [];
  for (let i = 0; i < runs; i += 1) {
    const started = performance.now();
    await call();
    samples.push(performance.now() - started);
  }
  return timing(samples);
}

async function measure() {
  const report: Record<string, unknown> = {};
  const row = await until(firstRow, 60_000);
  report.first_row_after_load_ms = round(firstRowAt);
  report.startup_commands = commands.slice();
  report.command_log_error = commandLogError;
  report.first_row = row.getAttribute("aria-label");
  report.window = {
    visibility: document.visibilityState,
    has_focus: document.hasFocus(),
    width: window.innerWidth,
    height: window.innerHeight,
    device_pixel_ratio: window.devicePixelRatio,
  };

  // Scrolling is timed in frames; without them it is reported as not measured.
  const drawn = await framesFlow();
  report.frames_flow = drawn;
  if (drawn) settled = frame;
  const notMeasured = { not_measured: "this window was not being drawn" };
  if (drawn) {
    const idle: number[] = [];
    let at = await frame();
    for (let i = 0; i < 240; i += 1) {
      const now = await frame();
      idle.push(now - at);
      at = now;
    }
    report.idle_frames = frames(idle);
  }

  const area = await until(scroller);
  report.scroll_collapsed = drawn ? await scrollFrames(area, SCROLL_MS) : notMeasured;

  const open = (index: number) => {
    const buttons = document.querySelectorAll<HTMLButtonElement>('article[aria-label] button[aria-expanded]');
    return buttons[index] ?? null;
  };
  const opening: number[] = [];
  for (let i = 0; i < 12; i += 1) {
    const button = await until(() => open(i % 8));
    const started = performance.now();
    button.click();
    await until(() => document.querySelector('article[aria-label] button[aria-expanded="true"]'));
    await settled();
    opening.push(performance.now() - started);
    const close = document.querySelector<HTMLButtonElement>('article[aria-label] button[aria-expanded="true"]')!;
    close.click();
    await until(() => !document.querySelector('article[aria-label] button[aria-expanded="true"]'));
    await settled();
  }
  report.open_row = timing(opening);

  (await until(() => open(4))).click();
  await until(() => document.querySelector('article[aria-label] button[aria-expanded="true"]'));
  report.scroll_with_open_row = drawn ? await scrollFrames(area, SCROLL_MS) : notMeasured;
  document.querySelector<HTMLButtonElement>('article[aria-label] button[aria-expanded="true"]')?.click();
  await until(() => !document.querySelector('article[aria-label] button[aria-expanded="true"]'));

  const paging: number[] = [];
  for (let i = 0; i < 20; i += 1) {
    const next = await until(() => [...document.querySelectorAll("button")].find((b) => b.textContent === "Next page"));
    const before = firstRow()?.getAttribute("aria-label");
    const started = performance.now();
    next.click();
    await until(() => firstRow()?.getAttribute("aria-label") !== before);
    await settled();
    paging.push(performance.now() - started);
  }
  report.next_page = timing(paging);

  const typed: Record<string, number[]> = {};
  const commandsPerWord: Record<string, unknown> = {};
  const search = await until(() => document.querySelector<HTMLInputElement>('input[aria-label="Search tasks and notes"]'));
  for (let round_ = 0; round_ < 5; round_ += 1) {
    for (const word of TASK_WORDS) {
      await settleAfter(area, () => setInput(search, ""));
      commands.length = 0;
      const spent = await settleAfter(area, () => setInput(search, word));
      (typed[word] ??= []).push(spent);
      commandsPerWord[word] = commands.map(({ command, took_ms }) => ({ command, took_ms }));
    }
  }
  await settleAfter(area, () => setInput(search, ""));
  report.search_commands = commandsPerWord;
  report.search_typed_to_settled = Object.fromEntries(
    Object.entries(typed).map(([word, samples]) => [word, timing(samples)]),
  );

  const ipc: Record<string, unknown> = {};
  for (const word of TASK_WORDS) {
    ipc[`tasks: ${word}`] = await timed(() => taskApi.list(null, "priority", { kind: "all" }, { ...NO_FILTERS, text: word }), IPC_RUNS);
  }
  ipc["tasks: first page"] = await timed(() => taskApi.list(), IPC_RUNS);
  for (const word of NOTE_WORDS) {
    ipc[`notes: ${word}`] = await timed(() => noteApi.list(null, { kind: "all" }, word), IPC_RUNS);
  }
  ipc["notes: first page"] = await timed(() => noteApi.list(), IPC_RUNS);
  report.search_through_ipc = ipc;

  const allNotes = await until(() => document.querySelector<HTMLElement>('[aria-label="All notes"]'));
  const started = performance.now();
  allNotes.click();
  const list = await until(() => document.querySelector<HTMLElement>('ul[aria-label="Notes"]'));
  await until(() => list.querySelector("li"));
  await settled();
  report.open_notes_ms = round(performance.now() - started);
  const listArea = list.closest<HTMLElement>(".note-list") ?? list;
  report.scroll_note_list = drawn
    ? await scrollFrames(listArea.scrollHeight > listArea.clientHeight ? listArea : list, SCROLL_MS / 2)
    : notMeasured;

  const noteSearch = await until(() => document.querySelector<HTMLInputElement>('input[aria-label="Search notes"]'));
  const noteTyped: Record<string, number[]> = {};
  for (let round_ = 0; round_ < 5; round_ += 1) {
    for (const word of NOTE_WORDS) {
      await settleAfter(list, () => setInput(noteSearch, ""));
      const spent = await settleAfter(list, () => setInput(noteSearch, word));
      (noteTyped[word] ??= []).push(spent);
    }
  }
  report.note_search_typed_to_settled = Object.fromEntries(
    Object.entries(noteTyped).map(([word, samples]) => [word, timing(samples)]),
  );
  return report;
}

export async function run() {
  let report: Record<string, unknown>;
  try {
    report = { ok: true, ...await measure() };
  } catch (error) {
    report = {
      ok: false,
      error: String(error),
      first_row_after_load_ms: round(firstRowAt),
      commands: commands.slice(0, 40),
    };
  }
  await invoke("qa_report", { report });
}
