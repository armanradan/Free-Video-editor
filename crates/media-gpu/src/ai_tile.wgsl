// RGB values stay in encoded SDR space: source and destination are unorm,
// never sRGB views. Model inputs are FP32 NCHW with uint8 / 255 normalization.
struct Plan {
    origin: vec2<u32>,
    extent: vec2<u32>,
    crop: vec2<u32>,
    interior: vec2<u32>,
}
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> input: array<f32>;
@group(0) @binding(2) var<uniform> plan: Plan;
@compute @workgroup_size(8, 8)
fn pack(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= plan.extent) { return; }
    let rgb = textureLoad(source, vec2<i32>(plan.origin + id.xy), 0).rgb;
    let plane = plan.extent.x * plan.extent.y;
    let offset = id.y * plan.extent.x + id.x;
    input[offset] = rgb.r;
    input[plane + offset] = rgb.g;
    input[2u * plane + offset] = rgb.b;
}
@group(1) @binding(0) var<storage, read> output: array<f32>;
@group(1) @binding(1) var destination: texture_storage_2d<rgba8unorm, write>;
@group(1) @binding(2) var<uniform> output_plan: Plan;
@compute @workgroup_size(8, 8)
fn unpack(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= output_plan.interior * 4u) { return; }
    let size = output_plan.extent * 4u;
    let pos = output_plan.crop * 4u + id.xy;
    let plane = size.x * size.y;
    let offset = pos.y * size.x + pos.x;
    let rgb = vec3<f32>(output[offset], output[plane + offset], output[2u * plane + offset]);
    let dest = (output_plan.origin + output_plan.crop) * 4u + id.xy;
    textureStore(destination, vec2<i32>(dest), vec4<f32>(clamp(rgb, vec3<f32>(0.), vec3<f32>(1.)), 1.));
}
