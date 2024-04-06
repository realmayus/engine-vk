pub mod buffer;
pub mod image;

use ash::vk::DeviceMemory;
use ash::{vk, Device};

use log::debug;

pub type Allocation = gpu_alloc::MemoryBlock<DeviceMemory>;
pub type Allocator = gpu_alloc::GpuAllocator<DeviceMemory>;

const LOG_ALLOCATIONS: bool = false;

#[derive(Clone)]
pub struct PoolSizeRatio {
    pub(crate) descriptor_type: vk::DescriptorType,
    pub(crate) ratio: f32,
}

pub struct DescriptorAllocator {
    ratios: Vec<PoolSizeRatio>,
    ready_pools: Vec<vk::DescriptorPool>,
    full_pools: Vec<vk::DescriptorPool>,
    sets_per_pool: u32,
}

impl DescriptorAllocator {
    pub fn new(device: &Device, max_sets: u32, pool_sizes: &[PoolSizeRatio]) -> Self {
        let pool = Self::create_pool(device, pool_sizes, max_sets);
        let sets_per_pool = (max_sets as f32 * 1.5) as u32;
        Self {
            ratios: pool_sizes.to_vec(),
            sets_per_pool,
            ready_pools: vec![pool],
            full_pools: vec![],
        }
    }

    pub fn clear_pools(&self, device: &Device) {
        for pool in self.ready_pools.as_slice() {
            unsafe { device.reset_descriptor_pool(*pool, vk::DescriptorPoolResetFlags::empty()).unwrap() }
        }
        for pool in self.full_pools.as_slice() {
            unsafe { device.reset_descriptor_pool(*pool, vk::DescriptorPoolResetFlags::empty()).unwrap() }
        }
    }

    pub fn destroy_pools(&self, device: &Device) {
        for pool in self.ready_pools.as_slice() {
            unsafe { device.destroy_descriptor_pool(*pool, None) }
        }
        for pool in self.full_pools.as_slice() {
            unsafe { device.destroy_descriptor_pool(*pool, None) }
        }
    }

    pub fn allocate(&mut self, device: &Device, layout: vk::DescriptorSetLayout) -> vk::DescriptorSet {
        let pool = self.get_or_create_pool(device);
        let layouts = [layout];
        let allocate_info = vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts);
        let (pool, descriptor_set) = match unsafe { device.allocate_descriptor_sets(&allocate_info) } {
            Ok(res) => (pool, res[0]),
            Err(vk::Result::ERROR_OUT_OF_POOL_MEMORY) | Err(vk::Result::ERROR_FRAGMENTED_POOL) => {
                self.full_pools.push(pool);
                let new_pool = self.get_or_create_pool(device);
                let new_allocate_info = vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(new_pool)
                    .set_layouts(&layouts);
                (new_pool, unsafe {
                    device
                        .allocate_descriptor_sets(&new_allocate_info)
                        .expect("Failed to allocate descriptor set")[0]
                })
            }
            Err(e) => panic!("Failed to allocate descriptor set: {:?}", e),
        };
        self.ready_pools.push(pool);
        descriptor_set
    }

    pub fn get_or_create_pool(&mut self, device: &Device) -> vk::DescriptorPool {
        if !self.ready_pools.is_empty() {
            self.ready_pools.pop().unwrap()
        } else {
            let new = Self::create_pool(device, &self.ratios, self.sets_per_pool);
            self.sets_per_pool = (self.sets_per_pool as f32 * 1.5) as u32;
            if self.sets_per_pool > 4092 {
                self.sets_per_pool = 4092;
            }
            new
        }
    }

    fn create_pool(device: &Device, ratios: &[PoolSizeRatio], sets_per_pool: u32) -> vk::DescriptorPool {
        let pool_sizes: Vec<vk::DescriptorPoolSize> = ratios
            .iter()
            .map(|pool_size| vk::DescriptorPoolSize {
                ty: pool_size.descriptor_type,
                descriptor_count: (sets_per_pool as f32 * pool_size.ratio).ceil() as u32,
            })
            .collect();
        let info = vk::DescriptorPoolCreateInfo::default()
            .max_sets(sets_per_pool)
            .pool_sizes(&pool_sizes);
        let pool = unsafe { device.create_descriptor_pool(&info, None).unwrap() };
        debug!("Created descriptor pool {:?}", pool);
        pool
    }
}

pub struct DescriptorBufferWriteInfo {
    pub binding: u32,
    pub array_index: u32,
    pub buffer: vk::Buffer,
    pub size: vk::DeviceSize,
    pub offset: vk::DeviceSize,
    pub ty: vk::DescriptorType,
}

pub struct DescriptorImageWriteInfo {
    pub binding: u32,
    pub array_index: u32,
    pub image_view: vk::ImageView,
    pub sampler: vk::Sampler,
    pub layout: vk::ImageLayout,
    pub ty: vk::DescriptorType,
}

pub fn update_set(
    device: &Device,
    set: vk::DescriptorSet,
    image_writes: &[DescriptorImageWriteInfo],
    buffer_writes: &[DescriptorBufferWriteInfo],
) {
    let mut writes = vec![];
    let buffer_infos = buffer_writes
        .iter()
        .map(|write| {
            [vk::DescriptorBufferInfo {
                buffer: write.buffer,
                offset: write.offset,
                range: write.size,
            }]
        })
        .collect::<Vec<_>>();
    for (i, write) in buffer_writes.iter().enumerate() {
        writes.push(
            vk::WriteDescriptorSet::default()
                .dst_binding(write.binding)
                .dst_set(set)
                .dst_array_element(write.array_index)
                .descriptor_type(write.ty)
                .buffer_info(&buffer_infos[i]),
        );
    }
    let image_infos = image_writes
        .iter()
        .map(|write| {
            [vk::DescriptorImageInfo {
                image_view: write.image_view,
                sampler: write.sampler,
                image_layout: write.layout,
            }]
        })
        .collect::<Vec<_>>();
    for (i, write) in image_writes.iter().enumerate() {
        writes.push(
            vk::WriteDescriptorSet::default()
                .dst_binding(write.binding)
                .dst_set(set)
                .dst_array_element(write.array_index)
                .descriptor_type(write.ty)
                .image_info(&image_infos[i]),
        );
    }

    // println!("Writes: {:#?}", writes);

    unsafe { device.update_descriptor_sets(&writes, &[]) }
}

pub enum AllocUsage {
    GpuOnly,
    Shared,
    UploadToHost,
}

impl AllocUsage {
    pub fn flags(&self) -> gpu_alloc::UsageFlags {
        match self {
            AllocUsage::GpuOnly => gpu_alloc::UsageFlags::FAST_DEVICE_ACCESS,
            AllocUsage::Shared => {
                gpu_alloc::UsageFlags::HOST_ACCESS
                    | gpu_alloc::UsageFlags::FAST_DEVICE_ACCESS
                    | gpu_alloc::UsageFlags::DOWNLOAD
                    | gpu_alloc::UsageFlags::UPLOAD
            }
            AllocUsage::UploadToHost => gpu_alloc::UsageFlags::DOWNLOAD | gpu_alloc::UsageFlags::UPLOAD,
        }
    }
}
