use serde::Deserialize;
use sha2::{Digest, Sha256};
use skirmish::{
    animation::{Channel, ChannelValue, DataOffset},
    menu::AnimationId,
    presentation::{
        instance::{
            InstanceDescriptor, InstanceId, JointDescriptor, JointLocal, MaterialDescriptor,
            SceneInstance, SourceTarget,
        },
        manifest::{
            AuthoredJointLocal, BindError, BindingDiagnosticKind, ClipScope, ClipSpec,
            HierarchySpec, OffsetSpace, PRESENTATION_MANIFEST_SCHEMA, PresentationManifest,
            PresentationUpdate, ResourceSpec, SampleError, SourceObjectKind, VisualOffsetSpaces,
        },
    },
};

const COVERAGE_FIXTURE: &str = include_str!("fixtures/melee-ui/back-panel-animation.json");

const MODEL: u32 = 0x100;
const ANIM_JOINT: u32 = 0x200;
const MAT_ANIM_JOINT: u32 = 0x300;
const MAT_ANIM: u32 = 0x340;
const DOBJ: u32 = 0x400;
const MOBJ: u32 = 0x440;
const MATERIAL: u32 = 0x460;
const TOBJ: u32 = 0x480;
const TEV: u32 = 0x4e0;
const TEX_ANIM: u32 = 0x520;
const IMAGE_TABLE: u32 = 0x540;
const IMAGE_0: u32 = 0x580;
const IMAGE_2: u32 = 0x5a0;

const JOINT_AOBJ: u32 = 72_184;
const TEXTURE_AOBJ: u32 = 352_476;
const MATERIAL_AOBJ: u32 = 352_732;

#[derive(Deserialize)]
struct Fixture {
    source: FixtureSource,
    data_ranges: Vec<ByteRange>,
}

#[derive(Deserialize)]
struct FixtureSource {
    data_section_size: u32,
}

#[derive(Deserialize)]
struct ByteRange {
    offset: u32,
    bytes_hex: String,
}

fn fixture_archive(with_texture: bool) -> (Vec<u8>, PresentationManifest) {
    let fixture: Fixture = serde_json::from_str(COVERAGE_FIXTURE).unwrap();
    let mut data = vec![0; fixture.source.data_section_size as usize];
    for range in fixture.data_ranges {
        let bytes = hex_bytes(&range.bytes_hex);
        let start = range.offset as usize;
        data[start..start + bytes.len()].copy_from_slice(&bytes);
    }

    put_u32(&mut data, MODEL + 0x10, DOBJ);
    put_f32(&mut data, MODEL + 0x20, 1.0);
    put_f32(&mut data, MODEL + 0x24, 1.0);
    put_f32(&mut data, MODEL + 0x28, 1.0);
    put_u32(&mut data, ANIM_JOINT + 8, JOINT_AOBJ);
    put_u32(&mut data, MAT_ANIM_JOINT + 8, MAT_ANIM);
    put_u32(&mut data, MAT_ANIM + 4, MATERIAL_AOBJ);
    put_u32(
        &mut data,
        MAT_ANIM + 8,
        if with_texture { TEX_ANIM } else { 0 },
    );
    put_u32(&mut data, DOBJ + 8, MOBJ);
    put_u32(&mut data, MOBJ + 8, TOBJ);
    put_u32(&mut data, MOBJ + 0x0c, MATERIAL);
    data[MATERIAL as usize + 4..MATERIAL as usize + 7].copy_from_slice(&[10, 20, 30]);
    put_f32(&mut data, MATERIAL + 0x0c, 0.75);

    put_u32(&mut data, TOBJ + 8, 7);
    put_f32(&mut data, TOBJ + 0x1c, 2.0);
    put_f32(&mut data, TOBJ + 0x20, 0.5);
    put_u32(&mut data, TOBJ + 0x4c, IMAGE_0);
    put_u32(&mut data, TOBJ + 0x58, TEV);
    data[TEV as usize + 16..TEV as usize + 24].copy_from_slice(&[11, 23, 47, 61, 67, 71, 79, 83]);
    put_u32(&mut data, TEX_ANIM + 4, 7);
    put_u32(&mut data, TEX_ANIM + 8, TEXTURE_AOBJ);
    put_u32(&mut data, TEX_ANIM + 0x0c, IMAGE_TABLE);
    put_u16(&mut data, TEX_ANIM + 0x14, 3);
    put_u32(&mut data, IMAGE_TABLE, IMAGE_0);
    put_u32(&mut data, IMAGE_TABLE + 4, 0);
    put_u32(&mut data, IMAGE_TABLE + 8, IMAGE_2);

    let mut archive = vec![0; 32];
    archive.extend_from_slice(&data);
    let archive_len = archive.len() as u32;
    put_u32(&mut archive, 0, archive_len);
    put_u32(&mut archive, 4, data.len() as u32);
    let sha256 = format!("{:x}", Sha256::digest(&archive));
    let manifest = PresentationManifest {
        schema: PRESENTATION_MANIFEST_SCHEMA.into(),
        resource: ResourceSpec {
            id: "fixture.dat".into(),
            sha256,
            data_section_file_offset: 32,
            data_section_size: data.len() as u32,
        },
        visual_offsets: VisualOffsetSpaces {
            joints: OffsetSpace::DataSection,
            materials: OffsetSpace::File,
            textures: OffsetSpace::File,
        },
        hierarchies: vec![HierarchySpec {
            id: "fixture".into(),
            model_root: MODEL,
            joint_animation_root: Some(ANIM_JOINT),
            material_animation_root: Some(MAT_ANIM_JOINT),
            shape_animation_root: None,
        }],
        clips: vec![ClipSpec {
            id: AnimationId::from("fixture.full"),
            hierarchy: "fixture".into(),
            scope: ClipScope::Subtree { joint: MODEL },
        }],
    };
    (archive, manifest)
}

