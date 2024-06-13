use crate::pipeline::PipelineBuilder;

use crate::util::{load_shader_module, DeletionQueue};
use ash::{vk, Device};
use bytemuck::{Pod, Zeroable};
use std::ffi::CStr;

pub struct GridPipeline {
    viewport: vk::Viewport,
    scissor: vk::Rect2D,
    pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    window_size: (u32, u32),
}
#[repr(C)]
#[derive(Pod, Zeroable, Copy, Clone, Debug)]
struct PushConstants {
    scene_data: vk::DeviceAddress,
}

impl GridPipeline {
    pub fn new(device: &ash::Device, window_size: (u32, u32), deletion_queue: &mut DeletionQueue) -> Self {
        let vertex_shader =
            load_shader_module(device, include_bytes!("../shaders/spirv/grid.vert.spv")).expect("Failed to load vertex shader module");
        let fragment_shader =
            load_shader_module(device, include_bytes!("../shaders/spirv/grid.frag.spv")).expect("Failed to load fragment shader module");

        let push_constant_range = [vk::PushConstantRange::default()
            .offset(0)
            .size(std::mem::size_of::<PushConstants>() as u32)
            .stage_flags(vk::ShaderStageFlags::VERTEX)];
        let layout_create_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&[])
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
            color_blend_attachment: vk::PipelineColorBlendAttachmentState::default()
                .blend_enable(true)
                .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
                .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .src_alpha_blend_factor(vk::BlendFactor::SRC_ALPHA)
                .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .color_write_mask(vk::ColorComponentFlags::RGBA)
                .alpha_blend_op(vk::BlendOp::ADD)
                .color_blend_op(vk::BlendOp::ADD),
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
            .height(-(window_size.1 as f32))
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
    pub fn draw(&self, device: &Device, cmd: vk::CommandBuffer, target_view: vk::ImageView, scene_data: vk::DeviceAddress) {
        let color_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(target_view)
            .image_layout(vk::ImageLayout::GENERAL);
        let color_attachments = [color_attachment];
        let render_info = vk::RenderingInfo::default()
            .color_attachments(&color_attachments)
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
            device.cmd_begin_rendering(cmd, &render_info);
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipeline);

            device.cmd_set_viewport(cmd, 0, &[self.viewport]);
            device.cmd_set_scissor(cmd, 0, &[self.scissor]);
            let push_constants = PushConstants { scene_data };
            device.cmd_push_constants(
                cmd,
                self.layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                bytemuck::cast_slice(&[push_constants]),
            );
            device.cmd_draw(cmd, 6, 1, 0, 0);

            device.cmd_end_rendering(cmd);
        }
    }
}
