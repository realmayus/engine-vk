#version 450
#include "globals.glsl"

layout (location = 0) out vec3 outColor;
layout (location = 1) out vec2 outUV;



//push constants block
layout( push_constant ) uniform constants
{
    mat4 transform;
    SceneDataBuffer sceneDataBuffer;
VertexBuffer vertexBuffer;
PbrMaterial pbrMaterial;
} PushConstants;

void main()
{
    //load vertex data from device adress
    Vertex v = PushConstants.vertexBuffer.vertices[gl_VertexIndex];
    SceneData sceneData = PushConstants.sceneDataBuffer.sceneData;
    //output data
    gl_Position = sceneData.viewproj * PushConstants.transform * vec4(v.position, 1.0f);
    outColor = v.color.xyz;
    outUV.x = v.uv_x;
    outUV.y = v.uv_y;
}