#[test]
fn structurally_derives_exact_targets_and_preserves_null_image_slots() {
    let (archive, manifest) = fixture_archive(true);
    let image_0_id = format!(
        "hsd/{}/image/{IMAGE_0:08x}",
        manifest.resource.source_namespace()
    );
    let image_2_id = format!(
        "hsd/{}/image/{IMAGE_2:08x}",
        manifest.resource.source_namespace()
    );
    let bound = manifest.bind_hsd_dat(&archive).unwrap();
    let hierarchy = bound.hierarchy("fixture").unwrap();

    assert_eq!(hierarchy.joints().len(), 1);
    assert_eq!(hierarchy.materials().len(), 1);
    assert_eq!(hierarchy.textures().len(), 1);
    assert_eq!(hierarchy.bindings().len(), 3);
    assert!(bound.diagnostics().is_empty());
    assert_eq!(
        hierarchy.joints()[0].identity.descriptor_offset.get(),
        MODEL
    );
    assert_eq!(
        hierarchy.materials()[0].identity.descriptor_offset.get(),
        MOBJ
    );
    assert_eq!(
        hierarchy.textures()[0].identity.descriptor_offset.get(),
        TOBJ
    );
    assert_eq!(hierarchy.materials()[0].diffuse, [10, 20, 30]);
    assert_eq!(hierarchy.materials()[0].alpha, 0.75);
    assert!(hierarchy.joints()[0].branch_recurses);
    assert!(matches!(
        hierarchy.joints()[0].local,
        AuthoredJointLocal::Srt(local) if local.scale == [1.0, 1.0, 1.0]
    ));
    assert_eq!(hierarchy.textures()[0].scale, [2.0, 0.5]);
    assert_eq!(hierarchy.textures()[0].konst, Some([11, 23, 47, 61]));
    assert_eq!(hierarchy.textures()[0].tev0, Some([67, 71, 79, 83]));
    assert_eq!(
        hierarchy.textures()[0]
            .current_image
            .as_ref()
            .map(|id| id.as_str()),
        Some(image_0_id.as_str())
    );
    assert_eq!(
        hierarchy.textures()[0]
            .image_slots
            .iter()
            .map(|slot| slot.image_descriptor.map(|offset| offset.get()))
            .collect::<Vec<_>>(),
        [Some(IMAGE_0), None, Some(IMAGE_2)]
    );
    assert_eq!(
        hierarchy.textures()[0]
            .image_slots
            .iter()
            .map(|slot| slot.source_id.as_ref().map(|id| id.as_str()))
            .collect::<Vec<_>>(),
        [Some(image_0_id.as_str()), None, Some(image_2_id.as_str())]
    );
    assert_eq!(
        bound
            .normalize_visual_offset(SourceObjectKind::Joint, MODEL)
            .unwrap()
            .get(),
        MODEL
    );
    assert_eq!(
        bound
            .normalize_visual_offset(SourceObjectKind::Material, MOBJ + 32)
            .unwrap()
            .get(),
        MOBJ
    );
    assert_eq!(
        bound
            .normalize_visual_offset(SourceObjectKind::Texture, TOBJ + 32)
            .unwrap()
            .get(),
        TOBJ
    );
    assert_eq!(
        hierarchy
            .targets_at(
                SourceObjectKind::Material,
                bound
                    .normalize_visual_offset(SourceObjectKind::Material, MOBJ + 32)
                    .unwrap()
            )
            .len(),
        1
    );
    assert_eq!(
        bound
            .clip(&AnimationId::from("fixture.full"))
            .unwrap()
            .bindings()
            .len(),
        3
    );
}

