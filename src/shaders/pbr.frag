#version 450
#include "globals.glsl"
#include "util.glsl"
#extension GL_EXT_nonuniform_qualifier : enable

const float Epsilon = 0.00001;

layout (location = 0) in vec3 worldPos;
layout (location = 1) in vec2 texCoords;
layout (location = 3) in mat3 tangentBasis;
layout (location = 0) out vec4 outFragColor;

layout( push_constant ) uniform constants
{
    mat4 transform;
    SceneDataBuffer sceneDataBuffer;
    VertexBuffer vertexBuffer;
    PbrMaterial pbrMaterial;
    LightBuffer lightBuffer;
} PushConstants;

layout (set = 0, binding = 2) uniform sampler2D tex[];

vec3 calcPointLight(Light light, vec3 Lo, vec3 F0, vec3 N, float metalness, float roughness, float cosLo, vec4 albedo)
{
    vec3 Li = normalize(light.position.xyz - worldPos);
    vec3 Lradiance = light.radiance;

    // Half-vector between Li and Lo.
    vec3 Lh = normalize(Li + Lo);

    // Calculate angles between surface normal and various light vectors.
    float cosLi = max(0.0, dot(N, Li));
    float cosLh = max(0.0, dot(N, Lh));

    // Calculate Fresnel term for direct lighting.
    vec3 F = fresnelSchlick(max(0.0, dot(Lh, Lo)), F0);
    // Calculate normal distribution for specular BRDF.
    float D = ndfGGX(cosLh, roughness);
    // Calculate geometric attenuation for specular BRDF.
    float G = gaSchlickGGX(cosLi, cosLo, roughness);

    // Diffuse scattering happens due to light being refracted multiple times by a dielectric medium.
    // Metals on the other hand either reflect or absorb energy, so diffuse contribution is always zero.
    // To be energy conserving we must scale diffuse BRDF contribution based on Fresnel factor & metalness.
    vec3 kd = mix(vec3(1.0) - F, vec3(0.0), metalness);

    // Lambert diffuse BRDF.
    // We don't scale by 1/PI for lighting & material units to be more convenient.
    // See: https://seblagarde.wordpress.com/2012/01/08/pi-or-not-to-pi-in-game-lighting-equation/
    vec3 diffuseBRDF = kd * albedo.rgb;

    // Cook-Torrance specular microfacet BRDF.
    vec3 specularBRDF = (F * D * G) / max(Epsilon, 4.0 * cosLi * cosLo);

    // Total contribution for this light.
    return (diffuseBRDF + specularBRDF) * Lradiance * cosLi;
}

void main()
{
    PbrMaterial material = PushConstants.pbrMaterial;
    vec4 albedoTexture = texture(tex[material.albedoTexture], texCoords) * material.albedo;
    vec3 normalTexture = texture(tex[material.normalTexture], texCoords).rgb;
    vec2 metallicRoughnessTexture = texture(tex[material.metallicRoughnessTexture], texCoords).rg;
    float metallicTexture = metallicRoughnessTexture.r * material.metallic;
    float roughnessTexture = metallicRoughnessTexture.g * material.roughness;

    vec3 N = normalize(2.0 * normalTexture - 1.0);
    N = normalize(tangentBasis * N);
    vec3 Lo = normalize(PushConstants.sceneDataBuffer.camera_position.xyz - worldPos);
    float cosLo = max(0.0, dot(N, Lo)); // Angle between surface normal and outgoing light direction
    vec3 Lr = 2.0 * cosLo * N - Lo; // Specular reflection vector.
    vec3 f0 = vec3(0.04); // Fresnel reflectance at normal incidence (for metals use albedo color).
    f0 = mix(f0, material.albedo.xyz, material.metallic);
    vec3 directLighting = vec3(0.0);
    for(int i = 0; i < PushConstants.sceneDataBuffer.num_lights; i++)
    {
        Light light = PushConstants.lightBuffer.lights[i];

        directLighting += calcPointLight(light, Lo, f0, N, metallicTexture, roughnessTexture, cosLo, albedoTexture);
    }

    vec3 ambient = vec3(0.03) * material.albedo.xyz ; // * ao
    vec3 color = ambient + directLighting;
    color = color / (color + vec3(1.0));
    color = pow(color, vec3(1.0/2.2));
    outFragColor = vec4(color, 1.0);
}