use crate::scene::mesh::Mesh;
use glam::{Vec2, Vec3};

pub fn generate_cube() -> Mesh {
    let vertices: Vec<Vec3> = vec![
        Vec3::from((1.0, 1.0, -1.0)),
        Vec3::from((1.0, 1.0, -1.0)),
        Vec3::from((1.0, 1.0, -1.0)),
        Vec3::from((1.0, -1.0, -1.0)),
        Vec3::from((1.0, -1.0, -1.0)),
        Vec3::from((1.0, -1.0, -1.0)),
        Vec3::from((1.0, 1.0, 1.0)),
        Vec3::from((1.0, 1.0, 1.0)),
        Vec3::from((1.0, 1.0, 1.0)),
        Vec3::from((1.0, -1.0, 1.0)),
        Vec3::from((1.0, -1.0, 1.0)),
        Vec3::from((1.0, -1.0, 1.0)),
        Vec3::from((-1.0, 1.0, -1.0)),
        Vec3::from((-1.0, 1.0, -1.0)),
        Vec3::from((-1.0, 1.0, -1.0)),
        Vec3::from((-1.0, -1.0, -1.0)),
        Vec3::from((-1.0, -1.0, -1.0)),
        Vec3::from((-1.0, -1.0, -1.0)),
        Vec3::from((-1.0, 1.0, 1.0)),
        Vec3::from((-1.0, 1.0, 1.0)),
        Vec3::from((-1.0, 1.0, 1.0)),
        Vec3::from((-1.0, -1.0, 1.0)),
        Vec3::from((-1.0, -1.0, 1.0)),
        Vec3::from((-1.0, -1.0, 1.0)),
    ];

    let indices = vec![
        1, 13, 19, 1, 19, 7, 9, 6, 18, 9, 18, 21, 23, 20, 14, 23, 14, 17, 16, 4, 10, 16, 10, 22, 5, 2, 8, 5, 8, 11, 15, 12, 0, 15, 0, 3,
    ];

    let normals: Vec<Vec3> = vec![
        Vec3::from((0.0, 0.0, -1.0)),
        Vec3::from((0.0, 1.0, 0.0)),
        Vec3::from((1.0, 0.0, 0.0)),
        Vec3::from((0.0, 0.0, -1.0)),
        Vec3::from((0.0, -1.0, 0.0)),
        Vec3::from((1.0, 0.0, 0.0)),
        Vec3::from((0.0, 0.0, 1.0)),
        Vec3::from((0.0, 1.0, 0.0)),
        Vec3::from((1.0, 0.0, 0.0)),
        Vec3::from((0.0, 0.0, 1.0)),
        Vec3::from((0.0, -1.0, 0.0)),
        Vec3::from((1.0, 0.0, 0.0)),
        Vec3::from((0.0, 0.0, -1.0)),
        Vec3::from((0.0, 1.0, 0.0)),
        Vec3::from((-1.0, 0.0, 0.0)),
        Vec3::from((0.0, 0.0, -1.0)),
        Vec3::from((0.0, -1.0, 0.0)),
        Vec3::from((-1.0, 0.0, 0.0)),
        Vec3::from((0.0, 0.0, 1.0)),
        Vec3::from((0.0, 1.0, 0.0)),
        Vec3::from((-1.0, 0.0, 0.0)),
        Vec3::from((0.0, 0.0, 1.0)),
        Vec3::from((0.0, -1.0, 0.0)),
        Vec3::from((-1.0, 0.0, 0.0)),
    ];

    let uvs: Vec<Vec2> = vec![
        Vec2::from((0.625, 0.5)),
        Vec2::from((0.625, 0.5)),
        Vec2::from((0.625, 0.5)),
        Vec2::from((0.375, 0.5)),
        Vec2::from((0.375, 0.5)),
        Vec2::from((0.375, 0.5)),
        Vec2::from((0.625, 0.75)),
        Vec2::from((0.625, 0.75)),
        Vec2::from((0.625, 0.75)),
        Vec2::from((0.375, 0.75)),
        Vec2::from((0.375, 0.75)),
        Vec2::from((0.375, 0.75)),
        Vec2::from((0.625, 0.25)),
        Vec2::from((0.875, 0.5)),
        Vec2::from((0.625, 0.25)),
        Vec2::from((0.375, 0.25)),
        Vec2::from((0.125, 0.5)),
        Vec2::from((0.375, 0.25)),
        Vec2::from((0.625, 1.0)),
        Vec2::from((0.875, 0.75)),
        Vec2::from((0.625, 0.0)),
        Vec2::from((0.375, 1.0)),
        Vec2::from((0.125, 0.75)),
        Vec2::from((0.375, 0.0)),
    ];

    Mesh {
        vertices,
        indices,
        normals,
        uvs,
        ..Default::default()
    }
}
