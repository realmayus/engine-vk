#version 450
#include "globals.glsl"
#include "util.glsl"
#extension GL_EXT_nonuniform_qualifier : enable


layout (location = 0) in vec3 worldPos;
layout (location = 1) in vec2 texCoords;
layout (location = 2) in vec3 normal;
layout (location = 3) in mat4 normalMatrix;
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

vec3 calcPointLight(Light light, vec3 v, vec3 f0, vec3 n)
{
    vec3 l = normalize(light.position.xyz - worldPos);
    vec3 h = normalize(v + l);
    float distance = length(light.position.xyz - worldPos);
    float attenuation = 1.0 / (distance * distance);

    // glTF intensity given in Candela
    // 570nm for white light, see https://discourse.threejs.org/t/luminous-intensity-calculation/19242/6
    float intensity = light.intensity / 570;
    vec3 radiance = light.color.rgb * attenuation * intensity;

    // Cook-Torrance BRDF
    float ndf = DistributionGGX(n, h, PushConstants.pbrMaterial.roughness);
    float g   = GeometrySmith(n, v, l, PushConstants.pbrMaterial.roughness);
    vec3 f    = fresnelSchlick(clamp(dot(h, v), 0.0, 1.0), f0);


    vec3 numerator    = ndf * g * f;
    float denominator = 4.0 * max(dot(n, v), 0.0) * max(dot(n, l), 0.0) + 0.0001; // + 0.0001 to prevent divide by zero
    vec3 specular = numerator / denominator;

    // kS is equal to Fresnel
    vec3 kS = f;
    // for energy conservation, the diffuse and specular light can't
    // be above 1.0 (unless the surface emits light); to preserve this
    // relationship the diffuse component (kD) should equal 1.0 - kS.
    vec3 kD = vec3(1.0) - kS;
    // multiply kD by the inverse metalness such that only non-metals
    // have diffuse lighting, or a linear blend if partly metal (pure metals
    // have no diffuse light).
    kD *= 1.0 - PushConstants.pbrMaterial.metallic;

    // scale light by NdotL
    float NdotL = max(dot(n, l), 0.0);

    // add to outgoing radiance Lo
    return (kD * PushConstants.pbrMaterial.albedo.rgb / PI + specular) * radiance * NdotL;  // note that we already multiplied the BRDF by the Fresnel (kS) so we won't multiply by kS again    return color;
}

void main()
{
    vec3 normal = normalize(normal);
    vec3 v = normalize(PushConstants.sceneDataBuffer.camera_position.xyz - worldPos);
    vec3 f0 = vec3(0.04);
    f0 = mix(f0, PushConstants.pbrMaterial.albedo.xyz, PushConstants.pbrMaterial.metallic);
    vec3 lo = vec3(0.0);
    for(int i = 0; i < PushConstants.sceneDataBuffer.num_lights; i++)
    {
            Light light = PushConstants.lightBuffer.lights[i];
            vec4 lightPos = PushConstants.sceneDataBuffer.view * vec4(light.position.xyz, 1.0);
            float spotFactor = 1.0;  // multiplier to account for spotlight
            vec4 eye_coords = PushConstants.sceneDataBuffer.view * vec4(worldPos, 1.0);
            vec3 L = normalize( lightPos.xyz/lightPos.w - eye_coords.xyz );
            if (light.cutoff_angle > 0.0) { // the light is a spotlight
                vec3 dir = (normalMatrix * light.direction).xyz;
                vec3 D = -normalize(dir);
                float spotCosine = dot(D,L);
                if (spotCosine >= light.cutoff_angle) {
                    spotFactor = 1.0;
                }
                else { // The point is outside the cone of light from the spotlight.
                    spotFactor = 0.0; // The light will add no color to the point.
                }
            }

            lo += calcPointLight(light, v, f0, normal) * 1.0;

    }

    vec3 ambient = vec3(0.03) * PushConstants.pbrMaterial.albedo.xyz; // * ao
    vec3 color = ambient + lo;
    color = color / (color + vec3(1.0));
    color = pow(color, vec3(1.0/2.2));
    outFragColor = vec4(color, 1.0) * texture(tex[PushConstants.pbrMaterial.texture], texCoords);
}