#[test]
fn exact_occurrence_lookup_builds_independent_runtime_instances() {
    const SECOND_DOBJ: u32 = 0x680;
    const SECOND_MAT_ANIM: u32 = 0x6a0;

    let (mut archive, mut manifest) = fixture_archive(true);
    put_u32(&mut archive, 32 + DOBJ + 4, SECOND_DOBJ);
    put_u32(&mut archive, 32 + SECOND_DOBJ + 8, MOBJ);
    put_u32(&mut archive, 32 + MAT_ANIM, SECOND_MAT_ANIM);
    put_u32(&mut archive, 32 + SECOND_MAT_ANIM + 4, MATERIAL_AOBJ);
    put_u32(&mut archive, 32 + SECOND_MAT_ANIM + 8, TEX_ANIM);
    manifest.resource.sha256 = format!("{:x}", Sha256::digest(&archive));

    let bound = manifest.bind_hsd_dat(&archive).unwrap();
    let hierarchy = bound.hierarchy("fixture").unwrap();
    assert_eq!(hierarchy.materials().len(), 2);
    assert_eq!(hierarchy.textures().len(), 2);

    let joint = &hierarchy.joints()[0];
    assert_eq!(
        hierarchy.target(joint.identity),
        Some(SourceTarget::Joint(joint.source_id.clone()))
    );

    let first_material = &hierarchy.materials()[0];
    let second_material = &hierarchy.materials()[1];
    assert_eq!(
        first_material.identity.descriptor_offset,
        second_material.identity.descriptor_offset
    );
    assert_ne!(first_material.identity, second_material.identity);
    assert_ne!(first_material.source_id, second_material.source_id);
    assert_eq!(
        hierarchy.target(first_material.identity),
        Some(SourceTarget::Material(first_material.source_id.clone()))
    );
    assert_eq!(
        hierarchy.target(second_material.identity),
        Some(SourceTarget::Material(second_material.source_id.clone()))
    );

    let first_texture = &hierarchy.textures()[0];
    let second_texture = &hierarchy.textures()[1];
    assert_eq!(
        first_texture.identity.descriptor_offset,
        second_texture.identity.descriptor_offset
    );
    assert_ne!(first_texture.identity, second_texture.identity);
    assert_ne!(first_texture.source_id, second_texture.source_id);
    assert_eq!(
        hierarchy.target(first_texture.identity),
        Some(SourceTarget::Texture(first_texture.source_id.clone()))
    );
    assert_eq!(
        hierarchy.target(second_texture.identity),
        Some(SourceTarget::Texture(second_texture.source_id.clone()))
    );

    let mut wrong_kind = first_texture.identity;
    wrong_kind.kind = SourceObjectKind::Material;
    let mut wrong_descriptor = first_texture.identity;
    wrong_descriptor.descriptor_offset = DataOffset::new(TOBJ + 4);
    let mut wrong_owner = first_texture.identity;
    wrong_owner.owner_joint_offset = DataOffset::new(MODEL + 4);
    let mut wrong_dobj = first_texture.identity;
    wrong_dobj.dobj_index = Some(99);
    let mut wrong_texture = first_texture.identity;
    wrong_texture.texture_index = Some(99);
    for nonexistent in [
        wrong_kind,
        wrong_descriptor,
        wrong_owner,
        wrong_dobj,
        wrong_texture,
    ] {
        assert_eq!(hierarchy.target(nonexistent), None);
    }

    let descriptor = hierarchy.instance_descriptor();
    assert_eq!(descriptor.joints.len(), hierarchy.joints().len());
    assert_eq!(descriptor.materials.len(), hierarchy.materials().len());
    assert_eq!(descriptor.textures.len(), hierarchy.textures().len());
    assert_eq!(descriptor.joints[0].source_id, joint.source_id);
    assert_eq!(descriptor.joints[0].parent, joint.parent);
    assert_eq!(
        descriptor.joints[0].local,
        joint.local.initial_runtime_local()
    );
    assert_eq!(descriptor.joints[0].visible, joint.visible);
    assert_eq!(descriptor.joints[0].branch_recurses, joint.branch_recurses);
    assert_eq!(descriptor.materials[0].source_id, first_material.source_id);
    assert_eq!(descriptor.materials[0].diffuse, first_material.diffuse);
    assert_eq!(descriptor.materials[0].alpha, first_material.alpha);
    assert_eq!(descriptor.textures[0].source_id, first_texture.source_id);
    assert_eq!(
        descriptor.textures[0].current_image,
        first_texture.current_image
    );
    assert_eq!(
        descriptor.textures[0].translation,
        first_texture.translation
    );
    assert_eq!(descriptor.textures[0].scale, first_texture.scale);
    assert_eq!(descriptor.textures[0].blend, first_texture.blend);
    assert_eq!(descriptor.textures[0].konst, first_texture.konst);
    assert_eq!(descriptor.textures[0].tev0, first_texture.tev0);
    assert_eq!(
        descriptor.textures[0].image_slots,
        first_texture
            .image_slots
            .iter()
            .map(|slot| slot.source_id.clone())
            .collect::<Vec<_>>()
    );

    let mut first = hierarchy.instantiate(InstanceId::new(100)).unwrap();
    let second = hierarchy.instantiate(InstanceId::new(101)).unwrap();
    let first_target = hierarchy.target(first_material.identity).unwrap();
    first
        .apply_channel(
            &first_target,
            ChannelValue {
                channel: Channel::MaterialDiffuseR,
                value: 0.5,
            },
        )
        .unwrap();
    assert_eq!(
        first.material(&first_material.source_id).unwrap().diffuse()[0],
        127
    );
    assert_eq!(
        second
            .material(&first_material.source_id)
            .unwrap()
            .diffuse()[0],
        first_material.diffuse[0]
    );
    assert_eq!(first.id(), InstanceId::new(100));
    assert_eq!(second.id(), InstanceId::new(101));
}

