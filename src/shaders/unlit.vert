#version 450
#include "globals.glsl"

layout (location = 0) out vec2 outUV;


layout( push_constant ) uniform constants
{
    mat4 transform;
    SceneDataBuffer sceneDataBuffer;
    VertexBuffer vertexBuffer;
    UnlitMaterial material;
    LightBuffer lightBuffer;

} PushConstants;

void main()
{
    //load vertex data from device adress
    Vertex v = PushConstants.vertexBuffer.vertices[gl_VertexIndex];
    SceneDataBuffer sceneData = PushConstants.sceneDataBuffer;

    outUV.x = v.uv_x;
    outUV.y = v.uv_y;
    vec3 worldPos = (PushConstants.transform * vec4(v.position, 1.0)).xyz;
    gl_Position = sceneData.viewproj * vec4(worldPos, 1.0f);
}