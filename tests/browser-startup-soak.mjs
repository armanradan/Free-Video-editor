import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

// Bounded reload regression, not proof that arbitrary browser/driver stalls are
// eliminated. The real embedded M1 decoder→GPU→encoder→decoder runs every time.
export async function runStartupSoak({send,navigate,click,directory,url='http://127.0.0.1:8084/',trials=6}) {
  assert.ok(Number.isInteger(trials)&&trials>0&&trials<=20);
  fs.mkdirSync(directory,{recursive:true});
  const evidence={status:'running',trials,cases:[]};
  const save=()=>fs.writeFileSync(path.join(directory,'evidence.json'),JSON.stringify(evidence,null,2));
  const evaluate=async expression=>{
    const value=await send('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});
    assert.ok(!value.exceptionDetails,JSON.stringify(value.exceptionDetails));return value.result.value;
  };
  const wait=async expression=>{
    const deadline=Date.now()+45000;
    do {const value=await evaluate(expression);if(value)return value;await new Promise(r=>setTimeout(r,50));}while(Date.now()<deadline);
    throw Error(`Startup/M1 timeout: ${await evaluate("document.querySelector('#status')?.textContent")}`);
  };
  try {
    for(let i=0;i<trials;i++) {
      await navigate(url);
      await wait("!!document.querySelector('details.regression button')");
      await click('details.regression summary');await click('details.regression button');
      const summary=await wait("/^(PASS|FAILED|CANCELLED):/.test(document.querySelector('#status')?.textContent)&&document.querySelector('#status').textContent");
      evidence.cases.push({trial:i+1,summary});save();
      assert.match(summary,/^PASS: 30\/30/);
      assert.match(summary,/Execution: dedicated worker/);
      assert.match(summary,/Startup stages .*WebGPU adapter request=.*WebGPU device request=.*ready=/);
      assert.match(summary,/Cleanup: 0 application-owned live frames/);
      assert.match(summary,/0 explicit CPU pixel readbacks in conversion/);
    }
    evidence.status='passed';return evidence;
  }catch(error){evidence.status='failed';evidence.error=String(error.stack);throw error;}
  finally{save();}
}
