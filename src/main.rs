extern crate core;

mod asset;
mod camera;
mod commands;
mod gltf;
mod pipeline;
mod resource;
mod scene;
mod ui;
mod util;

use crate::resource::{AllocUsage, Allocator, DescriptorAllocator, PoolSizeRatio};
use crate::util::{device_discovery, DeletionQueue};
use ash::khr::swapchain;
use ash::vk::{DescriptorSet, DescriptorSetLayout};
use ash::{khr, vk, Device, Instance};
use resource::immediate_submit::SubmitContext;

use crate::asset::material::MaterialManager;
use crate::commands::{Command, CommandHandler};
use crate::gltf::GltfReader;
use crate::pipeline::billboard::BillboardPipeline;
use asset::texture::TextureManager;
use glam::{Mat4, Vec4};
use gpu_alloc::GpuAllocator;
use gpu_alloc_ash::device_properties;
use log::{debug, info};
use notify::Watcher;
use pipeline::GpuSceneData;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use resource::buffer::{AllocatedBuffer, WrappedBuffer};
use resource::image::AllocatedImage;
use std::cell::RefCell;
use std::error::Error;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc;
use util::FrameData;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

use crate::pipeline::egui::EguiPipeline;
use crate::pipeline::grid::GridPipeline;
use crate::pipeline::mesh::MeshPipeline;

use crate::scene::light::LightManager;
use crate::scene::world::World;
use crate::ui::Gui;

const FRAME_OVERLAP: usize = 2;

struct App {
    entry: ash::Entry,
    instance: Instance,
    surface: vk::SurfaceKHR,
    surface_fn: ash::khr::surface::Instance,
    physical_device: vk::PhysicalDevice,
    device: Rc<Device>,
    graphics_queue: (vk::Queue, u32),
    present_queue: (vk::Queue, u32),
    swapchain: (swapchain::Device, vk::SwapchainKHR),
    swapchain_images: Vec<vk::Image>,
    swapchain_views: Vec<vk::ImageView>,
    frames: [FrameData; FRAME_OVERLAP],
    current_frame: u32,
    window: winit::window::Window,
    window_size: (u32, u32),
    allocator: Rc<RefCell<Allocator>>,
    main_deletion_queue: DeletionQueue,
    draw_image: Option<AllocatedImage>,
    unorm_draw_image_view: vk::ImageView,
    bindless_descriptor_pool: vk::DescriptorPool,
    mesh_pipeline: MeshPipeline,
    egui_pipeline: EguiPipeline,
    grid_pipeline: GridPipeline,
    billboard_pipeline: BillboardPipeline,
    immediate_fence: vk::Fence,
    immediate_command_pool: vk::CommandPool,
    immediate_command_buffer: vk::CommandBuffer,
    depth_image: Option<AllocatedImage>,
    scene_data: WrappedBuffer<GpuSceneData>,
    texture_manager: Rc<RefCell<TextureManager>>,
    material_manager: Rc<RefCell<MaterialManager>>,
    light_manager: Rc<RefCell<LightManager>>,
    camera: camera::Camera,
    world: Rc<RefCell<World>>,
    settings: AppSettings,
    gui: Gui,
    cmd_sender: mpsc::Sender<Command>,
    bindless_set_layout: DescriptorSetLayout,
    pipeline_deletion_queue: DeletionQueue,
}

struct AppSettings {
    show_gui: bool,
    show_grid: bool,
}

pub const SWAPCHAIN_IMAGE_FORMAT: vk::Format = vk::Format::B8G8R8A8_SRGB;
pub const DRAW_IMAGE_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;
pub const DEPTH_FORMAT: vk::Format = vk::Format::D32_SFLOAT;
pub const API_VERSION: u32 = vk::make_api_version(0, 1, 3, 0);

