use crate::asset::texture::TextureKind;
use crate::asset::texture::{Texture, TextureId, TextureManager};
use crate::pipeline::shadow_mapping::{ShadowMappingPipeline, SHADOW_MAP_SIZE};
use crate::resource::buffer::AllocatedBuffer;
use crate::resource::immediate_submit::SubmitContext;
use crate::resource::AllocUsage;
use crate::scene::mesh::Mesh;
use crate::DEPTH_FORMAT;
use ash::vk;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4, Vec4Swizzles};
use log::{debug, error};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::RwLock;

pub type LightId = usize;
pub struct Light {
    pub id: LightId,
    pub meta: LightMeta,
    pub data: RawLight,
}

// this is used for calculating e.g. viewproj on CPU and is not sent to the GPU
pub enum LightMeta {
    Spotlight { fov: f32, extent: (f32, f32) },
    Pointlight,
}

impl Light {
    pub fn new_spotlight(
        position: impl Into<[f32; 3]>,
        color: [f32; 3],
        fov_radians: f32,
        extent: (f32, f32),
        dir: impl Into<[f32; 3]> + Copy,
        intensity: f32,
    ) -> Self {
        let position = position.into();
        let dir = dir.into();
        let cutoff_angle = 180.0f32.to_radians().cos();
        let view = Mat4::look_to_lh(Vec3::from(position), Vec3::from(dir), -Vec3::Y);
        let proj = Mat4::orthographic_lh(-10.0, 10.0, -10.0, 10.0, 0.1, 100.0);
        Self {
            id: 0,
            meta: LightMeta::Spotlight { fov: fov_radians, extent },
            data: RawLight {
                position: [position[0], position[1], position[2], 1.0],
                color: [color[0], color[1], color[2], 1.0],
                viewproj: (proj * view).to_cols_array_2d(),
                direction: [dir[0], dir[1], dir[2], 0.0],
                intensity,
                cutoff_angle,
                inner_angle: 180.0f32.to_radians(),
                radius: 100.0,
                shadow_map: 0,
            },
        }
    }
}

#[repr(C)]
#[derive(Pod, Zeroable, Debug, Copy, Clone)]
pub struct RawLight {
    pub position: [f32; 4],
    pub color: [f32; 4],
    pub viewproj: [[f32; 4]; 4],
    pub direction: [f32; 4],
    pub intensity: f32,
    pub cutoff_angle: f32,
    pub inner_angle: f32,
    pub radius: f32,
    pub shadow_map: TextureId,
}

impl RawLight {
    pub fn update_viewproj(&mut self) {
        // let view = Mat4::look_to_rh(Vec4::from(self.position).xyz(), Vec4::from(self.direction).xyz(), -Vec3::Y);
        let view = Mat4::look_at_lh(Vec4::from(self.position).xyz(), Vec4::ZERO.xyz(), -Vec3::Y);
        let proj = Mat4::orthographic_lh(-10.0, 10.0, -10.0, 10.0, 0.1, 100.0);
        self.viewproj = (proj * view).to_cols_array_2d();
    }
}

pub struct LightManager {
    lights: Vec<Light>, // todo hashmap
    max_id: LightId,
    buffer: AllocatedBuffer,
    pub count_dirty: bool, // whether the light count is dirty
}

impl LightManager {
    const PREALLOC_COUNT: u64 = 16; // how many lights to preallocate space for
    pub fn new(device: &ash::Device, allocator: &mut crate::resource::Allocator) -> Self {
        let buffer = AllocatedBuffer::new(
            device,
            allocator,
            vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            AllocUsage::GpuOnly,
            Self::PREALLOC_COUNT * size_of::<RawLight>() as u64,
            Some("Light Buffer".to_string()),
        );

        Self {
            lights: Vec::new(),
            max_id: 0,
            buffer,
            count_dirty: false,
        }
    }

    // Adds a light to the manager and returns its id. Resizes buffer if needed.
    pub fn add_light(&mut self, mut light: Light, ctx: &mut SubmitContext, texture_manager: Rc<RefCell<TextureManager>>) -> LightId {
        light.id = self.max_id;

        let shadow_map = Texture::new(
            TextureManager::DEFAULT_SAMPLER_LINEAR,
            DEPTH_FORMAT,
            ctx,
            Some(format!("shadow_map_{}", light.id)),
            vk::Extent3D {
                width: SHADOW_MAP_SIZE.0,
                height: SHADOW_MAP_SIZE.1,
                depth: 1,
            },
            TextureKind::Depth,
        );
        let shadow_map_id = texture_manager.borrow_mut().add_texture(shadow_map, &ctx.device, true);
        light.data.shadow_map = shadow_map_id;

        self.lights.push(light);
        self.max_id += 1;
        if self.lights.len() as u64 > self.buffer.size / size_of::<RawLight>() as u64 {
            self.resize(ctx);
        }
        self.rewrite_buffer(ctx);

        self.count_dirty = true;
        self.max_id - 1
    }

    pub fn get_light(&self, id: LightId) -> Option<&Light> {
        self.lights.iter().find(|light| light.id == id)
    }

    // Resizes the buffer to double the current capacity.
    fn resize(&mut self, ctx: &mut SubmitContext) {
        let current_capacity = self.buffer.size;
        let new_capacity = current_capacity * 2;
        self.buffer.resize(ctx, new_capacity);
    }

    // Rewrites the whole buffer to GPU.
    fn rewrite_buffer(&mut self, ctx: &mut SubmitContext) {
        let cleanup = self.buffer.write(
            &self.lights.iter().map(|light| light.data).collect::<Vec<_>>(),
            0,
            &ctx.device,
            &mut ctx.allocator.borrow_mut(),
            ctx.cmd_buffer,
        );
        ctx.add_cleanup(cleanup);
    }

    // Rewrites a single light
    fn rewrite_light(&mut self, index: usize, data: RawLight, ctx: &mut SubmitContext) {
        let cleanup = self.buffer.write(
            &[data],
            index as u64 * std::mem::size_of::<RawLight>() as u64,
            &ctx.device,
            &mut ctx.allocator.borrow_mut(),
            ctx.cmd_buffer,
        );
        ctx.add_cleanup(cleanup);
    }

    // Removes a light and rewrites the entire buffer. Does not shrink the buffer.
    pub fn remove_light(&mut self, id: LightId, ctx: &mut SubmitContext) {
        self.lights.retain(|light| light.id != id);
        self.count_dirty = true;
        self.rewrite_buffer(ctx);
    }

    fn index_of(&self, id: LightId) -> Option<usize> {
        self.lights.iter().position(|light| light.id == id)
    }

    pub fn update_light(&mut self, id: LightId, update_fn: impl FnOnce(&mut RawLight), ctx: &mut SubmitContext) {
        debug!("Updating light");
        let mut found_light = None;
        if let Some((index, light)) = self.lights.iter_mut().enumerate().find(|(_, light)| light.id == id) {
            update_fn(&mut light.data);
            found_light = Some((index, light.data));
        } else {
            error!("Light with id {} not found", id);
        }
        let light = found_light.unwrap();
        self.rewrite_light(light.0, light.1, ctx);
    }

    pub fn device_address(&self, device: &ash::Device) -> vk::DeviceAddress {
        self.buffer.device_address(device)
    }

    pub fn count(&self) -> usize {
        self.lights.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Light> {
        self.lights.iter()
    }

    pub fn keys(&self) -> Vec<LightId> {
        self.lights.iter().map(|light| light.id).collect()
    }
}
