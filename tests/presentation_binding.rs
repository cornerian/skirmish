//! The exact visual-export contract, exercised across subsystem boundaries.
//!
//! A pinned `MnMaAll.dat` joint-rotation AObj is embedded in a synthetic
//! archive, bound through a presentation manifest, joined to a visual export
//! written to disk with the exact-occurrence contract, instantiated as a
//! rendered presentation, and ticked so a native joint-local delta flows through
//! the driver's routing decision. A blocked acceptance test then applies the
//! same contract to the real export once the producer emits it.

use std::{collections::HashMap, fs, path::Path};

use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use skirmish::{
    collision::bones::{self, LocalTransform},
    menu::{AnimationCue, AnimationId, FrameRange},
    presentation::{
        instance::{InstanceId, JointLocal},
        manifest::{
            ClipScope, ClipSpec, HierarchySpec, OffsetSpace, PRESENTATION_MANIFEST_SCHEMA,
            PresentationManifest, PresentationUpdate, ResourceSpec, VisualOffsetSpaces,
        },
    },
    renderer::{
        presentation::{
            PresentationUpdateRoute, RenderedPresentationInstance, RetainedPresentationReason,
            VisualPresentationBindError, VisualPresentationBinding,
        },
        scene::{GeometrySpace, Scene},
    },
};

const ROTATION_FIXTURE: &str = include_str!("fixtures/melee-ui/back-rotation-spline.json");
const CONTRACT_FIXTURE: &str = include_str!("fixtures/melee-ui/mnmaall-visual-contract.json");
const RESOURCE_ID: &str = "fixture.dat";
const ANIM_JOINT: u32 = 0x40;
const DATA_SECTION_SIZE: usize = 80_000;
const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

#[derive(Deserialize)]
struct RotationFixture {
    binding: RotationBinding,
    aobj: ByteRange,
    fobj: ByteRange,
    program: ByteRange,
    expected_samples: Vec<ExpectedSample>,
}

#[derive(Deserialize)]
struct RotationBinding {
    joint_offset: u32,
    channel: String,
}

#[derive(Deserialize)]
struct ByteRange {
    offset: u32,
    bytes_hex: String,
}

#[derive(Deserialize)]
struct ExpectedSample {
    frame: f32,
    value_bits: String,
}

