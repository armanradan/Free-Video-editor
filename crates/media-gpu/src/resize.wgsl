struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    let p = positions[index];
    var output: VertexOutput;
    output.position = vec4<f32>(p, 0.0, 1.0);
    output.uv = vec2<f32>((p.x + 1.0) * 0.5, (1.0 - p.y) * 0.5);
    return output;
}

@group(0) @binding(0) var source_texture: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;

struct FrameOrientation {
    rotation: u32,
    flip_horizontal: u32,
    _padding_0: u32,
    _padding_1: u32,
};

@group(0) @binding(2) var<uniform> orientation: FrameOrientation;

fn source_uv(output_uv: vec2<f32>) -> vec2<f32> {
    var uv = output_uv;
    // Forward geometry is clockwise rotation followed by a horizontal flip.
    // Sampling applies the inverse operations in reverse order.
    if orientation.flip_horizontal != 0u {
        uv.x = 1.0 - uv.x;
    }
    switch orientation.rotation {
        case 1u: { return vec2<f32>(uv.y, 1.0 - uv.x); }
        case 2u: { return vec2<f32>(1.0 - uv.x, 1.0 - uv.y); }
        case 3u: { return vec2<f32>(1.0 - uv.y, uv.x); }
        default: { return uv; }
    }
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let rgb = textureSample(source_texture, source_sampler, source_uv(input.uv)).rgb;
    return vec4<f32>(rgb, 1.0);
}
