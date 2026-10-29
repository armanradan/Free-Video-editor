// Diagnostic instrumentation only; never imported by application code.
export function installAiFailureInstrumentation() {
  const state=globalThis.__aiFailure={mode:null,activeRuns:0,callerTensors:0,owners:[],
    resources:new Set(),resourceInfo:new WeakMap(),patchedTensors:new WeakSet(),uniformsToTag:0,
    triggered:false,submittedDuringRun:0,dispatchesDuringRun:0,oomArmed:false};
  const scopes=new WeakMap();
  const push=GPUDevice.prototype.pushErrorScope,pop=GPUDevice.prototype.popErrorScope;
  GPUDevice.prototype.pushErrorScope=function(filter){
    const stack=scopes.get(this)??[];stack.push(filter);scopes.set(this,stack);return push.call(this,filter);
  };
  GPUDevice.prototype.popErrorScope=async function(){
    const filter=scopes.get(this)?.pop(),actual=await pop.call(this);
    if(filter==='out-of-memory'&&(state.mode==='oom-setup'||state.oomArmed)){
      state.mode=null;state.oomArmed=false;state.triggered=true;
      return new GPUOutOfMemoryError('INJECTED: diagnostic scoped allocation failure; not physical exhaustion');
    }
    return actual;
  };
  for(const [type,Constructor] of [['Buffer',GPUBuffer],['Texture',GPUTexture]]){
    const create=GPUDevice.prototype[`create${type}`],destroy=Constructor.prototype.destroy;
    GPUDevice.prototype[`create${type}`]=function(descriptor){
      const resource=create.call(this,descriptor);
      let tracked=descriptor.label?.startsWith('AI ');
      if(type==='Buffer'&&descriptor.label==='frame orientation uniform'&&state.uniformsToTag>0){
        tracked=true;state.uniformsToTag--;
      }
      if(tracked){state.resources.add(resource);state.resourceInfo.set(resource,{type,label:descriptor.label});}
      return resource;
    };
    Constructor.prototype.destroy=function(){const result=destroy.call(this);state.resources.delete(this);return result;};
  }
  const dispatch=GPUComputePassEncoder.prototype.dispatchWorkgroups;
  GPUComputePassEncoder.prototype.dispatchWorkgroups=function(...args){
    const result=dispatch.apply(this,args);
    if(state.activeRuns){
      state.dispatchesDuringRun++;
      if(state.mode==='cancel-active'){
        state.mode=null;state.triggered=true;state.cancelLocal();
      }
    }
    return result;
  };
  const submit=GPUQueue.prototype.submit;
  GPUQueue.prototype.submit=function(commands){
    const result=submit.call(this,commands);
    if(state.activeRuns||state.owners.some(record=>record.owner.busy)){
      state.submittedDuringRun++;
      if(state.mode==='device-loss-active'){
        state.mode=null;state.triggered=true;state.runDevice.destroy();
      }
    }
    return result;
  };
  state.snapshot=()=>({activeRuns:state.activeRuns,callerTensors:state.callerTensors,
    resources:[...state.resources].map(r=>state.resourceInfo.get(r)),
    sessions:state.owners.map(({owner,released,runs})=>({released,runs,busy:owner.busy,
      poisoned:owner.poisoned,active:!!owner.active,sessionRetained:!!owner.session})),
    triggered:state.triggered,dispatchesDuringRun:state.dispatchesDuringRun,submittedDuringRun:state.submittedDuringRun});
}

function instrumentOwner(owner) {
  const state=globalThis.__aiFailure;
  if(!state)throw Error('Missing diagnostic failure instrumentation');
  const Tensor=owner.ort.Tensor;
  if(!state.patchedTensors.has(Tensor)){
    const from=Tensor.fromGpuBuffer;
    Tensor.fromGpuBuffer=function(...args){
      const tensor=from.apply(this,args),dispose=tensor.dispose.bind(tensor);let closed=false;
      state.callerTensors++;
      tensor.dispose=()=>{const result=dispose();if(!closed){closed=true;state.callerTensors--;}return result;};
      return tensor;
    };
    state.patchedTensors.add(Tensor);
  }
  const session=owner.session,run=session.run.bind(session),release=session.release.bind(session);
  const record={owner,released:false,runs:0};state.owners.push(record);
  session.run=async(feeds,fetches)=>{
    state.activeRuns++;record.runs++;state.runDevice=owner.device;
    try{
      if(state.mode==='runtime-error'){
        state.mode=null;state.triggered=true;
        // Exercise the real runtime's input contract, not a fabricated rejection.
        return await run({wrong_input_name:feeds.input},fetches);
      }
      if(state.mode==='oom-run')state.oomArmed=true;
      return await run(feeds,fetches);
    }finally{state.activeRuns--;}
  };
  session.release=async()=>{
    await release();record.released=true;
    if(state.mode==='release-error-after-close'){
      state.mode=null;state.triggered=true;
      throw Error('INJECTED: session release response failed after actual runtime release');
    }
  };
  state.uniformsToTag=2; // The video stage's orientation + neutral uniforms.
  return owner;
}

export function instrumentAiBoundary(source) {
  const anchor='export async function createAiSession(';
  if(!source.includes(anchor))throw Error('AI diagnostic boundary anchor changed');
  return source.replace(anchor,'async function actualCreateAiSession(')+
    `\nexport async function createAiSession(...args){return (${instrumentOwner.toString()})(await actualCreateAiSession(...args));}\n`;
}
