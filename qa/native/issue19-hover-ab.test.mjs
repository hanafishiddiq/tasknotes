import test from "node:test";
import assert from "node:assert/strict";
import {phaseMarkerName,phasePlan,phaseMode,setVariant,percentile,isQaStartShortcut,isTrustedWheelEvent,validPhysicalPhase,comparablePhysicalPhases} from "./issue19-hover-ab.mjs";

function fakeDocument(){
  const card={className:"task-card"};
  const doc={
    documentElement:{dataset:{}},
    querySelector: selector=>selector===".task-card"?card:null
  };
  doc.defaultView={getComputedStyle:()=>({
    transitionProperty:doc.documentElement.dataset.issue19HoverVariant==="B"
      ?"none":"border-color, box-shadow"
  })};
  return doc;
}


test("trusted trackpad gate never counts scripted WheelEvent objects",()=>{
  assert.equal(isTrustedWheelEvent({type:"wheel",isTrusted:true}),true);
  assert.equal(isTrustedWheelEvent({type:"wheel",isTrusted:false}),false);
  assert.equal(isTrustedWheelEvent({type:"scroll",isTrusted:true}),false);
  assert.equal(isTrustedWheelEvent(null),false);
});
test("physical-scroll phase requires real wheel input, focus, travel and expected card transitions",()=>{
  const p={mode:"A",focusLost:false,timedOut:false,rAFSamples:40,
    wheelEvents:65,trustedWheelEvents:65,travelPx:1200,transitions:{run:25}};
  assert.equal(validPhysicalPhase(p),true);
  assert.equal(validPhysicalPhase({...p,mode:"B",transitions:{run:0}}),true);
  assert.equal(validPhysicalPhase({...p,trustedWheelEvents:0}),false);
  assert.equal(validPhysicalPhase({...p,wheelEvents:70}),false);
  assert.equal(validPhysicalPhase({...p,focusLost:true}),false);
  assert.equal(validPhysicalPhase({...p,travelPx:150}),false);
  assert.equal(validPhysicalPhase({...p,rAFSamples:5}),false);
  assert.equal(validPhysicalPhase({...p,mode:"B"}),false);
});
test("four physical phases must have comparable real wheel workload",()=>{
  const phases=[{phase:"A1",trustedWheelEvents:60,travelPx:1100},
    {phase:"B1",trustedWheelEvents:70,travelPx:1200},
    {phase:"B2",trustedWheelEvents:65,travelPx:1150},
    {phase:"A2",trustedWheelEvents:61,travelPx:1200}];
  assert.equal(comparablePhysicalPhases(phases),true);
  assert.equal(comparablePhysicalPhases(phases.map((p,i)=>i===1?{...p,trustedWheelEvents:200}:p)),false);
  assert.equal(comparablePhysicalPhases(phases.map((p,i)=>i===2?{...p,travelPx:4500}:p)),false);
  assert.equal(comparablePhysicalPhases(phases.slice(1)),false);
});

test("opt-in QA shortcut starts only on scoped Ctrl+Shift+Enter",()=>{
  const base={code:"Enter",ctrlKey:true,shiftKey:true,metaKey:false,altKey:false,repeat:false};
  assert.equal(isQaStartShortcut(base),true);
  assert.equal(isQaStartShortcut({...base,code:"Space"}),false);
  assert.equal(isQaStartShortcut({...base,ctrlKey:false}),false);
  assert.equal(isQaStartShortcut({...base,metaKey:true}),false);
  assert.equal(isQaStartShortcut({...base,repeat:true}),false);
});

test("one-variable A-B-B-A plan with precisely two variants",()=>{
  assert.deepEqual(phasePlan(),[
    {label:"A1",mode:"A"},{label:"B1",mode:"B"},
    {label:"B2",mode:"B"},{label:"A2",mode:"A"}
  ]);
  assert.equal(phaseMode("B2"),"B");
  assert.throws(()=>phaseMode("C1"),/Unsupported/);
});
test("only B suppresses .task-card transitions; A restores original",()=>{
  const doc=fakeDocument();
  assert.match(setVariant(doc,"A"),/box-shadow/);
  assert.equal(doc.documentElement.dataset.issue19HoverVariant,"A");
  assert.equal(setVariant(doc,"B"),"none");
  assert.equal(doc.documentElement.dataset.issue19HoverVariant,"B");
  assert.match(setVariant(doc,"A"),/border-color/);
  assert.throws(()=>setVariant(doc,"C"),/Unsupported/);
});
test("nearest-rank statistics and empty samples are deterministic",()=>{
  assert.equal(percentile([50,10,20,15,40],.95),50);
  assert.equal(percentile([50,10,20,15,40],.5),20);
  assert.equal(percentile([],.95),null);
});

test("native phase marker protocol binds exact trial without changing A-B-B-A labels",()=>{
  const id="11111111111111111111111111111111";
  assert.equal(phaseMarkerName("A1","BEGIN",id),
    "ISSUE19_HOVER_A1_BEGIN_11111111111111111111111111111111");
  assert.equal(phaseMarkerName("B2","END",id),
    "ISSUE19_HOVER_B2_END_11111111111111111111111111111111");
  assert.throws(()=>phaseMarkerName("C1","BEGIN",id),/Unsupported/);
  assert.throws(()=>phaseMarkerName("A1","START",id),/Unsupported/);
  assert.throws(()=>phaseMarkerName("A1","BEGIN",""),/Unsupported/);
});
