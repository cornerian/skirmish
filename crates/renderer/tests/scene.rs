use renderer::scene::{CullMode, Scene};
use serde_json::{Value, json};
use std::{fs, path::Path};

fn triangle() -> Value {
    json!({
        "schema": "skirmish-visual-v1",
        "meshes": [{"name":"triangle", "positions":[[0.,0.,0.],[1.,0.,0.],[0.,1.,0.]],
            "normals":[[0.,0.,2.],[0.,0.,2.],[0.,0.,2.]], "uv0":[[0.,0.],[1.,0.],[0.,1.]],
            "colors0":[[0.2,0.3,0.4,0.5],[1.,1.,1.,1.],[1.,1.,1.,1.]],
            "indices":[0,1,2], "material":{"cull_mode":"back"}}]
    })
}

fn load(root: &Path, document: &Value) -> anyhow::Result<Scene> {
    let path = root.join("scene.json");
    fs::write(&path, serde_json::to_vec(document)?)?;
    Scene::load(&path)
}

#[test]
fn gx_winding_and_source_visibility_are_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let mut document = triangle();
    let scene = load(directory.path(), &document).unwrap();
    assert_eq!(scene.meshes[0].indices, [0, 2, 1]);
    assert_eq!(scene.meshes[0].material.cull_mode, CullMode::Back);
    assert_eq!(scene.meshes[0].vertices[0].normal, [0., 0., 1.]);
    assert_eq!(scene.bounds(), Some(([0., 0., 0.], [1., 1., 0.])));
    document["source_winding"] = json!("ccw");
    document["meshes"][0]["hidden"] = json!(true);
    let hidden = load(directory.path(), &document).unwrap();
    assert_eq!(hidden.meshes[0].indices, [0, 1, 2]);
    assert!(hidden.meshes[0].hidden);
    assert_eq!(hidden.bounds(), None);
    document["meshes"][0]["hidden"] = json!(false);
    document["meshes"][0]["cull_mode"] = json!(3);
    assert_eq!(load(directory.path(), &document).unwrap().bounds(), None);
}

#[test]
fn malformed_attributes_and_topology_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    for (field, value) in [
        (
            "positions",
            json!([[1e100, 0., 0.], [1., 0., 0.], [0., 1., 0.]]),
        ),
        ("normals", json!([[0., 0., 1.]])),
        ("uv0", json!([[0., 0., 0.], [1., 0.], [0., 1.]])),
        ("colors0", json!([[1., 1., 1., 1.], [1., 1., 1., 1.]])),
        ("indices", json!([0, 1])),
        ("indices", json!([0, 1, 3])),
        ("line_indices", json!([0, 3])),
        ("point_indices", json!([3])),
    ] {
        let mut document = triangle();
        document["meshes"][0][field] = value;
        assert!(
            load(directory.path(), &document).is_err(),
            "accepted invalid {field}"
        );
    }
    let mut document = triangle();
    document["schema"] = json!("another-format");
    assert!(load(directory.path(), &document).is_err());
}

#[test]
fn missing_normals_follow_converted_winding() {
    let directory = tempfile::tempdir().unwrap();
    let mut document = triangle();
    document["meshes"][0]
        .as_object_mut()
        .unwrap()
        .remove("normals");
    let scene = load(directory.path(), &document).unwrap();
    assert_eq!(scene.meshes[0].vertices[0].normal, [0., 0., -1.]);
    assert!(scene.warnings.iter().any(|w| w.contains("missing normals")));
}

#[test]
fn first_texture_loads_exported_paths_and_material_flags() {
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir(directory.path().join("textures")).unwrap();
    let file = fs::File::create(directory.path().join("textures/check.png")).unwrap();
    let mut encoder = png::Encoder::new(file, 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[10, 20, 30, 128])
        .unwrap();
    let mut document = triangle();
    document["textures"] = json!([{"id":"check","path":"textures/check.png","width":1,"height":1}]);
    document["meshes"][0]["material"] = json!({"diffuse":[0.8,0.7,0.6,1.], "material_alpha":0.5,
        "uses_vertex_color":false,"uses_vertex_alpha":true,
        "textures":[{"texture_ids":["check"],"wrap_s":0},{"texture_id":"check"}]});
    let scene = load(directory.path(), &document).unwrap();
    assert_eq!(scene.textures[0].rgba, [10, 20, 30, 128]);
    assert_eq!(scene.meshes[0].material.texture, Some(0));
    assert_eq!(scene.meshes[0].material.color, [0.8, 0.7, 0.6, 1.]);
    assert_eq!(scene.meshes[0].vertices[0].color, [1., 1., 1., 0.5]);
    assert!(scene.warnings.iter().any(|w| w.contains("first of 2")));
    assert!(
        scene
            .warnings
            .iter()
            .any(|w| w.contains("wrapping differs"))
    );
    document["textures"][0]["width"] = json!(2);
    assert!(load(directory.path(), &document).is_err());
    document["textures"][0]["width"] = json!(1);
    document["textures"][0]["path"] = json!(directory.path().join("textures/check.png"));
    assert_eq!(
        load(directory.path(), &document).unwrap().textures[0].rgba,
        [10, 20, 30, 128]
    );
    fs::create_dir(directory.path().join("composed")).unwrap();
    document["textures"][0]["path"] = json!("../textures/check.png");
    assert_eq!(
        load(&directory.path().join("composed"), &document)
            .unwrap()
            .textures[0]
            .rgba,
        [10, 20, 30, 128]
    );
    document["textures"][0]["path"] = json!("textures/missing.png");
    let error = load(directory.path(), &document).unwrap_err();
    assert!(format!("{error:#}").contains("textures/missing.png"));
}

#[test]
fn demo_has_consistent_front_faces_and_no_external_dependencies() {
    let scene = Scene::demo();
    assert!(scene.bounds().is_some());
    assert_eq!(scene.textures[0].rgba.len(), 8 * 8 * 4);
    for mesh in scene.meshes {
        for t in mesh.indices.as_chunks::<3>().0 {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| mesh.vertices[i as usize]);
            let u = glam::Vec3::from(b.position) - glam::Vec3::from(a.position);
            let v = glam::Vec3::from(c.position) - glam::Vec3::from(a.position);
            assert!(u.cross(v).dot(glam::Vec3::from(a.normal)) > 0.);
        }
    }
}
