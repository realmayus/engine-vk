use crate::resource::image::AllocatedImage;
use crate::resource::immediate_submit::SubmitContext;
use crate::resource::{update_set, AllocUsage, Allocator, DescriptorImageWriteInfo};
use crate::util::transition_image;
use ash::{vk, Device};
use log::debug;
use std::mem;

pub type SamplerId = usize;
pub type TextureId = u32;

#[derive(Debug)]
pub struct Texture {
    pub id: TextureId,
    pub image: AllocatedImage,
    pub sampler: SamplerId,
    pub data: Vec<u8>,
    pub(crate) kind: TextureKind,
}
pub const TEXTURE_IMAGE_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;
impl Texture {
    pub fn new(
        sampler: SamplerId,
        format: vk::Format,
        ctx: &mut SubmitContext,
        label: Option<String>,
        extent: vk::Extent3D,
        kind: TextureKind,
    ) -> Self {
        let img = AllocatedImage::new(
            &ctx.device,
            &mut ctx.allocator.borrow_mut(),
            extent,
            format,
            vk::ImageUsageFlags::TRANSFER_DST
                | vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::SAMPLED
                | if kind == TextureKind::Depth {
                    vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
                } else {
                    vk::ImageUsageFlags::empty()
                },
            AllocUsage::GpuOnly,
            if kind == TextureKind::Depth {
                vk::ImageAspectFlags::DEPTH
            } else {
                vk::ImageAspectFlags::COLOR
            },
            vk::ImageCreateFlags::empty(),
            label.clone(),
        );
        Self {
            image: img,
            id: 0,
            sampler,
            data: vec![],
            kind,
        }
    }

    pub fn new_init(
        sampler: SamplerId,
        format: vk::Format,
        ctx: &mut SubmitContext,
        label: Option<String>,
        data: &[u8],
        extent: vk::Extent3D,
        kind: TextureKind,
    ) -> Self {
        let mut tex = Self::new(sampler, format, ctx, label, extent, kind);
        tex.image.write(data, ctx);
        tex.data = Vec::from(data);
        tex
    }

    /// Replaces the image data with the given data. Creates a new AllocatedImage and destroys the old one.
    pub fn replace_image(&mut self, ctx: &mut SubmitContext, label: Option<String>, data: &[u8], extent: vk::Extent3D) {
        let img = AllocatedImage::new(
            &ctx.device,
            &mut ctx.allocator.borrow_mut(),
            extent,
            TEXTURE_IMAGE_FORMAT,
            vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
            AllocUsage::GpuOnly,
            if self.kind == TextureKind::Depth {
                vk::ImageAspectFlags::DEPTH
            } else {
                vk::ImageAspectFlags::COLOR
            },
            vk::ImageCreateFlags::empty(),
            label,
        );

        img.write(data, ctx);

        let old = mem::replace(&mut self.image, img);

        old.destroy(&ctx.device, &mut ctx.allocator.borrow_mut());
    }

    /// Takes a texture and blits it onto the current one at the given position. Doesn't perform any allocations.
    pub fn patch(&mut self, ctx: SubmitContext, src: &Texture, pos: (i32, i32)) {
        ctx.immediate_submit(Box::new(|ctx: &mut SubmitContext| {
            transition_image(
                &ctx.device,
                ctx.cmd_buffer,
                src.image.image,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            );
            transition_image(
                &ctx.device,
                ctx.cmd_buffer,
                self.image.image,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            );
            let extent = vk::Extent3D {
                width: src.image.extent.width,
                height: src.image.extent.height,
                depth: 1,
            };
            let src_offset = vk::Offset3D { x: 0, y: 0, z: 0 };
            let dst_offset = vk::Offset3D { x: pos.0, y: pos.1, z: 0 };
            let src_subresource = vk::ImageSubresourceLayers::default()
                .aspect_mask(if self.kind == TextureKind::Depth {
                    vk::ImageAspectFlags::DEPTH
                } else {
                    vk::ImageAspectFlags::COLOR
                })
                .mip_level(0)
                .base_array_layer(0)
                .layer_count(1);
            let dst_subresource = vk::ImageSubresourceLayers::default()
                .aspect_mask(if self.kind == TextureKind::Depth {
                    vk::ImageAspectFlags::DEPTH
                } else {
                    vk::ImageAspectFlags::COLOR
                })
                .mip_level(0)
                .base_array_layer(0)
                .layer_count(1);
            let blit = vk::ImageBlit::default()
                .src_offsets([
                    src_offset,
                    vk::Offset3D {
                        x: extent.width as i32,
                        y: extent.height as i32,
                        z: 1,
                    },
                ])
                .dst_offsets([
                    dst_offset,
                    vk::Offset3D {
                        x: extent.width as i32 + pos.0,
                        y: extent.height as i32 + pos.1,
                        z: 1,
                    },
                ])
                .src_subresource(src_subresource)
                .dst_subresource(dst_subresource);
            unsafe {
                ctx.device.cmd_blit_image(
                    ctx.cmd_buffer,
                    src.image.image,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    self.image.image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[blit],
                    vk::Filter::NEAREST,
                );
            }
            transition_image(
                &ctx.device,
                ctx.cmd_buffer,
                src.image.image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );
            transition_image(
                &ctx.device,
                ctx.cmd_buffer,
                self.image.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );
        }));
    }
}

