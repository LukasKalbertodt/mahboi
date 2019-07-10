#version 450

layout(location = 0) in vec2 v_tex_coords;
layout(location = 0) out vec4 color;

layout(set = 0, binding = 0) uniform usampler2D tex;

void main() {
    vec3 normalized = vec3(texture(tex, v_tex_coords).rgb) / 31.0;
    color = vec4(normalized, 1.0);
}
