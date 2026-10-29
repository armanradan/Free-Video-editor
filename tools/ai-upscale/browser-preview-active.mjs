// Actual active inference, distinct decoded sources and sampled playback keys.
// This diagnostic does not claim ordinary AI On UI or audio synchronization.
export async function runAiPreviewActiveProbe(wasm,result,publish) {
  const load=async name=>new File([await(await fetch(`/fixture/${name}`)).arrayBuffer()],name,{lastModified:1234});
  const cut=await load('ai-upscale-cut.mp4'),other=await load('ai-upscale-vfr.mp4');
  const state=globalThis.__aiFailure;
  state.cancelLocal=()=>wasm.cancel_local();
  const command=scale=>`original~recommended~original~10/115/80~1/50~onnx-runtime/realesr-general-x4v3-strong-fp32-v1/${scale}`;
  const ownership=()=>JSON.parse(wasm.ownership());
  const empty=()=>{
    const owned=ownership(),runtime=state.snapshot();
    if(owned.liveLeases||owned.pendingSubmissions||owned.aiCreditHeld||owned.aiVideoRetained||owned.aiPreviewCached||
      runtime.activeRuns||runtime.callerTensors||runtime.resources.length||
      runtime.sessions.some(s=>!s.released||s.busy||s.active||s.sessionRetained))
      throw Error(`Retained failed preview ownership: ${JSON.stringify({owned,runtime})}`);
    return {owned,runtime};
  };
  globalThis.__DIAXUS_AI_INGRESS_PARITY__=true;
  globalThis.__DIAXUS_PARITY_FRAMES__='0';
  result.previewCases=[];result.activePreviewCases=[];result.retiredDeviceValidationErrors=[];
  const canvasPixels=async()=>{
    const frame=new VideoFrame(globalThis.__aiProcessingCanvas,{timestamp:0});
    try{
      const pixels=new Uint8Array(frame.allocationSize({format:'RGBA'}));
      await frame.copyTo(pixels,{format:'RGBA'});
      let binary='';
      for(let i=0;i<pixels.length;i+=8192)binary+=String.fromCharCode(...pixels.subarray(i,i+8192));
      return btoa(binary);
    }finally{frame.close();}
  };
  let identity=0;
  const preview=async(file,scale,label,seconds=.1)=>{
    result.boundary=`active preview ${scale} ${label}`;publish();
    const value=await wasm.execute_job(file,'preview-ai-pilot',`${++identity}@${seconds}@1`,'',command(scale),'memory:0:none',()=>{});
    if(!value.parity?.data||!value.summary.includes('Playing sampled source preview:'))throw Error('Missing sampled AI preview pixels');
    result.previewCases.push({request:{fixture:file.name,scale,label,seconds,playback:true},summary:value.summary,
      ownership:ownership(),parityFrames:[{...value.parity,index:0}]});
    return value.parity.data;
  };
  for(const scale of ['2x','4x']){
    const reference=await preview(cut,scale,'reference');
    const referenceGeneration=ownership().deviceGeneration;
    for(const mode of ['cancel-active','runtime-error','device-loss-active']){
      const canvasBefore=mode==='device-loss-active'?null:await canvasPixels();
      const before=ownership();state.mode=mode;state.triggered=false;
      state.dispatchesDuringRun=0;state.submittedDuringRun=0;
      let output,failure;
      try{output=await preview(cut,scale,mode,.24);}catch(error){failure=String(error);}
      if(output||!failure||!state.triggered)throw Error(`Active preview failure did not trigger: ${mode}: ${failure}`);
      if(mode==='cancel-active'&&!/cancelled/i.test(failure))throw Error(`Wrong cancellation error: ${failure}`);
      if(mode==='runtime-error'&&!/input.*missing|missing.*input/i.test(failure))throw Error(`Wrong runtime error: ${failure}`);
      if(mode==='device-loss-active'&&!/device|destroy|lost|buffer|GPU/i.test(failure))throw Error(`Wrong loss error: ${failure}`);
      const after=empty();
      if(after.owned.canvasCaptures!==before.canvasCaptures)throw Error('Failed selected preview presented an output frame');
      if(canvasBefore!==null&&await canvasPixels()!==canvasBefore)throw Error('Failed request changed the displayed canvas pixels');
      if(mode==='cancel-active'&&!after.runtime.dispatchesDuringRun)throw Error('Cancellation missed real inference dispatch');
      if(mode==='device-loss-active'){
        if(!after.runtime.submittedDuringRun)throw Error('Destruction missed real inference submission');
        if(globalThis.__aiDevices.length>1){
          const error=await globalThis.__aiDevices.at(-1).device.popErrorScope();
          result.retiredDeviceValidationErrors.push(error?.message??null);
        }
        globalThis.__aiCaptureNextDeviceScope=true;
      }
      if(await preview(cut,scale,`${mode}-retry`)!==reference)throw Error('Explicit preview retry changed selected pixels');
      const retried=ownership();
      if(mode==='device-loss-active'&&retried.deviceGeneration===referenceGeneration)throw Error('Loss retry reused the failed generation');
      // A genuinely different decoded source must replace the selected image,
      // then switching back must restore the original without stale cache pixels.
      const replacement=await preview(other,scale,`${mode}-source-replacement`);
      if(replacement===reference)throw Error('Distinct source reused preceding preview pixels');
      if(await preview(cut,scale,`${mode}-source-restore`)!==reference)throw Error('Source restore retained replacement pixels');
      result.activePreviewCases.push({scale,mode,failure,failedCommandReturnedOutput:false,
        successfulCanvasCaptureDelta:after.owned.canvasCaptures-before.canvasCaptures,
        canvasPixels:canvasBefore===null?'lost-device display content not asserted':'exact RGBA8 equality before/after failure; separate diagnostic readbacks',before,after,retried,
        selectedPixels:'exact retry/restore equality; different-source pixels differ'});
    }
    await wasm.execute_job(null,'clear-preview','','','original','memory:0:none',()=>{});empty();
  }
  result.finalOwnership=ownership();
  result.expectedDeviceRequests=3;
  result.retiredInitialScope=true;
  result.previewStatus='active-sampled-preview-failure-source-replacement-pilot';
}
