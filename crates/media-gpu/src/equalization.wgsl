// CLAHE v1: encoded sRGB luma; 1024 bins; bounded four-frame raw-map ring.
struct Params {
    geometry: vec4<u32>, // width, height, tile columns, tile rows
    control: vec4<u32>,  // current slot, previous slot, reset, reserved
    weights: vec4<f32>, // timestamp-aware finite history
    effect: vec4<f32>,  // strength, scene-cut L1 threshold, reserved
};
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var output: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> p: Params;
@group(0) @binding(3) var<storage, read_write> hist: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> maps: array<f32>;
@group(0) @binding(5) var<storage, read_write> epochs: array<u32>;
fn tiles() -> u32 { return p.geometry.z*p.geometry.w; }
fn hist_base(slot:u32) -> u32 { return slot*(tiles()+1u)*1024u; }
fn map_base(slot:u32,tile:u32) -> u32 { return (slot*tiles()+tile)*1024u; }
fn luma(rgb:vec3<f32>) -> f32 { return dot(rgb,vec3<f32>(0.2126,0.7152,0.0722)); }
@compute @workgroup_size(8,8)
fn histogram(@builtin(global_invocation_id) id:vec3<u32>) {
    if any(id.xy>=p.geometry.xy) { return; }
    let rgb=clamp(textureLoad(source,vec2<i32>(id.xy),0).rgb,vec3<f32>(0.0),vec3<f32>(1.0));
    let bin=u32(floor(luma(rgb)*1023.0+0.5));
    let tile=id.xy*p.geometry.zw/p.geometry.xy;
    atomicAdd(&hist[hist_base(p.control.x)+(tile.y*p.geometry.z+tile.x)*1024u+bin],1u);
    atomicAdd(&hist[hist_base(p.control.x)+tiles()*1024u+bin],1u);
}
@compute @workgroup_size(1)
fn scene_cut() {
    var delta=0.0;
    if p.control.z==0u {
        // Coarse bins avoid classifying small sensor/quantization noise as cuts.
        for(var coarse=0u;coarse<64u;coarse++) {
            var a=0u; var old=0u;
            for(var fine=0u;fine<16u;fine++) {
                let b=coarse*16u+fine;
                a+=atomicLoad(&hist[hist_base(p.control.x)+tiles()*1024u+b]);
                old+=atomicLoad(&hist[hist_base(p.control.y)+tiles()*1024u+b]);
            }
            delta+=abs(f32(a)-f32(old))/f32(p.geometry.x*p.geometry.y);
        }
    }
    let prior=epochs[p.control.y];
    if prior==0xffffffffu {
        for(var slot=0u;slot<4u;slot++) { epochs[slot]=0u; }
        epochs[p.control.x]=1u;
        return;
    }
    epochs[p.control.x]=prior+select(0u,1u,p.control.z!=0u || delta>p.effect.y);
}
@compute @workgroup_size(1)
fn mapping(@builtin(global_invocation_id) id:vec3<u32>) {
    if id.x>=tiles() { return; }
    let base=hist_base(p.control.x)+id.x*1024u;
    var total=0u; var occupied=0u;
    for(var b=0u;b<1024u;b++) {
        let count=atomicLoad(&hist[base+b]); total+=count; occupied+=select(0u,1u,count>0u);
    }
    // Flat tiles are identity, including black/white; no noise invented.
    let cap=max(1u,(4u*total+1023u)/1024u);
    var excess=0u;
    for(var b=0u;b<1024u;b++) { excess+=atomicLoad(&hist[base+b])-min(cap,atomicLoad(&hist[base+b])); }
    var cumulative=0u;
    for(var b=0u;b<1024u;b++) {
        let count=min(cap,atomicLoad(&hist[base+b]))+excess/1024u+select(0u,1u,b<excess%1024u);
        cumulative+=count;
        var value=(f32(cumulative)-0.5*f32(count))/f32(max(total,1u));
        if occupied<=1u { value=f32(b)/1023.0; }
        maps[map_base(p.control.x,id.x)+b]=value;
    }
}
fn mapped(tile:u32,y:f32) -> f32 {
    let bin=y*1023.0; let lo=u32(floor(bin)); let hi=min(1023u,lo+1u);
    var value=0.0; var weight=0.0;
    for(var slot=0u;slot<4u;slot++) {
        if p.weights[slot]>0.0 && epochs[slot]==epochs[p.control.x] {
            let base=map_base(slot,tile);
            value+=p.weights[slot]*mix(maps[base+lo],maps[base+hi],fract(bin));
            weight+=p.weights[slot];
        }
    }
    return value/weight; // current slot always has weight 1
}
@compute @workgroup_size(8,8)
fn apply(@builtin(global_invocation_id) id:vec3<u32>) {
    if any(id.xy>=p.geometry.xy) { return; }
    let rgb=clamp(textureLoad(source,vec2<i32>(id.xy),0).rgb,vec3<f32>(0.0),vec3<f32>(1.0));
    let y=luma(rgb);
    let pos=clamp((vec2<f32>(id.xy)+0.5)*vec2<f32>(p.geometry.zw)/vec2<f32>(p.geometry.xy)-0.5,
        vec2<f32>(0.0),vec2<f32>(p.geometry.zw)-1.0);
    let lo=vec2<u32>(floor(pos)); let hi=min(lo+1u,p.geometry.zw-1u); let f=fract(pos);
    let top=mix(mapped(lo.y*p.geometry.z+lo.x,y),mapped(lo.y*p.geometry.z+hi.x,y),f.x);
    let bottom=mix(mapped(hi.y*p.geometry.z+lo.x,y),mapped(hi.y*p.geometry.z+hi.x,y),f.x);
    let delta=(mix(top,bottom,f.y)-y)*p.effect.x;
    // Additive encoded-luma reconstruction preserves channel differences before
    // gamut clamp and avoids division/unstable gain at black.
    textureStore(output,vec2<i32>(id.xy),vec4<f32>(clamp(rgb+delta,vec3<f32>(0.0),vec3<f32>(1.0)),1.0));
}
