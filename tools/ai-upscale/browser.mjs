// Standalone diagnostic server/browser, sharing the real Rust GpuSession device.
// Usage: node browser.mjs <evaluation-root> <built-Dioxus-public-dir> <output-dir>
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import {pathToFileURL} from 'node:url';
import {installAllocationLedger} from './browser-memory.mjs';
import {installAiFailureInstrumentation,instrumentAiBoundary} from './browser-failure.mjs';
import {runAiPreviewProbe} from './browser-preview.mjs';
import {runAiPreviewHistoryProbe} from './browser-preview-history.mjs';
import {runAiPreviewLifecycleProbe} from './browser-preview-lifecycle.mjs';
import {runAiPreviewActiveProbe} from './browser-preview-active.mjs';
const [rootArg, publicArg, outputArg] = process.argv.slice(2);
if (!outputArg) throw Error('expected evaluation root, built public directory and output directory');
const root = path.resolve(rootArg), publicDir = path.resolve(publicArg), outputDir = path.resolve(outputArg);
const fixtureDir = path.resolve(process.env.AI_UPSCALE_FIXTURE_DIR ?? 'fixtures');
if (fixtureDir !== path.resolve('fixtures') &&
    (!fixtureDir.startsWith(root + path.sep) || !fs.statSync(fixtureDir).isDirectory())) {
  throw Error('Additional evaluation fixtures must be in a directory under the evaluation root');
}
fs.mkdirSync(outputDir, {recursive: false});
process.env.PLAYWRIGHT_BROWSERS_PATH = path.join(root, 'chromium');
const {chromium} = await import(pathToFileURL(path.join(root, 'browser/node_modules/playwright/index.mjs')));
const hook = `${process.env.AI_UPSCALE_MEMORY === '1' || process.env.AI_UPSCALE_TILE === '1' || process.env.AI_UPSCALE_VIDEO === '1' ? `(${installAllocationLedger.toString()})();` : ''}
${process.env.AI_UPSCALE_VIDEO_FAILURES === '1' || process.env.AI_UPSCALE_PREVIEW_LIFECYCLE === '1' || process.env.AI_UPSCALE_PREVIEW_ACTIVE === '1' ? `(${installAiFailureInstrumentation.toString()})();` : ''}
${process.env.AI_UPSCALE_PREVIEW === '1' ? `globalThis.__runAiPreviewProbe=(${(process.env.AI_UPSCALE_PREVIEW_ACTIVE==='1'?runAiPreviewActiveProbe:process.env.AI_UPSCALE_PREVIEW_LIFECYCLE==='1'?runAiPreviewLifecycleProbe:process.env.AI_UPSCALE_PREVIEW_HISTORY==='1'?runAiPreviewHistoryProbe:runAiPreviewProbe).toString()});` : ''}
globalThis.__aiDevices=[];
const original=GPUAdapter.prototype.requestDevice;
GPUAdapter.prototype.requestDevice=async function(descriptor){
  const device=await original.call(this,descriptor);
  if(globalThis.__aiCaptureNextDeviceScope){
    device.pushErrorScope('validation');globalThis.__aiCaptureNextDeviceScope=false;
    globalThis.__aiScopedReplacementDevice=device;
  }
  globalThis.__aiDevices.push({device,descriptor});return device;
};`;

