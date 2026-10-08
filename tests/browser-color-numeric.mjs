// Developer acceptance runner. Supply a tab-scoped CDP `send` and a file chooser
// callback from the supported browser runtime; this module opens no browser socket.
// All pixel readbacks here are diagnostic-only, outside conversion metrics.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

export function colorReference(rgba, { brightness, contrast, saturation }) {
  const output = Buffer.alloc(rgba.length / 4 * 3);
  for (let i = 0, j = 0; i < rgba.length; i += 4, j += 3) {
    const rgb = [rgba[i], rgba[i + 1], rgba[i + 2]].map(x => x / 255);
    const y = .2126 * rgb[0] + .7152 * rgb[1] + .0722 * rgb[2];
    rgb.forEach((x, channel) => {
      const saturated = y + saturation / 100 * (x - y);
      output[j + channel] = Math.round(255 * Math.max(0, Math.min(1,
        (saturated - .5) * contrast / 100 + .5 + brightness / 100)));
    });
  }
  return output;
}

export function errors(actual, expected) {
  assert.equal(actual.length, expected.length, "pixel geometry differs");
  const histogram = new Uint32Array(256);
  let sum = 0, maximum = 0;
  for (let i = 0; i < actual.length; i++) {
    const error = Math.abs(actual[i] - expected[i]);
    histogram[error]++; sum += error; maximum = Math.max(maximum, error);
  }
  let cumulative = 0, p95 = 0;
  for (; p95 < 255; p95++) {
    cumulative += histogram[p95];
    if (cumulative >= actual.length * .95) break;
  }
  return { maximum, mae: sum / actual.length, p95, channels: actual.length };
}

// Test-only oracle: rotate actual texels forward, then reflect the oriented
// raster, then resize. This deliberately does not copy the shader's inverse UV
// switch. Keep bilinear results fractional until the color chain's final clamp.
export function geometryColorReference(image, width, height, settings, rotation = 0, flip = false) {
  assert.ok([0, 90, 180, 270].includes(rotation));
  assert.equal(image.data.length, image.width * image.height * 4);
  assert.ok(Number.isInteger(width) && width > 0 && Number.isInteger(height) && height > 0);
  const ow = rotation % 180 ? image.height : image.width;
  const oh = rotation % 180 ? image.width : image.height;
  const oriented = Buffer.alloc(ow * oh * 4);
  for (let y = 0; y < image.height; y++) for (let x = 0; x < image.width; x++) {
    let dx, dy;
    if (rotation === 90) { dx = image.height - 1 - y; dy = x; }
    else if (rotation === 180) { dx = image.width - 1 - x; dy = image.height - 1 - y; }
    else if (rotation === 270) { dx = y; dy = image.width - 1 - x; }
    else { dx = x; dy = y; }
    if (flip) dx = ow - 1 - dx;
    image.data.copy(oriented, (dy * ow + dx) * 4, (y * image.width + x) * 4, (y * image.width + x + 1) * 4);
  }
  const resized = new Float64Array(width * height * 4);
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
    const sx = Math.max(0, Math.min(ow - 1, (x + .5) * ow / width - .5));
    const sy = Math.max(0, Math.min(oh - 1, (y + .5) * oh / height - .5));
    const x0 = Math.floor(sx), y0 = Math.floor(sy), x1 = Math.min(ow - 1, x0 + 1), y1 = Math.min(oh - 1, y0 + 1);
    const fx = sx - x0, fy = sy - y0;
    for (let c = 0; c < 3; c++) {
      const top = oriented[(y0 * ow + x0) * 4 + c] * (1 - fx) + oriented[(y0 * ow + x1) * 4 + c] * fx;
      const bottom = oriented[(y1 * ow + x0) * 4 + c] * (1 - fx) + oriented[(y1 * ow + x1) * 4 + c] * fx;
      resized[(y * width + x) * 4 + c] = top * (1 - fy) + bottom * fy;
    }
    resized[(y * width + x) * 4 + 3] = 255;
  }
  return colorReference(resized, settings);
}

