import { cp, readFile, writeFile, mkdir } from "node:fs/promises";
import { join, resolve } from "node:path";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";

const root = process.cwd();
const target = resolve(root, "src-tauri/target/issue19-native-ab");
const assets = join(target, "frontend");
const config = join(target, "issue19-ab.conf.json");
const appId = "store.adamaja.tasknotes.issue19benchmark";
const base = JSON.parse(await readFile(join(root, "src-tauri/tauri.conf.json"), "utf8"));
if (base.identifier === appId) throw Error("QA identifier must differ from actual user app");
const digest = (data) => createHash("sha256").update(data).digest("hex");

await execFileSync("pnpm", ["build"], {cwd:root, stdio:"inherit"});
await mkdir(target, {recursive:true});
await cp(join(root,"dist"), assets, {recursive:true,force:true});
const indexPath = join(assets,"index.html");
const html = await readFile(indexPath,"utf8");
if (!html.includes("</body>")) throw Error("No body tag for QA-only test script injection");
const nextHtml = html.replace("</body>", '<script defer src="./issue19-scroll-probe.js"></script></body>');
await writeFile(indexPath, nextHtml);
const probe = await readFile(join(root,"qa/native/release-debug-scroll-probe.js"));
await writeFile(join(assets,"issue19-scroll-probe.js"), probe);

const conf = {
  productName:"Tasknotes Issue19 Bench",
  identifier:appId,
  build:{beforeBuildCommand:"",frontendDist:"./target/issue19-native-ab/frontend"},
  app:{
    withGlobalTauri:true,
    windows:[{label:"main",title:"Tasknotes Issue19 QA",width:1120,height:740,
      minWidth:760,minHeight:540,center:true,visible:true}],
    security:{capabilities:["default","issue19-bench"]}
  }
};
await writeFile(config,JSON.stringify(conf,null,2)+"\n");
const manifest={appId,htmlSha256:digest(nextHtml),probeSha256:digest(probe),
  compiledFrontendPath:assets,configPath:config};
await writeFile(join(target,"manifest.json"),JSON.stringify(manifest,null,2)+"\n");
console.log("[Issue19 A/B] Prepared exact same frontend assets for debug + release");
console.log("[Issue19 A/B] SHA256:",manifest.htmlSha256, manifest.probeSha256);
console.log("[Issue19 A/B] QA-only identifier:",appId);
