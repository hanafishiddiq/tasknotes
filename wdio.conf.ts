import { basename, resolve } from "node:path";
import { tmpdir } from "node:os";

const testDir = process.env.TASKNOTES_DEV_DATA_DIR;
if (!testDir || !resolve(testDir).startsWith(resolve(tmpdir())) ||
    !basename(testDir).startsWith("tasknotes-wdio-")) {
  throw new Error("Native E2E tests require a private synthetic directory in TMPDIR");
}

const binary = resolve("src-tauri/target/debug/tasknotes");

export const config = {
  runner: "local",
  specs: ["./qa/native/*.spec.ts"],
  maxInstances: 1,
  logLevel: "warn",
  framework: "mocha",
  reporters: ["spec"],
  capabilities: [{
    browserName: "tauri",
    "tauri:options": { application: binary },
  }],
  services: [[
    "@wdio/tauri-service",
    {
      appBinaryPath: binary,
      driverProvider: "embedded",
      embeddedPort: 4445,
    },
  ]],
  waitforTimeout: 20000,
  connectionRetryTimeout: 30000,
  connectionRetryCount: 1,
  mochaOpts: {
    ui: "bdd",
    timeout: 90000,
  },
};
