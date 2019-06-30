#version 330

in vec2 position;
in vec2 tex_coords;
out vec2 v_tex_coords;

uniform vec2 scale_factor;


void main() {
    v_tex_coords = tex_coords;
    gl_Position = vec4(position / scale_factor, 0.0, 1.0);
}
