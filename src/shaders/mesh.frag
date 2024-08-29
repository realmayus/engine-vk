#version 450
#include "globals.glsl"
#include "util.glsl"
#extension GL_EXT_nonuniform_qualifier : enable


layout (location = 0) in vec3 worldPos;
layout (location = 1) in vec2 texCoords;
layout (location = 2) in vec3 inNormal;
layout (location = 0) out vec4 outFragColor;

layout( push_constant ) uniform constants {
    mat4 transform;
    SceneDataBuffer sceneDataBuffer;
    VertexBuffer vertexBuffer;
    PbrMaterial pbrMaterial;
    LightBuffer lightBuffer;
} PushConstants;

layout (set = 0, binding = 2) uniform sampler2D tex[];

float getSquareFalloffAttenuation(vec3 posToLight, float lightInvRadius) {
    float distanceSquare = dot(posToLight, posToLight);
    float factor = distanceSquare * lightInvRadius * lightInvRadius;
    float smoothFactor = max(1.0 - factor * factor, 0.0);
    return (smoothFactor * smoothFactor) / max(distanceSquare, 1e-4);
}

float getSpotAngleAttenuation(vec3 l, vec3 lightDir, float innerAngle, float outerAngle) {
    // the scale and offset computations can be done CPU-side
    float cosOuter = cos(outerAngle);
    float spotScale = 1.0 / max(cos(innerAngle) - cosOuter, 1e-4);
    float spotOffset = -cosOuter * spotScale;

    float cd = dot(normalize(-lightDir), l);
    float attenuation = clamp(cd * spotScale + spotOffset, 0.0, 1.0);
    return attenuation * attenuation;
}

vec3 BSDF(Light light, float roughness, vec3 f0, vec3 n, vec3 diffuseColor, vec3 l) {
    // view vector
    vec3 v = normalize(PushConstants.sceneDataBuffer.camera_position.xyz - worldPos);
    vec3 h = normalize(v + l);

    float NoV = abs(dot(n, v)) + 1e-5;
    float NoL = clamp(dot(n, l), 0.0, 1.0);
    float NoH = clamp(dot(n, h), 0.0, 1.0);
    float LoH = clamp(dot(l, h), 0.0, 1.0);


    float D = D_GGX(NoH, roughness);
    vec3  F = F_Schlick(LoH, f0);
    float V = V_SmithGGXCorrelated(NoV, NoL, roughness);

    // specular BRDF
    vec3 Fr = (D * V) * F;
//    vec3 energyCompensation = 1.0 + f0 * (1.0 / dfg.y - 1.0);
    vec3 energyCompensation = 1.0 + f0 * (1.0 / vec3(0.9, 0.1, 0.0) - 1.0); // todo implement DFG LUT
    // Scale the specular lobe to account for multiscattering
//    Fr *= pixel.energyCompensation;
//    Fr *= energyCompensation;


    // diffuse BRDF
    vec3 Fd = diffuseColor * Fd_Lambert();

    return Fr + Fd;
}

vec3 evaluatePunctualLight(Light light, float roughness, vec3 f0, vec3 n, vec3 diffuseColor) {
    // light incident vector
    vec3 l = normalize(light.position.xyz - worldPos);
    float NoL = clamp(dot(n, l), 0.0, 1.0);
    vec3 posToLight = light.position.xyz - worldPos;

    float attenuation;
    float invRadius = 1.0 / light.radius;
    attenuation  = getSquareFalloffAttenuation(posToLight, invRadius);
    attenuation *= getSpotAngleAttenuation(l, light.direction.xyz, light.inner_angle, light.outer_angle);

    vec3 luminance = (BSDF(light, roughness, f0, n, diffuseColor, l) * light.intensity * attenuation * NoL) * light.color.rgb; // = * light color
    return luminance;
}


const mat4 bias = mat4(
0.5, 0.0, 0.0, 0.0,
0.0, 0.5, 0.0, 0.0,
0.0, 0.0, 1.0, 0.0,
0.5, 0.5, 0.0, 1.0 );


float textureProj(vec4 shadowCoord, vec2 off, uint shadowMap)
{
    float shadow = 1.0;
    if ( shadowCoord.z > -1.0 && shadowCoord.z < 1.0 )
    {
        float dist = texture( tex[shadowMap], shadowCoord.st + off ).r;
        if ( shadowCoord.w > 0.0 && dist < shadowCoord.z )
        {
            shadow = 0.0;
        }
    }
    return shadow;
}

void main() {
    PbrMaterial mat = PushConstants.pbrMaterial;
//    float roughness = perceptualRoughness * perceptualRoughness;
    float roughness = mat.roughness;
    float metallic = mat.metallic;
    float reflectance = 0.0;
    vec3 baseColor = mat.albedo.rgb * texture(tex[mat.albedo_tex], texCoords).rgb;
    vec3 diffuseColor = (1.0 - metallic) * baseColor.rgb;
    vec3 acc = vec3(0.0);
    vec3 normal = inNormal;

    vec3 f0 = 0.16 * reflectance * reflectance * (1.0 - metallic) + baseColor * metallic;

    for(int i = 0; i < PushConstants.sceneDataBuffer.num_lights; i++)
    {
        Light light = PushConstants.lightBuffer.lights[i];
        vec4 lightPos = PushConstants.sceneDataBuffer.view * vec4(light.position.xyz, 1.0);

        vec4 fragPosLightSpace = bias * light.lightspace * vec4(worldPos.xyz, 1.0);
        vec4 projCoords = fragPosLightSpace / fragPosLightSpace.w;
        float shadow = textureProj(projCoords, vec2(0.0, 0.0), light.shadow_map);
        acc += shadow * evaluatePunctualLight(light, roughness, f0, normal, diffuseColor);
    }
    acc = clamp(acc, 0.0, 1.0);
    vec3 ambient = vec3(0.03, 0.03, 0.03);

    acc += baseColor * ambient;
    outFragColor = vec4(acc, 1.0);
}