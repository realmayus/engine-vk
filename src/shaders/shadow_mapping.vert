#version 450
#include "globals.glsl"

//push constants block
layout( push_constant ) uniform constants
{
    mat4 transform;
    SceneDataBuffer sceneDataBuffer;
    VertexBuffer vertexBuffer;
    LightBuffer lightBuffer;
} PushConstants;

void main()
{
    //load vertex data from device adress
    Vertex v = PushConstants.vertexBuffer.vertices[gl_VertexIndex];
    SceneDataBuffer sceneData = PushConstants.sceneDataBuffer;
    // light id = instance index
    Light light = PushConstants.lightBuffer.lights[gl_InstanceIndex];
    //output data
    vec3 outWorldPos = (PushConstants.transform * vec4(v.position, 1.0)).xyz;
    gl_Position = light.lightspace * vec4(outWorldPos, 1.0f);
//    gl_Position = sceneData.viewproj * vec4(outWorldPos, 1.0f);
}