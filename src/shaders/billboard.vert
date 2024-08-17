#version 450
#include "globals.glsl"

//push constants block
layout( push_constant ) uniform constants
{
    mat4 transform;  // model matrix
    vec4 center;
    vec4[4] uv;
    SceneDataBuffer sceneDataBuffer;
    PbrMaterial pbrMaterial;
    LightBuffer lightBuffer;
} PushConstants;


layout(location = 0) out vec2 fragOffset;
layout(location = 1) out vec2 texCoords;

const vec2[] vertices = vec2[](
    vec2(-1.0, -1.0),
    vec2(-1.0, 1.0),
    vec2(1.0, -1.0),

    vec2(1.0, -1.0),
    vec2(-1.0, 1.0),
    vec2(1.0, 1.0)
);


const int[] uv_map = int[](
    0, 2, 1, 1, 2, 3
);

const float size = 0.1;

void main()
{
    SceneDataBuffer sceneData = PushConstants.sceneDataBuffer;
    fragOffset = vertices[gl_VertexIndex];
    vec3 cameraRightWorld = {sceneData.view[0][0], sceneData.view[1][0], sceneData.view[2][0]};
    vec3 cameraUpWorld = {sceneData.view[0][1], sceneData.view[1][1], sceneData.view[2][1]};

    vec3 positionWorld = PushConstants.center.xyz
        + size * fragOffset.x * cameraRightWorld
        + size * fragOffset.y * cameraUpWorld;

    gl_Position = sceneData.proj * sceneData.view * vec4(positionWorld, 1.0f);

    texCoords.x = float(gl_VertexIndex);
    texCoords.y = float(gl_VertexIndex);
}