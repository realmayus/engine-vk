#version 450
#include "globals.glsl"
#include "util.glsl"
#extension GL_EXT_nonuniform_qualifier : enable

layout (location = 0) in vec2 fragOffset;

layout (location = 1) in vec2 texCoords;

layout( push_constant ) uniform constants
{
    mat4 transform;  // model matrix
    vec4 center;
    vec4[4] uv;
    SceneDataBuffer sceneDataBuffer;
    PbrMaterial pbrMaterial;
    LightBuffer lightBuffer;
} PushConstants;

layout (location = 0) out vec4 outFragColor;
layout (set = 0, binding = 2) uniform sampler2D tex[];


void main() {
    outFragColor = texture(tex[PushConstants.pbrMaterial.texture], texCoords);
    float c = sqrt(fragOffset.x * fragOffset.x + fragOffset.y * fragOffset.y);
    outFragColor = vec4(c, c, c, c);
}