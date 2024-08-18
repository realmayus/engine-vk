#version 450
#include "globals.glsl"

//push constants block
layout( push_constant, scalar ) uniform constants
{
    mat4 transform;  // model matrix
    vec2 size;       // size of the billboard
    vec2[4] uv;
    SceneDataBuffer sceneDataBuffer;
    PbrMaterial pbrMaterial;
    LightBuffer lightBuffer;
} PushConstants;

layout (location = 0) out vec2 texCoords;

const vec4[] vertices = vec4[](
    vec4(-1.0, -1.0, 0.0, 1.0),
    vec4(-1.0, 1.0, 0.0, 1.0),
    vec4(1.0, -1.0, 0.0, 1.0),
    vec4(-1.0, 1.0, 0.0, 1.0),
    vec4(1.0, -1.0, 0.0, 1.0),
    vec4(1.0, 1.0, 0.0, 1.0)
);

const int[] uv_map = int[](
0, 2, 1, 2, 1, 3
);

void main()
{
    mat4 view = PushConstants.sceneDataBuffer.view;
    mat4 viewproj = PushConstants.sceneDataBuffer.viewproj;
    vec4 camera_right_world = vec4(view[0][0], view[1][0], view[2][0], 0.0);
    vec4 camera_up_world = vec4(view[0][1], view[1][1], view[2][1], 0.0);
    vec4 position = vec4(PushConstants.transform[3].xyz, 1.0);
    // from position and size.x, size.y, calculate the center of the billboard
    vec4 world_pos = position
        + camera_right_world * vertices[gl_VertexIndex].x * PushConstants.size.x
        + camera_up_world * vertices[gl_VertexIndex].y * PushConstants.size.y;

    gl_Position = viewproj * vec4(world_pos.xyz, 1.0);

    int uv_index = uv_map[gl_VertexIndex];
    vec2 uv_coords = PushConstants.uv[uv_index].xy;
    texCoords.x = uv_coords.x;
    texCoords.y = uv_coords.y;
}