use crate::asset::material::RawMaterial;
use crate::asset::texture::{Texture, TextureId};
use crate::camera::Camera;
use crate::commands::Command;
use crate::observe;
use crate::resource::immediate_submit::SubmitContext;
use crate::scene::model::ModelId;
use crate::AppSettings;
use crate::TextureManager;
use crate::World;
use crate::{util, MaterialManager};
use egui::{RichText, TextBuffer, TextureFilter};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

#[derive(Default)]
struct GuiTexture {
    loaded_tex: Option<TextureId>,
    texture: Option<egui::TextureHandle>,
}

impl GuiTexture {
    fn ui(&mut self, ui: &mut egui::Ui, engine_texture: &Texture) {
        if self.loaded_tex != Some(engine_texture.id) {
            self.loaded_tex = Some(engine_texture.id);
            self.texture = None;
        }
        let texture: &egui::TextureHandle = self.texture.get_or_insert_with(|| {
            // Load the texture only once.
            ui.ctx().load_texture(
                "texture preview",
                egui::ColorImage::from_rgba_unmultiplied(
                    [
                        engine_texture.image.extent.width as usize,
                        engine_texture.image.extent.height as usize,
                    ],
                    &engine_texture.data,
                ),
                egui::TextureOptions {
                    magnification: if engine_texture.sampler == 0 {
                        TextureFilter::Nearest
                    } else {
                        TextureFilter::Linear
                    },
                    minification: if engine_texture.sampler == 0 {
                        TextureFilter::Nearest
                    } else {
                        TextureFilter::Linear
                    },
                    wrap_mode: Default::default(),
                },
            )
        });

        // Show the image:
        let size = util::size_image(
            engine_texture.image.extent.width as usize,
            engine_texture.image.extent.height as usize,
            256,
        );
        let size = (size.0 as f32, size.1 as f32);
        ui.image((texture.id(), egui::Vec2::from(size)));
    }
}

pub struct Gui {
    cmd_sender: mpsc::Sender<Command>,
    image: GuiTexture,
    image_lock: bool,
}