export async function createColorAcceptance({ send, selectFile, navigate, directory, reference = 'neutral-preview',
  geometry = { rotation: 0, flip: false }, expectedFrames = 60, expectedFps = 30, externalIngress = null }) {
  assert.ok(['neutral-preview', 'ingress', 'ingress-geometry', 'external-ingress-geometry'].includes(reference));
  if (reference === 'external-ingress-geometry') assert.ok(externalIngress, 'supply a recorded ingress snapshot of this same source frame');
  directory = path.resolve(directory);
  fs.mkdirSync(directory, { recursive: true });
  const evidence = { status: "running", reference, geometry, expectedFrames, expectedFps, cases: [], thresholds: {
    previewMaximum: 2, lossyMae: 6, lossyP95: 15,
  } };
  const save = () => fs.writeFileSync(path.join(directory, "evidence.json"), JSON.stringify(evidence, null, 2));
  const evaluate = async expression => {
    const result = await send("Runtime.evaluate", {
      expression: `(async()=>JSON.stringify(await (${expression})))()`,
      awaitPromise: true, returnByValue: true,
    });
    assert.ok(!result.exceptionDetails, JSON.stringify(result.exceptionDetails));
    return result.result.value === undefined ? undefined : JSON.parse(result.result.value);
  };
  const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
  const preview = 'document.querySelector("#source-preview-status")?.textContent';
  const status = 'document.querySelector("#status")?.textContent';
  const wait = async expression => {
    const deadline = Date.now() + 120_000;
    while (Date.now() < deadline) {
      const value = await evaluate(expression);
      if (value) return value;
      await delay(50);
    }
    throw Error(`Timeout ${expression}: ${await evaluate(status)}`);
  };
  const set = async (selector, value, event = "input") => evaluate(`(()=>{
    const n=document.querySelector(${JSON.stringify(selector)});
    n.value=${JSON.stringify(String(value))};
    n.dispatchEvent(new Event(${JSON.stringify(event)},{bubbles:true}));return true;
  })()`);
  const click = selector => evaluate(`(()=>{document.querySelector(${JSON.stringify(selector)}).click();return true;})()`);
  const settled = async previous => {
    await wait(`${preview}!==${JSON.stringify(previous)} && /cached frames=1/.test(${preview})`);
    // Allow the coalescer's trailing update to publish before a pixel snapshot.
    await delay(200);
    return evaluate(preview);
  };
  const capture = async () => {
    const main = await evaluate("document.querySelector('#worker-preview').hidden");
    if (main) {
      // Enable ?diagnostic-preview=1 for this context. The diagnostic owns one
      // byte snapshot; a later canvas read can be black after presentation.
      const value = await evaluate(`(()=>{const d=globalThis.__DIAXUS_PREVIEW_DIAGNOSTIC__;
        if(!d)throw Error('Enable diagnostic-preview=1 for main-thread numeric checks');
        let binary='';for(let i=0;i<d.pixels.length;i+=8192)binary+=String.fromCharCode(...d.pixels.subarray(i,i+8192));
        let ingress=null;if(d.ingress){let input='';for(let i=0;i<d.ingress.pixels.length;i+=8192)input+=String.fromCharCode(...d.ingress.pixels.subarray(i,i+8192));
          ingress={width:d.ingress.width,height:d.ingress.height,data:btoa(input)};}
        return {width:d.width,height:d.height,data:btoa(binary),ingress};})()`);
      return {...value,data:Buffer.from(value.data,'base64'),ingress:value.ingress?{...value.ingress,data:Buffer.from(value.ingress.data,'base64')}:null};
    }
    const value = await evaluate(`(async()=>{
      const worker=document.querySelector('#worker-preview');
      const c=worker&&!worker.hidden?worker.querySelector('canvas'):document.querySelector('#export-canvas');
      const f=new VideoFrame(c,{timestamp:0});
      try {
        const bytes=new Uint8Array(f.allocationSize({format:'RGBA'}));
        await f.copyTo(bytes,{format:'RGBA'});
        let binary='';for(let i=0;i<bytes.length;i+=8192)binary+=String.fromCharCode(...bytes.subarray(i,i+8192));
        return {width:f.displayWidth,height:f.displayHeight,data:btoa(binary)};
      } finally { f.close(); }
    })()`);
    return { ...value, data: Buffer.from(value.data, "base64") };
  };
  const rgb = rgba => colorReference(rgba, { brightness: 0, contrast: 100, saturation: 100 });
  let baseline, mode;
  const referencePixels = image => {
    if (reference === 'neutral-preview') return image.data;
    if (reference === 'external-ingress-geometry') return externalIngress.data;
    assert.ok(image.ingress, 'ingress reference requires main-thread diagnostic-preview=1');
    // This reference mode is specifically for an original-size, unrotated,
    // square-pixel fixture. Do not apply it to a resized/oriented image.
    if (reference === 'ingress') assert.deepEqual([image.ingress.width,image.ingress.height],[image.width,image.height]);
    return image.ingress.data;
  };
  const expectedPixels = settings => reference.endsWith('ingress-geometry')
    ? geometryColorReference(externalIngress ?? baseline.ingress, baseline.width, baseline.height, settings, geometry.rotation, geometry.flip)
    : colorReference(referencePixels(baseline), settings);
  const record = value => { evidence.cases.push(value); save(); return value; };
  const guard = async operation => {
    try { return await operation(); }
    catch (error) { evidence.status = "failed"; evidence.error = String(error.stack); save(); throw error; }
  };
  return {
    evidence, evaluate, wait, capture, set, click,
    async setup(url, name, fixture, resize = 'original') {
      mode = name;
      await navigate(url);
      await wait('!!document.querySelector("#source-file")');
      await selectFile(path.resolve(fixture));
      await wait(`!document.querySelector('#convert').disabled && /cached frames=1/.test(${preview})`);
      if (resize !== 'original') {
        const previous = await evaluate(preview);
        await set('#resize-preset', resize, 'change');
        await settled(previous);
        await wait("!document.querySelector('#convert').disabled");
      }
      await delay(200);
      baseline = await capture();
      assert.ok(baseline.data.some((x,i)=>i%4!==3 && x>20), 'blank baseline is not a color test');
      fs.writeFileSync(path.join(directory, `${mode}-reference.rgba`), baseline.data);
      if (reference !== 'neutral-preview') fs.writeFileSync(path.join(directory, `${mode}-ingress.rgba`), referencePixels(baseline));
      evidence.previewSize = [baseline.width, baseline.height];
      if (reference.endsWith('ingress-geometry')) {
        const neutral = errors(rgb(baseline.data), expectedPixels({brightness:0,contrast:100,saturation:100}));
        record({name:`${mode}-neutral-geometry`, ...neutral});
        assert.ok(neutral.maximum <= 2, `neutral geometry mismatch: ${JSON.stringify(neutral)}`);
      }
      evidence.environment = await evaluate(`({userAgent:navigator.userAgent,gpu:document.querySelector('#selected-gpu')?.textContent,
        execution:document.querySelector('#execution-context')?.textContent})`);
      save();
    },
    async previewCase(name, settings) { return guard(async () => {
      const previous = await evaluate(preview);
      for (const [key, value] of Object.entries(settings)) await set(`#color-${key}`, value);
      const summary = await settled(previous);
      const actual = await capture();
      assert.deepEqual([actual.width, actual.height], [baseline.width, baseline.height]);
      if(reference==='ingress' || reference==='ingress-geometry')assert.ok(referencePixels(actual).equals(referencePixels(baseline)), 'source ingress changed between reference and adjusted preview');
      const comparison = errors(rgb(actual.data), expectedPixels(settings));
      const roundedPreviewComparison = errors(rgb(actual.data), colorReference(baseline.data, settings));
      fs.writeFileSync(path.join(directory, `${mode}-${name}.rgba`),actual.data);
      record({ name: `${mode}-${name}`, settings, ...comparison, roundedPreviewComparison, summary });
      assert.ok(comparison.maximum <= 2, `pre-encode color mismatch: ${JSON.stringify(comparison)}`);
      return comparison;
    }); },
    async exportCase(name, settings, profile = "webm-vp8-video-only", samplingReference = false) { return guard(async () => {
      await set('#output-profile', profile, 'change');
      await wait("!document.querySelector('#convert').disabled");
      await click('#convert');
      const summary = await wait(`document.querySelector('#cancel').disabled && /^(PASS|FAILED|CANCELLED):/.test(${status}) && ${status}`);
      assert.match(summary, /^PASS:/);
      assert.match(summary, /full re-decode PASS/);
      assert.match(summary, /Cleanup: 0 application-held frame references, 0 samples/);
      const bytes = await evaluate(`(async()=>{const b=new Uint8Array(await (await fetch(document.querySelector('#download').href)).arrayBuffer());
        let binary='';for(let i=0;i<b.length;i+=8192)binary+=String.fromCharCode(...b.subarray(i,i+8192));return btoa(binary);})()`);
      const file = path.join(directory, `${mode}-${name}.${profile.startsWith('mp4') ? 'mp4' : 'webm'}`);
      fs.writeFileSync(file, Buffer.from(bytes, 'base64'));
      // Explicitly honor the output's actual range/matrix/transfer. swscale's
      // implicit RGB conversion is not a color-managed sRGB reference.
      const probe = spawnSync('ffprobe', ['-v','error','-show_streams','-of','json',file], {encoding:'utf8'});
      assert.equal(probe.status, 0, probe.stderr);
      const streams = JSON.parse(probe.stdout).streams;
      const metadata = streams.find(s=>s.codec_type==='video');
      if (!profile.includes('video-only')) assert.ok(streams.some(s=>s.codec_type==='audio'), 'missing output audio');
      assert.equal(metadata.color_space, 'bt709');
      assert.ok(['bt709','iec61966-2-1'].includes(metadata.color_transfer));
      assert.equal(metadata.color_primaries, 'bt709');
      assert.equal(metadata.color_range, 'tv');
      // VP8's decoded-frame defaults can disagree with the Matroska stream tags.
      // Pass the verified stream metadata explicitly rather than using defaults.
      const transfer = metadata.color_transfer === 'bt709' ? '709' : 'iec61966-2-1';
      const normalize = `zscale=matrixin=709:transferin=${transfer}:primariesin=709:rangein=limited:matrix=gbr:transfer=iec61966-2-1:primaries=709:range=full,format=gbrp`;
      const decoded = spawnSync('ffmpeg', ['-v','error','-i',file,'-vf',normalize,'-frames:v','1','-f','rawvideo','-pix_fmt','rgb24','-'],
        { timeout: 30_000, maxBuffer: 32 * 1024 * 1024 });
      assert.equal(decoded.status, 0, String(decoded.stderr));
      assert.deepEqual([metadata.width,metadata.height], [baseline.width,baseline.height], 'export/preview geometry differs');
      // H.264 may omit the optional SAR signal for square pixels. Full browser
      // re-decode above separately checks actual display/coded geometry.
      assert.ok(metadata.sample_aspect_ratio === undefined || metadata.sample_aspect_ratio === '1:1', 'non-square output pixels');
      assert.ok(!metadata.side_data_list?.some(d=>d.side_data_type==='Display Matrix'), 'orientation was not baked into output');
      const comparison = errors(decoded.stdout, expectedPixels(settings));
      let sampledComparison = null;
      if (samplingReference) {
        assert.equal(metadata.pix_fmt, 'yuv420p', 'sampling oracle is only for 8-bit 4:2:0');
        assert.equal(metadata.chroma_location, 'left', 'sampling oracle requires declared left chroma siting');
        assert.equal(metadata.color_transfer, 'iec61966-2-1', 'sampling oracle requires the tested sRGB transfer');
        // Keep raw RGB error above: mandatory 4:2:0 loss is not shader error.
        // Independently apply the declared sampling/range/matrix, without a
        // codec, then compare against this representable RGB reference.
        const sampled = spawnSync('ffmpeg', ['-v','error','-f','rawvideo','-pixel_format','rgb24',
          '-video_size',`${baseline.width}x${baseline.height}`,'-i','-', '-vf',
          'format=gbrp,zscale=matrixin=gbr:transferin=iec61966-2-1:primariesin=709:rangein=full:matrix=709:transfer=iec61966-2-1:primaries=709:range=limited:chromal=left,format=yuv420p,'+
          'zscale=matrixin=709:transferin=iec61966-2-1:primariesin=709:rangein=limited:matrix=gbr:transfer=iec61966-2-1:primaries=709:range=full:chromalin=left,format=gbrp',
          '-frames:v','1','-f','rawvideo','-pix_fmt','rgb24','-'],
          {input:expectedPixels(settings),timeout:30000,maxBuffer:32*1024*1024});
        assert.equal(sampled.status,0,String(sampled.stderr));
        sampledComparison = errors(decoded.stdout,sampled.stdout);
      }
      const timeline = spawnSync('ffprobe', ['-v','error','-select_streams','v:0','-show_frames',
        '-show_entries','frame=best_effort_timestamp_time','-of','json',file], {encoding:'utf8'});
      assert.equal(timeline.status, 0, timeline.stderr);
      const frames = JSON.parse(timeline.stdout).frames;
      assert.equal(frames.length, expectedFrames);
      const origin = Number(frames[0].best_effort_timestamp_time);
      frames.forEach((frame, i) => assert.ok(Math.abs(Number(frame.best_effort_timestamp_time)-origin-i/expectedFps) <= .0011));
      const fullDecode = spawnSync('ffmpeg', ['-v','error','-i',file,'-f','null','-'], {encoding:'utf8',timeout:30000});
      assert.equal(fullDecode.status, 0, fullDecode.stderr);
      record({name:`${mode}-${name}`, profile, settings, ...comparison, sampledComparison,
        acceptanceReference:samplingReference?'declared-4:2:0':'pre-subsampling-RGB', metadata, normalize, frames:frames.length, file, summary});
      const accepted = sampledComparison ?? comparison;
      assert.ok(accepted.mae <= 6 && accepted.p95 <= 15, `lossy color mismatch: ${JSON.stringify(accepted)}`);
      return comparison;
    }); },
    // Diagnostic boundary isolation, not an acceptance pass: encode the known
    // worker preview directly, bypassing demux, processing and the job runner.
    async isolateEncoder(settings, cpuFrame = false, alpha = 'keep', padded = false) { return guard(async () => {
      const value = await evaluate(`(async()=>{
        const canvas=document.querySelector('#worker-preview:not([hidden]) canvas');
        if(!canvas)throw Error('Encoder isolation requires a visible worker preview');
        let frame,decoded,encoder,decoder,failure,config;const packets=[];
        try {
          frame=new VideoFrame(canvas,{timestamp:0,duration:41667,alpha:${JSON.stringify(alpha)}});
          if(${JSON.stringify(padded)}) {
            frame.close();frame=null;
            const pad=new OffscreenCanvas(${JSON.stringify(padded)}==='copy'?canvas.width:Math.ceil(canvas.width/16)*16,
              ${JSON.stringify(padded)}==='copy'?canvas.height:Math.ceil(canvas.height/16)*16);
            pad.getContext('2d').drawImage(canvas,0,0);
            frame=new VideoFrame(pad,{timestamp:0,duration:41667,alpha:'discard',
              visibleRect:{x:0,y:0,width:canvas.width,height:canvas.height},displayWidth:canvas.width,displayHeight:canvas.height});
          }
          if(${JSON.stringify(cpuFrame)}) {
            const input=new Uint8Array(frame.allocationSize({format:'RGBA'}));
            await frame.copyTo(input,{format:'RGBA'});frame.close();frame=null;
            frame=new VideoFrame(input,{format:'RGBA',codedWidth:canvas.width,codedHeight:canvas.height,
              timestamp:0,duration:41667,colorSpace:{matrix:'rgb',primaries:'bt709',transfer:'iec61966-2-1',fullRange:true}});
          }
          encoder=new VideoEncoder({output:(p,m)=>{packets.push(p);config=m.decoderConfig??config;},error:e=>failure=e});
          encoder.configure({codec:'avc1.42001e',width:canvas.width,height:canvas.height,bitrate:8000000,framerate:24});
          encoder.encode(frame,{keyFrame:true});frame.close();frame=null;
          await encoder.flush();if(failure)throw failure;
          decoder=new VideoDecoder({output:v=>{decoded?.close();decoded=v;},error:e=>failure=e});
          decoder.configure(config);for(const p of packets)decoder.decode(p);
          await decoder.flush();if(failure)throw failure;if(!decoded)throw Error('No decoded encoder output');
          const bytes=new Uint8Array(decoded.allocationSize({format:'RGBA'}));
          await decoded.copyTo(bytes,{format:'RGBA'});
          let binary='';for(let i=0;i<bytes.length;i+=8192)binary+=String.fromCharCode(...bytes.subarray(i,i+8192));
          return {data:btoa(binary),width:decoded.displayWidth,height:decoded.displayHeight,
            config:{codec:config.codec,colorSpace:config.colorSpace},packets:packets.length};
        } finally {frame?.close();decoded?.close();if(encoder&&encoder.state!=='closed')encoder.close();
          if(decoder&&decoder.state!=='closed')decoder.close();}
      })()`);
      assert.deepEqual([value.width,value.height], [baseline.width,baseline.height]);
      const comparison = errors(rgb(Buffer.from(value.data,'base64')), expectedPixels(settings));
      return record({name:`${mode}-isolated-h264-encoder`, diagnosticOnly:true, cpuFrame, alpha, padded, settings,
        config:value.config,packets:value.packets,...comparison});
    }); },
    async stress() { return guard(async () => {
      await wait(`/cached frames=1/.test(${preview})`);
      const prior = await evaluate(preview);
      await evaluate(`(()=>{const n=document.querySelector('#color-brightness');for(let i=0;i<60;i++){
        n.value=String(i%20);n.dispatchEvent(new Event('input',{bubbles:true}));}return true;})()`);
      const latest = await settled(prior);
      const counts = value => [...value.matchAll(/(?:cache loads|renders)=(\d+)/g)].map(x=>Number(x[1]));
      assert.equal(counts(prior)[0],counts(latest)[0], 'slider decoded source again');
      assert.ok(counts(latest)[1]-counts(prior)[1] < 60, 'slider tasks were not coalesced');
      assert.equal(await evaluate("document.querySelector('#color-brightness').value"), '19');
      const before = await evaluate(preview);
      await evaluate(`(()=>{const n=document.querySelector('#source-preview-position');for(const p of [.25,.5,.75,1,1.25]){
        n.value=String(p);n.dispatchEvent(new Event('input',{bubbles:true}));}return true;})()`);
      const seek = await settled(before);
      assert.equal(await evaluate("document.querySelector('#source-preview-position').value"), '1.25');
      const adjustedSeek = await capture();
      const settings = await evaluate("Object.fromEntries(['brightness','contrast','saturation'].map(k=>[k,Number(document.querySelector('#color-'+k).value)]))");
      const comparePrior = await evaluate(preview);
      await click('#color-compare');
      await settled(comparePrior);
      const neutralSeek = await capture();
      assert.equal(await evaluate("document.querySelector('#color-brightness').value"), '19', 'Before mutated export settings');
      const seekComparison = errors(rgb(adjustedSeek.data), colorReference(neutralSeek.data, settings));
      assert.ok(seekComparison.maximum <= 2, 'rapid seek published mismatched position/color');
      await click('#color-compare');
      await delay(250);
      const resetPrior = await evaluate(preview);
      await click('#color-reset');
      await settled(resetPrior);
      assert.deepEqual(await evaluate("['brightness','contrast','saturation'].map(k=>document.querySelector('#color-'+k).value)"), ['0','100','100']);
      const saved = await evaluate(preview);
      await set('#source-preview-position', 0);
      await settled(saved);
      const restored = errors(rgb((await capture()).data), rgb(baseline.data));
      assert.equal(restored.maximum, 0, 'reset/seek did not restore initial image');
      return record({ name: `${mode}-rapid-controls`, prior, latest, seek, restored, seekComparison });
    }); },
    async cancel() { return guard(async () => {
      await wait("!document.querySelector('#convert').disabled");
      await click('#convert');
      await wait("!document.querySelector('#cancel').disabled");
      await click('#cancel');
      const summary = await wait(`document.querySelector('#cancel').disabled && /^CANCELLED:/.test(${status}) && ${status}`);
      assert.match(summary, /0 application-held frame references, 0 samples/);
      assert.equal(await evaluate("!!document.querySelector('#download')"), false, 'partial download published');
      return record({name:`${mode}-cancel`,summary});
    }); },
    async injectedFailure() { return guard(async () => {
      await click('#convert');
      const summary = await wait(`document.querySelector('#cancel').disabled && /^FAILED:/.test(${status}) && ${status}`);
      assert.match(summary, /0 application-held frame references, 0 samples/);
      assert.equal(await evaluate("!!document.querySelector('#download')"), false);
      return record({name:`${mode}-injected-failure`,summary});
    }); },
    finish() {
      assert.notEqual(evidence.status, 'failed', 'cannot mark a failed acceptance run passed');
      evidence.status = 'passed'; save(); return path.join(directory, 'evidence.json');
    },
  };
}
