// Serialized into the standalone main/worker diagnostic; no application import.
export async function runAiPreviewProbe(wasm,result,publish,fixture='ai-upscale-vfr.mp4',scale='4x') {
  const file=new File([await(await fetch(`/fixture/${fixture}`)).arrayBuffer()],
    fixture,{lastModified:1234});
  if(!['2x','4x'].includes(scale))throw Error('Invalid preview pilot scale');
  const ai=`onnx-runtime/realesr-general-x4v3-strong-fp32-v1/${scale}`;
  const command=(item,identity=ai,resize='original')=>`${resize}~recommended~original~${item.color??'0/100/100'}~${item.clahe?`1/${item.strength??50}`:'0/50'}~${identity}`;
  const variants=[{label:'neutral',clahe:false},{label:'manual',clahe:false,color:'10/115/80'},
    {label:'clahe-50',clahe:true,strength:50,color:'10/115/80'},
    {label:'clahe-80',clahe:true,strength:80,color:'10/115/80'}];
  result.previewCases=[];globalThis.__DIAXUS_PARITY_FRAMES__='0';
  globalThis.__DIAXUS_AI_INGRESS_PARITY__=true;
  const preview=async(item,{before=false,identity=ai,resize='original',label=item.label}={})=>{
    result.boundary=`preview ${label}`;publish();
    const value=await wasm.execute_job(file,before?'preview-ai-before-pilot':'preview-ai-pilot',
      'ai-preview-source@0@0','',command(item,identity,resize),'memory:0:none',()=>{});
    const ownership=JSON.parse(wasm.ownership());
    if(ownership.liveLeases||ownership.pendingSubmissions)throw Error('Preview submissions did not drain');
    if(identity!=='off'&&(!ownership.aiPreviewCached||ownership.aiProcessedFrames!==1))
      throw Error(`Preview reran or missed its selected-frame cache: ${JSON.stringify(ownership)}`);
    if(identity==='off'&&(ownership.aiCreditHeld||ownership.aiVideoRetained||ownership.aiPreviewCached))
      throw Error('AI Off retained preview inference ownership');
    if(!value.parity)throw Error('Missing test-only preview snapshot');
    const itemResult={request:{...item,label,fixture:file.name,scale:identity==='off'?'off':scale,before},
      summary:value.summary,ownership,parityFrames:[{...value.parity,index:0}]};
    result.previewCases.push(itemResult);return itemResult;
  };
  for(const item of variants)await preview(item);
  await preview(variants[3],{before:true,label:'before'});
  await preview(variants[3],{label:'after-restore'});
  globalThis.__DIAXUS_BACKEND__='ffmpeg-wasm';
  globalThis.__DIAXUS_FFMPEG_ASSETS__={classWorkerURL:'/ffmpeg-worker.js',coreURL:'/ffmpeg-core.js',wasmURL:'/ffmpeg-core.wasm'};
  result.rejectedExport=await wasm.execute_job(file,'probe','','',command(variants[3]),'auto:0:none',()=>{});
  if(result.rejectedExport.mp4Supported||!/constant.*frame rate/i.test(result.rejectedExport.mp4Reason))
    throw Error(`Expected Original-VFR FFmpeg export rejection: ${JSON.stringify(result.rejectedExport)}`);
  await preview(variants[3],{label:'preview-with-rejected-ffmpeg-export'});
  await preview({label:'off',clahe:false},{identity:'off',resize:'percent:50'});
  delete globalThis.__DIAXUS_BACKEND__;
  await wasm.execute_job(null,'clear-preview','','','original','memory:0:none',()=>{});
  result.afterClear=JSON.parse(wasm.ownership());
  for(const item of variants){
    result.boundary=`matching export ${item.label}`;publish();
    globalThis.__DIAXUS_PREVIEW_DIAGNOSTIC_ACTIVE__=true;
    const conversion=await wasm.execute_job(file,'convert-ai-pilot','mp4-h264-aac','no-preference',command(item),'memory:1:none',()=>{});
    const bytes=new Uint8Array(await conversion.blob.arrayBuffer());delete conversion.blob;
    result.artifacts.push({name:`video-${result.context}-${result.cases.length}.mp4`,bytes:Array.from(bytes)});
    result.cases.push({request:{...item,fixture:file.name,scale,fps:'original'},...conversion});
  }
  result.finalOwnership=JSON.parse(wasm.ownership());
  if(result.finalOwnership.aiCreditHeld||result.finalOwnership.aiVideoRetained||result.finalOwnership.liveLeases||
    result.finalOwnership.pendingSubmissions||result.finalOwnership.aiPreviewCached)throw Error('Preview/export handoff retained AI ownership');
  result.previewStatus='tiny-preview-cache-before-ffmpeg-independence-pilot';
}