async function probe() {
  const result = {status: 'running', context: typeof document === 'undefined' ? 'worker' : 'main', cases: []};
  globalThis.__aiProbeResult = result;
  const publish = () => {
    // Progress must not repeatedly clone prior encoded files or diagnostic
    // rasters. The final result transfers those once after the probe completes.
    const progress={context:result.context,status:result.status,boundary:result.boundary,
      lastStatus:result.lastStatus,completedCases:result.cases?.length};
    if (typeof document === 'undefined') self.postMessage({aiProgress:progress});
    else globalThis.__aiReportProgress?.(progress);
  };
  let session, device;
  let errorScope = false;
  try {
    const diagnosticConfig = await (await fetch('/diagnostic-config.json')).json();
    if(!diagnosticConfig.burn&&!diagnosticConfig.codec) result.ortVersion=diagnosticConfig.ortVersion;
    if(diagnosticConfig.video){
      result.mode='decoded-video-ai-composition-pilot';result.artifacts=[];
      if(diagnosticConfig.videoFfmpeg){
        globalThis.__DIAXUS_BACKEND__='ffmpeg-wasm';
        globalThis.__DIAXUS_FFMPEG_ASSETS__={classWorkerURL:'/ffmpeg-worker.js',coreURL:'/ffmpeg-core.js',wasmURL:'/ffmpeg-core.wasm'};
        result.codecBackend='ffmpeg-wasm';
      }
      if(diagnosticConfig.videoParity){
        globalThis.__DIAXUS_AI_INGRESS_PARITY__=true;
        globalThis.__DIAXUS_PREVIEW_DIAGNOSTIC_ACTIVE__=true;
        globalThis.__DIAXUS_PARITY_FRAMES__='0';
        result.diagnosticPixelReadbacks='Every ingress plus one pre-encoder frame per case; excluded from production/performance evidence';
      }
      const wasm=await import('/tile/probe.js');await wasm.default();
      globalThis.__aiProcessingCanvas=new OffscreenCanvas(160,90);
      await wasm.initialize(globalThis.__aiProcessingCanvas,()=>{});
      device=__aiDevices[0].device;device.pushErrorScope('validation');errorScope=true;
      result.baseline=__aiAllocationLedger.snapshot();
      await import('/media-probe.mjs');
      result.modelRuntimeProvisionedForDiagnostic=diagnosticConfig.preview||diagnosticConfig.videoFailures||
        diagnosticConfig.videoRecovery||diagnosticConfig.videoCases.some(item=>item.scale!=='off');
      if(result.modelRuntimeProvisionedForDiagnostic){
        const assets=await import('/ai-assets.js'),manifest=await(await fetch('/model-manifest.json')).json(),json=JSON.stringify(manifest);
        const model=await(await fetch('/reference/model.onnx')).arrayBuffer();
        await assets.importAiModel(new File([model],'model.onnx'),json,()=>false);
        await assets.acquireAiRuntime(json,()=>false);
      }
      if(diagnosticConfig.preview){
        await globalThis.__runAiPreviewProbe(wasm,result,publish,diagnosticConfig.previewFixture,diagnosticConfig.previewScale);
        if(result.retiredInitialScope){
          result.retiredDeviceValidationErrors.unshift((await device.popErrorScope())?.message??null);
          device=globalThis.__aiScopedReplacementDevice;
        }
        result.deviceRequests=__aiDevices.length;
        if(result.deviceRequests!==(result.expectedDeviceRequests??1))throw Error('Unexpected preview processing device count');
        result.status='video-pilot-passed-not-release-accepted';return result;
      }
      if(diagnosticConfig.videoFailures){
        const state=globalThis.__aiFailure;
        state.cancelLocal=()=>wasm.cancel_local();result.failureCases=[];
        const file=new File([await(await fetch('/fixture/ai-upscale-vfr.mp4')).arrayBuffer()],'ai-upscale-vfr.mp4');
        const command='original~recommended~original~0/100/100~0/50~onnx-runtime/realesr-general-x4v3-strong-fp32-v1/4x';
        const run=()=>wasm.execute_job(file,'convert-ai-pilot','mp4-h264-aac','no-preference',command,'memory:1:none',message=>{result.lastStatus=message;publish();});
        for(const kind of ['cancel-active','runtime-error','oom-setup','oom-run','device-loss-active']){
          result.boundary=`failure ${kind}`;publish();state.mode=kind;state.triggered=false;
          state.dispatchesDuringRun=0;state.submittedDuringRun=0;
          let output=null,failure;
          try{output=await run();}catch(error){failure=String(error);}
          if(output||!failure||!state.triggered){
            result.unexpectedFailureState=state.snapshot();
            throw Error(`Failure injection did not reject: ${kind}: ${failure}`);
          }
          const expected=kind==='cancel-active'?/cancelled/i:kind==='runtime-error'?/input.*missing|missing.*input/i:
            kind.startsWith('oom')?/Out of Memory/:/lost|destroy|buffer|device|GPU/i;
          if(!expected.test(failure))throw Error(`Wrong failure boundary ${kind}: ${failure}`);
          if(!kind.startsWith('oom-setup')&&!/Cleanup: 0 application-held frame references, 0 samples/.test(failure))
            throw Error(`Missing frame cleanup evidence: ${kind}: ${failure}`);
          const snapshot=state.snapshot();
          const rustOwnership=JSON.parse(wasm.ownership());
          if(rustOwnership.liveLeases||rustOwnership.pendingSubmissions||rustOwnership.aiCreditHeld||
            rustOwnership.aiVideoRetained||rustOwnership.aiVideoActive)throw Error(`Rust AI ownership retained: ${JSON.stringify(rustOwnership)}`);
          if(snapshot.activeRuns||snapshot.callerTensors||snapshot.resources.length||
            snapshot.sessions.some(s=>!s.released||s.busy||s.active||s.sessionRetained))
            throw Error(`AI ownership retained after ${kind}: ${JSON.stringify(snapshot)}`);
          const record={kind,failure,partialArtifactPublished:false,snapshot,rustOwnership,allocation:__aiAllocationLedger.snapshot()};
          result.failureCases.push(record);state.mode=null;
          if(kind==='device-loss-active'){
            record.oldDeviceValidationError=(await device.popErrorScope())?.message??null;errorScope=false;
            globalThis.__aiCaptureNextDeviceScope=true;
          }
          const conversion=await run();
          if(conversion.aiSourceFrames!==4||conversion.frameCount!==4)throw Error('Fresh-session retry failed source/frame counts');
          const afterRetry=state.snapshot();
          if(afterRetry.activeRuns||afterRetry.callerTensors||afterRetry.resources.length||afterRetry.sessions.some(s=>!s.released))
            throw Error('Fresh-session retry retained AI ownership');
          record.retry={sourceFrames:conversion.aiSourceFrames,frameCount:conversion.frameCount,ownership:afterRetry,rustOwnership:JSON.parse(wasm.ownership())};
          if(record.retry.rustOwnership.liveLeases||record.retry.rustOwnership.pendingSubmissions||record.retry.rustOwnership.aiCreditHeld)
            throw Error('Fresh retry retained Rust GPU ownership');
          const bytes=new Uint8Array(await conversion.blob.arrayBuffer());delete conversion.blob;
          result.artifacts.push({name:`video-${result.context}-${result.cases.length}.mp4`,bytes:Array.from(bytes)});
          result.cases.push({request:{fixture:'ai-upscale-vfr.mp4',scale:'4x',fps:'original',clahe:false},...conversion});
        }
      }
      if(diagnosticConfig.videoRecovery){
        const file=new File([await(await fetch('/fixture/ai-upscale-vfr.mp4')).arrayBuffer()],'ai-upscale-vfr.mp4');
        let failure;
        try{await wasm.execute_job(file,'convert-ai-pilot','mp4-h264-aac','no-preference',
          'original~recommended~original~0/100/100~0/50~onnx-runtime/realesr-general-x4v3-strong-fp32-v1/4x','memory:1:device-loss-once',()=>{});}
        catch(error){failure=String(error);}
        if(!failure?.includes('INJECTED: WebGPU AI device loss')||!failure.includes('Cleanup: 0 application-held frame references, 0 samples'))throw Error(`unexpected device-loss outcome: ${failure}`);
        result.recovery={failure,afterFailure:__aiAllocationLedger.snapshot(),partialArtifactPublished:false};
        result.oldDeviceValidationError=(await device.popErrorScope())?.message??null;errorScope=false;
        globalThis.__aiCaptureNextDeviceScope=true;
        result.recovery.retryUsesRetainedCanvas=true;
      }
      for(const [index,item] of diagnosticConfig.videoCases.entries()){
        const file=new File([await(await fetch(`/fixture/${item.fixture}`)).arrayBuffer()],item.fixture);
        result.boundary=`video ${index} ${item.fixture} ${item.scale} ${item.fps}`;publish();
        const ai=item.scale==='off'?'off':`onnx-runtime/realesr-general-x4v3-strong-fp32-v1/${item.scale}`;
        const command=`original~recommended~${item.fps}~${item.color??'0/100/100'}~${item.clahe?'1/50':'0/50'}~${ai}`;
        __aiAllocationLedger.beginWindow();const started=performance.now();
        const conversion=await wasm.execute_job(file,item.bilinear?'convert-bilinear-pilot':item.scale==='off'?'convert':'convert-ai-pilot',item.profile??'mp4-h264-aac','no-preference',command,diagnosticConfig.videoFfmpeg?'auto:1:none':'memory:1:none',message=>{result.lastStatus=message;publish();});
        if(item.scale!=='off'&&!item.bilinear&&conversion.aiSourceFrames!==conversion.inputFrameCount)throw Error('AI ran a different number of times than source frames');
        if(item.bilinear&&conversion.aiSourceFrames!==undefined)throw Error('Bilinear reference loaded inference');
        if(item.scale==='off'&&conversion.aiSourceFrames!==undefined)throw Error('AI Off created an inference session');
        const bytes=new Uint8Array(await conversion.blob.arrayBuffer());delete conversion.blob;
        result.artifacts.push({name:`video-${result.context}-${index}.${item.profile?.startsWith('webm')?'webm':'mp4'}`,bytes:Array.from(bytes)});
        result.cases.push({request:item,...conversion,totalDiagnosticMs:performance.now()-started,allocation:__aiAllocationLedger.snapshot()});
      }
      result.deviceRequests=__aiDevices.length;
      if(globalThis.__aiScopedReplacementDevice){device=globalThis.__aiScopedReplacementDevice;errorScope=true;}
      if(result.deviceRequests!==((diagnosticConfig.videoRecovery||diagnosticConfig.videoFailures)?2:1))throw Error('Unexpected processing device count');
      result.status='video-pilot-passed-not-release-accepted';return result;
    }
    if(diagnosticConfig.codec){
      result.mode='exact-enlarged-codec-probe-no-inference';
      const wasm=await import('/tile/probe.js');await wasm.default();
      const encoderCalls=[],encoderProbe=VideoEncoder.isConfigSupported.bind(VideoEncoder);
      VideoEncoder.isConfigSupported=async config=>{encoderCalls.push({...config});return encoderProbe(config);};
      await import('/media-probe.mjs');
      const file=new File([await(await fetch('/fixture/m2-h264-aac.mp4')).arrayBuffer()],'m2-h264-aac.mp4');
      const backend=globalThis.__DIAXUS_MEDIA_WEB__,original=backend.probeProfiles.bind(backend),calls=[];
      backend.probeProfiles=async(file,width,height,options)=>{calls.push({width,height,options:{...options}});return original(file,width,height,options);};
      for(const fps of ['original','60/1'])for(const scale of ['off','2x','4x']){
        const ai=scale==='off'?'off':`onnx-runtime/realesr-general-x4v3-strong-fp32-v1/${scale}`;
        const command=`percent:50~recommended~${fps}~0/100/100~0/50~${ai}`;
        const capabilities=await wasm.execute_job(file,'probe','','',command,'auto:0:none',()=>{});
        const factor=scale==='off'?.5:Number(scale[0]);
        if(capabilities.outputWidth!==640*factor||capabilities.outputHeight!==360*factor)throw Error('wrong enlarged probe geometry');
        if(capabilities.aiAvailable!==(scale==='off'))throw Error('codec support granted AI availability');
        if(capabilities.estimates.avc.bitrateBps!==calls.at(-1).options.avc)throw Error('estimate and exact encoder probe used different rates');
        result.cases.push({fps,scale,...calls.at(-1),capabilities});
      }
      const unsupported=await original(file,999998,999998,{avc:120000000,hevc:120000000,frameRate:60,fpsNumerator:60,fpsDenominator:1});
      if(unsupported.mp4Supported||unsupported.hevcSupported)throw Error('oversized exact codec configuration accepted');
      result.unsupportedExactCodec=unsupported;
      if(!encoderCalls.some(c=>c.width===2560&&c.height===1440&&c.framerate===60&&c.bitrate===8049568))throw Error('actual encoder probe did not receive enlarged 60 FPS configuration');
      result.actualEncoderProbeConfigs=encoderCalls;
      for(const operation of ['convert','preview']){
        let rejected=false;
        try{await wasm.execute_job(file,operation,'mp4-h264-aac','no-preference','original~recommended~original~0/100/100~0/50~onnx-runtime/realesr-general-x4v3-strong-fp32-v1/4x','auto:0:none',()=>{});}
        catch(error){rejected=String(error).includes('release gates');}
        if(!rejected)throw Error(`${operation} bypassed unavailable AI gate`);
      }
      result.unavailableConversionPreviewRejected=true;result.deviceRequests=__aiDevices.length;
      if(result.deviceRequests!==0)throw Error('codec probing allocated a processing device');
      result.status='exact-codec-probe-passed-not-ai-acceptance';return result;
    }
    if(diagnosticConfig.tile){
      result.mode='shared-Rust-pack-inference-unpack-pilot';
      const wasm=await import('/tile/probe.js'); await wasm.default();
      await wasm.initialize(new OffscreenCanvas(160,90),()=>{});
      if(__aiDevices.length!==1)throw Error('Tile adapter created a second processing device');
      device=__aiDevices[0].device;
      device.pushErrorScope('validation'); errorScope=true;
      const info=device.adapterInfo;
      result.gpu=info?{vendor:info.vendor,architecture:info.architecture,device:info.device,description:info.description}:null;
      result.baseline=__aiAllocationLedger.snapshot();
      const assets=await import('/ai-assets.js');
      const manifest=await(await fetch('/model-manifest.json')).json(),json=JSON.stringify(manifest);
      const model=await(await fetch('/reference/model.onnx')).arrayBuffer();
      await assets.importAiModel(new File([model],'model.onnx'),json,()=>false);
      await assets.acquireAiRuntime(json,()=>false);
      const floats=async url=>{const data=await(await fetch(url)).arrayBuffer(),view=new DataView(data);
        if(view.getUint8(6)!==1)throw Error('expected NPY v1');return new Float32Array(data.slice(10+view.getUint16(8,true)));};
      for(const base of ['/reference','/calibration']){
        const reference=await(await fetch(`${base}/reference.json`)).json();
        for(const {width,height} of reference.cases){
          const stem=`${width}x${height}`;result.boundary=`Rust tile adapter ${stem}`;publish();
          const input=await floats(`${base}/${stem}-input.npy`),expected=await floats(`${base}/${stem}-fp32.npy`);
          __aiAllocationLedger.beginWindow();const started=performance.now();
          const test=JSON.parse(await wasm.evaluate(width,height,input,expected));
          result.cases.push({stem,...test,totalDiagnosticMs:performance.now()-started,allocation:__aiAllocationLedger.snapshot()});
        }
      }
      result.deviceRequests=__aiDevices.length;
      result.boundary='Rust tile adapter lifecycle';publish();
      result.lifecycle=JSON.parse(await wasm.lifecycle());
      result.afterLifecycle=__aiAllocationLedger.snapshot();
      result.memoryScope='Requested resources less explicit destroy; includes diagnostic upload/readback and runtime pools, excludes opaque driver/compiler/surface memory and does not observe GC';
      result.status='numeric-device-probe-passed-not-product-accepted';return result;
    }
    if (diagnosticConfig.burn) {
      result.engine='Burn 0.22.1 / CubeCL 0.11.2';
      const wasm = await import('/burn/probe.js');
      await wasm.default();
      const weights = new Uint8Array(await (await fetch('/burn-weights/model.bpk')).arrayBuffer());
      result.boundary='Burn shared processing setup and generated weights';
      result.setup=JSON.parse(await wasm.initialize(new OffscreenCanvas(160,90),()=>{},weights));
      if (__aiDevices.length!==1 || __aiDevices[0].descriptor.label!=='browser video processing device') throw Error('Burn did not retain the one actual processing device');
      device=__aiDevices[0].device;
      const info=device.adapterInfo;
      result.gpu=info?{vendor:info.vendor,architecture:info.architecture,device:info.device,description:info.description}:null;
      result.deviceRequests=__aiDevices.length;
      result.processingLimits={maxComputeInvocationsPerWorkgroup:device.limits.maxComputeInvocationsPerWorkgroup};
      publish();
      const reference=await(await fetch('/reference/reference.json')).json();
      const floats=async url=>{const data=await(await fetch(url)).arrayBuffer();const view=new DataView(data);if(view.getUint8(6)!==1)throw Error('expected NPY v1');return new Float32Array(data.slice(10+view.getUint16(8,true)));};
      for(const {width,height} of reference.cases){
        const stem=`${width}x${height}`;
        result.boundary=`Burn forward ${stem}`;
        publish();
        const input=await floats(`/reference/${stem}-input.npy`), expected=await floats(`/reference/${stem}-fp32.npy`);
        const test=JSON.parse(await wasm.evaluate(width,height,input,expected));
        result.cases.push({stem,...test,diagnosticUploadBytes:input.byteLength});
      }
      wasm.release(); result.sessionReleased=true;
      result.status='numeric-device-probe-passed-not-product-accepted';
      return result;
    }
    const wasm = await import('/wasm/converter-web.js');
    await wasm.default();
    await wasm.initialize_worker(new OffscreenCanvas(160, 90), () => {});
    if (__aiDevices.length !== 1 || __aiDevices[0].descriptor.label !== 'browser video processing device') {
      throw Error('Expected one actual Rust processing device');
    }
    device = __aiDevices[0].device;
    device.pushErrorScope('validation'); errorScope = true;
    const info = device.adapterInfo;
    result.gpu = info ? {vendor: info.vendor, architecture: info.architecture,
      device: info.device, description: info.description} : null;
    result.processingLimits = {maxStorageBufferBindingSize: device.limits.maxStorageBufferBindingSize,
      maxBufferSize: device.limits.maxBufferSize,
      maxStorageBuffersPerShaderStage: device.limits.maxStorageBuffersPerShaderStage};
    if (diagnosticConfig.memory) result.processingAllocationBaseline=__aiAllocationLedger.snapshot();
    let ort;
    if(diagnosticConfig.assets){
      const assets=await import('/ai-assets.js');
      const manifest=await(await fetch('/model-manifest.json')).json(),json=JSON.stringify(manifest);
      const bytes=await(await fetch('/reference/model.onnx')).arrayBuffer();
      result.modelAcquisition=await assets.importAiModel(new File([bytes],'model.onnx'),json,()=>false);
      result.runtimeAcquisition=await assets.acquireAiRuntime(json,()=>false);
      globalThis.__aiCachedModel=await assets.readAiModel(json);
      ort=await assets.loadAiRuntime(json);
      result.verifiedBlobRuntime=true;
    }else ort = await import('/ort/ort.webgpu.min.mjs');
    ort.env.logLevel = 'verbose';
    ort.env.webgpu.device = device;
    ort.env.wasm.numThreads = 1;
    if(!diagnosticConfig.assets)ort.env.wasm.wasmPaths = '/ort/';
    if (diagnosticConfig.isolate) {
      result.mode = 'isolated-operators-no-cpu-fallback';
      const operators = await (await fetch('/isolated/manifest.json')).json();
      const floats = async url => {
        const data = await (await fetch(url)).arrayBuffer();
        const view = new DataView(data);
        if (view.getUint8(6) !== 1) throw Error('expected NPY v1');
        return new Float32Array(data.slice(10 + view.getUint16(8, true)));
      };
      for (const item of operators) {
        const outcome = {operator: item.operator, cases: []};
        result.cases.push(outcome);
        try {
          session = await ort.InferenceSession.create(`/isolated/${item.operator}.onnx`, {
            executionProviders: [{name:'webgpu', device}], preferredOutputLocation:'gpu-buffer',
            extra:{session:{disable_cpu_ep_fallback:'1'}}, logSeverityLevel:0, logVerbosityLevel:1,
          });
          for (const test of item.cases) {
            const input = await floats(`/isolated/${test.stem}-input.npy`);
            const expected = await floats(`/isolated/${test.stem}-expected.npy`);
            const buffer = device.createBuffer({size:Math.ceil(input.byteLength/16)*16,
              usage:GPUBufferUsage.STORAGE|GPUBufferUsage.COPY_DST|GPUBufferUsage.COPY_SRC});
            const tensor = ort.Tensor.fromGpuBuffer(buffer,{dataType:'float32',dims:test.shape});
            let output;
            try {
              device.queue.writeBuffer(buffer,0,input);
              output = (await session.run({input:tensor})).output;
              const actual = await output.getData();
              let maxAbs=0,sum=0;
              if (actual.length!==expected.length) throw Error('Wrong isolated output length');
              for(let i=0;i<actual.length;i++){const e=Math.abs(actual[i]-expected[i]);if(!Number.isFinite(e))throw Error('nonfinite output');maxAbs=Math.max(maxAbs,e);sum+=e;}
              outcome.cases.push({stem:test.stem,maxAbs,mae:sum/actual.length,shape:output.dims});
              if(maxAbs>.01||sum/actual.length>.001)throw Error('Isolated reference tolerance exceeded');
            } catch(error) {outcome.cases.push({stem:test.stem,status:'failed',error:String(error.stack??error)});}
            finally {output?.dispose();tensor.dispose();buffer.destroy();}
          }
          outcome.status=outcome.cases.some(test=>test.status==='failed')?'failed':'passed';
        } catch(error) {outcome.status='failed';outcome.error=String(error.stack??error);}
        finally {if(session){await session.release();session=null;}}
      }
      result.deviceRequests=__aiDevices.length;
      result.status='diagnostic-complete-not-model-acceptance';
      return result;
    }
    const started = performance.now();
    const model = globalThis.__aiCachedModel ?? await (await fetch('/reference/model.onnx')).arrayBuffer();
    result.modelSha256 = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', model)),
      byte => byte.toString(16).padStart(2, '0')).join('');
    if (result.modelSha256 !== '1793a6e7fdf15a53eed213ba269ea768b5373858ae433d56ee8e1b0424377cc5') throw Error('ONNX hash is not allowlisted');
    session = await ort.InferenceSession.create(model, {
      // 1.23's native WebGPU EP accepts the device in provider options.
      // Setting env.webgpu.device alone is insufficient in this build.
      executionProviders: [{name: 'webgpu', device}], preferredOutputLocation: 'gpu-buffer',
      enableCpuMemArena: false,
      extra: {session: {disable_cpu_ep_fallback: '1'}},
      logSeverityLevel: 0, logVerbosityLevel: 1,
    });
    result.loadMs = performance.now() - started;
    result.exactDeviceIdentity = (await ort.env.webgpu.device) === device;
    if (!result.exactDeviceIdentity || __aiDevices.length !== 1) throw Error('Inference changed GPU device');
    if (diagnosticConfig.memory) {
      const {calibrate} = await import('/calibration-probe.mjs');
      await calibrate({session,device,ort,result});
      result.deviceRequests=__aiDevices.length;
      if(result.deviceRequests!==1)throw Error('Additional device was created');
      result.status='numeric-device-probe-passed-not-product-accepted';
      return result;
    }
    const reference = await (await fetch('/reference/reference.json')).json();
    // Read .npy payloads as raw little-endian FP32 after the documented v1/v2 header.
    const floats = async url => {
      const bytes = await (await fetch(url)).arrayBuffer(), view = new DataView(bytes);
      if (view.getUint8(0) !== 0x93) throw Error('Not an NPY tensor');
      const version = view.getUint8(6), start = version === 1 ? 10 : 12;
      const length = version === 1 ? view.getUint16(8, true) : view.getUint32(8, true);
      const header = new TextDecoder().decode(new Uint8Array(bytes, start, length));
      if (!header.includes("'<f4'") || !header.includes("'fortran_order': False")) throw Error('Unsupported NPY encoding');
      return new Float32Array(bytes.slice(start + length));
    };
    for (const {width, height} of reference.cases) {
      const stem = `${width}x${height}`, input = await floats(`/reference/${stem}-input.npy`);
      const expected = await floats(`/reference/${stem}-fp32.npy`);
      // Reserve a complete final vector for runtimes using vectorized storage loads.
      // The logical tensor shape/payload remains unchanged; padding initializes to zero.
      const inputAllocationBytes = Math.ceil(input.byteLength / 16) * 16;
      const buffer = device.createBuffer({size: inputAllocationBytes,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC});
      const tensor = ort.Tensor.fromGpuBuffer(buffer, {dataType: 'float32', dims: [1, 3, height, width]});
      const suppliedBuffer=diagnosticConfig.preallocated?device.createBuffer({size:expected.byteLength,
        usage:GPUBufferUsage.STORAGE|GPUBufferUsage.COPY_SRC|GPUBufferUsage.COPY_DST}):null;
      const suppliedOutput=suppliedBuffer?ort.Tensor.fromGpuBuffer(suppliedBuffer,{dataType:'float32',dims:[1,3,height*4,width*4],
        download:async()=>{
          const staging=device.createBuffer({size:expected.byteLength,usage:GPUBufferUsage.COPY_DST|GPUBufferUsage.MAP_READ});
          try{const encoder=device.createCommandEncoder();encoder.copyBufferToBuffer(suppliedBuffer,0,staging,0,expected.byteLength);
            device.queue.submit([encoder.finish()]);await staging.mapAsync(GPUMapMode.READ);
            const data=new Float32Array(staging.getMappedRange()).slice();staging.unmap();return data;
          }finally{staging.destroy();}
        }}):null;
      let output;
      try {
        result.boundary = `session.run GPU input ${stem}`;
        device.queue.writeBuffer(buffer, 0, input);
        const started = performance.now();
        output = (await session.run({input: tensor},suppliedOutput?{output:suppliedOutput}:undefined)).output;
        if(suppliedOutput&&output.gpuBuffer!==suppliedBuffer)throw Error('Runtime replaced the caller-owned output buffer');
        await device.queue.onSubmittedWorkDone();
        const inferenceMs = performance.now() - started;
        if (output.location !== 'gpu-buffer') throw Error(`Unexpected output location ${output.location}`);
        const actual = await output.getData(); // One explicitly diagnostic readback.
        if (actual.length !== expected.length || output.dims.join() !== [1, 3, height*4, width*4].join()) throw Error('Wrong output geometry');
        let maxAbs = 0, sum = 0;
        for (let i = 0; i < actual.length; i++) {
          const error = Math.abs(actual[i] - expected[i]);
          if (!Number.isFinite(error)) throw Error('Nonfinite result');
          maxAbs = Math.max(maxAbs, error); sum += error;
        }
        result.cases.push({stem, maxAbs, mae: sum / actual.length, inferenceMs,
          inputAllocationBytes, diagnosticUploadBytes: input.byteLength,
          diagnosticReadbackBytes: actual.byteLength});
        if(suppliedOutput)result.cases.at(-1).callerOwnedOutputIdentity=true;
        if (maxAbs > .01 || sum / actual.length > .001) throw Error('Reference tolerance exceeded');
      } finally {
        output?.dispose(); if(suppliedOutput&&suppliedOutput!==output)suppliedOutput.dispose();
        suppliedBuffer?.destroy(); tensor.dispose(); buffer.destroy();
      }
    }
    result.deviceRequests = __aiDevices.length;
    if (result.deviceRequests !== 1) throw Error('Additional device was created');
    result.status = 'numeric-device-probe-passed-not-product-accepted';
  } catch (error) {
    result.status = 'failed'; result.error = String(error.stack ?? error);
    // Emscripten builds can throw the address of a C++ exception instead of Error.
    // Inspect only that object's first 24 bytes and bounded string fields, in this
    // isolated diagnostic process. Never infer a stable error code from its address.
    const runtime = globalThis.__aiOrtModule;
    if (typeof error === 'number' && runtime) {
      result.cppException = {address: error, fields: []};
      for (let offset = 0; offset < 24; offset += 4) {
        const pointer = runtime.getValue(error + offset, 'i32') >>> 0;
        let text = null;
        if (pointer > 1024 && pointer < runtime.HEAPU8.length - 512) {
          const bytes = runtime.HEAPU8.subarray(pointer, pointer + 512);
          const end = bytes.indexOf(0);
          if (end >= 0 && bytes.subarray(0, end).every(byte => byte === 9 || byte === 10 || byte === 13 || byte >= 32 && byte < 127)) {
            text = new TextDecoder().decode(Uint8Array.from(bytes.subarray(0, end)));
          }
        }
        result.cppException.fields.push({offset, pointer, text});
      }
    }
  } finally {
    if (session) {
      await session.release(); result.sessionReleased = true;
      if(globalThis.__aiAllocationLedger) result.afterSessionRelease=__aiAllocationLedger.snapshot();
      if (result.status === 'numeric-device-probe-passed-not-product-accepted') {
        const source = device.createBuffer({size:4,usage:GPUBufferUsage.COPY_DST|GPUBufferUsage.COPY_SRC});
        const target = device.createBuffer({size:4,usage:GPUBufferUsage.COPY_DST|GPUBufferUsage.MAP_READ});
        try {
          device.queue.writeBuffer(source,0,new Uint32Array([0x12345678]));
          const encoder=device.createCommandEncoder();encoder.copyBufferToBuffer(source,0,target,0,4);
          device.queue.submit([encoder.finish()]);await target.mapAsync(GPUMapMode.READ);
          result.processingDeviceUsableAfterRelease=new Uint32Array(target.getMappedRange())[0]===0x12345678;
          target.unmap();
          if(!result.processingDeviceUsableAfterRelease)throw Error('Processing device did not survive session release');
          result.lifecycleDiagnosticReadbackBytes=4;
        } finally {source.destroy();target.destroy();}
      }
    }
    if (errorScope) {
      result.gpuValidationError = (await device.popErrorScope())?.message ?? null;
      if(result.gpuValidationError)result.status='failed';
    }
  }
  return result;
}

