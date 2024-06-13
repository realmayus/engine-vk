#version 450
#include "globals.glsl"
#extension GL_EXT_nonuniform_qualifier : enable

layout (location = 0) in vec2 uv;
layout(location = 0) out vec4 fragColor;

layout( push_constant ) uniform constants
{
    mat4 transform;
    SceneDataBuffer sceneDataBuffer;
    UnlitMaterial material;
    VertexBuffer vertexBuffer;
    LightBuffer lightBuffer;
} PushConstants;

layout (set = 0, binding = 2) uniform sampler2D tex[];

void main()
{
    UnlitMaterial material = PushConstants.material;
    fragColor = texture(tex[material.albedoTexture], uv) * material.albedo;
}

