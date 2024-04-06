use crate::resource::buffer::AllocatedBuffer;
use crate::resource::{AllocUsage, Allocation, Allocator, LOG_ALLOCATIONS};
use crate::util::transition_image;
use crate::SubmitContext;
use ash::vk::DeviceSize;
use ash::{vk, Device};
use gpu_alloc_ash::AshMemoryDevice;
use log::debug;

#[derive(Debug)]
pub struct AllocatedImage {
    pub image: vk::Image,
    pub view: vk::ImageView,
    allocation: Allocation,
    pub extent: vk::Extent3D,
    format: vk::Format,
    label: Option<String>,
}

impl AllocatedImage {
    pub fn new(
        device: &Device,
        allocator: &mut Allocator,
        extent: vk::Extent3D,
        format: vk::Format,
        image_usages: vk::ImageUsageFlags,
        alloc_usages: AllocUsage,
        image_aspect: vk::ImageAspectFlags,
        flags: vk::ImageCreateFlags,
        label: Option<String>,
    ) -> Self {
        let info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(extent)
            .mip_levels(1)
            .flags(flags)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(image_usages)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let image = unsafe { device.create_image(&info, None) }.unwrap();
        let reqs = unsafe { device.get_image_memory_requirements(image) };
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
                "Creating image '{}' ({:?}) of size {:?} and format {:?}",
                label.clone().unwrap_or_default(),
                image,
                extent,
                format
            );
        }
        unsafe { device.bind_image_memory(image, *allocation.memory(), allocation.offset()).unwrap() };

        let view_create_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .components(
                vk::ComponentMapping::default()
                    .r(vk::ComponentSwizzle::IDENTITY)
                    .g(vk::ComponentSwizzle::IDENTITY)
                    .b(vk::ComponentSwizzle::IDENTITY)
                    .a(vk::ComponentSwizzle::IDENTITY),
            )
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(image_aspect)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1),
            );
        let view = unsafe { device.create_image_view(&view_create_info, None).unwrap() };
        Self {
            image,
            view,
            allocation,
            extent,
            format,
            label,
        }
    }

    // https://i.imgflip.com/8l3uzz.jpg
    pub fn write<'a>(&'a self, data: &'a [u8], ctx: &mut SubmitContext) {
        let mut staging = AllocatedBuffer::new(
            &ctx.device,
            &mut ctx.allocator.borrow_mut(),
            vk::BufferUsageFlags::TRANSFER_SRC,
            AllocUsage::UploadToHost,
            data.len() as DeviceSize,
            Some(format!("Staging buffer for image '{}'", self.label.clone().unwrap_or_default())),
        );
        unsafe {
            let data_ptr = staging
                .allocation
                .map(AshMemoryDevice::wrap(&ctx.device), 0, staging.size as usize)
                .unwrap();
            std::ptr::copy_nonoverlapping(data.as_ptr(), data_ptr.as_ptr(), data.len());
        }
        transition_image(
            &ctx.device,
            ctx.cmd_buffer,
            self.image,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        );
        let copy_region = vk::BufferImageCopy::default()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(
                vk::ImageSubresourceLayers::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .mip_level(0)
                    .base_array_layer(0)
                    .layer_count(1),
            )
            .image_extent(self.extent);
        let copy_regions = [copy_region];
        unsafe {
            ctx.device.cmd_copy_buffer_to_image(
                ctx.cmd_buffer,
                staging.buffer,
                self.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &copy_regions,
            );
        }
        transition_image(
            &ctx.device,
            ctx.cmd_buffer,
            self.image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        );
        ctx.cleanup = Some(Box::from(|device: &Device, allocator: &mut Allocator| {
            staging.destroy(device, allocator);
        }))
    }

    pub(crate) fn destroy(self, device: &Device, allocator: &mut Allocator) {
        if LOG_ALLOCATIONS {
            debug!(
                "Destroying image '{}' ({:?}) of size {:?} and format {:?}",
                self.label.unwrap_or_default(),
                self.image,
                self.extent,
                self.format
            );
        }
        unsafe { device.destroy_image_view(self.view, None) };
        unsafe { device.destroy_image(self.image, None) };
        unsafe { allocator.dealloc(AshMemoryDevice::wrap(device), self.allocation) };
    }
}