fn hex_bytes(value: &str) -> Vec<u8> {
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn put_u32(bytes: &mut [u8], offset: u32, value: u32) {
    let start = offset as usize;
    bytes[start..start + 4].copy_from_slice(&value.to_be_bytes());
}

/// One archive whose only model joint is the pinned Back joint, animated by
/// the pinned rotation AObj at its original data-section offsets.
struct SyntheticArchive {
    bytes: Vec<u8>,
    sha256: String,
    joint: u32,
    spaces: VisualOffsetSpaces,
}

fn synthetic_archive(fixture: &RotationFixture) -> SyntheticArchive {
    let mut data = vec![0_u8; DATA_SECTION_SIZE];
    let joint = fixture.binding.joint_offset;
    for field in [0x20, 0x24, 0x28] {
        put_u32(&mut data, joint + field, 1.0_f32.to_bits());
    }
    put_u32(&mut data, ANIM_JOINT + 8, fixture.aobj.offset);
    for range in [&fixture.aobj, &fixture.fobj, &fixture.program] {
        let bytes = hex_bytes(&range.bytes_hex);
        let start = range.offset as usize;
        data[start..start + bytes.len()].copy_from_slice(&bytes);
    }
    let mut bytes = vec![0_u8; 32];
    bytes.extend_from_slice(&data);
    let length = bytes.len() as u32;
    put_u32(&mut bytes, 0, length);
    put_u32(&mut bytes, 4, data.len() as u32);
    SyntheticArchive {
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        bytes,
        joint,
        spaces: VisualOffsetSpaces {
            joints: OffsetSpace::DataSection,
            materials: OffsetSpace::File,
            textures: OffsetSpace::File,
        },
    }
}

fn manifest(archive: &SyntheticArchive) -> PresentationManifest {
    PresentationManifest {
        schema: PRESENTATION_MANIFEST_SCHEMA.into(),
        resource: ResourceSpec {
            id: RESOURCE_ID.into(),
            sha256: archive.sha256.clone(),
            data_section_file_offset: 32,
            data_section_size: DATA_SECTION_SIZE as u32,
        },
        visual_offsets: archive.spaces,
        hierarchies: vec![HierarchySpec {
            id: "back".into(),
            model_root: archive.joint,
            joint_animation_root: Some(ANIM_JOINT),
            material_animation_root: None,
            shape_animation_root: None,
        }],
        clips: vec![ClipSpec {
            id: AnimationId::from("back.rotate"),
            hierarchy: "back".into(),
            scope: ClipScope::Subtree {
                joint: archive.joint,
            },
        }],
    }
}

fn export_document(archive: &SyntheticArchive, geometry_space: Option<&str>) -> Value {
    let mut mesh = json!({
        "name": "joint_7f68_p63c0",
        "joint": archive.joint,
        "resource_id": RESOURCE_ID,
        "dobj_index": 0,
        "positions": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        "indices": [0, 1, 2],
        "material": {}
    });
    if let Some(space) = geometry_space {
        mesh["geometry_space"] = json!(space);
    }
    json!({
        "schema": "skirmish-visual-v1",
        "source": RESOURCE_ID,
        "resources": [{
            "id": RESOURCE_ID,
            "sha256": archive.sha256,
            "offset_spaces": serde_json::to_value(archive.spaces).unwrap(),
        }],
        "joints": [{
            "name": "joint_7f68",
            "offset": archive.joint,
            "flags": 0,
            "local": IDENTITY,
            "world": IDENTITY,
            "inverse_bind": IDENTITY,
        }],
        "meshes": [mesh]
    })
}

fn load_export(directory: &Path, name: &str, document: &Value) -> anyhow::Result<Scene> {
    let path = directory.join(name);
    fs::write(&path, serde_json::to_vec(document)?)?;
    Scene::load(&path)
}

fn rotation_cue() -> AnimationCue {
    AnimationCue {
        id: AnimationId::from("back.rotate"),
        frames: FrameRange {
            start: 200.0,
            end: 260.0,
            loop_start: None,
        },
    }
}

#[test]
fn pinned_joint_rotation_flows_from_archive_bytes_to_an_explicit_route() {
    let fixture: RotationFixture = serde_json::from_str(ROTATION_FIXTURE).unwrap();
    assert_eq!(fixture.binding.channel, "rotation_x");
    let archive = synthetic_archive(&fixture);
    let presentation =
        std::sync::Arc::new(manifest(&archive).bind_hsd_dat(&archive.bytes).unwrap());
    let joint_source = presentation.hierarchy("back").unwrap().joints()[0]
        .source_id
        .clone();
    let directory = tempfile::tempdir().unwrap();

    for (space, expects_transform) in [
        (GeometrySpace::World, false),
        (GeometrySpace::JointLocal, true),
    ] {
        let scene = load_export(
            directory.path(),
            &format!("{}.json", space.as_str()),
            &export_document(&archive, Some(space.as_str())),
        )
        .unwrap();
        assert_eq!(scene.meshes[0].geometry_space, space);
        let binding = std::sync::Arc::new(
            VisualPresentationBinding::bind(&scene, presentation.clone(), "back").unwrap(),
        );
        assert_eq!(binding.joint_geometry_space(&joint_source), Some(space));

        let mut instance =
            RenderedPresentationInstance::instantiate(binding, InstanceId::new(7), rotation_cue())
                .unwrap();
        let pinned: HashMap<u32, u32> = fixture
            .expected_samples
            .iter()
            .map(|sample| {
                (
                    sample.frame.to_bits(),
                    u32::from_str_radix(&sample.value_bits, 16).unwrap(),
                )
            })
            .collect();
        let mut routed_transforms = 0;
        let mut checked_frames = 0;
        for _ in 0..=60 {
            let tick = instance.tick().unwrap();
            for update in tick.updates() {
                match update.update() {
                    PresentationUpdate::JointLocal {
                        source_id,
                        local: JointLocal::Srt(_),
                        ..
                    } => {
                        assert_eq!(source_id, &joint_source);
                        assert_eq!(
                            update.route(),
                            &PresentationUpdateRoute::Retained(
                                RetainedPresentationReason::ComposedIntoJointWorld
                            )
                        );
                    }
                    PresentationUpdate::JointWorld {
                        source_id, world, ..
                    } => {
                        assert_eq!(source_id, &joint_source);
                        let JointLocal::Srt(srt) =
                            instance.scene_instance().joint(source_id).unwrap().local()
                        else {
                            panic!("the pinned joint is SRT authored");
                        };
                        // The single-joint hierarchy composes exactly HSD_MtxSRT.
                        assert_eq!(
                            *world,
                            bones::srt(
                                LocalTransform {
                                    translation: srt.translation,
                                    rotation: srt.rotation,
                                    scale: srt.scale,
                                },
                                None,
                            )
                        );
                        if expects_transform {
                            assert!(matches!(
                                update.route(),
                                PresentationUpdateRoute::JointTransform(occurrence)
                                    if occurrence.visual_offset == archive.joint
                            ));
                            routed_transforms += 1;
                        } else {
                            assert_eq!(
                                update.route(),
                                &PresentationUpdateRoute::Retained(
                                    RetainedPresentationReason::BakedWorldGeometry
                                )
                            );
                        }
                    }
                    other => panic!("unexpected update {other:?}"),
                }
            }
            if let Some(&expected) = pinned.get(&tick.frame().to_bits()) {
                let JointLocal::Srt(srt) = instance
                    .scene_instance()
                    .joint(&joint_source)
                    .unwrap()
                    .local()
                else {
                    panic!("the pinned joint is SRT authored");
                };
                assert_eq!(
                    srt.rotation[0].to_bits(),
                    expected,
                    "frame {} under {space:?}",
                    tick.frame()
                );
                checked_frames += 1;
            }
        }
        assert_eq!(checked_frames, fixture.expected_samples.len());
        assert_eq!(
            routed_transforms > 0,
            expects_transform,
            "joint-local draws must receive composed transforms; baked draws must not"
        );
    }
}

#[test]
fn undeclared_or_mismatched_contract_metadata_is_rejected_before_binding() {
    let fixture: RotationFixture = serde_json::from_str(ROTATION_FIXTURE).unwrap();
    let archive = synthetic_archive(&fixture);
    let presentation =
        std::sync::Arc::new(manifest(&archive).bind_hsd_dat(&archive.bytes).unwrap());
    let directory = tempfile::tempdir().unwrap();

    let undeclared = load_export(
        directory.path(),
        "undeclared.json",
        &export_document(&archive, None),
    );
    assert!(
        format!("{:#}", undeclared.unwrap_err())
            .contains("exact occurrences must declare geometry_space")
    );

    let mut without_spaces = export_document(&archive, Some("world"));
    without_spaces["resources"][0]
        .as_object_mut()
        .unwrap()
        .remove("offset_spaces");
    let without_spaces = load_export(directory.path(), "no-spaces.json", &without_spaces);
    assert!(format!("{:#}", without_spaces.unwrap_err()).contains("must declare offset_spaces"));

    let mut mismatched = export_document(&archive, Some("world"));
    mismatched["resources"][0]["offset_spaces"]["joints"] = json!("file");
    let mismatched = load_export(directory.path(), "mismatched.json", &mismatched).unwrap();
    assert!(matches!(
        VisualPresentationBinding::bind(&mismatched, presentation.clone(), "back"),
        Err(VisualPresentationBindError::OffsetSpaceMismatch { declared, expected, .. })
            if declared.joints == OffsetSpace::File && expected.joints == OffsetSpace::DataSection
    ));

    let mut legacy = export_document(&archive, Some("world"));
    legacy.as_object_mut().unwrap().remove("resources");
    legacy["meshes"][0]
        .as_object_mut()
        .unwrap()
        .remove("resource_id");
    legacy["meshes"][0]
        .as_object_mut()
        .unwrap()
        .remove("dobj_index");
    let legacy = load_export(directory.path(), "legacy.json", &legacy).unwrap();
    assert!(matches!(
        VisualPresentationBinding::bind(&legacy, presentation, "back"),
        Err(VisualPresentationBindError::MissingVisualResource { .. })
    ));
}

#[derive(Deserialize)]
struct ContractFixture {
    resource: ContractResource,
    offset_spaces: VisualOffsetSpaces,
    hierarchies: Vec<ContractHierarchy>,
}

#[derive(Deserialize)]
struct ContractResource {
    id: String,
    sha256: String,
    data_section_file_offset: u32,
}

#[derive(Deserialize)]
struct ContractHierarchy {
    id: String,
    model_root: u32,
    joint_animation_root: u32,
    material_animation_root: u32,
    shape_animation_root: u32,
    expected: ContractExpectations,
}

#[derive(Deserialize)]
struct ContractExpectations {
    draws: usize,
    joints_with_draws: usize,
    world_baked_draws: usize,
    animated_materials: usize,
    animated_textures: usize,
    unmapped_animated_targets: Vec<String>,
}

/// Acceptance test for the producer's exact `MnMaAll.dat` export.
///
/// Run with `MNMAALL_DAT` (the pinned archive) and `MNMAALL_SCENE` (a
/// `skirmish-visual-v1` export carrying the contract in
/// `docs/resources.md`). It stays blocked until the resource project emits
/// `resources[].offset_spaces`, `resource_id`, `dobj_index`, `tobj_index`,
/// and `geometry_space`; a failure here is a contract report, not parity.
#[test]
#[ignore = "blocked: needs MNMAALL_DAT and an MNMAALL_SCENE export carrying the exact presentation contract"]
fn pinned_mnmaall_export_binds_every_animated_occurrence() {
    let fixture: ContractFixture = serde_json::from_str(CONTRACT_FIXTURE).unwrap();
    let archive = fs::read(std::env::var("MNMAALL_DAT").unwrap()).unwrap();
    let scene_path = std::env::var("MNMAALL_SCENE").unwrap();
    let roots: Vec<u32> = fixture
        .hierarchies
        .iter()
        .map(|hierarchy| hierarchy.model_root)
        .collect();
    let scene = Scene::load_joint_roots(Path::new(&scene_path), &roots).unwrap();
    let parents: HashMap<u32, Option<u32>> = scene
        .joints
        .iter()
        .map(|joint| (joint.offset, joint.parent))
        .collect();
    let root_of = |mut joint: u32| loop {
        if roots.contains(&joint) {
            return joint;
        }
        joint = parents[&joint].expect("selected joints descend from a requested root");
    };

    let manifest = PresentationManifest {
        schema: PRESENTATION_MANIFEST_SCHEMA.into(),
        resource: ResourceSpec {
            id: fixture.resource.id.clone(),
            sha256: fixture.resource.sha256.clone(),
            data_section_file_offset: fixture.resource.data_section_file_offset,
            data_section_size: u32::from_be_bytes(archive[4..8].try_into().unwrap()),
        },
        visual_offsets: fixture.offset_spaces,
        hierarchies: fixture
            .hierarchies
            .iter()
            .map(|hierarchy| HierarchySpec {
                id: hierarchy.id.clone(),
                model_root: hierarchy.model_root,
                joint_animation_root: Some(hierarchy.joint_animation_root),
                material_animation_root: Some(hierarchy.material_animation_root),
                shape_animation_root: Some(hierarchy.shape_animation_root),
            })
            .collect(),
        clips: vec![],
    };
    let presentation = std::sync::Arc::new(manifest.bind_hsd_dat(&archive).unwrap());

    let mut report = Vec::new();
    for hierarchy in &fixture.hierarchies {
        let bound = presentation.hierarchy(&hierarchy.id).unwrap();
        let binding =
            VisualPresentationBinding::bind(&scene, presentation.clone(), &hierarchy.id).unwrap();
        let draws: Vec<_> = scene
            .meshes
            .iter()
            .filter(|mesh| {
                mesh.source_occurrence.as_ref().is_some_and(|occurrence| {
                    root_of(occurrence.owner_joint.visual_offset) == hierarchy.model_root
                })
            })
            .collect();
        let joints_with_draws = draws
            .iter()
            .map(|mesh| mesh.joint.unwrap())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let world_baked = draws
            .iter()
            .filter(|mesh| mesh.geometry_space == GeometrySpace::World)
            .count();
        let unmapped: Vec<String> = bound
            .materials()
            .iter()
            .filter(|material| binding.material_occurrence(&material.source_id).is_none())
            .map(|material| material.source_id.as_str().to_owned())
            .chain(
                bound
                    .textures()
                    .iter()
                    .filter(|texture| binding.texture_occurrence(&texture.source_id).is_none())
                    .map(|texture| texture.source_id.as_str().to_owned()),
            )
            .collect();
        report.push((
            hierarchy.id.clone(),
            draws.len(),
            joints_with_draws,
            world_baked,
            bound.materials().len(),
            bound.textures().len(),
            unmapped.clone(),
        ));
        let expected = &hierarchy.expected;
        assert_eq!(draws.len(), expected.draws, "{} draws", hierarchy.id);
        assert_eq!(
            joints_with_draws, expected.joints_with_draws,
            "{} joints with draws",
            hierarchy.id
        );
        assert_eq!(
            world_baked, expected.world_baked_draws,
            "{} world-baked draws",
            hierarchy.id
        );
        assert_eq!(bound.materials().len(), expected.animated_materials);
        assert_eq!(bound.textures().len(), expected.animated_textures);
        assert_eq!(
            unmapped, expected.unmapped_animated_targets,
            "{}",
            hierarchy.id
        );
    }
    eprintln!("contract report: {report:#?}");
}