#[test]
fn sampled_batch_applies_atomically_and_exposes_state_deltas() {
    let (archive, manifest) = fixture_archive(false);
    let bound = manifest.bind_hsd_dat(&archive).unwrap();
    let hierarchy = bound.hierarchy("fixture").unwrap();
    let joint = &hierarchy.joints()[0];
    let material = &hierarchy.materials()[0];
    let local = joint.local.initial_runtime_local();
    let mut instance = SceneInstance::new(
        InstanceId::new(41),
        InstanceDescriptor {
            joints: vec![JointDescriptor {
                source_id: joint.source_id.clone(),
                parent: joint.parent.clone(),
                local,
                visible: joint.visible,
                branch_recurses: joint.branch_recurses,
            }],
            materials: vec![MaterialDescriptor {
                source_id: material.source_id.clone(),
                diffuse: material.diffuse,
                alpha: material.alpha,
            }],
            textures: vec![],
        },
    )
    .unwrap();
    let clip = bound.clip(&AnimationId::from("fixture.full")).unwrap();
    let batch = clip.sample(0.0).unwrap();
    let updates = batch.apply(&mut instance).unwrap();

    assert_eq!(instance.id(), InstanceId::new(41));
    assert!(updates.iter().any(|update| matches!(
        update,
        PresentationUpdate::Material { instance_id, source_id, .. }
            if *instance_id == InstanceId::new(41) && source_id == &material.source_id
    )));
    assert!(updates.iter().any(|update| matches!(
        update,
        PresentationUpdate::JointLocal { instance_id, source_id, .. }
            if *instance_id == InstanceId::new(41) && source_id == &joint.source_id
    )));
}

