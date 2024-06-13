#version 450
#include "globals.glsl"

layout (location = 0) out vec3 outWorldPos;
layout (location = 1) out vec2 outUV;
layout (location = 3) out mat3 outTangentBasis;



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

    vec3 bitangent = cross(v.normal, v.tangent.xyz) * v.tangent.w;
    vec3 tangent = v.tangent.xyz;
    outTangentBasis = mat3(PushConstants.transform) * mat3(tangent, bitangent, v.normal);
    outWorldPos = (PushConstants.transform * vec4(v.position, 1.0)).xyz;
    outUV.x = v.uv_x;
    outUV.y = v.uv_y;
    gl_Position = sceneData.viewproj * vec4(outWorldPos, 1.0f);
}