const server = http.createServer((req, res) => {
  const pathname = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
  res.setHeader('Cross-Origin-Opener-Policy', 'same-origin');
  res.setHeader('Cross-Origin-Embedder-Policy', 'require-corp');
  if (pathname === '/') {
    res.setHeader('Content-Type', 'text/html'); res.end('<div id="main"></div>'); return;
  }
  if (pathname === '/diagnostic-config.json') {
    res.setHeader('Content-Type', 'application/json');
    res.end(JSON.stringify({isolate: process.env.AI_UPSCALE_ISOLATE === '1', burn:process.env.AI_UPSCALE_BURN==='1', video:process.env.AI_UPSCALE_VIDEO==='1',previewScale:process.env.AI_UPSCALE_PREVIEW_SCALE??'4x',previewFixture:process.env.AI_UPSCALE_PREVIEW_FIXTURE??'ai-upscale-vfr.mp4',preview:process.env.AI_UPSCALE_PREVIEW==='1',videoFailures:process.env.AI_UPSCALE_VIDEO_FAILURES==='1',videoFfmpeg:process.env.AI_UPSCALE_VIDEO_FFMPEG==='1',videoRecovery:process.env.AI_UPSCALE_VIDEO_RECOVERY==='1',videoParity:process.env.AI_UPSCALE_VIDEO_PARITY==='1',videoCases:JSON.parse(process.env.AI_UPSCALE_VIDEO_CASES??'[{"fixture":"m35-vfr-offset.mp4","scale":"4x","fps":"original","clahe":false}]'), codec:process.env.AI_UPSCALE_CODEC==='1', tile:process.env.AI_UPSCALE_TILE==='1', memory:process.env.AI_UPSCALE_MEMORY==='1',assets:process.env.AI_UPSCALE_ASSETS==='1',preallocated:process.env.AI_UPSCALE_PREALLOCATED==='1',
      ortVersion:JSON.parse(fs.readFileSync(path.join(process.env.AI_UPSCALE_ORT_DIST ?? path.join(root,'browser/node_modules/onnxruntime-web/dist'),'../package.json'),'utf8')).version})); return;
  }
  if(pathname==='/ai-assets.js'){
    res.setHeader('Content-Type','text/javascript');
    fs.createReadStream(path.resolve('crates/media-web/src/ai-assets.js')).pipe(res);return;
  }
  if(['/ffmpeg-worker.js','/ffmpeg-core.js','/ffmpeg-core.wasm'].includes(pathname)){
    const file=pathname==='/ffmpeg-worker.js'?path.resolve('apps/web/assets/ffmpeg-worker.js'):
      path.resolve('apps/web/node_modules/@ffmpeg/core/dist/esm',pathname.slice(1));
    res.setHeader('Content-Type',pathname.endsWith('.wasm')?'application/wasm':'text/javascript');
    fs.createReadStream(file).pipe(res);return;
  }
  if(pathname==='/media-probe.mjs'){
    res.setHeader('Content-Type','text/javascript');
    fs.createReadStream(path.join(root,'media-probe.mjs')).pipe(res);return;
  }
  if(pathname==='/model-manifest.json'){
    const manifest=JSON.parse(fs.readFileSync('crates/media-core/src/ai-upscale-model.json','utf8'));
    for(const asset of manifest.browser_runtime.assets)asset.url=`http://127.0.0.1:${server.address().port}/ort/${asset.filename}`;
    res.setHeader('Content-Type','application/json');res.end(JSON.stringify(manifest));return;
  }
  if (pathname === '/diagnostic-worker.js') {
    res.setHeader('Content-Type', 'text/javascript');
    res.end(`${hook}\nself.onmessage=async()=>self.postMessage(await (${probe.toString()})());`); return;
  }
  if(pathname==='/calibration-probe.mjs'){
    res.setHeader('Content-Type','text/javascript');
    fs.createReadStream(new URL('./browser-memory.mjs',import.meta.url)).pipe(res);return;
  }
  const [base, relative] = pathname.startsWith('/fixture/')
    ? [fixtureDir,pathname.slice(9)]
    : pathname.startsWith('/tile/')
    ? [path.join(root,'tile-browser-wasm'),pathname.slice(6)]
    : pathname.startsWith('/burn/')
    ? [path.join(root, 'burn-browser-wasm'), pathname.slice(6)]
    : pathname.startsWith('/burn-weights/')
    ? [path.join(root, 'burn-generated'), pathname.slice(14)]
    : pathname.startsWith('/isolated/')
    ? [path.join(root, 'isolated-ops-final'), pathname.slice(10)]
    : pathname.startsWith('/ort/')
    ? [process.env.AI_UPSCALE_ORT_DIST ?? path.join(root, 'browser/node_modules/onnxruntime-web/dist'), pathname.slice(5)]
    : pathname.startsWith('/reference/') ? [path.join(root, 'reference'), pathname.slice(11)]
    : pathname.startsWith('/calibration/') ? [path.join(root, 'reference-envelope'), pathname.slice(13)]
    : [publicDir, pathname.slice(1)];
  const file = path.resolve(base, relative);
  if (!file.startsWith(path.resolve(base) + path.sep) || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
    res.writeHead(404); res.end(); return;
  }
  const type = {'.js': 'text/javascript', '.mjs': 'text/javascript', '.wasm': 'application/wasm', '.json': 'application/json'}[path.extname(file)] ?? 'application/octet-stream';
  if((process.env.AI_UPSCALE_VIDEO_FAILURES==='1'||process.env.AI_UPSCALE_PREVIEW_LIFECYCLE==='1'||process.env.AI_UPSCALE_PREVIEW_ACTIVE==='1')&&path.basename(file)==='ai-inference.js'){
    res.setHeader('Content-Type',type);res.end(instrumentAiBoundary(fs.readFileSync(file,'utf8')));return;
  }
  if (process.env.AI_UPSCALE_DECODE_CPP === '1' && path.basename(file) === 'ort-wasm-simd-threaded.asyncify.mjs') {
    res.setHeader('Content-Type', type);
    const source = fs.readFileSync(file, 'utf8');
    if (!source.includes('moduleRtn=ca;')) throw Error('Pinned ORT diagnostic instrumentation anchor changed');
    res.end(source.replace('moduleRtn=ca;', 'globalThis.__aiOrtModule=f;moduleRtn=ca;'));
    return;
  }
  res.setHeader('Content-Type', type); fs.createReadStream(file).pipe(res);
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
let browser, failed=false;
const timeoutMs=process.env.AI_UPSCALE_MEMORY==='1'||process.env.AI_UPSCALE_TILE==='1'||process.env.AI_UPSCALE_VIDEO==='1'?300000:120000;
try {
  browser = await chromium.launch({...(process.env.AI_UPSCALE_BROWSER_EXE
      ? {executablePath:process.env.AI_UPSCALE_BROWSER_EXE} : {channel:'chromium'}), headless: true,
    args: ['--enable-unsafe-webgpu', '--use-angle=d3d11',
      ...JSON.parse(process.env.AI_UPSCALE_BROWSER_ARGS ?? '[]')]});
  for (const context of ['main', 'worker']) {
    const page = await browser.newPage();
    let previousBoundary;
    await page.exposeFunction('__aiReportProgress',progress=>{
      if(progress.boundary!==previousBoundary){
        previousBoundary=progress.boundary;
        fs.writeFileSync(path.join(outputDir,`${context}-progress.json`),JSON.stringify(progress,null,2));
      }
    });
    const logs = [];
    page.on('console', message => logs.push(message.text()));
    await page.addInitScript(hook);
    await page.goto(`http://127.0.0.1:${server.address().port}/${process.env.AI_UPSCALE_PREVIEW==='1'?'?diagnostic-preview=1':''}`);
    let result;
    if (context === 'main') {
      let timer, errorHandler;
      const interrupted=new Promise(resolve=>{
        errorHandler=error=>resolve({harnessFailure:String(error)});
        page.once('pageerror',errorHandler);
        timer=setTimeout(()=>resolve({harnessFailure:`main diagnostic exceeded ${timeoutMs} ms`}),timeoutMs);
      });
      try {
        result=await Promise.race([page.evaluate(probe),interrupted]);
        if(result.harnessFailure){
          const error=result.harnessFailure;
          result=await page.evaluate(()=>globalThis.__aiProbeResult??{});
          result.status='failed';result.error=error;result.contextRetiredAfterError=true;
        }
      } finally {clearTimeout(timer);page.off('pageerror',errorHandler);}
    }
    else result = await page.evaluate(timeoutMs => new Promise((resolve, reject) => {
      const worker = new Worker('/diagnostic-worker.js'+location.search, {type: 'module'});
      let progress={context:'worker'};
      const fail=error=>{clearTimeout(timer);worker.terminate();resolve({...progress,status:'failed',error,contextRetiredAfterError:true});};
      const timer = setTimeout(() => fail(`worker diagnostic exceeded ${timeoutMs} ms`), timeoutMs);
      worker.onerror = event => {event.preventDefault();fail(event.message);};
      worker.onmessage = event => {
        if(event.data.aiProgress){progress=event.data.aiProgress;globalThis.__aiReportProgress?.(progress);return;}
        clearTimeout(timer);worker.terminate();resolve(event.data);
      };
      worker.postMessage({});
    }),timeoutMs);
    result.browserVersion = browser.version(); result.logs = logs;
    for(const [caseIndex,item] of (result.previewCases??[]).entries())for(const frame of item.parityFrames??[]){
      const filename=`preview-${context}-${caseIndex}.rgba`;
      fs.writeFileSync(path.join(outputDir,filename),Buffer.from(frame.data,'base64'));
      delete frame.data;frame.dataFile=filename;
      if(frame.ingress){
        const filename=`preview-ingress-${context}-${caseIndex}.rgba`;
        fs.writeFileSync(path.join(outputDir,filename),Buffer.from(frame.ingress.data,'base64'));
        delete frame.ingress.data;frame.ingress.dataFile=filename;
      }
    }
    for(const [caseIndex,item] of (result.cases??[]).entries())for(const frame of item.parityFrames??[]){
      const filename=`parity-${context}-${caseIndex}-${frame.index}.rgba`;
      fs.writeFileSync(path.join(outputDir,filename),Buffer.from(frame.data,'base64'));
      delete frame.data;frame.dataFile=filename;
      if(frame.ingress){
        const filename=`ingress-${context}-${caseIndex}-${frame.index}.rgba`;
        fs.writeFileSync(path.join(outputDir,filename),Buffer.from(frame.ingress.data,'base64'));
        delete frame.ingress.data;frame.ingress.dataFile=filename;
      }
    }
    for(const artifact of result.artifacts??[]){
      if(path.basename(artifact.name)!==artifact.name)throw Error('Unsafe diagnostic output name');
      fs.writeFileSync(path.join(outputDir,artifact.name),Buffer.from(artifact.bytes));
    }
    if(result.artifacts)result.artifacts=result.artifacts.map(({name,bytes})=>({name,bytes:bytes.length}));
    if(result.status==='failed')failed=true;
    fs.writeFileSync(path.join(outputDir, `${context}.json`), JSON.stringify(result, null, 2));
    const {logs: fullLogs, ...summary} = result;
    console.log(JSON.stringify({...summary, logEntries: fullLogs.length})); await page.close();
  }
} finally {
  await browser?.close(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve));
}
if(failed)process.exitCode=1;
