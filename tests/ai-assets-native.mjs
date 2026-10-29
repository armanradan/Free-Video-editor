// Real filesystem/cache CLI probe with the exact allowlisted export. All new data
// is confined to a newly created directory beneath this project's ignored tmp.
import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import assert from 'node:assert/strict';
const [binaryArg,modelArg,outputArg]=process.argv.slice(2);
if(!outputArg)throw Error('Expected native-convert binary, exported model, new output directory');
const output=path.resolve(outputArg),tempRoot=path.resolve('tmp');
if(!output.startsWith(tempRoot+path.sep))throw Error('Probe output must stay beneath project tmp');
fs.mkdirSync(output,{recursive:false});
const binary=path.resolve(binaryArg),model=path.resolve(modelArg);
const manifest=JSON.parse(fs.readFileSync('crates/media-core/src/ai-upscale-model.json','utf8'));
const run=(operation,cache,source=model)=>{
  const args=[operation,'--cache',cache];if(operation==='import-ai-model')args.push('--model',source);
  const result=spawnSync(binary,args,{encoding:'utf8',timeout:30000});
  if(result.error)throw result.error;
  return {status:result.status,stdout:result.stdout,stderr:result.stderr};
};
const cache=path.join(output,'valid-cache');
assert.equal(run('import-ai-model',cache).status,0);
const cached=path.join(cache,manifest.model.sha256+'.onnx'),mtime=fs.statSync(cached).mtimeMs;
assert.equal(run('ai-model-status',cache).status,0);
assert.equal(run('import-ai-model',cache).status,0);assert.equal(fs.statSync(cached).mtimeMs,mtime);
const corrupt=fs.readFileSync(model);corrupt[0]^=1;
const corruptPath=path.join(output,'corrupt.onnx'),shortPath=path.join(output,'truncated.onnx');
fs.writeFileSync(corruptPath,corrupt);fs.writeFileSync(shortPath,corrupt.subarray(0,100));
for(const source of [corruptPath,shortPath]){
  const target=path.join(output,path.basename(source)+'-cache');
  assert.notEqual(run('import-ai-model',target,source).status,0);assert.equal(fs.existsSync(target),false);
}
const invalidCache=path.join(output,'invalid-cache');fs.mkdirSync(invalidCache);
fs.writeFileSync(path.join(invalidCache,manifest.model.sha256+'.onnx'),corrupt);
assert.notEqual(run('ai-model-status',invalidCache).status,0);
assert.notEqual(run('import-ai-model',invalidCache).status,0);
assert.equal(run('remove-ai-model',invalidCache).status,0);
assert.equal(run('import-ai-model',invalidCache).status,0);
const occupied=path.join(output,'not-a-directory');fs.writeFileSync(occupied,'preserve this marker');
const failed=run('import-ai-model',path.join(occupied,'cache'));
assert.notEqual(failed.status,0);assert.equal(fs.readFileSync(occupied,'utf8'),'preserve this marker');
assert.deepEqual(fs.readdirSync(cache),[manifest.model.sha256+'.onnx']);
assert.deepEqual(fs.readdirSync(invalidCache),[manifest.model.sha256+'.onnx']);
const evidence={status:'native-cache-probe-passed-not-inference-acceptance',
  modelSha256:manifest.model.sha256,modelBytes:manifest.model.bytes,
  validImportAndSeparateProcessRead:true,verifiedReuseWithoutRewrite:true,
  corruptAndTruncatedSourceRejectedBeforeCacheCreation:true,corruptCacheRejected:true,
  explicitRemovalAndRecovery:true,unwritableCachePathRejected:{reason:'parent component is a regular file',stderr:failed.stderr},
  partialFilesRetained:0,sourceMediaUploads:0,runtimeInstallations:0};
fs.writeFileSync(path.join(output,'evidence.json'),JSON.stringify(evidence,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify(evidence));
