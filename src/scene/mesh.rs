use crate::asset::material::MaterialId;
use crate::pipeline::Vertex;
use crate::resource::buffer::AllocatedBuffer;
use crate::resource::immediate_submit::SubmitContext;
use crate::resource::{AllocUsage, Allocator};
use ash::{vk, Device};
use glam::{Mat4, Vec2, Vec3};
use gpu_alloc_ash::AshMemoryDevice;

pub struct GpuMesh {
    index_buffer: AllocatedBuffer,
    vertex_buffer: AllocatedBuffer,
    vertex_address: vk::DeviceAddress,
}

#[derive(Default)]
pub struct Mesh {
    pub mem: Option<GpuMesh>,
    pub vertices: Vec<Vec3>,
    pub indices: Vec<u32>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub material: MaterialId,
    pub transform: Mat4,
    pub parent_transform: Mat4,
}

impl Mesh {
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.normals.clear();
        self.uvs.clear();
    }
    pub fn upload(&mut self, ctx: &mut SubmitContext) {
        let vertices = self
            .vertices
            .iter()
            .zip(self.normals.iter())
            .zip(self.uvs.iter())
            .map(|((vertex, normal), uv)| Vertex {
                position: vertex.to_array(),
                normal: normal.to_array(),
                uv_x: uv.x,
                uv_y: uv.y,
                color: [0.4, 0.6, 0.3, 1.0],
            })
            .collect::<Vec<_>>();

        let vertex_buffer_size = (vertices.len() * std::mem::size_of::<Vertex>()) as vk::DeviceSize;
        let index_buffer_size = (self.indices.len() * std::mem::size_of::<u32>()) as vk::DeviceSize;
        let vertex_buffer = AllocatedBuffer::new(
            &ctx.device,
            &mut ctx.allocator.borrow_mut(),
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            AllocUsage::GpuOnly,
            vertex_buffer_size,
            Some("Vertex Buffer".into()),
        );
        let device_address_info = vk::BufferDeviceAddressInfo::default().buffer(vertex_buffer.buffer);
        let buffer_device_address = unsafe { ctx.device.get_buffer_device_address(&device_address_info) };
        let index_buffer = AllocatedBuffer::new(
            &ctx.device,
            &mut ctx.allocator.borrow_mut(),
            vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            AllocUsage::GpuOnly,
            index_buffer_size,
            Some("Index Buffer".into()),
        );

        let mut staging = AllocatedBuffer::new(
            &ctx.device,
            &mut ctx.allocator.borrow_mut(),
            vk::BufferUsageFlags::TRANSFER_SRC,
            AllocUsage::Shared,
            vertex_buffer_size + index_buffer_size,
            Some("Mesh Staging Buffer".into()),
        );

        let map = unsafe {
            staging
                .allocation
                .map(AshMemoryDevice::wrap(&ctx.device), 0, staging.size as usize)
                .unwrap()
        };
        // copy vertex buffer
        let vertex_buffer_ptr = map.as_ptr() as *mut Vertex;
        unsafe {
            vertex_buffer_ptr.copy_from_nonoverlapping(vertices.as_ptr(), vertices.len());
            let index_buffer_ptr = vertex_buffer_ptr.add(vertices.len()) as *mut u32;
            index_buffer_ptr.copy_from_nonoverlapping(self.indices.as_ptr(), self.indices.len());
        };

        let vertex_copy = vk::BufferCopy {
            src_offset: 0,
            dst_offset: 0,
            size: vertex_buffer_size,
        };
        unsafe {
            ctx.device
                .cmd_copy_buffer(ctx.cmd_buffer, staging.buffer, vertex_buffer.buffer, &[vertex_copy]);
        };
        let index_copy = vk::BufferCopy {
            src_offset: vertex_buffer_size,
            dst_offset: 0,
            size: index_buffer_size,
        };
        unsafe {
            ctx.device
                .cmd_copy_buffer(ctx.cmd_buffer, staging.buffer, index_buffer.buffer, &[index_copy]);
        };
        // return empty FnOnce closure

        self.mem = Some(GpuMesh {
            vertex_buffer,
            vertex_address: buffer_device_address,
            index_buffer,
        });

        ctx.add_cleanup(Box::from(move |device: &Device, allocator: &mut Allocator| {
            staging.destroy(device, allocator);
        }));
    }

    pub fn vertex_buffer_address(&self) -> vk::DeviceAddress {
        self.mem.as_ref().unwrap().vertex_address
    }
    pub fn index_buffer(&self) -> vk::Buffer {
        self.mem.as_ref().unwrap().index_buffer.buffer
    }

    pub fn destroy(&mut self, device: &Device, allocator: &mut Allocator) {
        if let Some(mem) = self.mem.take() {
            mem.vertex_buffer.destroy(device, allocator);
            mem.index_buffer.destroy(device, allocator);
        }
    }
}
