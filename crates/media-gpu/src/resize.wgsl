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
    target_is_srgb: u32,
    _padding_1: u32,
    // Encoded RGB controls; source is an Unorm texture, not an sRGB-sampling view.
    color: vec4<f32>,
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
    let uv = source_uv(input.uv);
    let dimensions = vec2<f32>(textureDimensions(source_texture));
    let pixel = uv * dimensions - 0.5;
    let center = round(pixel);
    var rgb: vec3<f32>;
    // At a texel center bilinear filtering is exactly that texel. Some hardware
    // samplers quantize a tiny floating-coordinate error into a nonzero neighbor
    // weight, amplified by strong color controls. Preserve the exact center;
    // retain ordinary bilinear sampling everywhere else (single mip level).
    if all(abs(pixel - center) < vec2<f32>(0.0001)) {
        let coordinate = vec2<i32>(clamp(center, vec2<f32>(0.0), dimensions - 1.0));
        rgb = textureLoad(source_texture, coordinate, 0).rgb;
    } else {
        rgb = textureSampleLevel(source_texture, source_sampler, uv, 0.0).rgb;
    }
    if orientation.color.w != 0.0 {
        let gray = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        rgb = clamp((vec3<f32>(gray) + orientation.color.z * (rgb - gray) - 0.5)
            * orientation.color.y + 0.5 + orientation.color.x, vec3<f32>(0.0), vec3<f32>(1.0));
    }
    // sRGB attachments encode linear output. Cancel that encoding for this
    // explicitly encoded-channel operation (including its neutral path).
    if orientation.target_is_srgb != 0u {
        rgb = select(pow((rgb + 0.055) / 1.055, vec3<f32>(2.4)), rgb / 12.92,
            rgb <= vec3<f32>(0.04045));
    }
    return vec4<f32>(rgb, 1.0);
}
