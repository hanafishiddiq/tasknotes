import test from "node:test";
import assert from "node:assert/strict";
import {phasePlan,phaseMode,setVariant,percentile,isQaStartShortcut} from "./issue19-hover-ab.mjs";

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
