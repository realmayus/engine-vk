use crate::pipeline::PipelineBuilder;
use crate::util::{load_shader_module, DeletionQueue};
use ash::{vk, Device};
use bytemuck::{Pod, Zeroable};

use crate::asset::material::MaterialManager;
use crate::scene::billboard::Billboard;
use crate::scene::light::LightManager;
use glam::{Mat4, Vec2, Vec4, Vec4Swizzles};
use image::EncodableLayout;
use std::ffi::CStr;
use std::fs;

pub struct BillboardPipeline {
    viewport: vk::Viewport,
    scissor: vk::Rect2D,
    pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    window_size: (u32, u32),
}
#[repr(C)]
#[derive(Pod, Zeroable, Copy, Clone, Debug)]
struct PushConstants {
    transform: [[f32; 4]; 4],
    size: [f32; 2],
    uvs: [[f32; 2]; 4],
    scene_data: vk::DeviceAddress,
    material_buffer: vk::DeviceAddress,
    light_buffer: vk::DeviceAddress,
}

impl BillboardPipeline {
    pub fn new(
        device: &Device,
        window_size: (u32, u32),
        deletion_queue: &mut DeletionQueue,
        bindless_set_layout: vk::DescriptorSetLayout,
    ) -> Self {
        let vertex_shader = load_shader_module(device, fs::read("src/shaders/spirv/billboard.vert.spv").unwrap().as_bytes())
            .expect("Failed to load vertex shader module");
        let fragment_shader = load_shader_module(device, fs::read("src/shaders/spirv/unlit.frag.spv").unwrap().as_bytes())
            .expect("Failed to load fragment shader module");

        let push_constant_range = [vk::PushConstantRange::default()
            .offset(0)
            .size(size_of::<PushConstants>() as u32)
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)];
        let binding = [bindless_set_layout];
        let layout_create_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&binding)
            .push_constant_ranges(&push_constant_range);
        let layout = unsafe { device.create_pipeline_layout(&layout_create_info, None).unwrap() };
        let pipeline_builder = PipelineBuilder {
            layout: Some(layout),
            shader_stages: vec![
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::VERTEX)
                    .module(vertex_shader)
                    .name(CStr::from_bytes_with_nul(b"main\0").unwrap()),
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::FRAGMENT)
                    .module(fragment_shader)
                    .name(CStr::from_bytes_with_nul(b"main\0").unwrap()),
            ],
            input_assembly: vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST),
            ..Default::default()
        };

        let pipeline = pipeline_builder.build(device);

        unsafe {
            device.destroy_shader_module(vertex_shader, None);
            device.destroy_shader_module(fragment_shader, None);
        }

        deletion_queue.push(move |device, _allocator| unsafe {
            device.destroy_pipeline_layout(layout, None);
            device.destroy_pipeline(pipeline, None);
        });

        let viewport = vk::Viewport::default()
            .width(window_size.0 as f32)
            .height((window_size.1 as f32))
            .y(window_size.1 as f32)
            .max_depth(1.0);
        let scissor = vk::Rect2D::default().extent(vk::Extent2D {
            width: window_size.0,
            height: window_size.1,
        });

        Self {
            viewport,
            scissor,
            pipeline,
            layout,
            window_size,
        }
    }

    pub fn resize(&mut self, window_size: (u32, u32)) {
        self.window_size = window_size;
        self.viewport = vk::Viewport::default()
            .width(window_size.0 as f32)
            .height(window_size.1 as f32)
            .max_depth(1.0);
        self.scissor = vk::Rect2D::default().extent(vk::Extent2D {
            width: window_size.0,
            height: window_size.1,
        });
    }

    pub fn draw(
        &self,
        device: &Device,
        cmd: vk::CommandBuffer,
        billboards: &Vec<&Billboard>,
        target_view: vk::ImageView,
        depth_view: vk::ImageView,
        bindless_descriptor_set: vk::DescriptorSet,
        scene_data: vk::DeviceAddress,
        material_manager: &MaterialManager,
        light_manager: &LightManager,
    ) {
        let color_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(target_view)
            .image_layout(vk::ImageLayout::GENERAL);
        let color_attachments = [color_attachment];
        let depth_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(depth_view)
            .image_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::LOAD)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 },
            });
        let render_info = vk::RenderingInfo::default()
            .color_attachments(&color_attachments)
            .depth_attachment(&depth_attachment)
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D {
                    width: self.window_size.0,
                    height: self.window_size.1,
                },
            })
            .layer_count(1)
            .view_mask(0);
        unsafe {
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.layout,
                0,
                &[bindless_descriptor_set],
                &[],
            );
            device.cmd_begin_rendering(cmd, &render_info);
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
            device.cmd_set_viewport(cmd, 0, &[self.viewport]);
            device.cmd_set_scissor(cmd, 0, &[self.scissor]);

            for billboard in billboards {
                let push_constants = PushConstants {
                    transform: Mat4::from_translation(billboard.center.xyz()).to_cols_array_2d(),
                    size: billboard.size.to_array(),
                    uvs: billboard.uvs.map(|v| Vec2::from((v.x, v.y)).to_array()),
                    scene_data,
                    material_buffer: material_manager.get_material(billboard.material).unwrap().device_address(device),
                    light_buffer: light_manager.device_address(device),
                };

                device.cmd_push_constants(
                    cmd,
                    self.layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    bytemuck::cast_slice(&[push_constants]),
                );
                device.cmd_draw(cmd, 6, 1, 0, 0);
            }

            device.cmd_end_rendering(cmd);
        }
    }
}
