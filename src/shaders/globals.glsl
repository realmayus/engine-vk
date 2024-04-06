#extension GL_EXT_buffer_reference : require


struct SceneData {
    mat4 view;
    mat4 proj;
    mat4 viewproj;
    mat4 unproj;
    vec4 ambient_color;
    vec4 sun_dir;
    vec4 sun_color;
};

layout(buffer_reference, std430) readonly buffer SceneDataBuffer {
    SceneData sceneData;
};