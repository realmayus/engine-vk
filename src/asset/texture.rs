use crate::resource::image::AllocatedImage;
use crate::resource::{update_set, AllocUsage, Allocator, DescriptorImageWriteInfo};
use crate::SubmitContext;
use ash::{vk, Device};
use log::info;
use std::mem;

pub type SamplerId = usize;
pub type TextureId = usize;

#[derive(Debug)]
pub struct Texture {
    pub id: TextureId,
    pub image: AllocatedImage,
    sampler: SamplerId, // rust doesn't like self-referential structs (samplers also live in TextureManager)
}

impl Texture {
    pub fn new(sampler: SamplerId, ctx: &mut SubmitContext, label: Option<String>, data: &[u8], extent: vk::Extent3D) -> Self {
        let img = AllocatedImage::new(
            &ctx.device,
            &mut ctx.allocator.borrow_mut(),
            extent,
            vk::Format::B8G8R8A8_SRGB,
            vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
            AllocUsage::GpuOnly,
            vk::ImageAspectFlags::COLOR,
            vk::ImageCreateFlags::empty(),
            label,
        );

        img.write(data, ctx);

        Self {
            image: img,
            id: 0,
            sampler,
        }
    }

    pub fn replace_image(&mut self, ctx: &mut SubmitContext, label: Option<String>, data: &[u8], extent: vk::Extent3D) {
        let img = AllocatedImage::new(
            &ctx.device,
            &mut ctx.allocator.borrow_mut(),
            extent,
            vk::Format::B8G8R8A8_SRGB,
            vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
            AllocUsage::GpuOnly,
            vk::ImageAspectFlags::COLOR,
            vk::ImageCreateFlags::empty(),
            label,
        );

        img.write(data, ctx);

        let old = mem::replace(&mut self.image, img);

        old.destroy(&ctx.device, &mut ctx.allocator.borrow_mut());
    }
}

pub struct TextureManager {
    textures: Vec<Option<Texture>>,
    samplers: Vec<vk::Sampler>,
    descriptor_set: vk::DescriptorSet,
}

impl TextureManager {
    pub const DEFAULT_SAMPLER_NEAREST: SamplerId = 0;
    pub const DEFAULT_SAMPLER_LINEAR: SamplerId = 1;
    pub const DEFAULT_TEXTURE_WHITE: TextureId = 0;
    pub const DEFAULT_TEXTURE_BLACK: TextureId = 1;
    pub const DEFAULT_TEXTURE_CHECKERBOARD: TextureId = 2;

    pub fn new(descriptor_set: vk::DescriptorSet, ctx: &mut SubmitContext) -> Self {
        let mut manager = Self {
            textures: vec![],
            samplers: vec![],
            descriptor_set,
        };

        let sampler_info = vk::SamplerCreateInfo::default()
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .address_mode_v(vk::SamplerAddressMode::REPEAT)
            .address_mode_w(vk::SamplerAddressMode::REPEAT)
            .mag_filter(vk::Filter::NEAREST)
            .min_filter(vk::Filter::NEAREST);
        let sampler_nearest = unsafe { ctx.device.create_sampler(&sampler_info, None).unwrap() };
        Self::add_sampler(&mut manager, sampler_nearest);

        let sampler_info = vk::SamplerCreateInfo::default()
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .address_mode_v(vk::SamplerAddressMode::REPEAT)
            .address_mode_w(vk::SamplerAddressMode::REPEAT)
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR);
        let sampler_linear = unsafe { ctx.device.create_sampler(&sampler_info, None).unwrap() };

        Self::add_sampler(&mut manager, sampler_linear);

        let white = [255u8, 255, 255, 255];
        let black = [0u8, 0, 0, 255];
        let magenta = [255u8, 0, 255, 255];
        let pixels: [[u8; 4]; 16 * 16] = core::array::from_fn(|i|
            // create a checkerboard pattern of white and magenta
            if (i / 16 + i % 16) % 2 == 0 {
                white
            } else {
                magenta
            }
        );
        let pixel_data = pixels.iter().flat_map(|p| p.iter().copied()).collect::<Vec<_>>();

        Self::add_texture(
            &mut manager,
            Texture::new(
                Self::DEFAULT_SAMPLER_NEAREST,
                ctx,
                Some("White".into()),
                &white,
                vk::Extent3D {
                    width: 1,
                    height: 1,
                    depth: 1,
                },
            ),
            &ctx.device,
            false,
        );
        let cleanup1 = ctx.cleanup.take().unwrap();
        Self::add_texture(
            &mut manager,
            Texture::new(
                Self::DEFAULT_SAMPLER_NEAREST,
                ctx,
                Some("Black".into()),
                &black,
                vk::Extent3D {
                    width: 1,
                    height: 1,
                    depth: 1,
                },
            ),
            &ctx.device,
            false,
        );
        let cleanup2 = ctx.cleanup.take().unwrap();
        Self::add_texture(
            &mut manager,
            Texture::new(
                Self::DEFAULT_SAMPLER_NEAREST,
                ctx,
                Some("Checkerboard".into()),
                &pixel_data,
                vk::Extent3D {
                    width: 16,
                    height: 16,
                    depth: 1,
                },
            ),
            &ctx.device,
            true,
        );
        let cleanup3 = ctx.cleanup.take().unwrap();

        ctx.cleanup = Some(Box::from(move |device: &Device, allocator: &mut Allocator| {
            cleanup1(device, allocator);
            cleanup2(device, allocator);
            cleanup3(device, allocator);
        }));

        manager
    }

    pub fn free(&mut self, to_free: TextureId, device: &Device, allocator: &mut Allocator) {
        let texture = self.textures[to_free].take().unwrap();
        texture.image.destroy(device, allocator);
        info!("Freed texture {:?}", to_free);
    }

    pub fn next_free_id(&self) -> TextureId {
        self.textures.iter().position(|t| t.is_none()).unwrap_or(self.textures.len())
    }

    pub fn descriptor_set(&self) -> vk::DescriptorSet {
        self.descriptor_set
    }

    pub fn add_texture(&mut self, mut texture: Texture, device: &Device, update_set: bool) -> TextureId {
        let id = self.next_free_id();
        texture.id = id;
        if id == self.textures.len() {
            self.textures.push(Some(texture));
        } else {
            self.textures[id] = Some(texture);
        }

        if update_set {
            self.update_set(device);
        }
        id
    }

    pub fn add_sampler(&mut self, sampler: vk::Sampler) {
        self.samplers.push(sampler);
    }

    pub fn update_set(&self, device: &Device) {
        update_set(
            device,
            self.descriptor_set,
            &self
                .textures
                .iter()
                .filter(|t| t.is_some())
                .map(|texture| DescriptorImageWriteInfo {
                    binding: 2,
                    array_index: texture.as_ref().unwrap().id as u32,
                    image_view: texture.as_ref().unwrap().image.view,
                    sampler: self.samplers[texture.as_ref().unwrap().sampler],
                    layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                })
                .collect::<Vec<_>>(),
            &[],
        );
    }

    pub fn texture_mut(&mut self, id: TextureId) -> &mut Texture {
        self.textures[id].as_mut().unwrap()
    }

    pub fn destroy(&mut self, device: &Device, allocator: &mut Allocator) {
        for texture in self.textures.drain(..).flatten() {
            texture.image.destroy(device, allocator);
        }
        for sampler in self.samplers.drain(..) {
            unsafe {
                device.destroy_sampler(sampler, None);
            }
        }
    }
}
