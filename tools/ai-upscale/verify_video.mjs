// Independent FFprobe grids + complete FFmpeg A/V decode, after browser checks.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {spawn,spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {expectedTimeline,assertTimeline} from '../../tests/color-timing-reference.mjs';
const [directory,outputName='independent-video.json']=process.argv.slice(2);
if(!directory)throw Error('expected diagnostic output directory');
assert.equal(path.basename(outputName),outputName);
const destination=path.join(directory,outputName);
const native=process.env.AI_UPSCALE_NATIVE_VIDEO==='1';
assert.ok(!fs.existsSync(destination),'preserve prior evidence');
const run=(exe,args,binary=false)=>{
  const r=spawnSync(exe,args,{encoding:binary?null:'utf8',timeout:60000,maxBuffer:64*1024*1024});
  assert.equal(r.status,0,r.stderr?.toString());return r.stdout;
};
const probe=(file,args)=>JSON.parse(run('ffprobe',['-v','error',...args,'-of','json',file]));
// Stream the full PCM decode instead of retaining a clip-length audio array.
const peakAudio=async(file)=>{
  const child=spawn('ffmpeg',['-v','error','-i',file,'-map','0:a:0','-f','f32le','-']);
  let errors='',peak=0,tail=Buffer.alloc(0);
  const exit=new Promise((resolve,reject)=>{child.once('error',reject);child.once('close',resolve);});
  child.stderr.on('data',chunk=>{errors=(errors+chunk.toString()).slice(-8192);});
  const timer=setTimeout(()=>child.kill(),60000);
  try{
    for await(const chunk of child.stdout){
      const data=tail.length?Buffer.concat([tail,chunk]):chunk;
      const end=data.length-data.length%4;
      for(let i=0;i<end;i+=4)peak=Math.max(peak,Math.abs(data.readFloatLE(i)));
      tail=Buffer.from(data.subarray(end));
    }
    assert.equal(await exit,0,errors);assert.equal(tail.length,0);return peak;
  }finally{clearTimeout(timer);if(child.exitCode===null)child.kill();}
};
const report={status:'running',cases:[]};
try{
  const contexts=native?['native']:JSON.parse(process.env.AI_UPSCALE_VERIFY_CONTEXTS??'["main","worker"]');
  assert.ok(contexts.length&&contexts.every(c=>/^(main|worker)(-(light|dark))?$/.test(c)||c==='native'));
  for(const context of contexts){
    const evidence=JSON.parse(fs.readFileSync(path.join(directory,`${context}.json`),'utf8'));
    const nativeUi=native&&evidence.status==='ordinary-native-ai-ui-exports-observed';
    const ui=evidence.status==='ordinary-ai-ui-acceptance-passed';
    if(ui)assert.deepEqual(evidence.pageErrors,[]);
    else if(!nativeUi) {
      assert.equal(evidence.status,native?'native-video-pilot-passed-not-release-accepted':'video-pilot-passed-not-release-accepted');
      if(!native)assert.equal(evidence.gpuValidationError,null);
    }
    for(const [index,item] of evidence.cases.entries()){
      const input=path.resolve(process.env.AI_UPSCALE_FIXTURE_DIR??'fixtures',item.request.fixture);
      const streams=probe(input,['-show_streams']).streams;
      const video=streams.find(s=>s.codec_type==='video'),audio=streams.find(s=>s.codec_type==='audio');
      const pts=probe(input,['-select_streams','v:0','-show_frames','-show_entries','frame=best_effort_timestamp_time']).frames.map(f=>Number(f.best_effort_timestamp_time));
      const packets=probe(input,['-select_streams','a:0','-show_packets','-show_entries','packet=pts']).packets;
      const videoPackets=probe(input,['-select_streams','v:0','-show_packets','-show_entries','packet=pts,duration']).packets;
      // VFR/B-frame MP4's declared track duration can omit the final presentation
      // interval. Derive its exact endpoint from all packet PTS+duration ticks;
      // never shorten real source coverage to a decode-duration header.
      const presentationEnd=videoPackets.reduce((end,p)=>{
        assert.ok(BigInt(p.duration)>0n);
        const next=BigInt(p.pts)+BigInt(p.duration);return next>end?next:end;
      },BigInt(video.start_pts));
      const presentationStreams=streams.map(s=>s===video?{...s,duration_ts:String(presentationEnd-BigInt(video.start_pts))}:s);
      const profile=item.request.profile??'mp4-h264-aac';
      const [n,d]=(profile==='webm-vp8-video-only'?video:audio).time_base.split('/').map(BigInt);
      // A video-only export excludes the earlier audio origin from its grid.
      // Native FFmpeg decoding honors AAC skip-samples/edit-list trimming and
      // uses the stream presentation origin. Browser demux exposes packet PTS,
      // including priming; retain that separately measured route's oracle.
      // In particular, a negative priming packet is not native audible start.
      const origin=native?null:{n:BigInt(profile==='webm-vp8-video-only'?video.start_pts:packets[0].pts)*n,d};
      assert.equal(item.inputFrameCount,pts.length);
      if(item.request.scale!=='off'&&!item.request.bilinear&&!nativeUi)assert.equal(item.aiSourceFrames,pts.length);
      if(nativeUi){
        assert.equal(item.aiSourceFrames,null,'ordinary UI does not expose inference counters');
      }else if(native){
        assert.equal(item.report.equalization_compute_passes,item.request.clahe?pts.length*4:0);
        assert.equal(item.report.ai.source_frames,item.request.bilinear?0:pts.length);
      }else{
        if(item.request.clahe)assert.match(item.summary,new RegExp(`CLAHE: ${pts.length} recorded source frames`));
        assert.match(item.summary,/Cleanup: 0 application-held frame references, 0 samples/);
        assert.match(item.summary,/leases live\/peak=0\//);
      }
      const file=path.join(directory,evidence.artifacts[index].name);
      const actual=probe(file,['-select_streams','v:0','-show_frames','-show_entries','frame=best_effort_timestamp_time']).frames.map(f=>Number(f.best_effort_timestamp_time));
      const timeline=assertTimeline(actual,expectedTimeline(presentationStreams,pts,item.request.fps,origin));
      const output=probe(file,['-show_streams']).streams,ov=output.find(s=>s.codec_type==='video'),oa=output.find(s=>s.codec_type==='audio');
      const outputPackets=probe(file,['-select_streams','v:0','-show_packets','-show_entries','packet=pts,duration']).packets;
      const outputEnd=outputPackets.reduce((end,p)=>{
        assert.ok(BigInt(p.duration)>0n);
        const next=BigInt(p.pts)+BigInt(p.duration);return next>end?next:end;
      },BigInt(ov.start_pts));
      const [on,od]=ov.time_base.split('/').map(Number);
      let expectedEnd;
      if(item.request.fps==='original'){
        const [vn,vd]=video.time_base.split('/').map(Number);
        const nativeOrigin=Math.min(Number(video.start_time),audio?Number(audio.start_time):Infinity);
        expectedEnd=Number(presentationEnd)*vn/vd-(native?nativeOrigin:Number(origin.n)/Number(origin.d));
      }else{
        const [fn,fd=1]=item.request.fps.split('/').map(Number);
        expectedEnd=actual.at(-1)+fd/fn;
      }
      const endpointErrorSeconds=Math.abs(Number(outputEnd)*on/od-expectedEnd);
      assert.ok(endpointErrorSeconds<=.0011,`presentation endpoint differs by ${endpointErrorSeconds} seconds`);
      assert.ok([undefined,'N/A','1:1'].includes(video.sample_aspect_ratio));
      const rotated=video.side_data_list?.some(s=>Math.abs(s.rotation)%180===90);
      const factor=item.request.scale==='off'?1:Number(item.request.scale[0]);
      assert.deepEqual([ov.width,ov.height],rotated?[video.height*factor,video.width*factor]:[video.width*factor,video.height*factor]);
      if(profile==='webm-vp8-video-only')assert.equal(oa,undefined);
      else assert.equal(oa.codec_name,profile==='webm-vp8-opus'?'opus':'aac');
      run('ffmpeg',['-v','error','-i',file,'-f','null','-']);
      let audioPeak=null;
      if(oa){
        audioPeak=await peakAudio(file);
        assert.ok(audioPeak>.01);
      }
      report.cases.push({context,request:item.request,timeline,video:ov,audio:oa??null,audioPeak,
        outputPresentationEndTicks:String(outputEnd),endpointErrorSeconds,
        sourceFrames:pts.length,aiSourceFrames:nativeUi?null:item.aiSourceFrames??0,
        sourceDeclaredVideoDurationTicks:video.duration_ts,sourcePresentationEndTicks:String(presentationEnd),
        bytes:fs.statSync(file).size,sha256:createHash('sha256').update(fs.readFileSync(file)).digest('hex')});
    }
  }
  report.status='passed';
}catch(error){report.status='failed';report.error=String(error.stack);throw error;}
finally{fs.writeFileSync(destination,JSON.stringify(report,null,2));}
console.log(JSON.stringify({status:report.status,cases:report.cases.length}));
