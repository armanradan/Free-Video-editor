// Diagnostic instrumentation only. Counts requested WebGPU resource bytes, not VRAM.
export function installAllocationLedger() {
  const buffers = new WeakMap(), textures = new WeakMap();
  const state = {bufferBytes:0, textureBytes:0, buffers:0, textures:0,
    peakBytes:0, windowPeakBytes:0, largestBufferBytes:0, createdBuffers:0,
    unknownTextureRequests:0, rejectedRequests:0, capBytes:384*1024*1024};
  const peak = () => {
    const bytes=state.bufferBytes+state.textureBytes;
    state.peakBytes=Math.max(state.peakBytes,bytes);
    state.windowPeakBytes=Math.max(state.windowPeakBytes,bytes);
  };
  const reserve = bytes => {
    if (!Number.isSafeInteger(bytes) || bytes<0 || state.bufferBytes+state.textureBytes+bytes>state.capBytes) {
      state.rejectedRequests++;
      throw Error('Diagnostic WebGPU allocation cap rejected request before allocation');
    }
  };
  const createBuffer=GPUDevice.prototype.createBuffer, destroyBuffer=GPUBuffer.prototype.destroy;
  GPUDevice.prototype.createBuffer=function(descriptor){
    const bytes=Number(descriptor.size); reserve(bytes);
    const buffer=createBuffer.call(this,descriptor);
    buffers.set(buffer,bytes); state.bufferBytes+=bytes; state.buffers++;
    state.createdBuffers++; state.largestBufferBytes=Math.max(state.largestBufferBytes,bytes); peak();
    return buffer;
  };
  GPUBuffer.prototype.destroy=function(){
    const bytes=buffers.get(this);
    const result=destroyBuffer.call(this);
    if(bytes!==undefined){state.bufferBytes-=bytes;state.buffers--;buffers.delete(this);}
    return result;
  };
  const createTexture=GPUDevice.prototype.createTexture, destroyTexture=GPUTexture.prototype.destroy;
  GPUDevice.prototype.createTexture=function(descriptor){
    const size=descriptor.size;
    const width=Array.isArray(size)?size[0]:size.width;
    const height=(Array.isArray(size)?size[1]:size.height)??1;
    const depth=(Array.isArray(size)?size[2]:size.depthOrArrayLayers)??1;
    // Refuse to invent byte counts for unhandled formats/mipmap layouts.
    if(!['rgba8unorm','rgba8unorm-srgb','bgra8unorm','bgra8unorm-srgb','r32float','rgba16float'].includes(descriptor.format)
        || (descriptor.mipLevelCount??1)!==1 || (descriptor.dimension??'2d')!=='2d') {
      state.unknownTextureRequests++;
      throw Error('Diagnostic texture ledger does not handle this format/layout');
    }
    const bytes=width*height*depth*(descriptor.format==='rgba16float'?8:4)*(descriptor.sampleCount??1); reserve(bytes);
    const texture=createTexture.call(this,descriptor);
    textures.set(texture,bytes);state.textureBytes+=bytes;state.textures++;peak();return texture;
  };
  GPUTexture.prototype.destroy=function(){
    const bytes=textures.get(this);
    const result=destroyTexture.call(this);
    if(bytes!==undefined){state.textureBytes-=bytes;state.textures--;textures.delete(this);}
    return result;
  };
  globalThis.__aiAllocationLedger={snapshot:()=>({...state}),
    beginWindow:()=>{state.windowPeakBytes=state.bufferBytes+state.textureBytes;}};
}

