// Actual native CLI/codec acceptance. Pixel-math oracle remains in GPU tests.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { assertTimeline, expectedTimeline } from './color-timing-reference.mjs';

const directory = path.resolve(`tmp/clahe/native-${process.pid}`);
fs.mkdirSync(directory, { recursive: true });
const evidence = { status: 'running', cases: [], rejections: [] };
const call = (program, args, binary=false) => spawnSync(program, args, {
  encoding: binary ? null : 'utf8', timeout: 120000, maxBuffer: 64*1024*1024,
});
const run = (program, args, binary=false) => {
  const result = call(program,args,binary);
  assert.equal(result.status,0,`${program}: ${result.stderr}`);
  return result.stdout;
};
const probe = file => JSON.parse(run('ffprobe',['-v','error','-show_streams','-of','json',file])).streams;
const pts = file => JSON.parse(run('ffprobe',['-v','error','-select_streams','v:0','-show_frames',
  '-show_entries','frame=best_effort_timestamp_time','-of','json',file])).frames.map(f=>Number(f.best_effort_timestamp_time));
const decode = file => run('ffmpeg',['-v','error','-i',file,'-frames:v','1','-f','rawvideo','-pix_fmt','rgb24','-'],true);
try {
  run('cargo',['build','--locked','-p','media-native','--bin','native-convert']);
  const metadata=JSON.parse(run('cargo',['metadata','--no-deps','--format-version','1']));
  const exe=path.join(metadata.target_directory,'debug',process.platform==='win32'?'native-convert.exe':'native-convert');
  const adapters=JSON.parse(run(exe,['list-gpus']));
  const adapter=adapters.find(a=>a.vendor===0x10de);
  evidence.adapter=adapter??null;
  evidence.ffmpeg=run('ffmpeg',['-version']).split('\n')[0];
  const source=path.resolve('fixtures/m35-vfr-offset.mp4');
  const inputStreams=probe(source), inputPts=pts(source);
  const convert = (route,fps,strength,tag) => {
    const output=path.join(directory,`${tag}.mp4`);
    const args=['convert','--input',source,'--output',output,'--route',route,'--fps',fps,
      '--resize','50','--bitrate','8','--brightness','10','--contrast','80','--saturation','60',
      ...(strength===null?[]:['--equalization-strength',String(strength)]),
      ...(adapter?['--adapter-key',adapter.key]:[])];
    const report=JSON.parse(run(exe,args));
    const streams=probe(output),video=streams.find(s=>s.codec_type==='video'),audio=streams.find(s=>s.codec_type==='audio');
    const timeline=assertTimeline(pts(output),expectedTimeline(inputStreams,inputPts,fps));
    assert.equal(report.frames_processed,inputPts.length,'statistics must process all source frames before FPS resampling');
    assert.equal(report.output_frame_count,timeline.frames);
    assert.equal(report.equalization.strength,strength===null?50:strength);
    assert.equal(report.equalization.enabled,strength!==null);
    assert.equal(report.equalization_compute_passes,strength?inputPts.length*4:0);
    assert.ok(strength?report.equalization_gpu_scratch_bytes>0:report.equalization_gpu_scratch_bytes===0);
    const inputVideo=inputStreams.find(s=>s.codec_type==='video');
    assert.equal(report.explicit_cpu_to_gpu_bytes,inputVideo.width*inputVideo.height*4*inputPts.length);
    assert.equal(report.explicit_gpu_to_cpu_bytes,video.width*video.height*4*inputPts.length);
    assert.deepEqual([video.width,video.height],[report.output_width,report.output_height]);
    assert.equal(video.sample_aspect_ratio,'1:1');
    for(const [key,value] of Object.entries({color_space:'bt709',color_transfer:'bt709',color_primaries:'bt709',color_range:'tv'}))assert.equal(video[key],value);
    assert.equal(audio.codec_name,'aac');
    const inputAudio=inputStreams.find(s=>s.codec_type==='audio');
    const origin=Math.min(...inputStreams.map(s=>Number(s.start_time)));
    assert.ok(Math.abs(Number(audio.start_time)-(Number(inputAudio.start_time)-origin))<=.025);
    assert.ok(Math.abs(Number(audio.duration)-Number(inputAudio.duration))<=.025);
    const pcm=run('ffmpeg',['-v','error','-i',output,'-map','0:a:0','-f','f32le','-'],true);
    let peak=0;for(let i=0;i<pcm.length;i+=4)peak=Math.max(peak,Math.abs(pcm.readFloatLE(i)));
    assert.ok(peak>.01,'audio must not be silent');
    run('ffmpeg',['-v','error','-i',output,'-fps_mode','passthrough','-f','null','-']);
    evidence.cases.push({route,fps,strength,output,report,video,timeline,audio,peak});
    console.log(`${route} ${fps} strength=${strength}: ${timeline.frames} frames PASS`);
    return output;
  };
  const off=convert('wgpu','original',null,'off');
  const zero=convert('wgpu','original',0,'zero');
  assert.deepEqual(decode(off),decode(zero),'zero strength must preserve the old fused path');
  let enabled=null;
  for(const route of ['wgpu',...(adapter?['wgpu-nvidia']:[])]) {
    for(const fps of ['original','15','60']) {
      const output=convert(route,fps,50,`${route}-${fps}`);
      if(route==='wgpu'&&fps==='original')enabled=output;
    }
  }
  assert.notDeepEqual(decode(enabled),decode(off),'enabled CLAHE must reach the encoded pixels');
  for(const [route,strength,profile,reason] of [
    ['direct','50','mp4-h264-aac','CLAHE is not supported'],
    ['nvidia','50','mp4-h264-aac','CLAHE is not supported'],
    ['wgpu','101','mp4-h264-aac','strength must be 0..100'],
    ['wgpu','50','mp4-h265-main10-aac','cannot preserve 10-bit'],
  ]) {
    const output=path.join(directory,`reject-${route}-${strength}-${profile}.mp4`);
    const result=call(exe,['convert','--input',source,'--output',output,'--route',route,
      '--profile',profile,'--equalization-strength',strength]);
    assert.notEqual(result.status,0);
    assert.ok(result.stderr.includes(reason),result.stderr);
    assert.ok(!fs.existsSync(output));
    assert.ok(!fs.readdirSync(directory).some(name=>name.startsWith(path.basename(output)+'.')&&name.endsWith('.partial.mp4')));
    evidence.rejections.push({route,strength,profile,reason});
  }
  evidence.status='passed';
} catch(error) {evidence.status='failed';evidence.error=String(error.stack);throw error;}
finally {fs.writeFileSync(path.join(directory,'evidence.json'),JSON.stringify(evidence,null,2));console.log(`Evidence: ${directory}`);}