impl App {
    fn new(event_loop: &EventLoop<()>, cmd_sender: mpsc::Sender<Command>) -> Result<Self, Box<dyn Error>> {
        let window_size = (1500, 850);
        let (instance, surface_khr, surface, entry, window) = unsafe {
            let entry = ash::Entry::load()?;
            let surface_extensions = ash_window::enumerate_required_extensions(event_loop.display_handle()?.as_raw())?;
            let app_desc = vk::ApplicationInfo::default().api_version(API_VERSION);
            let instance_desc = vk::InstanceCreateInfo::default()
                .application_info(&app_desc)
                .enabled_extension_names(surface_extensions);

            let instance = entry.create_instance(&instance_desc, None)?;

            let window = WindowBuilder::new()
                .with_inner_size(PhysicalSize::<u32>::from(window_size))
                .with_title("Vulkan Engine")
                .build(event_loop)?;

            // Create a surface from winit window.
            let surface_khr = ash_window::create_surface(
                &entry,
                &instance,
                window.display_handle()?.as_raw(),
                window.window_handle()?.as_raw(),
                None,
            )?;
            let surface = khr::surface::Instance::new(&entry, &instance);
            (instance, surface_khr, surface, entry, window)
        };
        let mut deletion_queue = DeletionQueue::default();
        let physical_device = device_discovery::pick_physical_device(&instance, &surface, surface_khr);
        let (device, graphics_queue, present_queue) =
            Self::create_logical_device_and_queue(&instance, &surface, surface_khr, physical_device);
        let config = gpu_alloc::Config::i_am_prototyping();
        let device_properties = unsafe { device_properties(&instance, API_VERSION, physical_device)? };
        let mut allocator = GpuAllocator::new(config, device_properties);

        let capabilities = unsafe { surface.get_physical_device_surface_capabilities(physical_device, surface_khr) }?;
        let ((swapchain, swapchain_khr), swapchain_images, swapchain_views, draw_image, unorm_draw_image_view, depth_image) =
            Self::create_swapchain(&instance, &device, surface_khr, capabilities, &mut allocator, window_size);
        let (immediate_command_pool, immediate_command_buffer, immediate_fence, frames) =
            Self::init_commands(graphics_queue.1, &device, &mut deletion_queue);
        let (bindless_descriptor_pool, bindless_descriptor_set, bindless_set_layout) = Self::init_bindless(&device);
        let grid_pipeline = GridPipeline::new(&device, window_size, &mut deletion_queue);
        let mut scene_data_buffer = WrappedBuffer {
            dirty: false,
            buffer: AllocatedBuffer::new(
                &device,
                &mut allocator,
                vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                AllocUsage::GpuOnly,
                std::mem::size_of::<GpuSceneData>() as vk::DeviceSize,
                Some("Scene Data Buffer".into()),
            ),
            data: GpuSceneData {
                view: Default::default(),
                proj: Default::default(),
                viewproj: Default::default(),
                unproj: Default::default(),
                ambient_color: Default::default(),
                camera_position: Default::default(),
                light_count: 0,
                padding: Default::default(),
            },
        };

        let device = Rc::new(device);
        let allocator = Rc::new(RefCell::new(allocator));
        SubmitContext::new(
            device.clone(),
            allocator.clone(),
            immediate_fence,
            immediate_command_buffer,
            graphics_queue.0,
        )
        .immediate_submit(Box::new(|ctx| scene_data_buffer.write(ctx)));

        let texture_manager = SubmitContext::new(
            device.clone(),
            allocator.clone(),
            immediate_fence,
            immediate_command_buffer,
            graphics_queue.0,
        )
        .immediate_submit(Box::new(|ctx| TextureManager::new(bindless_descriptor_set, ctx)));
        let material_manager = SubmitContext::new(
            device.clone(),
            allocator.clone(),
            immediate_fence,
            immediate_command_buffer,
            graphics_queue.0,
        )
        .immediate_submit(Box::new(|ctx| MaterialManager::new(ctx)));

        let light_manager = LightManager::new(&device, &mut allocator.borrow_mut());

        let camera = camera::Camera::new(window_size.0 as f32, window_size.1 as f32);

        let mut pipeline_deletion_queue = DeletionQueue::default();

        let mesh_pipeline = MeshPipeline::new(&device, window_size, &mut pipeline_deletion_queue, bindless_set_layout);
        let egui_pipeline = EguiPipeline::new(
            &device,
            window_size,
            &mut pipeline_deletion_queue,
            bindless_set_layout,
            &window,
            SubmitContext::new(
                device.clone(),
                allocator.clone(),
                immediate_fence,
                immediate_command_buffer,
                graphics_queue.0,
            ),
        );
        let billboard_pipeline = BillboardPipeline::new(&device, window_size, &mut pipeline_deletion_queue, bindless_set_layout);

        info!("Init done.");

        Ok(App {
            entry,
            instance,
            surface: surface_khr,
            surface_fn: surface,
            physical_device,
            device,
            graphics_queue,
            present_queue,
            swapchain: (swapchain, swapchain_khr),
            swapchain_images,
            swapchain_views,
            frames,
            current_frame: 0,
            window,
            window_size,
            allocator,
            pipeline_deletion_queue,
            main_deletion_queue: deletion_queue,
            draw_image: Some(draw_image), // must be present at all times, Option<_> because we need ownership when destroying
            unorm_draw_image_view,
            depth_image: Some(depth_image),
            bindless_set_layout,
            bindless_descriptor_pool,
            mesh_pipeline,
            egui_pipeline,
            grid_pipeline,
            billboard_pipeline,
            immediate_command_pool,
            immediate_command_buffer,
            immediate_fence,
            scene_data: scene_data_buffer,
            texture_manager: Rc::new(RefCell::new(texture_manager)),
            material_manager: Rc::new(RefCell::new(material_manager)),
            light_manager: Rc::new(RefCell::new(light_manager)),
            camera,
            settings: AppSettings {
                show_gui: true,
                show_grid: true,
            },
            gui: Gui::new(cmd_sender.clone()),
            world: Rc::new(RefCell::new(World::default())),
            cmd_sender,
        })
    }

