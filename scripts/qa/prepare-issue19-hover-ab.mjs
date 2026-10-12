// Build a native QA app that preserves Tasknotes' real compiled frontend.
// Only the ignored copy gets injected with the test-only hover A/B script.
import {execFileSync} from "node:child_process";
import {readFile,cp,writeFile,mkdir,rm} from "node:fs/promises";
import {join,resolve} from "node:path";
import {createHash} from "node:crypto";

const root=process.cwd();
const out=resolve(root,"src-tauri/target/issue19-hover-ab");
const frontend=join(out,"frontend");
const appId="store.adamaja.tasknotes.issue19hoverab";
const qaName="Tasknotes Issue19 Hover A B QA";
const hash=data=>createHash("sha256").update(data).digest("hex");
const normal=JSON.parse(await readFile("src-tauri/tauri.conf.json","utf8"));
if(normal.identifier===appId)throw Error("Refusing to reuse personal app identifier");
const css=await readFile("src/App.css","utf8");
for(const needle of [".task-card:hover","border-color 0.15s","box-shadow 0.15s"]){
  if(!css.includes(needle))throw Error("Expected baseline CSS is missing: "+needle);
}
const base=await readFile("qa/native/issue19-hover-ab.mjs");
if(!base.includes('ISSUE19_HOVER_')||!base.includes("transition:none !important")){
  throw Error("Wrong versioned QA script");
}
execFileSync("pnpm",["build"],{cwd:root,stdio:"inherit",env:{...process.env,VITE_NATIVE_E2E:"0"}});
await mkdir(out,{recursive:true});
if(await import("node:fs").then(fs=>fs.existsSync(frontend)))await rm(frontend,{recursive:true,force:true});
await cp(join(root,"dist"),frontend,{recursive:true});
const indexPath=join(frontend,"index.html");
const index=await readFile(indexPath,"utf8");
if(!index.includes("</body>")||index.includes("issue19-"))throw Error("Not a pristine Tasknotes frontend");
const markup=index.replace("</body>",'<script type="module" src="./issue19-hover-ab.mjs"></script></body>');
await writeFile(indexPath,markup);
await writeFile(join(frontend,"issue19-hover-ab.mjs"),base);
const config={
  productName:qaName,
  identifier:appId,
  build:{beforeBuildCommand:"",frontendDist:"./target/issue19-hover-ab/frontend"},
  app:{
    withGlobalTauri:true,
    windows:[{
      label:"main",title:qaName,width:1120,height:740,
      minWidth:760,minHeight:540,center:true,visible:true,
      // Default to a visible one-click Start after Safari Inspector attaches;
      // opt-in ?issue19-autorun=1 remains supported for unattended QA.
      url:"index.html"
    }],
    security:{capabilities:["default","issue19-bench"]}
  }
};
await writeFile(join(out,"hover-ab.conf.json"),JSON.stringify(config,null,2)+"\n");
const manifest={
  appId,productName:qaName,
  frontendHtmlSha256:hash(markup),
  qaScriptSha256:hash(base),
  sourceCssSha256:hash(css),
  phaseSequence:["A1","B1","B2","A2"],
  onlyDifference:'html[data-issue19-hover-variant="B"] .task-card {transition:none !important;}'
};
await writeFile(join(out,"manifest.json"),JSON.stringify(manifest,null,2)+"\n");
console.log("[ISSUE19-HOVER-QA] prepared isolated copied frontend + one CSS-variant change");
console.log("[ISSUE19-HOVER-QA] hashes",manifest.frontendHtmlSha256,manifest.qaScriptSha256);
console.log("[ISSUE19-HOVER-QA] synthetic-only identifier",appId);
