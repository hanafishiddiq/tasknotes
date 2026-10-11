import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawn } from "node:child_process";

function run(command, args, env) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      stdio: "inherit",
      env,
      shell: false,
    });
    child.on("error", reject);
    child.on("exit", (code) =>
      code === 0 ? resolve() : reject(new Error(command + " exited with " + code)),
    );
  });
}

const directory = await mkdtemp(join(tmpdir(), "tasknotes-wdio-"));
const env = {
  ...process.env,
  TASKNOTES_DEV_DATA_DIR: directory,
  TAURI_WEBDRIVER_PORT: "4445",
};

let allPassed = false;
try {
  console.log("[Tasknotes QA] Using a private synthetic database for both native sessions");
  for (const spec of [
    "qa/native/01-create-and-update.spec.ts",
    "qa/native/02-verify-reopen.spec.ts",
  ]) {
    console.log("[Tasknotes QA] Running native WebDriver:", spec);
    await run("pnpm", ["exec", "wdio", "run", "wdio.conf.ts", "--spec", spec], env);
  }
  await run("python3", ["scripts/qa/verify_native_wdio.py", directory], env);
  console.log("[Tasknotes QA] Fully automatic native acceptance passed");
  allPassed = true;
} finally {
  if (!allPassed && process.env.KEEP_QA_DATA === "1") {
    console.error("[Tasknotes QA] Failed synthetic fixture retained for diagnosis:", directory);
  } else {
    await rm(directory, { recursive: true, force: true });
  }
}
