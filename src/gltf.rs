use crate::asset::material::MaterialManager;
use crate::asset::material::{Material, MaterialId, PbrMaterial, RawMaterial};
use crate::asset::texture::{Texture, TextureManager};
use crate::resource::immediate_submit::SubmitContext;
use crate::resource::Allocator;
use crate::scene::light::{Light, LightManager, LightMeta};
use crate::scene::mesh::Mesh;
use crate::scene::model::{Model, ModelId};
use crate::scene::world::World;
use ash::{vk, Device};
use glam::{Mat4, Vec2, Vec3, Vec4, Vec4Swizzles};
use gltf::khr_lights_punctual::Kind;
use hashbrown::HashMap;
use log::info;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

pub struct GltfReader {
    world: Rc<RefCell<World>>,
    texture_manager: Rc<RefCell<TextureManager>>,
    material_manager: Rc<RefCell<MaterialManager>>,
    light_manager: Rc<RefCell<LightManager>>,
    material_mappings: HashMap<usize, MaterialId>,
}

struct ImageData {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl GltfReader {
    pub fn new(
        world: Rc<RefCell<World>>,
        texture_manager: Rc<RefCell<TextureManager>>,
        material_manager: Rc<RefCell<MaterialManager>>,
        light_manager: Rc<RefCell<LightManager>>,
    ) -> Self {
        Self {
            world,
            texture_manager,
            material_manager,
            light_manager,
            material_mappings: HashMap::new(),
        }
    }

    pub fn load(&mut self, path: &Path, ctx: SubmitContext) {
        let (gltf, buffers, images) = gltf::import(path).unwrap();
        let images = images.iter().map(|image| image.to_rgba8()).collect::<Vec<_>>();
        let images = images
            .iter()
            .map(|image| ImageData {
                width: image.width(),
                height: image.height(),
                data: image.to_vec(),
            })
            .collect::<Vec<_>>();
        let device = ctx.device.clone();
        ctx.immediate_submit(Box::new(|ctx| {
            let mut mapping = HashMap::<usize, ModelId>::new();
            for node in gltf.nodes() {
                let model = self.load_model(&node, &buffers, &images, ctx, Mat4::IDENTITY);
                mapping.insert(node.index(), model);
            }
            for node in gltf.nodes() {
                let model = mapping.get(&node.index()).unwrap();
                for child in node.children() {
                    self.world
                        .borrow_mut()
                        .models
                        .get_mut(model)
                        .unwrap()
                        .children
                        .push(*mapping.get(&child.index()).unwrap());
                }
            }
            let models = self.world.borrow_mut().get_toplevel_model_ids().clone();
            for model in models {
                self.world
                    .borrow_mut()
                    .update_transforms(model, Mat4::IDENTITY, &mut self.light_manager.borrow_mut(), ctx);
            }
        }));
        self.texture_manager.borrow_mut().update_set(&device);
    }