pub struct TextureManager {
    textures: Vec<Option<Texture>>,
    samplers: Vec<vk::Sampler>,
    descriptor_set: vk::DescriptorSet,
}
#[allow(dead_code)]
impl TextureManager {
    pub const DEFAULT_SAMPLER_NEAREST: SamplerId = 0;
    pub const DEFAULT_SAMPLER_LINEAR: SamplerId = 1;
    pub const DEFAULT_TEXTURE_WHITE: TextureId = 0;
    pub const DEFAULT_TEXTURE_BLACK: TextureId = 1;
    pub const DEFAULT_TEXTURE_CHECKERBOARD: TextureId = 2;
    pub const DEFAULT_TEXTURE_NORMAL: TextureId = 3;

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
        let normal = [128u8, 128, 255, 255];
        let pixels: [[u8; 4]; 16 * 16] = core::array::from_fn(|i|
            // create a checkerboard pattern of white and magenta
            if (i / 16 + i % 16) % 2 == 0 {
                white
            } else {
                magenta
            }
        );
        let pixel_data = pixels.iter().flat_map(|p| p.iter().copied()).collect::<Vec<_>>();

        ctx.nest(Box::new(|ctx| {
            Self::add_texture(
                &mut manager,
                Texture::new_init(
                    Self::DEFAULT_SAMPLER_NEAREST,
                    TEXTURE_IMAGE_FORMAT,
                    ctx,
                    Some("White".into()),
                    &white,
                    vk::Extent3D {
                        width: 1,
                        height: 1,
                        depth: 1,
                    },
                    TextureKind::Color,
                ),
                &ctx.device,
                false,
            );
        }));
        ctx.nest(Box::new(|ctx| {
            Self::add_texture(
                &mut manager,
                Texture::new_init(
                    Self::DEFAULT_SAMPLER_NEAREST,
                    TEXTURE_IMAGE_FORMAT,
                    ctx,
                    Some("Black".into()),
                    &black,
                    vk::Extent3D {
                        width: 1,
                        height: 1,
                        depth: 1,
                    },
                    TextureKind::Color,
                ),
                &ctx.device,
                false,
            );
        }));
        ctx.nest(Box::new(|ctx| {
            Self::add_texture(
                &mut manager,
                Texture::new_init(
                    Self::DEFAULT_SAMPLER_NEAREST,
                    TEXTURE_IMAGE_FORMAT,
                    ctx,
                    Some("Checkerboard".into()),
                    &pixel_data,
                    vk::Extent3D {
                        width: 16,
                        height: 16,
                        depth: 1,
                    },
                    TextureKind::Color,
                ),
                &ctx.device,
                false,
            );
        }));
        ctx.nest(Box::new(|ctx| {
            Self::add_texture(
                &mut manager,
                Texture::new_init(
                    Self::DEFAULT_SAMPLER_NEAREST,
                    TEXTURE_IMAGE_FORMAT,
                    ctx,
                    Some("Normal".into()),
                    &normal,
                    vk::Extent3D {
                        width: 1,
                        height: 1,
                        depth: 1,
                    },
                    TextureKind::Color,
                ),
                &ctx.device,
                true,
            );
        }));
        manager
    }

    pub fn free(&mut self, to_free: TextureId, device: &Device, allocator: &mut Allocator) {
        let texture = self.textures[to_free as usize].take().unwrap();
        let label = texture.image.label.clone().unwrap_or_default();
        texture.image.destroy(device, allocator);
        debug!("Freed texture {} ({:?}) ", label, to_free);
    }

    pub fn next_free_id(&self) -> TextureId {
        self.textures.iter().position(|t| t.is_none()).unwrap_or(self.textures.len()) as TextureId
    }

    pub fn descriptor_set(&self) -> vk::DescriptorSet {
        self.descriptor_set
    }

    pub fn iter_textures(&self) -> impl Iterator<Item = &Texture> {
        self.textures.iter().filter_map(|t| t.as_ref())
    }

    pub fn add_texture(&mut self, mut texture: Texture, device: &Device, update_set: bool) -> TextureId {
        let id = self.next_free_id();
        texture.id = id;
        if id as usize == self.textures.len() {
            self.textures.push(Some(texture));
        } else {
            self.textures[id as usize] = Some(texture);
        }

        if update_set {
            self.update_set(device);
        }
        id
    }

    pub fn get_texture(&self, id: TextureId) -> Option<&Texture> {
        self.textures[id as usize].as_ref()
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
                    array_index: texture.as_ref().unwrap().id,
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
        self.textures[id as usize].as_mut().unwrap()
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TextureKind {
    Color,
    ColorInternal, // don't show in UI
    Depth,         // depth is always internal
}
