#version 450
#include "globals.glsl"
#extension GL_EXT_nonuniform_qualifier : enable


//shader input
layout (location = 0) in vec3 inColor;
layout (location = 1) in vec2 inUV;


layout( push_constant ) uniform constants
{
    mat4 transform;
    SceneDataBuffer sceneDataBuffer;
    VertexBuffer vertexBuffer;
    PbrMaterial pbrMaterial;
} PushConstants;



layout (set = 0, binding = 2) uniform sampler2D tex[];

//output write
layout (location = 0) out vec4 outFragColor;

void main()
{
    PbrMaterial material = PushConstants.pbrMaterial;
    outFragColor = material.albedo * texture(tex[material.texture], inUV);
}