impl Gui {
    pub fn new(cmd_sender: mpsc::Sender<Command>) -> Self {
        Self {
            cmd_sender,
            image: GuiTexture::default(),
            image_lock: false,
        }
    }
    pub fn draw(
        &mut self,
        ctx: egui::Context,
        app_settings: &mut AppSettings,
        camera: &mut Camera,
        world: Rc<RefCell<World>>,
        texture_manager: Rc<RefCell<TextureManager>>,
        material_manager: Rc<RefCell<MaterialManager>>,
        submit_context: SubmitContext,
    ) {
        ctx.style_mut(|style| {
            style.visuals.window_shadow = egui::epaint::Shadow::NONE;
        });

        egui::Window::new("World").show(&ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Load").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("glTF", &["gltf", "glb"])
                        .set_directory(std::env::current_dir().unwrap())
                        .pick_file()
                    {
                        self.cmd_sender.send(Command::LoadScene(path)).unwrap();
                    }
                }
                if ui.button("Import").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("glTF", &["gltf", "glb"])
                        .set_directory(std::env::current_dir().unwrap())
                        .pick_file()
                    {
                        self.cmd_sender.send(Command::ImportModel(path)).unwrap();
                    }
                }
            });
            ui.checkbox(&mut app_settings.show_grid, "Show grid");
            egui::CollapsingHeader::new("Camera".as_str()).show(ui, |ui| {
                observe!(
                    camera.position,
                    {
                        ui.add(egui::Slider::new(&mut camera.position.x, -5.0..=5.0).text("X"));
                        ui.add(egui::Slider::new(&mut camera.position.y, -5.0..=5.0).text("Y"));
                        ui.add(egui::Slider::new(&mut camera.position.z, -5.0..=5.0).text("Z"));
                    },
                    |_v| {
                        camera.dirty = true;
                    }
                );
                observe!(
                    camera.fov,
                    {
                        ui.add(egui::Slider::new(&mut camera.fov, 0.0..=180.0).text("FOV"));
                    },
                    |_v| {
                        camera.dirty = true;
                    }
                );
            });
            ui.label(RichText::new("Scene").size(16.0));
            ui.label("Models");
            let models = world.borrow().get_toplevel_model_ids();
            for model in models {
                self.model_div(ui, model, &mut world.borrow_mut())
            }
        });

        egui::Window::new("Assets").show(&ctx, |ui| {
            let mut texture_manager = texture_manager.borrow_mut();
            ui.label("Textures");
            self.image_lock = false;
            for texture in texture_manager.iter_textures() {
                ui.collapsing(
                    format!("{} ({})", texture.image.label.clone().unwrap_or("Untitled".into()), texture.id),
                    |ui| {
                        ui.label(format!("Width: {}", texture.image.extent.width));
                        ui.label(format!("Height: {}", texture.image.extent.height));
                        ui.label(format!("Format: {:?}", texture.image.format));
                        ui.label(format!("Sampler: {}", texture.sampler));
                        if !self.image_lock {
                            self.image_lock = true;
                            self.image.ui(ui, texture);
                        }
                    },
                );
            }

            ui.label("Materials");
            let material_manager = material_manager.borrow();
            for material in material_manager.iter_materials() {
                match material.data {
                    RawMaterial::Unlit(data) => {
                        ui.collapsing(
                            format!("Unlit {} ({})", material.label.clone().unwrap_or("Untitled".into()), material.id),
                            |ui| {
                                ui.label(format!("Texture: {}", data.texture));
                                ui.label(format!("Base color: {:?}", data.color));
                            },
                        );
                    }
                    RawMaterial::Pbr(data) => {
                        ui.collapsing(
                            format!("PBR {} ({})", material.label.clone().unwrap_or("Untitled".into()), material.id),
                            |ui| {
                                ui.label(format!("Texture: {}", data.texture));
                                ui.label(format!("Base color: {:?}", data.albedo));
                                ui.label(format!("Metallic factor: {}", data.metallic));
                                ui.label(format!("Roughness factor: {}", data.roughness));
                            },
                        );
                    }
                }
            }
        });
    }
    fn model_div(&self, ui: &mut egui::Ui, model: ModelId, world: &mut World) {
        let model_name = {
            let model = &world.models[model];
            let model_name = format!("Untitled ({})", model.id);
            model
                .label
                .clone()
                .map(|s| s + format!(" ({})", model.id).as_str())
                .unwrap_or(model_name)
        };

        ui.collapsing(model_name, |ui| {
            observe!(
                world.models[model].transform.w_axis,
                {
                    ui.horizontal(|ui| {
                        ui.label("Position");
                        ui.add(
                            egui::DragValue::new(&mut world.models[model].transform.w_axis.x)
                                .speed(0.01)
                                .prefix("X: "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut world.models[model].transform.w_axis.y)
                                .speed(0.01)
                                .prefix("Y: "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut world.models[model].transform.w_axis.z)
                                .speed(0.01)
                                .prefix("Z: "),
                        );
                    });
                },
                |_v| {
                    world.update_transforms(model, glam::Mat4::IDENTITY);
                }
            );
            ui.label("Meshes");
            let model = &world.models[model];
            for (i, mesh) in model.meshes.iter().enumerate() {
                self.mesh_div(ui, mesh, format!("Mesh {}", i));
            }
            for child in model.children.clone().as_slice() {
                self.model_div(ui, *child, world);
            }
        });
    }

    fn mesh_div(&self, ui: &mut egui::Ui, mesh: &crate::scene::mesh::Mesh, name: String) {
        ui.collapsing(name, |ui| {
            ui.label(format!("#V / #I: {} / {}", mesh.vertices.len(), mesh.indices.len()));
            ui.label(format!("Material: {}", mesh.material));
        });
    }
}
