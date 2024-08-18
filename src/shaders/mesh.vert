#version 450
#include "globals.glsl"

layout (location = 0) out vec3 outWorldPos;
layout (location = 1) out vec2 outUV;
layout (location = 2) out vec3 outNormal;



//push constants block
layout( push_constant ) uniform constants
{
    mat4 transform;
    SceneDataBuffer sceneDataBuffer;
    VertexBuffer vertexBuffer;
    PbrMaterial pbrMaterial;
    LightBuffer lightBuffer;
} PushConstants;

void main()
{
    //load vertex data from device adress
    Vertex v = PushConstants.vertexBuffer.vertices[gl_VertexIndex];
    SceneDataBuffer sceneData = PushConstants.sceneDataBuffer;
    //output data
    outWorldPos = (PushConstants.transform * vec4(v.position, 1.0)).xyz;
    gl_Position = sceneData.viewproj * vec4(outWorldPos, 1.0f);
    outUV.x = v.uv_x;
    outUV.y = v.uv_y;
    mat4 normal_matrix = transpose(inverse(PushConstants.transform));
    outNormal = v.normal;
}