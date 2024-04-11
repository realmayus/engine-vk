use crate::resource::immediate_submit::SubmitContext;
use crate::resource::{AllocUsage, Allocation, Allocator, LOG_ALLOCATIONS};
use ash::vk::DeviceSize;
use ash::{vk, Device};
use gpu_alloc_ash::AshMemoryDevice;
use log::debug;
use std::mem;

pub struct WrappedBuffer<T>
where
    T: Clone,
{
    pub dirty: bool,
    pub buffer: AllocatedBuffer,
    pub data: T,
}

impl<T> WrappedBuffer<T>
where
    T: Clone,
{
    pub fn write(&mut self, ctx: &mut SubmitContext) {
        let cleanup = self.buffer.write(
            &[self.data.clone()],
            0,
            &ctx.device,
            &mut ctx.allocator.borrow_mut(),
            ctx.cmd_buffer,
        );
        ctx.add_cleanup(cleanup);
    }
}

#[derive(Debug)]
pub struct AllocatedBuffer {
    pub buffer: vk::Buffer,
    pub(crate) allocation: Allocation,
    pub(crate) size: DeviceSize,
    pub label: Option<String>,
    flags: vk::BufferUsageFlags,
    alloc_flags: AllocUsage,
}

impl AllocatedBuffer {
    pub fn new(
        device: &Device,
        allocator: &mut Allocator,
        buffer_usages: vk::BufferUsageFlags,
        alloc_usages: AllocUsage,
        size: DeviceSize,
        label: Option<String>,
    ) -> Self {
        debug_assert!(size > 0);
        let usage_flags = buffer_usages | vk::BufferUsageFlags::TRANSFER_SRC | vk::BufferUsageFlags::TRANSFER_DST;
        let info = vk::BufferCreateInfo::default().size(size).usage(usage_flags); // we want to resize almost all buffers, and there's no performance penalty, really...
        let buffer = unsafe { device.create_buffer(&info, None) }.unwrap();
        let reqs = unsafe { device.get_buffer_memory_requirements(buffer) };
        let allocation = unsafe {
            allocator
                .alloc(
                    AshMemoryDevice::wrap(device),
                    gpu_alloc::Request {
                        size: reqs.size,
                        align_mask: reqs.alignment - 1,
                        usage: alloc_usages.flags(),
                        memory_types: reqs.memory_type_bits,
                    },
                    label.clone(),
                )
                .unwrap()
        };

        if LOG_ALLOCATIONS {
            debug!(
                "Creating buffer '{}' ({:?}) of size {} B",
                label.clone().unwrap_or_default(),
                buffer,
                size
            );
        }

        unsafe {
            device
                .bind_buffer_memory(buffer, *allocation.memory(), allocation.offset())
                .unwrap()
        };

        Self {
            buffer,
            allocation,
            size,
            label,
            flags: usage_flags,
            alloc_flags: alloc_usages,
        }
    }

    pub fn write<T>(
        &mut self,
        data: &[T],
        offset: u64,
        device: &Device,
        allocator: &mut Allocator,
        cmd_buffer: vk::CommandBuffer,
    ) -> Box<dyn FnOnce(&Device, &mut Allocator)> {
        debug_assert!(offset + (mem::size_of::<T>() as u64 * data.len() as u64) <= self.size);

        let size = mem::size_of::<T>() as u64 * data.len() as u64;
        let mut staging = AllocatedBuffer::new(
            device,
            allocator,
            vk::BufferUsageFlags::TRANSFER_SRC,
            AllocUsage::Shared,
            size,
            Some(format!(
                "Staging Buffer for AllocatedBuffer {}",
                self.label.as_deref().unwrap_or_default()
            )),
        );

        let map = unsafe {
            staging
                .allocation
                .map(AshMemoryDevice::wrap(device), 0, staging.size as usize)
                .unwrap()
        };

        let staging_ptr = map.as_ptr() as *mut T;
        unsafe {
            staging_ptr.copy_from_nonoverlapping(data.as_ptr(), data.len());
        }
        let staging_copy = vk::BufferCopy {
            src_offset: 0,
            dst_offset: offset,
            size,
        };
        unsafe {
            device.cmd_copy_buffer(cmd_buffer, staging.buffer, self.buffer, &[staging_copy]);
        }
        Box::new(|device, allocator| {
            staging.destroy(device, allocator);
        })
    }

    pub fn device_address(&self, device: &Device) -> vk::DeviceAddress {
        debug_assert!(self.flags.contains(vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS));
        unsafe { device.get_buffer_device_address(&vk::BufferDeviceAddressInfo::default().buffer(self.buffer)) }
    }

    pub(crate) fn destroy(self, device: &Device, allocator: &mut Allocator) {
        if LOG_ALLOCATIONS {
            debug!(
                "Destroying buffer '{}' ({:?}) of size {}",
                self.label.unwrap_or_default(),
                self.buffer,
                self.size
            );
        }
        unsafe { device.destroy_buffer(self.buffer, None) };
        unsafe { allocator.dealloc(AshMemoryDevice::wrap(device), self.allocation) };
    }

    // destroy old buffer, create new buffer and copy all data
    pub fn resize(&mut self, ctx: &mut SubmitContext, new_size: DeviceSize) {
        let new = Self::new(
            &ctx.device,
            &mut ctx.allocator.borrow_mut(),
            self.flags,
            self.alloc_flags,
            new_size,
            self.label.clone(),
        );
        debug!(
            "Resizing buffer '{}' from {} B ({:?}) to {} B ({:?})",
            self.label.as_deref().unwrap_or_default(),
            self.size,
            self.buffer,
            new_size,
            new.buffer
        );
        let copy = vk::BufferCopy {
            src_offset: 0,
            dst_offset: 0,
            size: self.size.min(new_size),
        };
        unsafe {
            ctx.device.cmd_copy_buffer(ctx.cmd_buffer, self.buffer, new.buffer, &[copy]);
        }
        let old = mem::replace(self, new);
        ctx.add_cleanup(Box::new(move |device, allocator| {
            old.destroy(device, allocator);
        }));
    }
}