    fn load_model(
        &mut self,
        node: &gltf::Node,
        buffers: &[gltf::buffer::Data],
        images: &[ImageData],
        ctx: &mut SubmitContext,
        parent_transform: Mat4,
    ) -> ModelId {
        let node_transform = Mat4::from_cols_array_2d(&node.transform().matrix());
        let mut model = Model {
            label: node.name().map(|x| x.to_string()),
            transform: node_transform,
            ..Default::default()
        };

        if let Some(light) = node.light() {
            info!("Node has a light!");
            match light.kind() {
                Kind::Directional => {
                    info!("Directional light");
                }
                Kind::Point => {
                    info!("Point light");
                    let light = Light::new_pointlight(node_transform.w_axis.xyz(), [1.0, 1.0, 1.0], 60.0f32.to_radians());
                    let light = self.light_manager.borrow_mut().add_light(light, ctx);
                    model.light = Some(light);
                }
                Kind::Spot {
                    inner_cone_angle,
                    outer_cone_angle,
                } => {
                    info!("Spot light");
                    // get the direction of the spotlight by applying node_transform:
                    let dir = (-Vec4::Y).normalize(); // todo check this

                    let light = Light::new_spotlight(
                        node_transform.w_axis.xyz(),
                        [1.0, 1.0, 1.0],
                        60.0f32.to_radians(),
                        (1000.0, 1000.0),
                        dir.xyz(),
                        light.intensity(),
                    );
                    let light = self.light_manager.borrow_mut().add_light(light, ctx);
                    model.light = Some(light);
                }
            }
        }

        if node.name().map(|x| x.starts_with("LightStrip")).unwrap_or(false) {
            println!("{:#?}", node_transform);
        }
        for mesh in node.mesh().iter() {
            for primitive in mesh.primitives() {
                let mut vertices = Vec::new();
                let mut indices = Vec::new();
                let mut normals = Vec::new();
                let mut uvs = Vec::new();
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
                if let Some(iter) = reader.read_positions() {
                    for position in iter {
                        vertices.push(Vec3::from(position));
                    }
                }
                if let Some(iter) = reader.read_indices() {
                    for index in iter.into_u32() {
                        indices.push(index);
                    }
                }
                if let Some(iter) = reader.read_normals() {
                    for normal in iter {
                        normals.push(Vec3::from(normal));
                    }
                }
                if let Some(iter) = reader.read_tex_coords(0) {
                    for uv in iter.into_f32() {
                        uvs.push(Vec2::new(uv[0], uv[1]));
                    }
                }
                let gltf_material = primitive.material();
                let material_id = if let Some(index) = gltf_material.index() {
                    self.material_mappings
                        .get(&index)
                        .copied()
                        .unwrap_or_else(|| self.load_material(gltf_material, images, ctx))
                } else {
                    MaterialManager::DEFAULT_MATERIAL
                };
                let mut mesh = Mesh {
                    mem: None,
                    vertices,
                    indices,
                    normals,
                    uvs,
                    material: material_id,
                    transform: parent_transform,
                };
                ctx.nest(Box::new(|ctx| {
                    mesh.upload(ctx);
                }));
                model.meshes.push(mesh);
            }
        }
        self.world.borrow_mut().add_model(model)
    }

    fn load_material(&mut self, material: gltf::Material, images: &[ImageData], ctx: &mut SubmitContext) -> MaterialId {
        if material.index().is_none() {
            return 0;
        }

        let pbr = material.pbr_metallic_roughness();
        let albedo = pbr.base_color_factor();
        let texture = pbr.base_color_texture().map(|info| {
            let image = images.get(info.texture().source().index()).unwrap();

            let texture = ctx.nest(Box::new(|ctx| {
                Texture::new(
                    TextureManager::DEFAULT_SAMPLER_NEAREST,
                    vk::Format::R8G8B8A8_SRGB,
                    ctx,
                    Some(info.texture().name().map(|x| x.to_string()).unwrap_or(format!(
                        "Albedo, Material: {} ({})",
                        material.name().unwrap_or_default(),
                        self.material_manager.borrow().next_free_id()
                    ))),
                    &image.data,
                    vk::Extent3D {
                        width: image.width,
                        height: image.height,
                        depth: 1,
                    },
                    false,
                )
            }));
            self.texture_manager.borrow_mut().add_texture(texture, &ctx.device, false)
        });
        let engine_material = ctx.nest(Box::new(|ctx| {
            Material::new(
                Some(material.name().unwrap_or_default().to_string()),
                RawMaterial::Pbr(PbrMaterial {
                    albedo_tex: texture.unwrap_or(TextureManager::DEFAULT_TEXTURE_WHITE),
                    metallic_roughness_tex: TextureManager::DEFAULT_TEXTURE_WHITE,
                    albedo,
                    metallic: pbr.metallic_factor(),
                    roughness: pbr.roughness_factor(),
                    padding: 0.0,
                }),
                ctx,
            )
        }));

        let engine_material = self.material_manager.borrow_mut().add_material(engine_material);
        self.material_mappings.insert(material.index().unwrap(), engine_material);
        engine_material
    }
}
