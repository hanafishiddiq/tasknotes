/**
 * Issue 19: prepare IDENTICAL synthetic Tasknotes assets for native parent/
 * child WebViews. Does not rebuild or modify ordinary Tasknotes distribution.
 */
import { cp, readFile, writeFile, mkdir } from "node:fs/promises";
import { resolve, join } from "node:path";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";

const root = process.cwd();
const qa = resolve(root, "src-tauri/target/issue19-placement");
const frontend = join(qa, "frontend");
// Prepare from versioned source, not an ignored target/ folder from a past session.
// The exact same generated HTML+JS is then shared by all runtime modes.
const source = join(root, "dist");
const appId = "store.adamaja.tasknotes.issue19placement";
const original = JSON.parse(await readFile(join(root, "src-tauri/tauri.conf.json"), "utf8"));
if (appId === original.identifier) throw Error("Refusing the user Tasknotes bundle identifier");
const hash = (data) => createHash("sha256").update(data).digest("hex");

execFileSync("pnpm", ["build"], {
  cwd: root, stdio: "inherit", env: {...process.env, VITE_NATIVE_E2E: "0"}
});
await mkdir(qa, {recursive:true});
await cp(source, frontend, {recursive:true,force:true});
const index = await readFile(join(frontend, "index.html"));
const probe = await readFile(join(root,"qa/native/main-child-scroll-probe.js"));
await writeFile(join(frontend,"issue19-placement-probe.js"),probe);
if (!index.toString().includes("</body>")) {
  throw new Error("QA frontend is missing </body> insertion anchor");
}
if (index.toString().includes("issue19-")) {
  throw new Error("Expected an ordinary, not previously instrumented, frontend build");
}
const updatedIndex = index.toString().replace(
  "</body>", '<script defer src="./issue19-placement-probe.js"></script></body>'
);
await writeFile(join(frontend,"index.html"),updatedIndex);
if (!updatedIndex.includes("issue19-placement-probe.js")) {
  throw Error("Expected verified local synthetic benchmark script in exact copied frontend");
}
const manifest = {
  source: "normal pnpm build / dist copied into QA-only bundle",
  htmlSha256: hash(Buffer.from(updatedIndex)),
  probeSha256: hash(probe),
  applicationId: appId,
  placementModes: ["top", "child", "child-no-throttle"],
};
const config = {
  productName: "Tasknotes Issue19 Placement Bench",
  identifier: appId,
  build: {
    beforeBuildCommand: "",
    frontendDist: "./target/issue19-placement/frontend",
  },
  app: {
    windows: [], // Both variants constructed by ONE feature-gated Rust setup.
    withGlobalTauri: true,
    security: { capabilities: ["default", "issue19-bench"] },
  },
};
await writeFile(join(qa,"manifest.json"),JSON.stringify(manifest,null,2)+"\n");
await writeFile(join(qa,"issue19-placement.conf.json"),JSON.stringify(config,null,2)+"\n");
console.log("[Issue19 placement] Shared HTML SHA256",manifest.htmlSha256);
console.log("[Issue19 placement] Shared JS SHA256",manifest.probeSha256);
console.log("[Issue19 placement] Config has 0 automatic windows; both modes use same Rust binary.");
console.log("[Issue19 placement] Synthetic QA bundle id:",appId);