#[test]
fn manifest_v1_sampling_is_explicitly_limited_to_integral_hsd_ticks() {
    let (archive, manifest) = fixture_archive(false);
    let bound = manifest.bind_hsd_dat(&archive).unwrap();
    let clip = bound.clip(&AnimationId::from("fixture.full")).unwrap();

    assert!(matches!(
        clip.sample(0.5),
        Err(SampleError::FractionalFrame { frame_bits }) if frame_bits == 0.5_f32.to_bits()
    ));
    assert!(matches!(
        clip.sample(f32::NAN),
        Err(SampleError::NonFiniteFrame)
    ));
    assert!(clip.sample(1.0).is_ok());
}

#[test]
fn provenance_and_visual_coordinate_spaces_are_strict() {
    let (archive, mut manifest) = fixture_archive(false);
    let replacement = if manifest.resource.sha256.starts_with('0') {
        "1"
    } else {
        "0"
    };
    manifest.resource.sha256.replace_range(0..1, replacement);
    assert!(manifest.bind_hsd_dat(&archive).is_err());

    let (_, manifest) = fixture_archive(false);
    let bound = manifest.bind_hsd_dat(&archive).unwrap();
    assert!(
        bound
            .normalize_visual_offset(SourceObjectKind::Material, 31)
            .is_err()
    );

    let (archive_a, manifest_a) = fixture_archive(false);
    let (mut archive_b, mut manifest_b) = fixture_archive(false);
    archive_b[32 + 0x80] = 1;
    manifest_b.resource.sha256 = format!("{:x}", Sha256::digest(&archive_b));
    assert_eq!(manifest_a.resource.id, manifest_b.resource.id);
    let bound_a = manifest_a.bind_hsd_dat(&archive_a).unwrap();
    let bound_b = manifest_b.bind_hsd_dat(&archive_b).unwrap();
    assert_ne!(
        bound_a.hierarchy("fixture").unwrap().joints()[0].source_id,
        bound_b.hierarchy("fixture").unwrap().joints()[0].source_id
    );
    assert!(
        bound
            .normalize_visual_offset(SourceObjectKind::Joint, manifest.resource.data_section_size)
            .is_err()
    );
}

