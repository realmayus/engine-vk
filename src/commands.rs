use crate::asset::texture::{Texture, TextureManager};
use crate::resource::immediate_submit::SubmitContext;
use crate::scene::model::ModelId;
use crate::App;
use ash::vk;
use image::{EncodableLayout, GenericImageView, ImageReader};
use log::info;
use std::path::PathBuf;
use std::sync::mpsc;

pub enum Command {
    LoadScene(PathBuf),
    ImportModel(PathBuf),
    DeleteModel(ModelId),
    ImportTexture(PathBuf),
    ReloadShaders,
}

pub struct CommandHandler {
    receiver: mpsc::Receiver<Command>,
}

impl CommandHandler {
    pub fn new(receiver: mpsc::Receiver<Command>) -> Self {
        Self { receiver }
    }
    pub fn handle_command(&self, app: &mut App) {
        while let Ok(command) = self.receiver.try_recv() {
            match command {
                Command::LoadScene(path) => {
                    unsafe {
                        app.device.device_wait_idle().unwrap();
                        for frame in app.frames.as_mut_slice() {
                            app.device
                                .reset_command_buffer(frame.command_buffer, vk::CommandBufferResetFlags::empty())
                                .unwrap();
                        }
                    }
                    app.world.borrow_mut().clear(&app.device, &mut app.allocator.borrow_mut());
                    let mut reader = crate::gltf::GltfReader::new(
                        app.world.clone(),
                        app.texture_manager.clone(),
                        app.material_manager.clone(),
                        app.light_manager.clone(),
                    );
                    let ctx = SubmitContext::from_app(app);
                    reader.load(&path, ctx);
                }
                Command::ImportModel(path) => {
                    let mut reader = crate::gltf::GltfReader::new(
                        app.world.clone(),
                        app.texture_manager.clone(),
                        app.material_manager.clone(),
                        app.light_manager.clone(),
                    );
                    let ctx = SubmitContext::from_app(app);
                    reader.load(&path, ctx);
                }
                Command::DeleteModel(id) => {
                    println!("Delete model: {:?}", id);
                    unsafe {
                        app.device.device_wait_idle().unwrap();
                        for frame in app.frames.as_mut_slice() {
                            app.device
                                .reset_command_buffer(frame.command_buffer, vk::CommandBufferResetFlags::empty())
                                .unwrap();
                        }
                    }
                    app.world.borrow_mut().models.remove(&id);
                }
                Command::ImportTexture(path) => {
                    let img = ImageReader::open(path.clone()).unwrap().decode().unwrap();
                    let dimensions = img.dimensions();
                    let ctx = SubmitContext::from_app(app);
                    ctx.immediate_submit(Box::new(|ctx| {
                        let texture = Texture::new(
                            TextureManager::DEFAULT_SAMPLER_NEAREST,
                            vk::Format::R8G8B8A8_SRGB,
                            ctx,
                            Some(path.file_name().unwrap().to_string_lossy().into()),
                            img.to_rgba8().as_bytes(),
                            vk::Extent3D {
                                width: dimensions.0,
                                height: dimensions.1,
                                depth: 1,
                            },
                            false,
                        );
                        app.texture_manager.borrow_mut().add_texture(texture, &ctx.device, true);
                    }));
                    info!("Imported texture: {:?}", path);
                }
                Command::ReloadShaders => {
                    app.recreate_pipelines();
                    info!("Recreated pipelines and reloaded shaders.");
                }
            }
        }
    }
}