    fn recreate_pipelines(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
        }
        self.pipeline_deletion_queue.flush(&self.device, &mut self.allocator.borrow_mut());
        self.egui_pipeline.destroy(&self.device, &mut self.allocator.borrow_mut());

        self.mesh_pipeline = MeshPipeline::new(
            &self.device,
            self.window_size,
            &mut self.pipeline_deletion_queue,
            self.bindless_set_layout,
        );

        self.egui_pipeline = EguiPipeline::new(
            &self.device,
            self.window_size,
            &mut self.pipeline_deletion_queue,
            self.bindless_set_layout,
            &self.window,
            SubmitContext::new(
                self.device.clone(),
                self.allocator.clone(),
                self.immediate_fence,
                self.immediate_command_buffer,
                self.graphics_queue.0,
            ),
        );

        self.billboard_pipeline = BillboardPipeline::new(
            &self.device,
            self.window_size,
            &mut self.pipeline_deletion_queue,
            self.bindless_set_layout,
        );
        self.resize(self.window_size);
    }

    /// Pick the first physical device that supports graphics and presentation queue families.
    fn create_logical_device_and_queue(
        instance: &Instance,
        surface: &khr::surface::Instance,
        surface_khr: vk::SurfaceKHR,
        device: vk::PhysicalDevice,
    ) -> (Device, (vk::Queue, u32), (vk::Queue, u32)) {
        let (graphics_family_index, present_family_index) = device_discovery::find_queue_families(instance, surface, surface_khr, device);
        let graphics_family_index = graphics_family_index.unwrap();
        let present_family_index = present_family_index.unwrap();

        // Vulkan specs does not allow passing an array containing duplicated family indices.
        // And since the family for graphics and presentation could be the same we need to
        // deduplicate it.
        let mut indices = vec![graphics_family_index, present_family_index];
        indices.dedup();

        // Now we build an array of `DeviceQueueCreateInfo`.
        // One for each different family index.
        let queue_priorities = [1.0f32];
        let queue_create_infos = indices
            .iter()
            .map(|index| {
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(*index)
                    .queue_priorities(&queue_priorities)
            })
            .collect::<Vec<_>>();

        let device_features = vk::PhysicalDeviceFeatures::default().geometry_shader(true);
        let mut vk12_features = vk::PhysicalDeviceVulkan12Features::default()
            .buffer_device_address(true)
            .descriptor_indexing(true)
            .runtime_descriptor_array(true)
            .descriptor_binding_partially_bound(true)
            .shader_storage_buffer_array_non_uniform_indexing(true)
            .shader_storage_image_array_non_uniform_indexing(true)
            .shader_sampled_image_array_non_uniform_indexing(true)
            .descriptor_binding_storage_buffer_update_after_bind(true)
            .descriptor_binding_storage_image_update_after_bind(true)
            .descriptor_binding_sampled_image_update_after_bind(true);
        let mut vk13_features = vk::PhysicalDeviceVulkan13Features::default()
            .synchronization2(true)
            .dynamic_rendering(true);

        let binding = [khr::swapchain::NAME.as_ptr()];
        let device_create_info_builder = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&binding)
            .enabled_features(&device_features)
            .push_next(&mut vk12_features)
            .push_next(&mut vk13_features);

        // Build device and queues
        let device = unsafe {
            instance
                .create_device(device, &device_create_info_builder, None)
                .expect("Failed to create logical device.")
        };
        let graphics_queue = unsafe { device.get_device_queue(graphics_family_index, 0) };
        let present_queue = unsafe { device.get_device_queue(present_family_index, 0) };

        (
            device,
            (graphics_queue, graphics_family_index),
            (present_queue, present_family_index),
        )
    }

    fn create_swapchain(
        instance: &Instance,
        device: &Device,
        surface_khr: vk::SurfaceKHR,
        capabilities: vk::SurfaceCapabilitiesKHR,
        allocator: &mut Allocator,
        window_size: (u32, u32),
    ) -> (
        (khr::swapchain::Device, vk::SwapchainKHR),
        Vec<vk::Image>,
        Vec<vk::ImageView>,
        AllocatedImage,
        vk::ImageView,
        AllocatedImage,
    ) {
        let create_info = vk::SwapchainCreateInfoKHR {
            surface: surface_khr,
            image_format: SWAPCHAIN_IMAGE_FORMAT,
            present_mode: vk::PresentModeKHR::FIFO, // hard vsync
            image_extent: vk::Extent2D {
                width: window_size.0,
                height: window_size.1,
            },
            image_usage: vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::COLOR_ATTACHMENT,
            pre_transform: vk::SurfaceTransformFlagsKHR::IDENTITY,
            composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
            image_array_layers: 1,
            min_image_count: capabilities.min_image_count,

            ..Default::default()
        };
        let swapchain = swapchain::Device::new(instance, device);
        let swapchain_khr = unsafe { swapchain.create_swapchain(&create_info, None).unwrap() };

        let images = unsafe { swapchain.get_swapchain_images(swapchain_khr).unwrap() };
        debug!("Swapchain images: {:?}", images);
        let image_views = images
            .iter()
            .map(|image| {
                let create_info = vk::ImageViewCreateInfo::default()
                    .image(*image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(SWAPCHAIN_IMAGE_FORMAT)
                    .components(vk::ComponentMapping {
                        r: vk::ComponentSwizzle::IDENTITY,
                        g: vk::ComponentSwizzle::IDENTITY,
                        b: vk::ComponentSwizzle::IDENTITY,
                        a: vk::ComponentSwizzle::IDENTITY,
                    })
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    });
                unsafe { device.create_image_view(&create_info, None).unwrap() }
            })
            .collect::<Vec<_>>();

        let draw_image = AllocatedImage::new(
            device,
            allocator,
            vk::Extent3D {
                width: window_size.0,
                height: window_size.1,
                depth: 1,
            },
            DRAW_IMAGE_FORMAT,
            vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::COLOR_ATTACHMENT,
            AllocUsage::GpuOnly,
            vk::ImageAspectFlags::COLOR,
            vk::ImageCreateFlags::MUTABLE_FORMAT,
            Some("Draw Image".into()),
        );

        let unorm_draw_image_view = unsafe {
            device
                .create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(draw_image.image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(vk::Format::R8G8B8A8_UNORM)
                        .components(vk::ComponentMapping {
                            r: vk::ComponentSwizzle::IDENTITY,
                            g: vk::ComponentSwizzle::IDENTITY,
                            b: vk::ComponentSwizzle::IDENTITY,
                            a: vk::ComponentSwizzle::IDENTITY,
                        })
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        }),
                    None,
                )
                .unwrap()
        };

        let depth_image = AllocatedImage::new(
            device,
            allocator,
            vk::Extent3D {
                width: window_size.0,
                height: window_size.1,
                depth: 1,
            },
            DEPTH_FORMAT,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            AllocUsage::GpuOnly,
            vk::ImageAspectFlags::DEPTH,
            vk::ImageCreateFlags::empty(),
            Some("Depth Image".into()),
        );

        (
            (swapchain, swapchain_khr),
            images,
            image_views,
            draw_image,
            unorm_draw_image_view,
            depth_image,
        )
    }

    fn init_commands(
        queue_family_index: u32,
        device: &Device,
        deletion_queue: &mut DeletionQueue,
    ) -> (vk::CommandPool, vk::CommandBuffer, vk::Fence, [FrameData; FRAME_OVERLAP]) {
        let command_pool_create_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER) // we want to be able to reset individual command buffers, not the entire pool at once
            .queue_family_index(queue_family_index);
        let fence_create_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
        let semaphore_create_info = vk::SemaphoreCreateInfo::default();

        let immediate_command_pool = unsafe { device.create_command_pool(&command_pool_create_info, None).unwrap() };
        let immediate_alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(immediate_command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let immediate_command_buffer = unsafe { device.allocate_command_buffers(&immediate_alloc_info).unwrap()[0] };
        let immediate_fence = unsafe { device.create_fence(&fence_create_info, None).unwrap() };
        deletion_queue.push(move |device, _allocator| unsafe {
            device.destroy_command_pool(immediate_command_pool, None);
            device.destroy_fence(immediate_fence, None);
        });
        (
            immediate_command_pool,
            immediate_command_buffer,
            immediate_fence,
            core::array::from_fn(|_| {
                let command_pool = unsafe { device.create_command_pool(&command_pool_create_info, None).unwrap() };
                let command_buffer = unsafe {
                    device
                        .allocate_command_buffers(
                            &vk::CommandBufferAllocateInfo::default()
                                .command_pool(command_pool)
                                .level(vk::CommandBufferLevel::PRIMARY)
                                .command_buffer_count(1),
                        )
                        .unwrap()[0]
                };
                let render_fence = unsafe { device.create_fence(&fence_create_info, None).unwrap() };
                let swapchain_semaphore = unsafe { device.create_semaphore(&semaphore_create_info, None).unwrap() };
                let render_semaphore = unsafe { device.create_semaphore(&semaphore_create_info, None).unwrap() };

                FrameData {
                    command_pool,
                    command_buffer,
                    swapchain_semaphore,
                    render_semaphore,
                    render_fence,
                    deletion_queue: DeletionQueue::default(),
                    descriptor_allocator: DescriptorAllocator::new(
                        device,
                        1000,
                        &[
                            PoolSizeRatio {
                                descriptor_type: vk::DescriptorType::STORAGE_IMAGE,
                                ratio: 3.0,
                            },
                            PoolSizeRatio {
                                descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
                                ratio: 3.0,
                            },
                            PoolSizeRatio {
                                descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                                ratio: 3.0,
                            },
                            PoolSizeRatio {
                                descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                                ratio: 4.0,
                            },
                        ],
                    ),
                    stale_buffers: Vec::new(),
                }
            }),
        )
    }
    pub const STORAGE_BUFFER_BINDING: u32 = 0;
    pub const STORAGE_IMAGE_BINDING: u32 = 1;
    pub const TEXTURE_BINDING: u32 = 2;
    fn init_bindless(device: &Device) -> (vk::DescriptorPool, DescriptorSet, DescriptorSetLayout) {
        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 65536,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_IMAGE,
                descriptor_count: 65536,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 65536,
            },
        ];
        let descriptor_pool = unsafe {
            device
                .create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .pool_sizes(&pool_sizes)
                        .max_sets(1)
                        .flags(vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND),
                    None,
                )
                .unwrap()
        };
        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(Self::STORAGE_BUFFER_BINDING)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(65536)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(Self::STORAGE_IMAGE_BINDING)
                .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                .descriptor_count(65536)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(Self::TEXTURE_BINDING)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(65536)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];

        let set_layout = unsafe {
            device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default()
                        .bindings(&bindings)
                        .flags(vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL)
                        .push_next(&mut vk::DescriptorSetLayoutBindingFlagsCreateInfo::default().binding_flags(&[
                            vk::DescriptorBindingFlags::PARTIALLY_BOUND | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
                            vk::DescriptorBindingFlags::PARTIALLY_BOUND | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
                            vk::DescriptorBindingFlags::PARTIALLY_BOUND | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND,
                        ])),
                    None,
                )
                .unwrap()
        };

        (
            descriptor_pool,
            unsafe {
                device
                    .allocate_descriptor_sets(
                        &vk::DescriptorSetAllocateInfo::default()
                            .descriptor_pool(descriptor_pool)
                            .set_layouts(&[set_layout]),
                    )
                    .unwrap()[0]
            },
            set_layout,
        )
    }

    unsafe fn destroy_swapchain(&mut self) {
        self.swapchain.0.destroy_swapchain(self.swapchain.1, None);
        for view in self.swapchain_views.drain(..) {
            self.device.destroy_image_view(view, None);
        }
    }

    fn resize(&mut self, size: (u32, u32)) {
        debug!("Resizing to {:?}", size);
        self.resize_swapchain(size);
        self.mesh_pipeline.resize(size);
        self.billboard_pipeline.resize(size);
        self.egui_pipeline.resize(size);
        self.grid_pipeline.resize(size);
        self.camera.resize(size.0 as f32, size.1 as f32);
    }
    fn resize_swapchain(&mut self, size: (u32, u32)) {
        unsafe {
            self.device.device_wait_idle().unwrap();
            self.destroy_swapchain();
            self.draw_image
                .take()
                .unwrap()
                .destroy(&self.device, &mut self.allocator.borrow_mut());
            self.depth_image
                .take()
                .unwrap()
                .destroy(&self.device, &mut self.allocator.borrow_mut());
            self.device.destroy_image_view(self.unorm_draw_image_view, None);
        }
        self.window_size = size;
        let capabilities = unsafe {
            self.surface_fn
                .get_physical_device_surface_capabilities(self.physical_device, self.surface)
                .unwrap()
        };
        let (swapchain, swapchain_images, swapchain_views, draw_image, unorm_draw_image_view, depth_image) = Self::create_swapchain(
            &self.instance,
            &self.device,
            self.surface,
            capabilities,
            &mut self.allocator.borrow_mut(),
            self.window_size,
        );
        self.swapchain = swapchain;
        self.swapchain_images = swapchain_images;
        self.swapchain_views = swapchain_views;
        self.draw_image = Some(draw_image);
        self.depth_image = Some(depth_image);
        self.unorm_draw_image_view = unorm_draw_image_view;
    }

    fn current_frame(&self) -> &FrameData {
        &self.frames[(self.current_frame % FRAME_OVERLAP as u32) as usize]
    }

    fn draw(&mut self) {
        unsafe {
            // wait until GPU has finished rendering the last frame, with a timeout of 1
            self.device
                .wait_for_fences(&[self.current_frame().render_fence], true, 1000000000)
                .unwrap();
            let device = self.device.clone();
            frame!(self).deletion_queue.flush(&device, &mut self.allocator.borrow_mut());
            frame!(self).descriptor_allocator.clear_pools(&device);
            for buffer in frame!(self).stale_buffers.drain(..) {
                buffer.destroy(&device, &mut self.allocator.borrow_mut());
            }
            self.device.reset_fences(&[self.current_frame().render_fence]).unwrap();

            // acquire the next image
            let (image_index, _) = self
                .swapchain
                .0
                .acquire_next_image(
                    self.swapchain.1,
                    1000000000,
                    self.current_frame().swapchain_semaphore,
                    vk::Fence::null(),
                )
                .unwrap();
            let cmd_buffer = self.current_frame().command_buffer;
            self.device
                .reset_command_buffer(cmd_buffer, vk::CommandBufferResetFlags::empty())
                .unwrap();

            //begin command buffer recording
            let begin_info = vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            self.device.begin_command_buffer(cmd_buffer, &begin_info).unwrap();

            // transition draw image into writable mode before rendering. undefined = we don't care, we're fine with the GPU destroying the image. general = general purpose layout which allows reading and writing from the image.
            util::transition_image(
                &self.device,
                cmd_buffer,
                self.draw_image.as_ref().unwrap().image,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::GENERAL,
            );

            self.draw_background(cmd_buffer);

            util::transition_image(
                &self.device,
                cmd_buffer,
                self.draw_image.as_ref().unwrap().image,
                vk::ImageLayout::GENERAL,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            );
            util::transition_image(
                &self.device,
                cmd_buffer,
                self.depth_image.as_ref().unwrap().image,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
            );

            self.mesh_pipeline.draw(
                &self.device,
                cmd_buffer,
                &self.world.borrow().get_meshes(),
                self.draw_image.as_ref().unwrap().view,
                self.depth_image.as_ref().unwrap().view,
                self.texture_manager.borrow().descriptor_set(),
                self.scene_data.buffer.device_address(&self.device),
                &self.material_manager.borrow(),
                &self.light_manager.borrow(),
            );
            {
                let world = self.world.borrow();
                let billboards = world.get_billboards();
                self.billboard_pipeline.draw(
                    &self.device,
                    cmd_buffer,
                    &billboards,
                    self.draw_image.as_ref().unwrap().view,
                    self.depth_image.as_ref().unwrap().view,
                    self.texture_manager.borrow().descriptor_set(),
                    self.scene_data.buffer.device_address(&self.device),
                    &self.material_manager.borrow(),
                    &self.light_manager.borrow(),
                );
            }

            if self.settings.show_grid {
                self.grid_pipeline.draw(
                    &self.device,
                    cmd_buffer,
                    self.draw_image.as_ref().unwrap().view,
                    self.scene_data.buffer.device_address(&self.device),
                );
            }

            if self.settings.show_gui {
                let ctx = SubmitContext::from_app(self);
                self.egui_pipeline.begin_frame(&self.window);

                self.gui.draw(
                    self.egui_pipeline.context().clone(),
                    &mut self.settings,
                    &mut self.camera,
                    self.world.clone(),
                    self.texture_manager.clone(),
                    self.material_manager.clone(),
                    self.light_manager.clone(),
                    ctx.clone(),
                );

                let output = self.egui_pipeline.end_frame(
                    &self.window,
                    &mut self.texture_manager.borrow_mut(),
                    &self.device,
                    &mut self.allocator.borrow_mut(),
                );
                let meshes = self
                    .egui_pipeline
                    .context()
                    .tessellate(output.shapes, self.window.scale_factor() as f32);
                let descriptor_set = self.texture_manager.borrow().descriptor_set();
                self.egui_pipeline.draw(
                    &self.device,
                    cmd_buffer,
                    self.unorm_draw_image_view,
                    descriptor_set,
                    output.textures_delta,
                    meshes,
                    &mut self.texture_manager.borrow_mut(),
                    ctx,
                    (self.current_frame % FRAME_OVERLAP as u32) as usize,
                );
            }

            // prepare copying of the draw image to the swapchain image
            util::transition_image(
                &self.device,
                cmd_buffer,
                self.draw_image.as_ref().unwrap().image,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            );
            util::transition_image(
                &self.device,
                cmd_buffer,
                self.swapchain_images[image_index as usize],
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            );
            // copy the draw image to the swapchain image
            util::copy_image_to_image(
                &self.device,
                cmd_buffer,
                self.draw_image.as_ref().unwrap().image,
                self.swapchain_images[image_index as usize],
                vk::Extent2D {
                    width: self.draw_image.as_ref().unwrap().extent.width,
                    height: self.draw_image.as_ref().unwrap().extent.height,
                },
                vk::Extent2D {
                    width: self.window_size.0,
                    height: self.window_size.1,
                },
            );
            // transition the swapchain image to present mode
            util::transition_image(
                &self.device,
                cmd_buffer,
                self.swapchain_images[image_index as usize],
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::PRESENT_SRC_KHR,
            );

            self.device.end_command_buffer(cmd_buffer).unwrap();

            // submit command buffer to queue
            let cmd_buffer_submit_info = vk::CommandBufferSubmitInfo::default().command_buffer(cmd_buffer);
            // we want to wait on the swapchain semaphore, as that signals when the swapchain image is available for rendering
            let wait_info = vk::SemaphoreSubmitInfo::default()
                .semaphore(self.current_frame().swapchain_semaphore)
                .stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT);
            // we want to signal the render semaphore, as that signals when the rendering is done
            let signal_info = vk::SemaphoreSubmitInfo::default()
                .semaphore(self.current_frame().render_semaphore)
                .stage_mask(vk::PipelineStageFlags2::ALL_GRAPHICS);
            let command_buffer_infos = [cmd_buffer_submit_info];
            let wait_infos = [wait_info];
            let signal_semaphore_infos = [signal_info];
            let submit = vk::SubmitInfo2::default()
                .command_buffer_infos(&command_buffer_infos)
                .wait_semaphore_infos(&wait_infos)
                .signal_semaphore_infos(&signal_semaphore_infos);
            self.device
                .queue_submit2(self.graphics_queue.0, &[submit], self.current_frame().render_fence)
                .unwrap();

            // present the image
            let swapchains = [self.swapchain.1];
            let indices = [image_index];
            let semaphores = [self.current_frame().render_semaphore];
            let present_info = vk::PresentInfoKHR::default()
                .swapchains(&swapchains)
                .image_indices(&indices)
                .wait_semaphores(&semaphores);
            self.swapchain.0.queue_present(self.present_queue.0, &present_info).unwrap();
        }
        self.current_frame = self.current_frame.wrapping_add(1);
    }

    fn draw_background(&self, cmd: vk::CommandBuffer) {
        let clear_range = vk::ImageSubresourceRange::default()
            .level_count(vk::REMAINING_MIP_LEVELS)
            .layer_count(vk::REMAINING_ARRAY_LAYERS)
            .aspect_mask(vk::ImageAspectFlags::COLOR);

        unsafe {
            self.device.cmd_clear_color_image(
                cmd,
                self.draw_image.as_ref().unwrap().image,
                vk::ImageLayout::GENERAL,
                &vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
                &[clear_range],
            );
        }
    }

    fn input_window(&mut self, event: &winit::event::WindowEvent) {
        if !self.egui_pipeline.input(&self.window, event) {
            match event {
                winit::event::WindowEvent::MouseInput { button, state, .. } => {
                    if *button == winit::event::MouseButton::Left && *state == winit::event::ElementState::Pressed {
                        self.camera.on_mouse_drag(true);
                    } else if *button == winit::event::MouseButton::Left && *state == winit::event::ElementState::Released {
                        self.camera.on_mouse_drag(false);
                    }
                }
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            repeat: false,
                            state: ElementState::Pressed,
                            physical_key: PhysicalKey::Code(key_code),
                            ..
                        },
                    ..
                } => {
                    if *key_code == KeyCode::F10 {
                        self.settings.show_gui = !self.settings.show_gui;
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    self.camera.on_mouse_scroll(*delta);
                }
                _ => {}
            }
        }
    }

    fn input_device(&mut self, event: &winit::event::DeviceEvent) {
        match event {
            winit::event::DeviceEvent::MouseMotion { delta } => {
                self.camera.on_mouse_move((delta.0 as f32, delta.1 as f32));
            }
            _ => {}
        }
    }

    fn update(&mut self) {
        if self.camera.dirty {
            self.camera.dirty = false;
            let view = self.camera.view();
            let proj = self.camera.proj();
            let viewproj = proj * view;
            self.scene_data.data.view = view.to_cols_array_2d();
            self.scene_data.data.proj = proj.to_cols_array_2d();
            self.scene_data.data.unproj = (view.inverse() * self.camera.proj().inverse()).to_cols_array_2d();
            self.scene_data.data.viewproj = viewproj.to_cols_array_2d();
            self.scene_data.dirty = true;
        }
        if self.light_manager.borrow().count_dirty {
            self.light_manager.borrow_mut().count_dirty = false;
            self.scene_data.data.light_count = self.light_manager.borrow().count() as u32;
            self.scene_data.dirty = true;
        }
        if self.scene_data.dirty {
            SubmitContext::from_app(self).immediate_submit(Box::new(|ctx| self.scene_data.write(ctx)));
            self.scene_data.dirty = false;
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
            self.main_deletion_queue.flush(&self.device, &mut self.allocator.borrow_mut());
            self.pipeline_deletion_queue.flush(&self.device, &mut self.allocator.borrow_mut());
            self.device.destroy_descriptor_pool(self.bindless_descriptor_pool, None);
            self.world.borrow_mut().destroy(&self.device, &mut self.allocator.borrow_mut());
            self.device.destroy_descriptor_set_layout(self.bindless_set_layout, None);

            self.device.destroy_image_view(self.unorm_draw_image_view, None);
            self.draw_image
                .take()
                .unwrap()
                .destroy(&self.device, &mut self.allocator.borrow_mut());
            self.depth_image
                .take()
                .unwrap()
                .destroy(&self.device, &mut self.allocator.borrow_mut());
            self.texture_manager
                .borrow_mut()
                .destroy(&self.device, &mut self.allocator.borrow_mut());
            for frame in self.frames.iter_mut() {
                frame.deletion_queue.flush(&self.device, &mut self.allocator.borrow_mut()); // take care of frames that were prepared but not reached in the render loop yet
                for buffer in frame.stale_buffers.drain(..) {
                    buffer.destroy(&self.device, &mut self.allocator.borrow_mut());
                }

                self.device.destroy_command_pool(frame.command_pool, None);
                self.device.destroy_fence(frame.render_fence, None);
                self.device.destroy_semaphore(frame.swapchain_semaphore, None);
                self.device.destroy_semaphore(frame.render_semaphore, None);
                frame.descriptor_allocator.destroy_pools(&self.device);
            }
            self.egui_pipeline.destroy(&self.device, &mut self.allocator.borrow_mut());
            self.destroy_swapchain();
            self.device.destroy_device(None);
            self.surface_fn.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let event_loop = EventLoop::new();
    let cmd_channel = mpsc::channel();
    let mut app = App::new(event_loop.as_ref().unwrap(), cmd_channel.0)?;
    let cmd_handler = CommandHandler::new(cmd_channel.1);
    let ctx = SubmitContext::from_app(&app);
    let mut gltf_loader = GltfReader::new(
        app.world.clone(),
        app.texture_manager.clone(),
        app.material_manager.clone(),
        app.light_manager.clone(),
    );
    gltf_loader.load(Path::new("assets/cube.glb"), ctx);

    // todo restrict to "watch" feature
    let (watch_tx, watch_rx) = std::sync::mpsc::channel();
    let mut watcher = notify::RecommendedWatcher::new(watch_tx, notify::Config::default()).unwrap();
    watcher
        .watch(
            std::env::current_dir().unwrap().join("src").join("shaders").join("spirv").as_path(),
            notify::RecursiveMode::Recursive,
        )
        .unwrap();

    Ok(event_loop.unwrap().run(move |event, target| match event {
        Event::WindowEvent {
            event: WindowEvent::RedrawRequested,
            ..
        } => {
            if watch_rx.try_recv().is_ok() {
                app.recreate_pipelines();
                info!("Shader files changed - recreated pipelines and reloaded shaders.");
            }
            app.update();
            app.draw();
            cmd_handler.handle_command(&mut app);
            app.window.request_redraw();
        }
        Event::WindowEvent {
            event: WindowEvent::Resized(size),
            ..
        } => {
            app.resize((size.width, size.height));
        }
        Event::WindowEvent {
            event:
                WindowEvent::CloseRequested
                | WindowEvent::KeyboardInput {
                    event:
                        winit::event::KeyEvent {
                            logical_key: winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape),
                            ..
                        },
                    ..
                },
            window_id: _,
        } => {
            target.exit();
        }

        Event::WindowEvent { event, .. } => {
            app.input_window(&event);
        }
        Event::DeviceEvent { event, .. } => {
            app.input_device(&event);
        }
        _ => {}
    })?)
}
