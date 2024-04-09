use crate::immediate_submit::SubmitContext;
use crate::App;
use ash::vk;
use std::path::PathBuf;
use std::sync::mpsc;

pub enum Command {
    LoadScene(PathBuf),
    ImportModel(PathBuf),
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
                    let mut reader =
                        crate::gltf::GltfReader::new(app.world.clone(), app.texture_manager.clone(), app.material_manager.clone());
                    let ctx = SubmitContext::from_app(app);
                    reader.load(&path, ctx);
                }
                Command::ImportModel(path) => {
                    let mut reader =
                        crate::gltf::GltfReader::new(app.world.clone(), app.texture_manager.clone(), app.material_manager.clone());
                    let ctx = SubmitContext::from_app(app);
                    reader.load(&path, ctx);
                }
            }
        }
    }
}
