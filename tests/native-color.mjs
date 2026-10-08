// Actual native color-route gate; synthetic CC0 fixtures, no user media.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
const directory=path.resolve(`tmp/color-native/${process.pid}`);fs.mkdirSync(directory,{recursive:true});
const evidence={status:'running',cases:[]};
const execute=(program,args,encoding='utf8')=>spawnSync(program,args,{encoding,timeout:120000,maxBuffer:32*1024*1024});
const run=(program,args,encoding='utf8')=>{const value=execute(program,args,encoding);assert.equal(value.status,0,String(value.stderr));return value.stdout;};
run('cargo',['build','--locked','-p','media-native','--bin','native-convert']);
const metadata=JSON.parse(run('cargo',['metadata','--no-deps','--format-version','1']));
const exe=path.join(metadata.target_directory,'debug','native-convert.exe');
const adapter=JSON.parse(run(exe,['list-gpus'])).find(value=>value.vendor===0x10de);
assert.ok(adapter,'NVIDIA adapter required for explicit staged-hardware gate');evidence.adapter=adapter;
let index=0;
try {
  for(const [route,source,profile,resize,fps] of [
    ['direct','m2-h264-aac.mp4','mp4-h264-aac','original','original'],
    ['wgpu','m2-h264-aac.mp4','mp4-h264-aac','original','original'],
    ['wgpu-nvidia','m2-h264-aac.mp4','mp4-h264-aac','50','15'],
    ['direct','m4-10bit-sdr.mp4','mp4-h265-main10-aac','original','original'],
    ['direct','m35-geometry-color.mp4','mp4-h264-aac','50','original'],
    ['direct','m35-vfr-offset.mp4','mp4-h264-aac','original','original'],
    ['wgpu','m35-vfr-offset.mp4','mp4-h264-aac','50','15'],
  ]) {
    const output=path.join(directory,`${++index}-${route}.mp4`);
    const report=JSON.parse(run(exe,['convert','--input',path.resolve('fixtures',source),'--output',output,'--route',route,'--profile',profile,'--resize',resize,'--fps',fps,'--brightness','10','--contrast','150','--saturation','0','--adapter-key',adapter.key]));
    const info=JSON.parse(run('ffprobe',['-v','error','-show_streams','-show_format','-of','json',output]));
    const video=info.streams.find(s=>s.codec_type==='video');assert.ok(info.streams.some(s=>s.codec_type==='audio'));
    const frames=JSON.parse(run('ffprobe',['-v','error','-select_streams','v:0','-show_frames','-show_entries','frame=best_effort_timestamp_time','-of','json',output])).frames;
    if(fps==='original')assert.equal(frames.length,report.input.frame_count);
    else frames.forEach((frame,i)=>assert.ok(Math.abs(Number(frame.best_effort_timestamp_time)-Number(frames[0].best_effort_timestamp_time)-i/Number(fps))<.00002));
    assert.equal(video.width,report.output_width);assert.equal(video.height,report.output_height);
    assert.deepEqual(report.color,{brightness:10,contrast:150,saturation:0});
    if(profile.includes('main10'))assert.equal(video.pix_fmt,'yuv420p10le');
    const pixels=run('ffmpeg',['-v','error','-i',output,'-frames:v','1','-f','rawvideo','-pix_fmt','rgb24','-'],null);
    let spread=0,low=255,high=0;
    for(let i=0;i<pixels.length;i+=3){const rgb=[pixels[i],pixels[i+1],pixels[i+2]];spread=Math.max(spread,Math.max(...rgb)-Math.min(...rgb));low=Math.min(low,rgb[0]);high=Math.max(high,rgb[0]);}
    assert.ok(spread<=4,`not gray: ${spread}`);assert.ok(high-low>20,'blank image');
    run('ffmpeg',['-v','error','-i',output,'-f','null','-']);
    evidence.cases.push({route,source,profile,resize,fps,output,report,video,frames:frames.length,graySpread:spread});
    console.log(`${route} ${source} ${profile}: ${frames.length} frames PASS`);
  }
  // Lossy acceptance limits are deliberately separate from the <=2-code-value
  // pre-encoder shader/CPU gate. Compare identical 640x360 geometry and seeks.
  const normalized='zscale=matrixin=709:transferin=709:primariesin=709:rangein=limited:matrix=gbr:transfer=iec61966-2-1:primaries=709:range=full,format=gbrp,format=rgba';
  const decode=(input,position)=>run('ffmpeg',['-v','error','-ss',String(position),'-i',input,'-vf',normalized,'-frames:v','1','-f','rawvideo','-pix_fmt','rgba','-'],null);
  const source=path.resolve('fixtures/m2-h264-aac.mp4');
  for(const route of ['direct','wgpu','wgpu-nvidia']) {
    const output=path.join(directory,`${++index}-${route}-combined.mp4`);
    const report=JSON.parse(run(exe,['convert','--input',source,'--output',output,'--route',route,'--resize','original','--fps','original','--brightness','10','--contrast','150','--saturation','50','--adapter-key',adapter.key]));
    const comparisons=[];
    for(const position of [0,.5]) {
      const original=decode(source,position),actual=decode(output,position);
      assert.equal(original.length,640*360*4);assert.equal(actual.length,original.length);
      const errors=[];let total=0;
      for(let i=0;i<original.length;i+=4) {
        const rgb=[original[i]/255,original[i+1]/255,original[i+2]/255];
        const y=.2126*rgb[0]+.7152*rgb[1]+.0722*rgb[2];
        for(let channel=0;channel<3;channel++) {
          const expected=Math.round(255*Math.max(0,Math.min(1,((y+.5*(rgb[channel]-y))-.5)*1.5+.5+.1)));
          const error=Math.abs(actual[i+channel]-expected);errors.push(error);total+=error;
        }
        assert.equal(actual[i+3],255);
      }
      errors.sort((a,b)=>a-b);
      const metric={position,mean:total/errors.length,p95:errors[Math.floor(errors.length*.95)],p99:errors[Math.floor(errors.length*.99)],max:errors.at(-1),limits:{mean:6,p95:15}};
      comparisons.push(metric);
      console.log(`${route} same-position ${position}s: ${JSON.stringify(metric)}`);
      assert.ok(metric.mean<=6&&metric.p95<=15,'lossy preview/export difference exceeds declared bounds');
    }
    const frames=JSON.parse(run('ffprobe',['-v','error','-select_streams','v:0','-show_frames','-show_entries','frame=best_effort_timestamp_time','-of','json',output])).frames;
    assert.equal(frames.length,60);
    frames.forEach((frame,i)=>assert.ok(Math.abs(Number(frame.best_effort_timestamp_time)-i/30)<.00002));
    run('ffmpeg',['-v','error','-i',output,'-f','null','-']);
    evidence.cases.push({route,output,report,comparisons,frames:frames.length});
  }
  for(const [route,args,pattern] of [
    ['nvidia',['--saturation','0'],/Direct NVIDIA color adjustments are not supported/],
    ['direct',['--brightness','101'],/brightness must be/],
    ['direct',['--contrast','201'],/brightness must be/],
  ]) {
    const output=path.join(directory,`rejected-${++index}.mp4`);
    const result=execute(exe,['convert','--input',path.resolve('fixtures/m2-h264-aac.mp4'),'--output',output,'--route',route,...args]);
    assert.notEqual(result.status,0);assert.match(result.stderr,pattern);assert.equal(fs.existsSync(output),false);
    assert.equal(fs.readdirSync(directory).some(name=>name.includes('.partial')),false);
    evidence.cases.push({route,rejection:result.stderr.trim()});
  }
  evidence.status='passed';
} catch(error){evidence.status='failed';evidence.error=String(error.stack);throw error;}
finally{fs.writeFileSync(path.join(directory,'evidence.json'),JSON.stringify(evidence,null,2));console.log(`Evidence: ${directory}`);}
