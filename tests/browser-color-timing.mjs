// Supply the supported Browser runtime's tab-scoped CDP, file chooser and goto.
// No separate browser socket; test-only pixel readbacks stay out of conversion.
import assert from 'node:assert/strict';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { createColorAcceptance } from './browser-color-numeric.mjs';
import { expectedTimeline } from './color-timing-reference.mjs';

function fixtureTimeline(file) {
  const source=path.resolve(file);
  const probe=args=>{
    const value=spawnSync('ffprobe',['-v','error',...args,'-of','json',source],{encoding:'utf8',timeout:30000});
    assert.equal(value.status,0,value.stderr);return JSON.parse(value.stdout);
  };
  const streams=probe(['-show_streams']).streams;
  const pts=probe(['-select_streams','v:0','-show_frames','-show_entries','frame=best_effort_timestamp_time']).frames.map(f=>Number(f.best_effort_timestamp_time));
  // Browser demux exposes AAC priming packets, unlike native stream start.
  const packets=probe(['-select_streams','a:0','-show_packets','-show_entries','packet=pts']).packets;
  const audio=streams.find(s=>s.codec_type==='audio');
  const [num,den]=audio.time_base.split('/').map(BigInt);
  const origin={n:BigInt(packets[0].pts)*num,d:den};
  return {source,streams,pts,origin};
}

export async function runColorTimingAcceptance({send,selectFile,navigate,directory,baseUrl='http://127.0.0.1:8084/',contexts=['worker','main']}) {
  const {source,streams,pts,origin}=fixtureTimeline('fixtures/m35-vfr-offset.mp4');
  const settings={brightness:10,contrast:80,saturation:60};
  const reports=[];
  for(const context of contexts) {
    const gate=await createColorAcceptance({send,selectFile,navigate,directory:path.join(directory,context),expectedFrames:pts.length});
    const url=new URL(baseUrl);url.searchParams.set('verify','full');url.searchParams.set('diagnostic-preview','1');
    if(context==='main')url.searchParams.set('execution','main');
    await gate.setup(url.href,context,source);
    assert.match(gate.evidence.environment.execution,context==='worker'?/dedicated worker/:/main-thread/);
    await gate.previewCase('combined',settings);
    const profiles=await gate.evaluate("Array.from(document.querySelector('#output-profile').options).filter(o=>!o.disabled).map(o=>o.value)");
    for(const profile of ['mp4-h264-aac','webm-vp8-opus']) {
      assert.ok(profiles.includes(profile),`Required test profile unavailable: ${profile}`);
      for(const fps of ['original','15','30000/1001']) {
        await gate.set('#output-fps',fps,'change');
        await gate.wait("!document.querySelector('#convert').disabled");
        await gate.exportCase(`adjusted-${profile}-${fps.replace('/','_')}`,settings,profile,false,expectedTimeline(streams,pts,fps,origin));
      }
    }
    await gate.finish();reports.push(gate.evidence);
  }
  return reports;
}

export async function runFfmpegColorTimingAcceptance({send,selectFile,navigate,directory,baseUrl='http://127.0.0.1:8084/'}) {
  const {source,streams,pts,origin}=fixtureTimeline('fixtures/m2-h264-aac.mp4');
  const settings={brightness:10,contrast:80,saturation:60};
  const gate=await createColorAcceptance({send,selectFile,navigate,directory,expectedFrames:pts.length});
  const url=new URL(baseUrl);
  for(const [key,value] of [['verify','full'],['diagnostic-preview','1'],['backend','ffmpeg-wasm']])url.searchParams.set(key,value);
  await gate.setup(url.href,'ffmpeg',source);
  await gate.set('#video-bitrate','custom','change');
  await gate.set('#custom-bitrate','8','input');
  await gate.previewCase('combined',settings);
  for(const fps of ['15','30000/1001']) {
    await gate.set('#output-fps',fps,'change');await gate.wait("!document.querySelector('#convert').disabled");
    await gate.exportCase(`fixed-${fps.replace('/','_')}`,settings,'mp4-h264-aac',false,expectedTimeline(streams,pts,fps,origin));
  }
  await gate.finish();return gate.evidence;
}
