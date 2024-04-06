#version 450
#extension GL_EXT_buffer_reference : require

layout (location = 0) out vec3 outColor;
layout (location = 1) out vec2 outUV;

struct SceneData {
    mat4 view;
    mat4 proj;
    mat4 viewproj;
    vec4 ambient_color;
    vec4 sun_dir;
    vec4 sun_color;
};

struct Vertex {
    vec3 position;
    float uv_x;
    vec3 normal;
    float uv_y;
    vec4 color;
};

layout(buffer_reference, std430) readonly buffer SceneDataBuffer {
    SceneData sceneData;
};

layout(buffer_reference, std430) readonly buffer VertexBuffer {
    Vertex vertices[];
};

//push constants block
layout( push_constant ) uniform constants
{
    SceneDataBuffer sceneDataBuffer;
    VertexBuffer vertexBuffer;
} PushConstants;

void main()
{
    //load vertex data from device adress
    Vertex v = PushConstants.vertexBuffer.vertices[gl_VertexIndex];
    SceneData sceneData = PushConstants.sceneDataBuffer.sceneData;
    //output data
    gl_Position = sceneData.viewproj * vec4(v.position, 1.0f);
    outColor = v.color.xyz;
    outUV.x = v.uv_x;
    outUV.y = v.uv_y;
}