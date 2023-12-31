use egui_winit_vulkano::egui::Ui;
use egui_winit_vulkano::{egui, Gui};
use glam::Mat4;
use log::info;

use lib::scene::Model;
use lib::scene_serde::WorldSerde;
use lib::Dirtyable;
use systems::io;

use crate::commands::{
    Command, DeleteModelCommand, ImportGltfCommand, LoadWorldCommand, UpdateModelCommand,
};
use crate::renderer_impl::GlobalState;

fn draw_model_collapsing(
    ui: &mut Ui,
    model: &Model,
    parent_transform: Mat4,
    commands: &mut Vec<Box<dyn Command>>,
) {
    ui.push_id(model.id, |ui| {
        ui.collapsing(
            format!("{} {}", model.name.clone().unwrap_or_default(), model.id),
            |ui| {
                if ui.button("Remove").clicked() {
                    commands.push(Box::new(DeleteModelCommand {
                        to_delete: model.id,
                    }));
                }
                ui.label("Translation:");
                let mut local_transform = model.local_transform;
                if ui
                    .add(egui::Slider::new(&mut local_transform.w_axis.x, -10.0..=10.0).text("X"))
                    .changed()
                {
                    commands.push(Box::new(UpdateModelCommand {
                        to_update: model.id,
                        parent_transform,
                        local_transform,
                    }));
                }

                if ui
                    .add(egui::Slider::new(&mut local_transform.w_axis.y, -10.0..=10.0).text("Y"))
                    .changed()
                {
                    commands.push(Box::new(UpdateModelCommand {
                        to_update: model.id,
                        parent_transform,
                        local_transform,
                    }));
                }

                if ui
                    .add(egui::Slider::new(&mut local_transform.w_axis.z, -10.0..=10.0).text("Z"))
                    .changed()
                {
                    commands.push(Box::new(UpdateModelCommand {
                        to_update: model.id,
                        parent_transform,
                        local_transform,
                    }));
                }

                ui.label("Meshes:");
                for mesh in model.meshes.as_slice() {
                    ui.push_id(mesh.id, |ui| {
                        ui.collapsing("Mesh", |ui| {
                            ui.label(format!(
                                "# of vert/norm/in: {}/{}/{}",
                                mesh.vertices.len(),
                                mesh.normals.len(),
                                mesh.indices.len()
                            ));
                            ui.label(
                                "Material: ".to_owned()
                                    + &*String::from(
                                        mesh.material.borrow().name.clone().unwrap_or_default(),
                                    ),
                            );
                            if ui.button("Log material").clicked() {
                                info!("{:?}", mesh.material);
                            }
                        })
                    });
                }
                ui.separator();
                ui.label("Children:");
                for child in model.children.as_slice() {
                    draw_model_collapsing(
                        ui,
                        child,
                        parent_transform * model.local_transform,
                        commands,
                    );
                }
            },
        );
    });
}

pub(crate) fn render_gui(gui: &mut Gui, state: &mut GlobalState) {
    let ctx = gui.context();
    egui::Window::new("Scene").show(&ctx, |ui| {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::default()), |ui| {
            if ui.button("Load").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("World file", &["json"])
                    .pick_file()
                {
                    state
                        .commands
                        .push(Box::new(LoadWorldCommand { path: path.clone() }));
                    state.inner_state.opened_file = Some(path);
                }
            }
            if ui.button("Save as…").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    io::world_saver::save(
                        path.as_path(),
                        WorldSerde::from(
                            &state.inner_state.world.textures,
                            &state.inner_state.world.materials,
                            state.inner_state.world.scenes.clone(),
                        ),
                    )
                    .expect("Couldn't save world");
                }
            }
            if ui
                .add_enabled(
                    state.inner_state.opened_file.is_some(),
                    egui::Button::new("Save"),
                )
                .clicked()
            {
                io::world_saver::save(
                    state
                        .inner_state
                        .opened_file
                        .as_ref()
                        .unwrap()
                        .as_path()
                        .parent()
                        .unwrap(),
                    WorldSerde::from(
                        &state.inner_state.world.textures,
                        &state.inner_state.world.materials,
                        state.inner_state.world.scenes.clone(),
                    ),
                )
                .expect("Couldn't save world");
            }
        });
        if ui.button("Import glTF").clicked() {
            if let Some(paths) = rfd::FileDialog::new()
                .add_filter("glTF scenes", &["gltf", "glb"])
                .pick_files()
            {
                for path in paths {
                    state.commands.push(Box::new(ImportGltfCommand { path }));
                }
            }
        }
        ui.label("Loaded models:");
        for scene in state.inner_state.world.scenes.as_slice() {
            ui.push_id(scene.id, |ui| {
                ui.collapsing(String::from(scene.name.clone().unwrap_or_default()), |ui| {
                    ui.label(format!("# of models: {}", scene.models.len()));
                    for model in scene.models.as_slice() {
                        draw_model_collapsing(ui, model, Mat4::default(), &mut state.commands);
                    }
                });
            });
        }
    });

    egui::Window::new("Camera").show(&ctx, |ui| {
        ui.label(format!("Eye: {}", &state.inner_state.camera.eye));
        ui.label(format!("Target: {}", &state.inner_state.camera.target));
        ui.label(format!("Up: {}", &state.inner_state.camera.up));
        ui.add(egui::Slider::new(&mut state.inner_state.camera.speed, 0.03..=0.3).text("Speed"));
        ui.add(
            egui::Slider::new(&mut state.inner_state.camera.fovy, 0.0..=180.0)
                .text("Field of view"),
        );
        if ui.button("Reset").clicked() {
            state.inner_state.camera.reset();
        }
    });

    egui::Window::new("Materials").show(&ctx, |ui| {
        for mat in state.inner_state.world.materials.iter() {
            let (id, name) = { (mat.borrow().id, mat.borrow().name.clone()) };
            ui.push_id(id, |ui| {
                ui.collapsing(
                    format!("{} (id: {})", name.unwrap_or_default(), mat.borrow().id),
                    |ui| {
                        if ui.button("Update").clicked() {
                            mat.clone().borrow_mut().set_dirty(true);
                        }
                        ui.label(format!("Base color factors: {}", mat.borrow().albedo));
                        ui.label(format!(
                            "Metallic roughness factors: {}",
                            mat.borrow().metallic_roughness_factors
                        ));
                        ui.label(format!(
                            "Emissive factors: {}",
                            mat.borrow().emissive_factors
                        ));
                        ui.label(format!(
                            "Occlusion strength: {}",
                            mat.borrow().occlusion_factor
                        ));
                        ui.separator();
                        ui.label(format!(
                            "Base color texture: {:?}",
                            mat.borrow().albedo_texture
                        ));
                        ui.label(format!("Normal texture: {:?}", mat.borrow().normal_texture));
                        ui.label(format!(
                            "Metallic roughness texture: {:?}",
                            mat.borrow().metallic_roughness_texture
                        ));
                        ui.label(format!(
                            "Emissive texture: {:?}",
                            mat.borrow().emissive_texture
                        ));
                        ui.label(format!(
                            "Occlusion texture: {:?}",
                            mat.borrow().occlusion_texture
                        ));
                    },
                );
            });
        }
    });

    egui::Window::new("Textures").show(&ctx, |ui| {
        for tex in state.inner_state.world.textures.iter() {
            ui.label(format!("Id: {}", tex.id));
            ui.label(format!(
                "Name: {}",
                String::from(tex.name.clone().unwrap_or_default())
            ));
        }
    });
}
