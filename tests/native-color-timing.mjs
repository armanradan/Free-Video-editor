// Real native adjusted VFR/CFR/Main 10 gate. CC0 inputs; evidence stays in tmp.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { expectedTimeline, assertTimeline, colorReference16, errors16 } from './color-timing-reference.mjs';

const directory = path.resolve(`tmp/color-timing/native-${process.pid}`);
fs.mkdirSync(directory, { recursive: true });
const evidence = { status:'running', cases:[], versions:{} };
const run = (program,args,binary=false,input) => {
  const value = spawnSync(program,args,{encoding:binary?null:'utf8',input,timeout:120000,maxBuffer:64*1024*1024});
  assert.equal(value.status,0,`${program}: ${value.stderr}`); return value.stdout;
};
const probe = file => JSON.parse(run('ffprobe',['-v','error','-show_streams','-of','json',file])).streams;
const pts = file => JSON.parse(run('ffprobe',['-v','error','-select_streams','v:0','-show_frames',
  '-show_entries','frame=best_effort_timestamp_time','-of','json',file])).frames.map(f=>Number(f.best_effort_timestamp_time));
const normalize = 'zscale=matrixin=709:transferin=709:primariesin=709:rangein=limited:matrix=gbr:transfer=iec61966-2-1:primaries=709:range=full,format=gbrp16le';
const settings = { brightness:10,contrast:80,saturation:60 };
const decode = file => run('ffmpeg',['-v','error','-i',file,'-vf',normalize,'-frames:v','1',
  '-f','rawvideo','-pix_fmt','rgb48le','-'],true);
try {
  evidence.versions.ffmpeg=run('ffmpeg',['-version']).split('\n')[0];
  evidence.versions.rust=run('rustc',['--version']).trim();
  run('cargo',['build','--locked','-p','media-native','--bin','native-convert']);
  const metadata=JSON.parse(run('cargo',['metadata','--no-deps','--format-version','1']));
  const exe=path.join(metadata.target_directory,'debug',process.platform==='win32'?'native-convert.exe':'native-convert');
  const adapters=JSON.parse(run(exe,['list-gpus']));
  const adapter=adapters.find(a=>a.vendor===0x10de);
  evidence.adapter=adapter??null;
  const routes=['direct','wgpu',...(adapter?['wgpu-nvidia']:[])];
  if(!adapter)evidence.hardwareSkipped='No NVIDIA adapter; staged route not tested';
  const cases=routes.flatMap(route=>['original','15','30000/1001'].map(fps=>
    ({route,fps,fixture:'m35-vfr-offset',profile:'mp4-h264-aac',resize:'original'})));
  cases.push(...['original','15','30000/1001'].map(fps=>
    ({route:'direct',fps,fixture:'m4-10bit-sdr',profile:'mp4-h265-main10-aac',resize:'original'})));
  cases.push({route:'direct',fps:'15',fixture:'m4-10bit-sdr',profile:'mp4-h265-main10-aac',resize:'50'});
  for (const item of cases) {
    const source=path.resolve(`fixtures/${item.fixture}.mp4`);
    const output=path.join(directory,`${evidence.cases.length}-${item.fixture}-${item.route}-${item.fps.replace('/','_')}-${item.resize}.mp4`);
    const inputStreams=probe(source), inputPts=pts(source);
    const report=JSON.parse(run(exe,['convert','--input',source,'--output',output,'--route',item.route,
      '--profile',item.profile,'--resize',item.resize,'--fps',item.fps,'--bitrate','8',
      '--brightness',String(settings.brightness),'--contrast',String(settings.contrast),'--saturation',String(settings.saturation),
      ...(adapter?['--adapter-key',adapter.key]:[])]));
    const streams=probe(output), video=streams.find(s=>s.codec_type==='video'), audio=streams.find(s=>s.codec_type==='audio');
    const timeline=assertTimeline(pts(output),expectedTimeline(inputStreams,inputPts,item.fps));
    assert.equal(report.frames_processed,inputPts.length);
    assert.equal(report.output_frame_count,timeline.frames);
    assert.deepEqual(report.color,settings);
    assert.deepEqual([video.width,video.height],[report.output_width,report.output_height]);
    assert.ok(!video.side_data_list?.some(s=>s.side_data_type==='Display Matrix'));
    assert.equal(video.sample_aspect_ratio,'1:1');
    for (const [key,value] of Object.entries({color_space:'bt709',color_transfer:'bt709',color_primaries:'bt709',color_range:'tv'})) assert.equal(video[key],value);
    const inputAudio=inputStreams.find(s=>s.codec_type==='audio');
    assert.equal(audio.codec_name,'aac');
    const origin=Math.min(...inputStreams.map(s=>Number(s.start_time)));
    assert.ok(Math.abs(Number(audio.start_time)-(Number(inputAudio.start_time)-origin))<=.025);
    assert.ok(Math.abs(Number(audio.duration)-Number(inputAudio.duration))<=.025);
    const pcm=run('ffmpeg',['-v','error','-i',output,'-map','0:a:0','-f','f32le','-'],true);
    let audioPeak=0; for(let i=0;i<pcm.length;i+=4)audioPeak=Math.max(audioPeak,Math.abs(pcm.readFloatLE(i)));
    assert.ok(audioPeak>.01,'silent audio');
    if(item.profile.includes('main10')) {
      assert.equal(video.pix_fmt,'yuv420p10le');assert.equal(video.profile,'Main 10');assert.equal(video.codec_tag_string,'hvc1');
    }
    run('ffmpeg',['-v','error','-i',output,'-fps_mode','passthrough','-enc_time_base:v','demux','-f','null','-']);
    let comparison=null;
    if(item.resize==='original') {
      const expected=colorReference16(decode(source),settings);
      comparison=errors16(decode(output),expected);
      assert.ok(comparison.mae8Equivalent<=6&&comparison.p95_8Equivalent<=15,JSON.stringify(comparison));
    }
    let non8BitLuma=null;
    if(item.profile.includes('main10')) {
      const yuv=run('ffmpeg',['-v','error','-i',output,'-frames:v','1','-f','rawvideo','-pix_fmt','yuv420p10le','-'],true);
      non8BitLuma=0;for(let i=0;i<video.width*video.height*2;i+=2)if(yuv.readUInt16LE(i)%4!==0)non8BitLuma++;
      assert.ok(non8BitLuma>0,'output has no sub-8-bit luma codes; not alone proof of precision');
    }
    evidence.cases.push({...item,settings,output,report,video,timeline,audio,audioPeak,comparison,non8BitLuma});
    fs.writeFileSync(path.join(directory,'evidence.json'),JSON.stringify(evidence,null,2));
    console.log(`${item.route} ${item.fixture} ${item.resize} ${item.fps}: ${timeline.frames} frames PASS`);
  }
  evidence.status='passed';
} catch(error) {evidence.status='failed';evidence.error=String(error.stack);throw error;}
finally {fs.writeFileSync(path.join(directory,'evidence.json'),JSON.stringify(evidence,null,2));console.log(`Evidence: ${directory}`);}
