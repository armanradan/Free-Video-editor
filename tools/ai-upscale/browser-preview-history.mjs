// Isolated real-browser probe; chronological export is the temporal reference.
export async function runAiPreviewHistoryProbe(wasm,result,publish) {
  const fixture='ai-upscale-cut.mp4';
  const file=new File([await(await fetch(`/fixture/${fixture}`)).arrayBuffer()],fixture,{lastModified:1234});
  const command=(scale,strength=50,model='realesr-general-x4v3-strong-fp32-v1')=>
    `original~recommended~original~10/115/80~1/${strength}~onnx-runtime/${model}/${scale}`;
  globalThis.__DIAXUS_AI_INGRESS_PARITY__=true;
  globalThis.__DIAXUS_PARITY_FRAMES__='0,1,2,3';
  result.previewCases=[];
  const preview=async(label,seconds,scale,strength=50,before=false)=>{
    result.boundary=`history ${label}`;publish();
    const value=await wasm.execute_job(file,before?'preview-ai-before-pilot':'preview-ai-pilot',
      `ai-history-source@${seconds}@0`,'',command(scale,strength),'memory:0:none',()=>{});
    const ownership=JSON.parse(wasm.ownership());
    const expectedPts=seconds===.24?1483333:seconds===.066667?1316666:1350000;
    const expectedPredecessors=seconds===.24?0:seconds===.066667?1:2;
    if(value.parity?.sourceTimestamp!==expectedPts)throw Error(`Wrong selected PTS ${label}: ${value.parity?.sourceTimestamp} != ${expectedPts}`);
    if(!before&&!value.summary.includes(`preroll: ${expectedPredecessors} predecessor frames`))
      throw Error(`Wrong finite history ${label}: ${value.summary}`);
    if(ownership.liveLeases||ownership.pendingSubmissions)throw Error('Undrained history submissions');
    if(!value.parity)throw Error('No temporal comparison pixels');
    const item={request:{label,seconds,scale,strength,before,fixture},summary:value.summary,
      ownership,parityFrames:[{...value.parity,index:0}]};
    result.previewCases.push(item);return item;
  };
  await preview('cut-with-boundary-history',.1,'4x');
  await preview('seek-back',.066667,'4x');
  await preview('gap-expires-history',.24,'4x');
  await preview('seek-back-to-cut',.1,'4x');
  const edited=await preview('strength-edit',.1,'4x',80);
  const before=await preview('before',.1,'4x',80,true);
  const restored=await preview('after-restore',.1,'4x',80);
  if(edited.ownership.aiProcessedFrames!==before.ownership.aiProcessedFrames||
    edited.ownership.aiProcessedFrames!==restored.ownership.aiProcessedFrames)
    throw Error('Before/After reran selected source inference');
  await preview('scale-change',.1,'2x',80);
  await preview('scale-restore',.1,'4x',80);
  let invalidModelError;
  try {await wasm.execute_job(file,'preview-ai-pilot','ai-history-source@0.1@0','',
    command('4x',80,'unvalidated-model'),'memory:0:none',()=>{});}
  catch(error){invalidModelError=String(error);}
  if(!invalidModelError)throw Error('Unvalidated model accepted');
  result.invalidModelError=invalidModelError;
  await preview('validated-model-restore',.1,'4x',80);
  await wasm.execute_job(null,'clear-preview','','','original','memory:0:none',()=>{});
  for(const scale of ['2x','4x'])for(const strength of [50,80]){
    result.boundary=`chronological ${scale} strength ${strength}`;publish();
    globalThis.__DIAXUS_PREVIEW_DIAGNOSTIC_ACTIVE__=true;
    const conversion=await wasm.execute_job(file,'convert-ai-pilot','mp4-h264-aac',
      'no-preference',command(scale,strength),'memory:1:none',()=>{});
    const bytes=new Uint8Array(await conversion.blob.arrayBuffer());delete conversion.blob;
    result.artifacts.push({name:`video-${result.context}-${result.cases.length}.mp4`,bytes:Array.from(bytes)});
    result.cases.push({request:{fixture,scale,strength,fps:'original',clahe:true},...conversion});
  }
  result.finalOwnership=JSON.parse(wasm.ownership());
  if(result.finalOwnership.aiCreditHeld||result.finalOwnership.aiVideoRetained||
    result.finalOwnership.liveLeases||result.finalOwnership.pendingSubmissions)
    throw Error('Temporal preview/export retained ownership');
  result.previewStatus='finite-history-ai-preview-pilot';
}
