#extension GL_EXT_buffer_reference : require
#extension GL_EXT_scalar_block_layout : require

layout(buffer_reference, scalar) readonly buffer SceneDataBuffer {
    mat4 view;
    mat4 proj;
    mat4 viewproj;
    mat4 unproj;
    vec4 ambient_color;
    vec4 camera_position;
    uint num_lights;
};

layout(buffer_reference, scalar) readonly buffer PbrMaterial {
    uint albedo_tex;
    uint metallic_roughness_tex;
    vec4 albedo;
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

layout(buffer_reference, scalar) readonly buffer VertexBuffer {
    Vertex vertices[];
};

struct Light {
    vec4 position;
    vec4 color;
    mat4 lightspace; // only for spotlights; identity matrix for point lights
    vec4 direction;
    float intensity;
    float outer_angle;
    float inner_angle;
    float radius;
    uint shadow_map;
};

layout(buffer_reference, scalar) readonly buffer LightBuffer {
    Light lights[];
};