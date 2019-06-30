#version 330

in vec2 v_tex_coords;
out vec4 color;
uniform usampler2D tex;

void main() {
    vec3 normalized = vec3(texture(tex, v_tex_coords).rgb) / 31.0;
    color = vec4(normalized, 1.0);
}