#[test]
fn versioned_json_rejects_unknown_fields() {
    let (_, manifest) = fixture_archive(false);
    let bytes = serde_json::to_vec(&manifest).unwrap();
    assert_eq!(
        PresentationManifest::from_json_slice(&bytes)
            .unwrap()
            .schema,
        PRESENTATION_MANIFEST_SCHEMA
    );

    let mut value = serde_json::to_value(manifest).unwrap();
    value["unversioned_extension"] = serde_json::json!(true);
    assert!(PresentationManifest::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn instance_joint_stops_branch_recursion() {
    let (mut archive, mut manifest) = fixture_archive(false);
    const INSTANCE_TARGET: u32 = 0x600;
    put_u32(&mut archive, 32 + MODEL + 4, 1 << 12);
    put_u32(&mut archive, 32 + MODEL + 8, INSTANCE_TARGET);
    manifest.resource.sha256 = format!("{:x}", Sha256::digest(&archive));

    let bound = manifest.bind_hsd_dat(&archive).unwrap();
    let hierarchy = bound.hierarchy("fixture").unwrap();
    assert_eq!(hierarchy.joints().len(), 1);
    assert!(!hierarchy.joints()[0].branch_recurses);
    assert_eq!(
        hierarchy.joints()[0]
            .instance_target
            .expect("instance reference retained")
            .get(),
        INSTANCE_TARGET
    );
    assert!(
        hierarchy
            .targets_at(
                SourceObjectKind::Joint,
                skirmish::animation::DataOffset::new(INSTANCE_TARGET)
            )
            .is_empty()
    );
}

#[test]
fn user_defined_joint_starts_at_identity_without_misusing_envelope_matrix() {
    const ENVELOPE_MATRIX: u32 = 0x640;
    let (mut archive, mut manifest) = fixture_archive(false);
    put_u32(&mut archive, 32 + MODEL + 4, 1 << 23);
    put_f32(&mut archive, 32 + MODEL + 0x14, 0.25);
    put_f32(&mut archive, 32 + MODEL + 0x2c, 7.0);
    put_u32(&mut archive, 32 + MODEL + 0x38, ENVELOPE_MATRIX);
    for index in 0..12 {
        put_f32(
            &mut archive,
            32 + ENVELOPE_MATRIX + 4 * index,
            100.0 + index as f32,
        );
    }
    manifest.resource.sha256 = format!("{:x}", Sha256::digest(&archive));

    let bound = manifest.bind_hsd_dat(&archive).unwrap();
    let local = bound.hierarchy("fixture").unwrap().joints()[0].local;
    let AuthoredJointLocal::UserDefined {
        authored_srt,
        envelope_matrix,
    } = local
    else {
        panic!("USER_DEF_MTX must remain explicit")
    };
    assert_eq!(authored_srt.rotation[0], 0.25);
    assert_eq!(authored_srt.translation[0], 7.0);
    assert_eq!(envelope_matrix.unwrap()[0], 100.0);
    assert_eq!(
        local.initial_runtime_local(),
        JointLocal::Matrix([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    );
}

#[test]
fn unsupported_fobjs_do_not_suppress_supported_tracks_on_the_same_aobj() {
    const UNSUPPORTED_CHANNEL: u32 = 0x600;
    const UNSUPPORTED_ENCODING: u32 = 0x620;
    let (baseline_archive, baseline_manifest) = fixture_archive(false);
    let baseline = baseline_manifest.bind_hsd_dat(&baseline_archive).unwrap();
    let baseline_track_count = baseline
        .hierarchy("fixture")
        .unwrap()
        .bindings()
        .iter()
        .find(|binding| binding.aobj_offset.get() == JOINT_AOBJ)
        .unwrap()
        .animation
        .tracks
        .len();

    let (mut archive, mut manifest) = fixture_archive(false);
    let first_supported = get_u32(&archive, 32 + JOINT_AOBJ + 8);
    put_u32(&mut archive, 32 + JOINT_AOBJ + 8, UNSUPPORTED_CHANNEL);
    put_u32(&mut archive, 32 + UNSUPPORTED_CHANNEL, UNSUPPORTED_ENCODING);
    archive[(32 + UNSUPPORTED_CHANNEL + 12) as usize] = u8::MAX;
    put_u32(&mut archive, 32 + UNSUPPORTED_ENCODING, first_supported);
    archive[(32 + UNSUPPORTED_ENCODING + 12) as usize] = 5;
    archive[(32 + UNSUPPORTED_ENCODING + 13) as usize] = u8::MAX;
    manifest.resource.sha256 = format!("{:x}", Sha256::digest(&archive));

    let bound = manifest.bind_hsd_dat(&archive).unwrap();
    let binding = bound
        .hierarchy("fixture")
        .unwrap()
        .bindings()
        .iter()
        .find(|binding| binding.aobj_offset.get() == JOINT_AOBJ)
        .unwrap();
    assert_eq!(binding.animation.tracks.len(), baseline_track_count);
    assert_eq!(bound.diagnostics().len(), 2);
    assert!(bound.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind,
        BindingDiagnosticKind::UnsupportedChannel { fobj_offset, .. }
            if fobj_offset.get() == UNSUPPORTED_CHANNEL
    )));
    assert!(bound.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind,
        BindingDiagnosticKind::UnsupportedFractionEncoding { fobj_offset, .. }
            if fobj_offset.get() == UNSUPPORTED_ENCODING
    )));
}

