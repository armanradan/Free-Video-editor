// Actual AI preview/conversion command boundaries; diagnostic-only pixel IO.
export async function runAiPreviewLifecycleProbe(wasm,result,publish) {
  const fixture='ai-upscale-cut.mp4';
  const file=new File([await(await fetch(`/fixture/${fixture}`)).arrayBuffer()],fixture,{lastModified:1234});
  const state=globalThis.__aiFailure;
  const command=scale=>`original~recommended~original~10/115/80~1/50~onnx-runtime/realesr-general-x4v3-strong-fp32-v1/${scale}`;
  globalThis.__DIAXUS_AI_INGRESS_PARITY__=true;
  globalThis.__DIAXUS_PARITY_FRAMES__='0,1,2,3';
  result.previewCases=[];result.lifecycleCases=[];
  const ownership=()=>JSON.parse(wasm.ownership());
  const empty=()=>{
    const owned=ownership(),runtime=state.snapshot();
    if(owned.liveLeases||owned.pendingSubmissions||owned.aiCreditHeld||owned.aiVideoRetained||owned.aiPreviewCached||
      runtime.activeRuns||runtime.callerTensors||runtime.resources.length||
      runtime.sessions.some(item=>!item.released||item.busy||item.active||item.sessionRetained))
      throw Error(`Unreleased ownership: ${JSON.stringify({owned,runtime})}`);
    return {owned,runtime};
  };
  const preview=async(scale,label)=>{
    result.boundary=`preview lifecycle ${scale} ${label}`;publish();
    const value=await wasm.execute_job(file,'preview-ai-pilot','lifecycle-source@0.1@0','',command(scale),'memory:0:none',()=>{});
    if(!value.parity?.data||!ownership().aiPreviewCached)throw Error('Missing enhanced preview/cache');
    result.previewCases.push({request:{fixture,scale,label},summary:value.summary,ownership:ownership(),parityFrames:[{...value.parity,index:0}]});
    return value.parity.data;
  };
  const convert=async(scale,label)=>{
    result.boundary=`implicit handoff ${scale} ${label}`;publish();
    globalThis.__DIAXUS_PREVIEW_DIAGNOSTIC_ACTIVE__=true;
    const value=await wasm.execute_job(file,'convert-ai-pilot','mp4-h264-aac','no-preference',command(scale),'memory:1:none',()=>{});
    const bytes=new Uint8Array(await value.blob.arrayBuffer());delete value.blob;
    result.artifacts.push({name:`video-${result.context}-${result.cases.length}.mp4`,bytes:Array.from(bytes)});
    result.cases.push({request:{fixture,scale,fps:'original',clahe:true,label},...value});
    return empty();
  };
  for(const scale of ['2x','4x']){
    const pixels=await preview(scale,'before-conversion');
    const initial=ownership(),afterSuccess=await convert(scale,'successful-handoff');
    if(await preview(scale,'after-conversion')!==pixels)throw Error('Preview pixels changed after conversion handoff');
    state.mode='release-error-after-close';state.triggered=false;
    let output,failure,progressCalls=0;
    try{output=await wasm.execute_job(file,'convert-ai-pilot','mp4-h264-aac','no-preference',command(scale),'memory:1:none',()=>{progressCalls++;});}
    catch(error){failure=String(error);}
    if(output||!state.triggered||!/preview retirement failed.*release response failed/.test(failure)||progressCalls)
      throw Error(`Retirement failure did not block conversion startup: ${JSON.stringify({failure,progressCalls})}`);
    const afterFailure=empty();
    const devicesBeforeRetry=globalThis.__aiDevices.length;
    globalThis.__aiCaptureNextDeviceScope=true;
    if(await preview(scale,'explicit-fresh-context-retry')!==pixels)throw Error('Fresh context changed preview pixels');
    const retried=ownership();
    if(retried.deviceGeneration===initial.deviceGeneration||globalThis.__aiDevices.length!==devicesBeforeRetry+1)
      throw Error('Retry did not create exactly one fresh processing context');
    const afterRetry=await convert(scale,'fresh-context-handoff');
    result.lifecycleCases.push({scale,initial,afterSuccess,failure,progressCalls,afterFailure,retried,afterRetry,
      pixelEquality:'exact RGBA8, before/after conversion and explicit fresh-context retry',
      faultScope:'injected release response failure after actual runtime release; not a real broken-runtime cleanup claim'});
  }
  for(const {device} of globalThis.__aiDevices.slice(1)){
    const error=await device.popErrorScope();
    if(error)throw Error(`Fresh context validation: ${error.message}`);
  }
  result.expectedDeviceRequests=3;
  result.finalOwnership=ownership();empty();
  result.previewStatus='implicit-preview-conversion-retirement-lifecycle-pilot';
}
