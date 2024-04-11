#extension GL_EXT_buffer_reference : require


layout(buffer_reference, std430) readonly buffer SceneDataBuffer {
    mat4 view;
    mat4 proj;
    mat4 viewproj;
    mat4 unproj;
    vec4 ambient_color;
    vec4 camera_position;
    uint num_lights;
};

layout(buffer_reference, std430) readonly buffer PbrMaterial {
    vec4 albedo;
    uint texture;
    float metallic;
    float roughness;
};


struct Vertex {
    vec3 position;
    float uv_x;
    vec3 normal;
    float uv_y;
    vec4 color;
};

layout(buffer_reference, std430) readonly buffer VertexBuffer {
    Vertex vertices[];
};

struct Light {
    vec4 position;
    vec4 color;
    mat4 modelview; // only for spotlights; identity matrix for point lights
    vec4 direction;
    uint type;  // 0 - spotlight, 1 - point light
    float intensity;
    float cutoff_angle;
    // uint shadow_map; // texture index
};

layout(buffer_reference, std430) readonly buffer LightBuffer {
    Light lights[];
};