#[test]
fn empty_texture_image_table_is_valid_without_timg_and_rejected_with_it() {
    let (mut archive, mut manifest) = fixture_archive(true);
    let first_fobj = get_u32(&archive, 32 + TEXTURE_AOBJ + 8);
    put_u32(&mut archive, 32 + first_fobj, 0);
    archive[(32 + first_fobj + 12) as usize] = 2;
    put_u32(&mut archive, 32 + TEX_ANIM + 0x0c, 0);
    put_u16(&mut archive, 32 + TEX_ANIM + 0x14, 0);
    manifest.resource.sha256 = format!("{:x}", Sha256::digest(&archive));

    let expected_initial = format!(
        "hsd/{}/image/{IMAGE_0:08x}",
        manifest.resource.source_namespace()
    );
    let bound = manifest.bind_hsd_dat(&archive).unwrap();
    let texture = &bound.hierarchy("fixture").unwrap().textures()[0];
    assert!(texture.image_slots.is_empty());
    assert_eq!(
        texture.current_image.as_ref().map(|id| id.as_str()),
        Some(expected_initial.as_str())
    );

    archive[(32 + first_fobj + 12) as usize] = 1;
    manifest.resource.sha256 = format!("{:x}", Sha256::digest(&archive));
    assert!(matches!(
        manifest.bind_hsd_dat(&archive),
        Err(BindError::TextureImageTrackWithoutTable {
            tobj_offset: TOBJ,
            aobj_offset: TEXTURE_AOBJ,
            ..
        })
    ));
}

#[test]
fn texture_color_track_requires_a_tev_descriptor() {
    let (mut archive, mut manifest) = fixture_archive(true);
    let first_fobj = get_u32(&archive, 32 + TEXTURE_AOBJ + 8);
    put_u32(&mut archive, 32 + first_fobj, 0);
    archive[(32 + first_fobj + 12) as usize] = 2;
    put_u32(&mut archive, 32 + TOBJ + 0x58, 0);
    manifest.resource.sha256 = format!("{:x}", Sha256::digest(&archive));

    let bound = manifest.bind_hsd_dat(&archive).unwrap();
    let texture = &bound.hierarchy("fixture").unwrap().textures()[0];
    assert_eq!(texture.konst, None);
    assert_eq!(texture.tev0, None);

    archive[(32 + first_fobj + 12) as usize] = 12;
    manifest.resource.sha256 = format!("{:x}", Sha256::digest(&archive));

    assert!(matches!(
        manifest.bind_hsd_dat(&archive),
        Err(BindError::TextureColorTrackWithoutTev {
            tobj_offset: TOBJ,
            aobj_offset: TEXTURE_AOBJ,
            ..
        })
    ));
}

#[test]
fn malformed_shape_animation_root_is_rejected_before_diagnostic() {
    let (archive, mut manifest) = fixture_archive(false);
    manifest.hierarchies[0].shape_animation_root = Some(manifest.resource.data_section_size - 4);

    assert!(manifest.bind_hsd_dat(&archive).is_err());
}

