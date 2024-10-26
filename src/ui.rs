use crate::asset::material::{Material, RawMaterial};
use crate::asset::texture::{Texture, TextureId, TextureKind};
use crate::camera::Camera;
use crate::commands::Command;
use crate::observe;
use crate::resource::immediate_submit::SubmitContext;
use crate::scene::billboard::Billboard;
use crate::scene::light::{LightManager, LightMeta};
use crate::scene::model::{Model, ModelId};
use crate::AppSettings;
use crate::TextureManager;
use crate::World;
use crate::{util, MaterialManager};
use egui::{Align2, Color32, Rgba, RichText, TextBuffer, TextureFilter, Ui, Widget};
use glam::{Mat4, Vec2, Vec4};
use std::cell::{RefCell, RefMut};
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
        light_manager: Rc<RefCell<LightManager>>,
        mut _submit_context: SubmitContext,
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

                ui.menu_button("Add", |ui| {
                    if ui.button("Add Billboard").clicked() {
                        let billboard = Billboard::new(
                            Vec4::new(0.0, 0.0, 1.0, 1.0),
                            Vec2::new(1.0, 1.0),
                            [Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)],
                            MaterialManager::DEFAULT_MATERIAL,
                        );
                        let model = Model::new(Vec::new(), Mat4::IDENTITY, None, Some(billboard), Some("Untitled Billboard".into()));
                        let added = world.borrow_mut().add_model(model);
                        println!(
                            "Added billboard: {}, world's billboards: {:?}",
                            added,
                            world.borrow().get_billboards()
                        );
                    }
                });
                if ui.button("Reload Shaders").clicked() {
                    self.cmd_sender.send(Command::ReloadShaders).unwrap();
                }
            });

            ui.checkbox(&mut app_settings.show_grid, "Show grid");
            ui.checkbox(&mut app_settings.view_as_light, "View as light");
            egui::CollapsingHeader::new("Camera".as_str()).show(ui, |ui| {
                observe!(
                    camera.target,
                    {
                        ui.add(egui::Slider::new(&mut camera.target.x, -5.0..=5.0).text("X"));
                        ui.add(egui::Slider::new(&mut camera.target.y, -5.0..=5.0).text("Y"));
                        ui.add(egui::Slider::new(&mut camera.target.z, -5.0..=5.0).text("Z"));
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
                self.model_div(
                    ui,
                    model,
                    world.clone(),
                    material_manager.clone(),
                    &mut light_manager.borrow_mut(),
                    _submit_context.clone(),
                );
            }
            ui.separator();
            ui.label("Lights");
            let lights = light_manager.borrow().keys();
            for light_id in lights {
                ui.collapsing(format!("Light {}", light_id), |ui| {
                    let mgr = light_manager.borrow();
                    let light = mgr.get_light(light_id).unwrap();
                    let mut cutoff_angle = light.data.cutoff_angle.to_degrees();
                    let mut inner_angle = light.data.inner_angle.to_degrees();
                    let mut radius = light.data.radius;

                    let mut intensity = light.data.intensity;
                    let mut dir = light.data.direction;
                    let mut color = light.data.color;
                    drop(mgr);
                    observe!(
                        (cutoff_angle, inner_angle, radius, intensity, dir, color),
                        {
                            ui.add(egui::Slider::new(&mut cutoff_angle, 0.0..=180.0).text("Cutoff"));
                            ui.add(egui::Slider::new(&mut inner_angle, 0.0..=180.0).text("Inner"));
                            ui.add(egui::Slider::new(&mut radius, 0.0..=100.0).text("Radius"));
                            ui.add(egui::Slider::new(&mut intensity, 0.0..=150.0).text("Intensity"));
                            ui.horizontal(|ui| {
                                ui.label("Direction");
                                ui.add(egui::DragValue::new(&mut dir[0]).speed(0.01).range(-2.0..=2.0).prefix("X "));
                                ui.add(egui::DragValue::new(&mut dir[1]).speed(0.01).range(-2.0..=2.0).prefix("Y "));
                                ui.add(egui::DragValue::new(&mut dir[2]).speed(0.01).range(-2.0..=2.0).prefix("Z "));
                            });
                            //color picker
                            ui.color_edit_button_rgba_unmultiplied(&mut color);
                        },
                        |v| {
                            _submit_context.clone().immediate_submit(Box::new(|ctx| {
                                light_manager.borrow_mut().update_light(
                                    light_id,
                                    |light| {
                                        light.cutoff_angle = cutoff_angle.to_radians();
                                        light.intensity = intensity;
                                        light.direction = dir;
                                        light.radius = radius;
                                        light.inner_angle = inner_angle.to_radians();
                                        light.color = color;
                                        light.update_viewproj();
                                    },
                                    ctx,
                                );
                            }));
                        }
                    );
                });
            }

            ui.separator();
            ui.label("Billboards");
            // list all models with billboards, add appropriate UI controls for modifying center of billboard
            // get all (model_id, billboard) where model has billboard (is some)
            let billboards = world
                .borrow()
                .models
                .iter()
                .filter_map(|(id, model)| model.billboard.as_ref().map(|billboard| (*id, billboard.clone())))
                .collect::<Vec<_>>();

            for (id, billboard) in billboards {
                ui.collapsing(format!("Billboard {}", id), |ui| {
                    ui.label(format!("Center: {:?}", billboard.center));
                    let mut center = billboard.center;
                    observe!(
                        center,
                        {
                            ui.add(egui::DragValue::new(&mut center.x).speed(0.01).prefix("X"));
                            ui.add(egui::DragValue::new(&mut center.y).speed(0.01).prefix("Y"));
                            ui.add(egui::DragValue::new(&mut center.z).speed(0.01).prefix("Z"));
                        },
                        |v| {
                            world.borrow_mut().update_billboard(id, v, billboard.uvs);
                        }
                    );
                    ui.horizontal(|ui| {
                        ui.label("Material");
                        egui::ComboBox::from_id_source("Material")
                            .selected_text(
                                material_manager
                                    .borrow()
                                    .get_material(billboard.material)
                                    .unwrap()
                                    .label
                                    .clone()
                                    .unwrap_or("Untitled".into()),
                            )
                            .show_ui(ui, |ui| {
                                let material_manager = material_manager.borrow();
                                for (mid, mlabel, _) in material_manager.iter_materials() {
                                    ui.selectable_value(
                                        &mut world.borrow_mut().models.get_mut(&id).unwrap().billboard.as_mut().unwrap().material,
                                        mid,
                                        mlabel.clone().unwrap_or("Untitled".into()),
                                    );
                                }
                            });
                    });

                    // allow setting UVs
                    ui.label("UVs");
                    let mut uvs = billboard.uvs;
                    observe!(
                        uvs,
                        {
                            for (i, uv) in billboard.uvs.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("UV {}", i));
                                    let mut uv = *uv;
                                    ui.add(egui::DragValue::new(&mut uv.x).range(0.0..=1.0).speed(0.01).prefix("X"));
                                    ui.add(egui::DragValue::new(&mut uv.y).range(0.0..=1.0).speed(0.01).prefix("Y"));
                                    uvs[i] = uv;
                                });
                            }
                        },
                        |v| {
                            world.borrow_mut().update_billboard(id, billboard.center, v);
                        }
                    );
                });
            }
        });
        let pos = (ctx.screen_rect().size().x - 15.0, 15.0);
        egui::Window::new("Assets")
            .pivot(Align2::RIGHT_TOP)
            .default_pos(pos)
            .show(&ctx, |ui| {
                let texture_manager = texture_manager.borrow_mut();
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Textures").size(16.0));
                    if ui.button("Import").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Images", &["png", "jpg", "jpeg", "tga", "bmp"])
                            .set_directory(std::env::current_dir().unwrap())
                            .pick_file()
                        {
                            self.cmd_sender.send(Command::ImportTexture(path)).unwrap();
                        }
                    }
                });
                self.image_lock = false;
                for texture in texture_manager.iter_textures().filter(|t| t.kind == TextureKind::Color) {
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
                ui.separator();
                Self::materials(material_manager, _submit_context, ui, texture_manager);
            });
    }

    fn materials(
        material_manager: Rc<RefCell<MaterialManager>>,
        mut _submit_context: SubmitContext,
        ui: &mut Ui,
        texture_manager: RefMut<TextureManager>,
    ) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Materials").size(16.0));
            if ui.button("Add").clicked() {
                let mid = _submit_context.clone().immediate_submit(Box::new(|ctx| {
                    let material = RawMaterial::Pbr(Default::default());
                    let material = Material::new(Some("Untitled".into()), material, ctx);
                    material_manager.borrow_mut().add_material(material)
                }));
                println!("Added material: {}", mid);
            }
        });
        let materials = material_manager.borrow().iter_materials().collect::<Vec<_>>();
        for (mid, mlabel, material) in materials {
            match material {
                RawMaterial::Unlit(data) => {
                    ui.collapsing(format!("Unlit {} ({})", mlabel.clone().unwrap_or("Untitled".into()), mid), |ui| {
                        ui.label(format!("Texture: {}", data.texture));
                        ui.label(format!("Base color: {:?}", data.color));
                    });
                }
                RawMaterial::Pbr(data) => {
                    ui.collapsing(format!("PBR {} ({})", mlabel.clone().unwrap_or("Untitled".into()), mid), |ui| {
                        // allow setting texture in dropdown
                        ui.label("Texture");
                        let mut mat = data;
                        let mut albedo = Color32::from(Rgba::from_rgba_premultiplied(
                            mat.albedo[0],
                            mat.albedo[1],
                            mat.albedo[2],
                            mat.albedo[3],
                        ));
                        observe!(
                            (mat, albedo),
                            {
                                egui::ComboBox::from_id_source("Texture")
                                    .selected_text(
                                        texture_manager
                                            .get_texture(mat.albedo_tex)
                                            .unwrap()
                                            .image
                                            .label
                                            .clone()
                                            .unwrap_or("Untitled".into()),
                                    )
                                    .show_ui(ui, |ui| {
                                        for texture in texture_manager.iter_textures().filter(|t| t.kind == TextureKind::Color) {
                                            ui.selectable_value(
                                                &mut mat.albedo_tex,
                                                texture.id,
                                                texture.image.label.clone().unwrap_or("Untitled".into()),
                                            );
                                        }
                                    });
                                ui.horizontal(|ui| {
                                    ui.label("Albedo");
                                    ui.color_edit_button_srgba(&mut albedo);
                                });
                                ui.add(egui::Slider::new(&mut mat.metallic, 0.0..=1.0).text("Metallic"));
                                ui.add(egui::Slider::new(&mut mat.roughness, 0.0..=1.0).text("Roughness"));
                            },
                            |v| {
                                _submit_context.clone().immediate_submit(Box::new(|ctx| {
                                    material_manager.borrow_mut().get_material_mut(mid).unwrap().update(
                                        |m| {
                                            if let RawMaterial::Pbr(to_change) = m {
                                                to_change.metallic = mat.metallic;
                                                to_change.roughness = mat.roughness;
                                                to_change.albedo_tex = mat.albedo_tex;
                                                to_change.albedo = Rgba::from(albedo).to_rgba_unmultiplied();
                                            }
                                        },
                                        ctx,
                                    );
                                }));
                            }
                        );
                    });
                }
            }
        }
    }
    fn model_div(
        &self,
        ui: &mut egui::Ui,
        model: ModelId,
        world: Rc<RefCell<World>>,
        material_manager: Rc<RefCell<MaterialManager>>,
        light_manager: &mut LightManager,
        ctx: SubmitContext,
    ) {
        let model_name = {
            let model = &world.borrow().models[&model];
            let model_name = format!("Untitled ({})", model.id);
            model
                .label
                .clone()
                .map(|s| s + format!(" ({})", model.id).as_str())
                .unwrap_or(model_name)
        };

        ui.collapsing(model_name, |ui| {
            ui.menu_button("Actions", |ui| {
                if ui.button("Delete").clicked() {
                    self.cmd_sender.send(Command::DeleteModel(model)).unwrap();
                }
            });
            observe!(
                world.borrow().models[&model].transform.w_axis,
                {
                    ui.horizontal(|ui| {
                        ui.label("Position");
                        ui.add(
                            egui::DragValue::new(&mut world.borrow_mut().models.get_mut(&model).unwrap().transform.w_axis.x)
                                .speed(0.01)
                                .prefix("X: "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut world.borrow_mut().models.get_mut(&model).unwrap().transform.w_axis.y)
                                .speed(0.01)
                                .prefix("Y: "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut world.borrow_mut().models.get_mut(&model).unwrap().transform.w_axis.z)
                                .speed(0.01)
                                .prefix("Z: "),
                        );
                    });
                },
                |_v| {
                    ctx.clone().immediate_submit(Box::new(|ctx| {
                        world
                            .borrow_mut()
                            .update_transforms(model, glam::Mat4::IDENTITY, light_manager, ctx)
                    }));
                }
            );
            ui.label("Meshes");
            let children = {
                let mut world = world.borrow_mut();

                let model = world.models.get_mut(&model).unwrap();
                for (i, mesh) in model.meshes.iter_mut().enumerate() {
                    self.mesh_div(ui, mesh, format!("Mesh {}", i), material_manager.clone());
                }
                model.children.clone()
            };
            for child in children {
                self.model_div(ui, child, world.clone(), material_manager.clone(), light_manager, ctx.clone());
            }
        });
    }

    fn mesh_div(
        &self,
        ui: &mut egui::Ui,
        mesh: &mut crate::scene::mesh::Mesh,
        name: String,
        material_manager: Rc<RefCell<MaterialManager>>,
    ) {
        ui.collapsing(name, |ui| {
            ui.label(format!("#V / #I: {} / {}", mesh.vertices.len(), mesh.indices.len()));

            ui.horizontal(|ui| {
                ui.label("Material");
                egui::ComboBox::from_id_source("Material")
                    .selected_text(
                        material_manager
                            .borrow()
                            .get_material(mesh.material)
                            .unwrap()
                            .label
                            .clone()
                            .unwrap_or("Untitled".into()),
                    )
                    .show_ui(ui, |ui| {
                        let material_manager = material_manager.borrow();
                        for (mid, mlabel, _) in material_manager.iter_materials() {
                            ui.selectable_value(&mut mesh.material, mid, mlabel.clone().unwrap_or("Untitled".into()));
                        }
                    });
            });
        });
    }
}