export async function calibrate({session,device,ort,result}) {
  const ledger=globalThis.__aiAllocationLedger;
  if(!ledger)throw Error('Missing allocation instrumentation');
  result.mode='tiled-allocation-calibration-not-video-acceptance';
  result.memoryScope='Requested buffer/known texture bytes less explicit destroy; includes runtime pools and diagnostic buffers, excludes driver/compiler/internal surface memory. GC is not observed.';
  result.calibrations=[];
  const reference=await(await fetch('/calibration/reference.json')).json();
  const floats=async(name,hash)=>{
    const bytes=await(await fetch(`/calibration/${name}`)).arrayBuffer();
    const actualHash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),v=>v.toString(16).padStart(2,'0')).join('');
    if(actualHash!==hash)throw Error(`Reference hash mismatch: ${name}`);
    const view=new DataView(bytes);
    if(view.getUint8(6)!==1)throw Error('Expected NPY v1');
    return new Float32Array(bytes.slice(10+view.getUint16(8,true)));
  };
  const publish=()=>{if(typeof document==='undefined')self.postMessage({aiProgress:result});};
  for(const item of reference.cases){
    const {width,height}=item, ow=width*4, oh=height*4;
    const stem=`${width}x${height}`;
    const input=await floats(`${stem}-input.npy`,item.input_sha256);
    const expected=await floats(`${stem}-fp32.npy`,item.reference_sha256);
    if(input.length!==width*height*3||expected.length!==ow*oh*3)throw Error('Bad reference shape');
    let previousSize=null;
    for(const interior of [128,256]){
      let previousOrder=null;
      for(const reverse of [false,true]){
        result.boundary=`calibration ${stem} tile ${interior} reverse ${reverse}`;
        const record={stem,interior,halo:34,reverse,tiles:0,maxTileWidth:0,maxTileHeight:0,
          diagnosticUploadBytes:0,diagnosticReadbackBytes:expected.byteLength,
          baseline:ledger.snapshot()};
        result.calibrations.push(record); publish(); ledger.beginWindow();
        // A full-frame tensor for GPU-only row assembly plus a separate validation readback.
        // This deliberately conservative diagnostic layout is not a production tensor policy.
        let assembled,readback,outputReservation;
        try {
          assembled=device.createBuffer({label:'diagnostic full-frame NCHW assembly',size:expected.byteLength,
            usage:GPUBufferUsage.COPY_SRC|GPUBufferUsage.COPY_DST});
          outputReservation=device.createTexture({label:'diagnostic RGBA8 output reservation (not rendered)',
            size:[ow,oh],format:'rgba8unorm',usage:GPUTextureUsage.RENDER_ATTACHMENT|GPUTextureUsage.TEXTURE_BINDING});
          const positions=[];
          for(let top=0;top<height;top+=interior)for(let left=0;left<width;left+=interior)positions.push([top,left]);
          if(reverse)positions.reverse();
          const started=performance.now();
          for(const [top,left] of positions){
            const bottom=Math.min(top+interior,height),right=Math.min(left+interior,width);
            const y0=Math.max(0,top-34),x0=Math.max(0,left-34);
            const y1=Math.min(height,bottom+34),x1=Math.min(width,right+34);
            const tw=x1-x0,th=y1-y0,tow=tw*4,toh=th*4;
            record.maxTileWidth=Math.max(record.maxTileWidth,tw);record.maxTileHeight=Math.max(record.maxTileHeight,th);
            const pixels=new Float32Array(tw*th*3);
            for(let c=0;c<3;c++)for(let row=0;row<th;row++)pixels.set(
              input.subarray(c*width*height+(y0+row)*width+x0,c*width*height+(y0+row)*width+x1),c*tw*th+row*tw);
            let buffer,tensor,output;
            try{
              buffer=device.createBuffer({size:Math.ceil(pixels.byteLength/16)*16,
                usage:GPUBufferUsage.STORAGE|GPUBufferUsage.COPY_DST|GPUBufferUsage.COPY_SRC});
              device.queue.writeBuffer(buffer,0,pixels);record.diagnosticUploadBytes+=pixels.byteLength;
              tensor=ort.Tensor.fromGpuBuffer(buffer,{dataType:'float32',dims:[1,3,th,tw]});
              output=(await session.run({input:tensor})).output;
              if(output.location!=='gpu-buffer'||output.dims.join()!==[1,3,toh,tow].join())throw Error('Invalid tile output');
              const encoder=device.createCommandEncoder();
              for(let c=0;c<3;c++)for(let row=0;row<(bottom-top)*4;row++){
                const source=(c*tow*toh+((top-y0)*4+row)*tow+(left-x0)*4)*4;
                const target=(c*ow*oh+(top*4+row)*ow+left*4)*4;
                encoder.copyBufferToBuffer(output.gpuBuffer,source,assembled,target,(right-left)*4*4);
              }
              device.queue.submit([encoder.finish()]);await device.queue.onSubmittedWorkDone();
              record.tiles++;
            } finally {output?.dispose();tensor?.dispose();buffer?.destroy();}
          }
          record.inferenceAndAssemblyMs=performance.now()-started;
          record.beforeReadback=ledger.snapshot();
          // Validation must not double full-frame GPU storage. Drain a bounded 8 MiB
          // staging buffer; these copies remain explicitly diagnostic.
          const chunkBytes=Math.min(expected.byteLength,8*1024*1024);
          record.diagnosticReadbackBufferBytes=chunkBytes;
          readback=device.createBuffer({size:chunkBytes,usage:GPUBufferUsage.COPY_DST|GPUBufferUsage.MAP_READ});
          const actual=new Float32Array(expected.length);
          for(let offset=0;offset<expected.byteLength;offset+=chunkBytes){
            const bytes=Math.min(chunkBytes,expected.byteLength-offset);
            const encoder=device.createCommandEncoder();encoder.copyBufferToBuffer(assembled,offset,readback,0,bytes);
            device.queue.submit([encoder.finish()]);await readback.mapAsync(GPUMapMode.READ);
            actual.set(new Float32Array(readback.getMappedRange(),0,bytes/4),offset/4);readback.unmap();
          }
          const metrics=(other,region)=>{
            let maxAbs=0,sum=0,count=0;
            for(let c=0;c<3;c++)for(let y=0;y<oh;y++)for(let x=0;x<ow;x++){
              if(region&&!region(x,y))continue;
              const i=c*ow*oh+y*ow+x,e=Math.abs(actual[i]-other[i]);
              if(!Number.isFinite(e))throw Error('Nonfinite tile output');maxAbs=Math.max(maxAbs,e);sum+=e;count++;
            }
            const value={maxAbs,mae:sum/count,count};
            if(maxAbs>.01||value.mae>.001)throw Error('Tiled reference tolerance exceeded');return value;
          };
          record.numeric=metrics(expected);
          record.edges=metrics(expected,(x,y)=>x<4||y<4||x>=ow-4||y>=oh-4);
          const step=interior*4;
          const nearSeam=(v,extent)=>{
            const index=Math.round(v/step);
            return index>0&&index*step<extent&&Math.abs(v-index*step)<4;
          };
          record.seams=metrics(expected,(x,y)=>nearSeam(x,ow)||nearSeam(y,oh));
          if(previousOrder)record.orderDifference=metrics(previousOrder);
          if(previousSize)record.tileSizeDifference=metrics(previousSize);
          previousOrder=actual;
          record.withReadback=ledger.snapshot();
        } finally {readback?.destroy();assembled?.destroy();outputReservation?.destroy();}
        record.afterCleanup=ledger.snapshot();publish();
      }
      previousSize=previousOrder;
    }
  }
  // Exercise admission failure without attempting an enormous GPU allocation.
  try {device.createBuffer({size:ledger.snapshot().capBytes+4,usage:GPUBufferUsage.STORAGE});
    throw Error('Allocation admission guard did not reject');
  } catch(error){if(!String(error).includes('cap rejected request'))throw error;
    result.allocationGuardRejectedBeforeGpuCall=true;}
  result.allocationSummary=ledger.snapshot();
}
