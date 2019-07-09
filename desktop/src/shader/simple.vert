#version 450

layout(location = 0) in vec2 position;
layout(location = 1) in vec2 tex_coords;
layout(location = 0) out vec2 v_tex_coords;

// layout(push_constant) uniform PushConstants {
//     vec2 scale_factor;
// } consts;

void main() {
    v_tex_coords = tex_coords;
    // gl_Position = vec4(position / consts.scale_factor, 0.0, 1.0);
    gl_Position = vec4(position, 0.0, 1.0);
}
