import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawn } from "node:child_process";

const dir = await mkdtemp(join(tmpdir(), "tasknotes-wdio-perf-"));
const env = {
  ...process.env,
  TASKNOTES_DEV_DATA_DIR: dir,
  TAURI_WEBDRIVER_PORT: "4445",
};
const started = performance.now();
// A small explicit allowlist: never execute arbitrary QA paths from env.
const allowedSpecs = new Set([
  "qa/native/perf-lag.spec.ts",
  "qa/native/perf-scroll.spec.ts",
]);
const spec = process.env.PERF_SPEC || "qa/native/perf-lag.spec.ts";
if (!allowedSpecs.has(spec)) throw new Error("Unapproved performance spec");

try {
  await new Promise((resolve, reject) => {
    const child = spawn(
      "pnpm",
      ["exec", "wdio", "run", "wdio.conf.ts", "--spec", spec],
      {
        cwd: process.cwd(),
        env,
        stdio: "inherit",
      },
    );
    child.once("error", reject);
    child.once("exit", (code) =>
      code === 0
        ? resolve()
        : reject(new Error("Native perf spec exited " + code)),
    );
  });
  console.log(
    "[Tasknotes Perf] Native driver benchmark complete in " +
      ((performance.now() - started) / 1000).toFixed(2) +
      "s",
  );
} finally {
  // Never use or leave a synthetic fixture in ordinary user application data.
  await rm(dir, { recursive: true, force: true });
}
