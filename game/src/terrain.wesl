// Weight-blended TQ2 landscape layers. Weights come from the landscape's own layer maps
// (sampled with the landscape-wide UV_1); ground textures tile in world space.
// Only used on landscape meshes, which always have UV_1 and tangents.
import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
};

struct TerrainParams {
    layer_count: u32,
    tiling: f32,
    _pad0: f32,
    _pad1: f32,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> params: TerrainParams;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var weights: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var weights_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(103) var albedo: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(104) var albedo_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(105) var normals: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(106) var normals_sampler: sampler;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let tile_uv = in.world_position.xz * params.tiling;

    var color = vec3<f32>(0.0);
    var tangent_normal = vec3<f32>(0.0);
    var total = 0.0;
    // Sample every layer (uniform control flow keeps derivatives valid for mipmapping).
    for (var i = 0u; i < params.layer_count; i = i + 1u) {
        let w = textureSample(weights, weights_sampler, in.uv_b, i32(i / 4u))[i % 4u];
        color += textureSample(albedo, albedo_sampler, tile_uv, i32(i)).rgb * w;
        tangent_normal += (textureSample(normals, normals_sampler, tile_uv, i32(i)).xyz * 2.0 - 1.0) * w;
        total += w;
    }
    if (total > 0.01) {
        pbr_input.material.base_color = vec4<f32>(color / total, 1.0);
        pbr_input.material.perceptual_roughness = 0.9;
        // UE normal maps are DirectX style (green down).
        let n = normalize(vec3<f32>(tangent_normal.x, -tangent_normal.y, max(tangent_normal.z, 0.05)));
        let N = normalize(in.world_normal);
        let T = normalize(in.world_tangent.xyz - N * dot(in.world_tangent.xyz, N));
        let B = cross(N, T) * in.world_tangent.w;
        pbr_input.N = normalize(T * n.x + B * n.y + N * n.z);
    }

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
