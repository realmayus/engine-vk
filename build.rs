use serde::Serialize;
use std::collections::{BinaryHeap, HashMap};
use std::fs::File;
use texture_packer::exporter::ImageExporter;
use texture_packer::importer::ImageImporter;
use texture_packer::texture::Texture;
use texture_packer::{MultiTexturePacker, TexturePackerConfig};

fn main() {
    println!("cargo:rerun-if-changed=assets/icons");

    bake_icon_atlas();
}

#[derive(Serialize)]
struct Atlas {
    atlases: Vec<String>,
    entries: HashMap<String, AtlasEntry>,
}

#[derive(Serialize)]
struct AtlasEntry {
    atlas_index: usize,
    location: (u32, u32),
    size: (u32, u32),
}

fn bake_icon_atlas() {
    let config = TexturePackerConfig {
        max_width: 2048,
        max_height: 2048,
        texture_padding: 2,
        trim: false,
        ..Default::default()
    };
    println!("Baking icon atlas...");

    let icon_dir = std::path::Path::new("assets/icons");
    let mut packer = MultiTexturePacker::new_skyline(config);
    for entry in std::fs::read_dir(icon_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() && path.extension().unwrap() == "png" {
            let texture = ImageImporter::import_from_file(&path).unwrap();
            packer
                .pack_own(path.file_stem().unwrap().to_os_string().into_string().unwrap(), texture)
                .unwrap()
        }
    }

    let mut entries = HashMap::new();
    let mut atlases = Vec::new();
    for (i, page) in packer.get_pages().iter().enumerate() {
        for (name, frame) in page.get_frames() {
            entries.insert(
                name.clone(),
                AtlasEntry {
                    atlas_index: i,
                    location: (frame.frame.x, frame.frame.y),
                    size: (frame.frame.w, frame.frame.h),
                },
            );
        }
        //
        // Save the result
        //
        let exporter = ImageExporter::export(page, None).unwrap();
        let path = format!("assets/icons/atlas/atlas-{}.png", i);
        let mut file = File::create(&path).unwrap();
        atlases.push(path.to_string());
        exporter.write_to(&mut file, image::ImageFormat::Png).unwrap();
    }

    // save the atlas entries to assets/icons/atlas/atlas.json using serde
    let mut file = File::create("assets/icons/atlas/atlas.json").unwrap();
    serde_json::to_writer_pretty(&mut file, &Atlas { atlases, entries }).unwrap();
}
