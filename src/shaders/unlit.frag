#version 450
#include "globals.glsl"
#extension GL_EXT_nonuniform_qualifier : enable

layout (location = 0) in vec2 texCoords;


layout( push_constant, scalar ) uniform constants
{
    mat4 transform;  // model matrix
    vec2 size;       // size of the billboard
    vec2[4] uv;
    SceneDataBuffer sceneDataBuffer;
    PbrMaterial pbrMaterial;
    LightBuffer lightBuffer;
} PushConstants;

layout (location = 0) out vec4 outFragColor;
layout (set = 0, binding = 2) uniform sampler2D tex[];


void main() {

    outFragColor = PushConstants.pbrMaterial.albedo * texture(tex[PushConstants.pbrMaterial.albedo_tex], texCoords) * vec4(1.0);
}