fn hex_bytes(value: &str) -> Vec<u8> {
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn put_u16(bytes: &mut [u8], offset: u32, value: u16) {
    let start = offset as usize;
    bytes[start..start + 2].copy_from_slice(&value.to_be_bytes());
}

fn put_u32(bytes: &mut [u8], offset: u32, value: u32) {
    let start = offset as usize;
    bytes[start..start + 4].copy_from_slice(&value.to_be_bytes());
}

fn get_u32(bytes: &[u8], offset: u32) -> u32 {
    let start = offset as usize;
    u32::from_be_bytes(bytes[start..start + 4].try_into().unwrap())
}

fn put_f32(bytes: &mut [u8], offset: u32, value: f32) {
    put_u32(bytes, offset, value.to_bits());
}

#[test]
#[ignore = "developer audit over an externally supplied native resource"]
fn audit_pinned_mnmaall_bindings_and_color_domains() {
    use skirmish::animation::Channel;
    use std::{collections::BTreeMap, fs};

    let path = std::env::var("MNMAALL_DAT").unwrap();
    let archive = fs::read(path).unwrap();
    const PINNED_SHA256: &str = "895e1895e004f84b2cec5583402dedc25ff6173b179f294d50e76945bae32ff0";
    let roots = [
        ("back", 26_664, 76_892, 83_380, 108_592),
        ("panel", 140_584, 350_292, 354_560, 426_336),
        ("contop", 435_944, 518_904, 526_880, 751_680),
        ("cursor", 756_884, 776_500, 779_528, 1_042_288),
    ];
    let manifest = PresentationManifest {
        schema: PRESENTATION_MANIFEST_SCHEMA.into(),
        resource: ResourceSpec {
            id: "MnMaAll.dat".into(),
            sha256: PINNED_SHA256.into(),
            data_section_file_offset: 32,
            data_section_size: u32::from_be_bytes(archive[4..8].try_into().unwrap()),
        },
        visual_offsets: VisualOffsetSpaces {
            joints: OffsetSpace::DataSection,
            materials: OffsetSpace::File,
            textures: OffsetSpace::File,
        },
        hierarchies: roots
            .into_iter()
            .map(|(id, model, joint, material, shape)| HierarchySpec {
                id: id.into(),
                model_root: model,
                joint_animation_root: Some(joint),
                material_animation_root: Some(material),
                shape_animation_root: Some(shape),
            })
            .collect(),
        clips: vec![],
    };
    let bound = manifest.bind_hsd_dat(&archive).unwrap();
    for (id, expected) in [
        ("back", (102, 0, 15, 55)),
        ("panel", (106, 1, 3, 78)),
        ("contop", (42, 13, 6, 41)),
        ("cursor", (14, 1, 4, 11)),
    ] {
        let hierarchy = bound.hierarchy(id).unwrap();
        assert_eq!(
            (
                hierarchy.joints().len(),
                hierarchy.materials().len(),
                hierarchy.textures().len(),
                hierarchy.bindings().len(),
            ),
            expected
        );
        eprintln!(
            "{id}: joints={} materials={} textures={} bindings={}",
            hierarchy.joints().len(),
            hierarchy.materials().len(),
            hierarchy.textures().len(),
            hierarchy.bindings().len()
        );
    }
    eprintln!("diagnostics={:?}", bound.diagnostics());
    assert_eq!(bound.diagnostics().len(), 4);

    let mut extrema: BTreeMap<String, (f32, f32)> = BTreeMap::new();
    for hierarchy in ["back", "panel", "contop", "cursor"]
        .into_iter()
        .map(|id| bound.hierarchy(id).unwrap())
    {
        for binding in hierarchy.bindings() {
            for frame in 0..=binding.animation.end_frame.ceil() as u32 {
                for value in binding
                    .animation
                    .sample_requested_frame(frame as f32)
                    .unwrap()
                {
                    if matches!(
                        value.channel,
                        Channel::MaterialDiffuseR
                            | Channel::MaterialDiffuseG
                            | Channel::MaterialDiffuseB
                            | Channel::MaterialAlpha
                            | Channel::TextureKonstR
                            | Channel::TextureKonstG
                            | Channel::TextureKonstB
                            | Channel::TextureKonstAlpha
                            | Channel::TextureTev0R
                            | Channel::TextureTev0G
                            | Channel::TextureTev0B
                            | Channel::TextureTev0Alpha
                    ) {
                        let key = format!("{:?}", value.channel);
                        let entry = extrema.entry(key).or_insert((value.value, value.value));
                        entry.0 = entry.0.min(value.value);
                        entry.1 = entry.1.max(value.value);
                    }
                }
            }
        }
    }

    eprintln!("color_extrema={extrema:#?}");
    assert!(!extrema.is_empty());
    assert!(
        extrema
            .values()
            .all(|&(minimum, maximum)| minimum >= 0.0 && maximum <= 1.0)
    );
}
