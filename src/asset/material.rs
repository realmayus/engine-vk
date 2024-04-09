use crate::asset::texture::{TextureId, TextureManager};
use crate::resource::buffer::AllocatedBuffer;
use crate::resource::immediate_submit::SubmitContext;
use crate::resource::AllocUsage;
use ash::{vk, Device};
use bytemuck::{Pod, Zeroable};
use hashbrown::HashMap;

pub type MaterialId = usize;

#[derive(Debug)]
pub struct Material {
    pub id: MaterialId,
    pub label: Option<String>,
    buffer: AllocatedBuffer,
    pub data: RawMaterial,
}
impl Material {
    pub fn new(label: Option<String>, data: RawMaterial, ctx: &mut SubmitContext) -> Self {
        let size = match data {
            RawMaterial::Unlit(_) => std::mem::size_of::<UnlitMaterial>(),
            RawMaterial::Pbr(_) => std::mem::size_of::<PbrMaterial>(),
        };
        let mut buffer = AllocatedBuffer::new(
            &ctx.device,
            &mut ctx.allocator.borrow_mut(),
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            AllocUsage::GpuOnly,
            size as vk::DeviceSize,
            label.clone(),
        );
        let cleanup = match data {
            RawMaterial::Unlit(unlit) => buffer.write(&[unlit], 0, &ctx.device, &mut ctx.allocator.borrow_mut(), ctx.cmd_buffer),
            RawMaterial::Pbr(pbr) => buffer.write(&[pbr], 0, &ctx.device, &mut ctx.allocator.borrow_mut(), ctx.cmd_buffer),
        };
        ctx.add_cleanup(Box::new(move |device, allocator| {
            cleanup(device, allocator);
        }));
        Self {
            id: 0,
            label,
            buffer,
            data,
        }
    }

    pub fn buffer_address(&self, device: &Device) -> vk::DeviceAddress {
        self.buffer.device_address(device)
    }
}

#[derive(Debug)]
pub enum RawMaterial {
    Unlit(UnlitMaterial),
    Pbr(PbrMaterial),
}

#[repr(C)]
#[derive(Pod, Zeroable, Copy, Clone, Debug)]
pub struct UnlitMaterial {
    pub texture: TextureId, // 0 if no texture
    pub color: [f32; 3],
}

impl Default for UnlitMaterial {
    fn default() -> Self {
        Self {
            texture: 0,
            color: [1.0, 1.0, 1.0],
        }
    }
}

#[repr(C)]
#[derive(Pod, Zeroable, Copy, Clone, Debug)]
pub struct PbrMaterial {
    pub albedo: [f32; 4],
    pub texture: TextureId, // 0 if no texture
    pub metallic: f32,
    pub roughness: f32,
    pub padding: f32,
}

impl Default for PbrMaterial {
    fn default() -> Self {
        Self {
            texture: 0,
            albedo: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.0,
            padding: 0.0,
        }
    }
}

pub struct MaterialManager {
    materials: HashMap<MaterialId, Material>,
    max_id: MaterialId,
}

impl MaterialManager {
    pub const DEFAULT_MATERIAL: MaterialId = 0;
    pub fn new(ctx: &mut SubmitContext) -> Self {
        let default_material = ctx.nest(Box::new(|ctx| {
            Material::new(
                Some("Default material".into()),
                RawMaterial::Pbr(PbrMaterial {
                    texture: TextureManager::DEFAULT_TEXTURE_WHITE,
                    albedo: [1.0, 1.0, 1.0, 1.0],
                    metallic: 0.0,
                    roughness: 0.0,
                    padding: 0.0,
                }),
                ctx,
            )
        }));

        Self {
            materials: HashMap::from([(0, default_material)]),
            max_id: 1,
        }
    }

    pub fn add_material(&mut self, mut material: Material) -> MaterialId {
        material.id = self.max_id;
        self.materials.insert(self.max_id, material);
        self.max_id += 1;
        self.max_id - 1
    }

    pub fn iter_materials(&self) -> impl Iterator<Item = &Material> {
        self.materials.values()
    }

    pub fn next_free_id(&self) -> MaterialId {
        self.max_id + 1
    }

    pub fn get_material(&self, id: MaterialId) -> Option<&Material> {
        self.materials.get(&id)
    }

    pub fn get_material_mut(&mut self, id: MaterialId) -> Option<&mut Material> {
        self.materials.get_mut(&id)
    }
}
