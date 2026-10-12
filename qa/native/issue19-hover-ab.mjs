/* Issue #19 synthetic-only Tasknotes hover-transition A/B.
 * Native React/Tauri/SQLite app unchanged; JS injected into QA frontend copy.
 * The baseline keeps original .task-card transitions; B disables only those.
 * Mark all four phases in one WebKit Timeline via console.timeStamp/profile.
 */
const PREFIX="__ISSUE19_HOVER_QA__";
const METRICS=PREFIX+"METRICS";
const LABELS=Object.freeze(["A1","B1","B2","A2"]);
const TICKS=200;
export const phasePlan=()=>LABELS.map(label=>({label,mode:label[0]}));
// QA app only: scoped keyboard entrypoint when WebKit AX does not expose buttons.
export const isQaStartShortcut=event=>event.code==="Enter"&&event.ctrlKey&&event.shiftKey&&
  !event.metaKey&&!event.altKey&&!event.repeat;
export function phaseMode(name){
  if(!LABELS.includes(name))throw Error("Unsupported QA phase");
  return name[0];
}
export function percentile(numbers,quantile){
  if(!numbers.length)return null;
  const a=[...numbers].sort((x,y)=>x-y);
  return Math.round(a[Math.ceil(a.length*quantile)-1]*100)/100;
}
export function setVariant(doc,mode){
  if(mode!=="A"&&mode!=="B")throw Error("Unsupported QA variant");
  doc.documentElement.dataset.issue19HoverVariant=mode;
  const card=doc.querySelector(".task-card");
  if(!card)throw Error("Missing real Tasknotes card");
  const value=doc.defaultView.getComputedStyle(card).transitionProperty;
  if(mode==="B"&&value!=="none")throw Error("B transition not disabled");
  if(mode==="A"&&(!value.includes("box-shadow")||!value.includes("border-color")))throw Error("A source transitions missing");
  return value;
}
const wait=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function until(fn,limit=20000){
  const end=performance.now()+limit;
  while(performance.now()<end){const value=await fn();if(value)return value;await wait(110)}
  throw Error("QA UI readiness timeout");
}
function check(ok,message){if(!ok)throw Error("Synthetic QA safety: "+message)}
async function init(){
  const invoke=window.__TAURI__?.core?.invoke;
  const win=window.__TAURI__?.window?.getCurrentWindow?.();
  if(typeof invoke!=="function"||!win){
    console.error("[ISSUE19-HOVER-QA] missing opt-in Tauri global API");
    return;
  }
  const stylesheet=document.createElement("style");
  stylesheet.dataset.issue19HoverQa="only";
  stylesheet.textContent=[
    ".task-card.issue19-force-hover {border-color:#dcd9f0;box-shadow:0 5px 15px #42428c0d;}",
    'html[data-issue19-hover-variant="B"] .task-card {transition:none !important;}',
    "#issue19-hover-controls {position:fixed;bottom:8px;left:8px;z-index:2147483000;",
    "background:#fff;color:#111;border:1px solid #888;border-radius:7px;",
    "padding:9px;max-width:275px;font:12px/1.4 system-ui,sans-serif}",
    "#issue19-hover-controls button {font:12px system-ui;background:#eee;",
    "color:#222;border:1px solid #aaa;padding:7px;border-radius:4px}",
    "#issue19-hover-controls button:disabled {opacity:.5}"
  ].join("");
  document.head.append(stylesheet);
  const control=document.createElement("aside");
  control.id="issue19-hover-controls";
  control.innerHTML=[
    '<b>Issue19 · Hover CSS A/B</b>',
    '<p role="status" id="issue19-hover-status">Preparing synthetic Tasks…</p>',
    '<button type="button" id="issue19-hover-start" disabled>',
    'Start one A–B–B–A trace</button>',
    '<p>QA shortcut: Ctrl+Shift+Enter (while this window is focused)</p>',
    '<p id="issue19-hover-results"></p>'
  ].join("");
  document.body.append(control);
  const button=control.querySelector("button");
  const status=control.querySelector("#issue19-hover-status");
  const results=control.querySelector("#issue19-hover-results");
  const label=s=>{status.textContent=s;document.title="Issue19 Hover QA — "+s.slice(0,76)};
  let metric=null, busy=false;
  const api=(command,args)=>invoke(command,args);

  async function fixture(){
    await until(()=>document.querySelector('select[aria-label="Sort tasks"]'));
    let page=await api("list_tasks",{limit:100,cursor:null,sort:"priority"});
    check(!page.nextCursor,"too many tasks");
    check(page.items.every(t=>t.title.startsWith(PREFIX)),"non-synthetic Task encountered");
    if(page.items.length===0){
      for(let i=0;i<31;i++){
        await api("create_task",{input:{
          title:PREFIX+"TASK-"+String(i).padStart(2,"0"),
          description:"Synthetic issue19 scrolling row "+i
        }});
      }
      metric=await api("create_task",{input:{title:METRICS,description:JSON.stringify({schema:1,runs:[]})}});
      const select=document.querySelector('select[aria-label="Sort tasks"]');
      select.value=select.value==="priority"?"newest":"priority";
      select.dispatchEvent(new Event("input",{bubbles:true}));
      select.dispatchEvent(new Event("change",{bubbles:true}));
    }else{
      check(page.items.length===32,"wrong synthetic fixture count");
      metric=page.items.find(t=>t.title===METRICS);
      check(metric&&metric.id,"missing metrics Task");
    }
    await until(()=>document.querySelectorAll("article.task-card").length===32);
    check(document.scrollingElement.scrollHeight>innerHeight+500,"page not scrollable");
    setVariant(document,"A");
    button.disabled=false;
    label("Ready • attach Safari Inspector, then press Start once");
  }

  async function phase(name){
    const mode=name[0];
    document.querySelectorAll(".issue19-force-hover").forEach(e=>e.classList.remove("issue19-force-hover"));
    setVariant(document,mode);
    window.scrollTo({top:0,behavior:"instant"});
    await wait(350);
    let count=0,wheel=0,moved=0,focusLost=false,timedOut=false;
    const frameGaps=[],events={run:0,start:0,end:0,cancel:0},properties={};
    let lastFrame=null,rafId=null,live=true,lastHover=null;
    function frame(now){
      if(!live)return;
      if(lastFrame!==null)frameGaps.push(now-lastFrame);
      lastFrame=now;rafId=requestAnimationFrame(frame);
    }
    function onTransition(e){
      if(!e.target?.classList?.contains("task-card"))return;
      const key=e.type.replace("transition","");
      if(Object.hasOwn(events,key))events[key]++;
      if(e.type==="transitionrun")properties[e.propertyName]=(properties[e.propertyName]||0)+1;
    }
    const names=["transitionrun","transitionstart","transitionend","transitioncancel"];
    names.forEach(n=>document.addEventListener(n,onTransition,true));
    const onWheel=()=>wheel++;
    let beforeY=window.scrollY;
    const onScroll=()=>{moved+=Math.abs(window.scrollY-beforeY);beforeY=window.scrollY};
    window.addEventListener("wheel",onWheel,{passive:true});
    window.addEventListener("scroll",onScroll,{passive:true});
    rafId=requestAnimationFrame(frame);
    function marker(suffix){
      const markerName="ISSUE19_HOVER_"+name+"_"+suffix;
      console.timeStamp(markerName);
      window.addEventListener(markerName,()=>{}, {once:true});
      window.dispatchEvent(new Event(markerName));
      return {name:markerName,pageSeconds:performance.now()/1000};
    }
    const start=marker("BEGIN");
    let timer;
    await new Promise(resolve=>{
      const max=setTimeout(()=>{
        timedOut=true;
        clearInterval(timer);resolve();
      },9500);
      timer=setInterval(()=>{
        if(document.visibilityState!=="visible"||!document.hasFocus()){
          focusLost=true;
          clearInterval(timer);clearTimeout(max);resolve();return;
        }
        const delta=count<TICKS/2?26:-26;
        window.dispatchEvent(new WheelEvent("wheel",{
          deltaY:delta,bubbles:true,cancelable:true
        }));
        window.scrollBy(0,delta);
        // Same stationarity/hover-proxy algorithm in BOTH A and B.
        // Real pointer trackpad behavior is not claimed by this synthetic load.
        if(count%5===0){
          const card=document.elementFromPoint(
            Math.floor(innerWidth*.65),Math.floor(innerHeight*.63)
          )?.closest?.(".task-card");
          if(card&&card!==lastHover){
            lastHover?.classList.remove("issue19-force-hover");
            card.classList.add("issue19-force-hover");
            lastHover=card;
          }
        }
        count++;
        if(count>=TICKS){clearInterval(timer);clearTimeout(max);resolve()}
      },16);
    });
    await wait(160);
    const end=marker("END");
    live=false;if(rafId!==null)cancelAnimationFrame(rafId);
    names.forEach(n=>document.removeEventListener(n,onTransition,true));
    window.removeEventListener("wheel",onWheel);
    window.removeEventListener("scroll",onScroll);
    lastHover?.classList.remove("issue19-force-hover");
    const record={
      phase:name,mode,start,end,ticks:count,
      wheelEvents:wheel,travelPx:Math.round(moved),
      focusLost,timedOut,
      rAFSamples:frameGaps.length,
      rAFP50Ms:percentile(frameGaps,.50),
      rAFP95Ms:percentile(frameGaps,.95),
      transitions:events,transitionProperties:properties
    };
    record.valid=(!focusLost&&!timedOut&&count===TICKS&&
      frameGaps.length>=12&&moved>200&&
      (mode==="A"?events.run>0:events.run===0));
    console.info("[ISSUE19-HOVER-PHASE]",JSON.stringify(record));
    return record;
  }

  async function persist(result){
    check(metric?.id,"no QA metrics task");
    const current=JSON.parse(metric.description||'{"runs":[]}');
    const runs=[...(current.runs||[]),result];
    check(runs.length<=10,"no more than ten QA runs");
    metric=await api("update_task",{input:{
      id:metric.id,title:METRICS,
      description:JSON.stringify({schema:1,runs}),
      expectedRevision:metric.revision
    }});
  }

  async function execute(){
    if(busy)return;
    busy=true;button.disabled=true;
    const output={schema:1,startedAt:Date.now(),phases:[],valid:false};
    try{
      try{await win.show();await win.setFocus()}catch{}
      check(document.visibilityState==="visible"&&document.hasFocus(),
        "native window must be visible and focused");
      label("Starting single Safari trace in 3 seconds…");
      await wait(3000);
      console.profile("ISSUE19_HOVER_ABBA");
      try{
        for(const name of LABELS){
          label("Phase "+name+" running…");
          const result=await phase(name);
          output.phases.push(result);
          if(!result.valid){label("INCONCLUSIVE: phase "+name);break}
          await wait(250);
        }
      }finally{console.profileEnd("ISSUE19_HOVER_ABBA")}
      output.valid=output.phases.length===4&&output.phases.every(p=>p.valid);
      output.completedAt=Date.now();
      await persist(output);
      results.textContent=output.valid
        ?"4/4 phases valid. Export ONE Safari timeline."
        :"INCONCLUSIVE. No performance claim.";
      label(output.valid?"Completed A–B–B–A: export one Safari trace":"INCONCLUSIVE: focus or events");
    }catch(e){
      output.error=String(e?.message||e).slice(0,175);
      try{await persist(output)}catch{}
      console.error("[ISSUE19-HOVER-QA-ERROR]",output.error);
      label("INCONCLUSIVE: "+output.error);
    }finally{
      busy=false;button.disabled=false;
      document.documentElement.dataset.issue19HoverVariant="A";
      document.querySelectorAll(".issue19-force-hover").forEach(e=>e.classList.remove("issue19-force-hover"));
    }
  }
  button.addEventListener("click",()=>void execute());
  window.addEventListener("keydown",event=>{
    if(!button.disabled&&isQaStartShortcut(event)){
      event.preventDefault();
      void execute();
    }
  });
  await fixture();
  if(new URLSearchParams(location.search).get("issue19-autorun")==="1"){
    // Exercise one end-to-end QA pass autonomously; invalid focus is rejected
    // and the Start button remains available for the real Safari capture.
    setTimeout(()=>{void execute()},4000);
  }
}
if(typeof window!=="undefined"){
  init().catch(error=>{
    console.error("[ISSUE19-HOVER-QA-INIT]",String(error));
    document.title="Issue19 QA initialization error";
  });
}
