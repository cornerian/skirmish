//! Exact joins between immutable native presentation data and visual exports.
//!
//! Native descriptor offsets are meaningful only inside the exact archive that
//! supplied them. This adapter verifies resource provenance, normalizes the
//! visual export's declared offset spaces, and retains complete occurrence
//! identity before any sampled animation is allowed to reach the renderer.

use std::{collections::HashMap, sync::Arc};

use thiserror::Error;

use crate::{
    animation::DataOffset,
    presentation::{
        instance::{SourceJointId, SourceMaterialId, SourceTarget, SourceTextureId},
        manifest::{
            BindError, BoundHierarchy, BoundPresentation, SourceBindingIdentity, SourceObjectKind,
        },
    },
};

use super::scene::{
    Scene, VisualJointOccurrence, VisualMaterialOccurrence, VisualTextureOccurrence,
};

/// A provenance-checked correspondence between one native hierarchy and its
/// exact visual occurrences.
#[derive(Clone, Debug)]
pub struct VisualPresentationBinding {
    presentation: Arc<BoundPresentation>,
    hierarchy: String,
    joints: HashMap<SourceJointId, VisualJointOccurrence>,
    materials: HashMap<SourceMaterialId, VisualMaterialOccurrence>,
    textures: HashMap<SourceTextureId, VisualTextureOccurrence>,
}

impl VisualPresentationBinding {
    /// Bind one hierarchy without using filenames, bare offsets, or exporter
    /// instance strings as identity fallbacks.
    pub fn bind(
        scene: &Scene,
        presentation: Arc<BoundPresentation>,
        hierarchy: &str,
    ) -> Result<Self, VisualPresentationBindError> {
        let bound_hierarchy = presentation.hierarchy(hierarchy).ok_or_else(|| {
            VisualPresentationBindError::MissingHierarchy {
                hierarchy: hierarchy.to_owned(),
            }
        })?;
        validate_resource(scene, presentation.as_ref())?;
        let maps = bind_exact_occurrences(scene, presentation.as_ref(), bound_hierarchy)?;
        if maps.joints.is_empty() && maps.materials.is_empty() && maps.textures.is_empty() {
            return Err(VisualPresentationBindError::NoExactOccurrences {
                resource_id: presentation.resource().id.clone(),
                hierarchy: hierarchy.to_owned(),
            });
        }
        Ok(Self {
            presentation,
            hierarchy: hierarchy.to_owned(),
            joints: maps.joints,
            materials: maps.materials,
            textures: maps.textures,
        })
    }

    pub fn presentation(&self) -> &BoundPresentation {
        self.presentation.as_ref()
    }

    pub fn hierarchy(&self) -> &str {
        &self.hierarchy
    }

    pub fn joint_occurrence(&self, source: &SourceJointId) -> Option<&VisualJointOccurrence> {
        self.joints.get(source)
    }

    pub fn material_occurrence(
        &self,
        source: &SourceMaterialId,
    ) -> Option<&VisualMaterialOccurrence> {
        self.materials.get(source)
    }

    pub fn texture_occurrence(&self, source: &SourceTextureId) -> Option<&VisualTextureOccurrence> {
        self.textures.get(source)
    }
}

#[derive(Debug, Error)]
pub enum VisualPresentationBindError {
    #[error("presentation hierarchy {hierarchy:?} does not exist")]
    MissingHierarchy { hierarchy: String },
    #[error("visual scene does not declare presentation resource {resource_id:?}")]
    MissingVisualResource { resource_id: String },
    #[error("visual scene declares presentation resource {resource_id:?} more than once")]
    AmbiguousVisualResource { resource_id: String },
    #[error("visual resource {resource_id:?} hash mismatch: expected {expected}, found {actual}")]
    HashMismatch {
        resource_id: String,
        expected: String,
        actual: String,
    },
    #[error("visual {kind:?} offset {visual_offset:#x} cannot be normalized: {source}")]
    InvalidVisualOffset {
        kind: SourceObjectKind,
        visual_offset: u32,
        #[source]
        source: BindError,
    },
    #[error("visual resource {resource_id:?} has no exact occurrences in hierarchy {hierarchy:?}")]
    NoExactOccurrences {
        resource_id: String,
        hierarchy: String,
    },
    #[error("source target {target:?} maps to conflicting visual occurrences")]
    ConflictingOccurrence { target: SourceTarget },
    #[error(
        "texture source {texture} owner material mismatch: expected {expected:#x}, found {actual:#x}"
    )]
    TextureOwnerMaterialMismatch {
        texture: SourceTextureId,
        expected: u32,
        actual: u32,
    },
}

#[derive(Default)]
struct ExactOccurrenceMaps {
    joints: HashMap<SourceJointId, VisualJointOccurrence>,
    materials: HashMap<SourceMaterialId, VisualMaterialOccurrence>,
    textures: HashMap<SourceTextureId, VisualTextureOccurrence>,
}

fn validate_resource(
    scene: &Scene,
    presentation: &BoundPresentation,
) -> Result<(), VisualPresentationBindError> {
    let expected = presentation.resource();
    let mut matches = scene
        .resources
        .iter()
        .filter(|resource| resource.id().as_str() == expected.id);
    let Some(resource) = matches.next() else {
        return Err(VisualPresentationBindError::MissingVisualResource {
            resource_id: expected.id.clone(),
        });
    };
    if matches.next().is_some() {
        return Err(VisualPresentationBindError::AmbiguousVisualResource {
            resource_id: expected.id.clone(),
        });
    }
    if resource.sha256() != expected.sha256 {
        return Err(VisualPresentationBindError::HashMismatch {
            resource_id: expected.id.clone(),
            expected: expected.sha256.clone(),
            actual: resource.sha256().to_owned(),
        });
    }
    Ok(())
}

fn bind_exact_occurrences(
    scene: &Scene,
    presentation: &BoundPresentation,
    hierarchy: &BoundHierarchy,
) -> Result<ExactOccurrenceMaps, VisualPresentationBindError> {
    let resource_id = presentation.resource().id.as_str();
    let mut maps = ExactOccurrenceMaps::default();
    for mesh in &scene.meshes {
        if let Some(occurrence) = mesh.source_occurrence.as_ref()
            && occurrence.owner_joint.resource_id.as_str() == resource_id
        {
            let identity = joint_identity(presentation, &occurrence.owner_joint)?;
            if let Some(target) = hierarchy.target(identity) {
                let SourceTarget::Joint(source) = &target else {
                    unreachable!("a bound joint identity resolves only to a joint target");
                };
                insert_unique(
                    &mut maps.joints,
                    source.clone(),
                    occurrence.owner_joint.clone(),
                    target,
                )?;
            }
        }

        if let Some(occurrence) = mesh.material.source_occurrence.as_ref()
            && occurrence.owner_dobj.owner_joint.resource_id.as_str() == resource_id
        {
            let identity = material_identity(presentation, occurrence)?;
            if let Some(target) = hierarchy.target(identity) {
                let SourceTarget::Material(source) = &target else {
                    unreachable!("a bound material identity resolves only to a material target");
                };
                insert_unique(
                    &mut maps.materials,
                    source.clone(),
                    occurrence.clone(),
                    target,
                )?;
            }
        }

        for stage in &mesh.material.texture_sources {
            let Some(occurrence) = stage.occurrence.as_ref() else {
                continue;
            };
            if occurrence
                .owner_material
                .owner_dobj
                .owner_joint
                .resource_id
                .as_str()
                != resource_id
            {
                continue;
            }
            let (identity, owner_material_offset) = texture_identity(presentation, occurrence)?;
            if let Some(target) = hierarchy.target(identity) {
                let SourceTarget::Texture(source) = &target else {
                    unreachable!("a bound texture identity resolves only to a texture target");
                };
                let expected_owner = hierarchy
                    .textures()
                    .iter()
                    .find(|bound| bound.source_id == *source)
                    .expect("a resolved texture target belongs to this hierarchy")
                    .owner_material_offset;
                if owner_material_offset != expected_owner {
                    return Err(VisualPresentationBindError::TextureOwnerMaterialMismatch {
                        texture: source.clone(),
                        expected: expected_owner.get(),
                        actual: owner_material_offset.get(),
                    });
                }
                insert_unique(
                    &mut maps.textures,
                    source.clone(),
                    occurrence.clone(),
                    target,
                )?;
            }
        }
    }
    Ok(maps)
}

fn insert_unique<K, V>(
    map: &mut HashMap<K, V>,
    key: K,
    occurrence: V,
    target: SourceTarget,
) -> Result<(), VisualPresentationBindError>
where
    K: std::hash::Hash + Eq,
    V: PartialEq,
{
    if let Some(previous) = map.get(&key)
        && previous != &occurrence
    {
        return Err(VisualPresentationBindError::ConflictingOccurrence { target });
    }
    map.insert(key, occurrence);
    Ok(())
}

fn normalize(
    presentation: &BoundPresentation,
    kind: SourceObjectKind,
    visual_offset: u32,
) -> Result<DataOffset, VisualPresentationBindError> {
    presentation
        .normalize_visual_offset(kind, visual_offset)
        .map_err(|source| VisualPresentationBindError::InvalidVisualOffset {
            kind,
            visual_offset,
            source,
        })
}

fn joint_identity(
    presentation: &BoundPresentation,
    occurrence: &VisualJointOccurrence,
) -> Result<SourceBindingIdentity, VisualPresentationBindError> {
    let offset = normalize(
        presentation,
        SourceObjectKind::Joint,
        occurrence.visual_offset,
    )?;
    Ok(SourceBindingIdentity {
        kind: SourceObjectKind::Joint,
        descriptor_offset: offset,
        owner_joint_offset: offset,
        dobj_index: None,
        texture_index: None,
    })
}

fn material_identity(
    presentation: &BoundPresentation,
    occurrence: &VisualMaterialOccurrence,
) -> Result<SourceBindingIdentity, VisualPresentationBindError> {
    Ok(SourceBindingIdentity {
        kind: SourceObjectKind::Material,
        descriptor_offset: normalize(
            presentation,
            SourceObjectKind::Material,
            occurrence.visual_offset.get(),
        )?,
        owner_joint_offset: normalize(
            presentation,
            SourceObjectKind::Joint,
            occurrence.owner_dobj.owner_joint.visual_offset,
        )?,
        dobj_index: Some(occurrence.owner_dobj.dobj_index),
        texture_index: None,
    })
}

fn texture_identity(
    presentation: &BoundPresentation,
    occurrence: &VisualTextureOccurrence,
) -> Result<(SourceBindingIdentity, DataOffset), VisualPresentationBindError> {
    let owner_material_offset = normalize(
        presentation,
        SourceObjectKind::Material,
        occurrence.owner_material.visual_offset.get(),
    )?;
    Ok((
        SourceBindingIdentity {
            kind: SourceObjectKind::Texture,
            descriptor_offset: normalize(
                presentation,
                SourceObjectKind::Texture,
                occurrence.visual_offset.get(),
            )?,
            owner_joint_offset: normalize(
                presentation,
                SourceObjectKind::Joint,
                occurrence
                    .owner_material
                    .owner_dobj
                    .owner_joint
                    .visual_offset,
            )?,
            dobj_index: Some(occurrence.owner_material.owner_dobj.dobj_index),
            texture_index: Some(occurrence.tobj_index),
        },
        owner_material_offset,
    ))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use serde::Deserialize;
    use serde_json::json;
    use sha2::{Digest, Sha256};

    use crate::{
        presentation::manifest::{
            HierarchySpec, OffsetSpace, PRESENTATION_MANIFEST_SCHEMA, PresentationManifest,
            ResourceSpec, VisualOffsetSpaces,
        },
        renderer::scene::{VisualDObjOccurrence, VisualResourceId},
    };

    use super::*;

    const RESOURCE_ID: &str = "menu.dat";
    const MODEL_ROOT: u32 = 0x20;
    const ANIMATED_MODEL: u32 = 0x100;
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
    const SECOND_DOBJ: u32 = 0x680;
    const SECOND_MAT_ANIM: u32 = 0x6a0;
    const TEXTURE_AOBJ: u32 = 352_476;
    const MATERIAL_AOBJ: u32 = 352_732;
    const COVERAGE_FIXTURE: &str =
        include_str!("../../tests/fixtures/melee-ui/back-panel-animation.json");

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

    fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }

    fn put_data_u32(bytes: &mut [u8], offset: u32, value: u32) {
        put_u32(bytes, offset as usize, value);
    }

    fn put_data_u16(bytes: &mut [u8], offset: u32, value: u16) {
        let start = offset as usize;
        bytes[start..start + 2].copy_from_slice(&value.to_be_bytes());
    }

    fn put_data_f32(bytes: &mut [u8], offset: u32, value: f32) {
        put_data_u32(bytes, offset, value.to_bits());
    }

    fn hex_bytes(value: &str) -> Vec<u8> {
        let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
        assert!(remainder.is_empty());
        pairs
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    fn one_joint_presentation(offset_space: OffsetSpace) -> Arc<BoundPresentation> {
        let mut data = vec![0_u8; 0x80];
        for field in [0x20, 0x24, 0x28] {
            put_u32(&mut data, MODEL_ROOT as usize + field, 1.0_f32.to_bits());
        }
        let mut archive = vec![0_u8; 32];
        archive.extend_from_slice(&data);
        let archive_len = archive.len() as u32;
        put_u32(&mut archive, 0, archive_len);
        put_u32(&mut archive, 4, data.len() as u32);
        let manifest = PresentationManifest {
            schema: PRESENTATION_MANIFEST_SCHEMA.into(),
            resource: ResourceSpec {
                id: RESOURCE_ID.into(),
                sha256: format!("{:x}", Sha256::digest(&archive)),
                data_section_file_offset: 32,
                data_section_size: data.len() as u32,
            },
            visual_offsets: VisualOffsetSpaces {
                joints: offset_space,
                materials: offset_space,
                textures: offset_space,
            },
            hierarchies: vec![HierarchySpec {
                id: "root".into(),
                model_root: MODEL_ROOT,
                joint_animation_root: None,
                material_animation_root: None,
                shape_animation_root: None,
            }],
            clips: Vec::new(),
        };
        Arc::new(manifest.bind_hsd_dat(&archive).unwrap())
    }

    fn repeated_material_presentation() -> Arc<BoundPresentation> {
        let fixture: Fixture = serde_json::from_str(COVERAGE_FIXTURE).unwrap();
        let mut data = vec![0; fixture.source.data_section_size as usize];
        for range in fixture.data_ranges {
            let bytes = hex_bytes(&range.bytes_hex);
            let start = range.offset as usize;
            data[start..start + bytes.len()].copy_from_slice(&bytes);
        }

        put_data_u32(&mut data, ANIMATED_MODEL + 0x10, DOBJ);
        for field in [0x20, 0x24, 0x28] {
            put_data_f32(&mut data, ANIMATED_MODEL + field, 1.0);
        }
        put_data_u32(&mut data, MAT_ANIM_JOINT + 8, MAT_ANIM);
        put_data_u32(&mut data, MAT_ANIM, SECOND_MAT_ANIM);
        put_data_u32(&mut data, MAT_ANIM + 4, MATERIAL_AOBJ);
        put_data_u32(&mut data, MAT_ANIM + 8, TEX_ANIM);
        put_data_u32(&mut data, SECOND_MAT_ANIM + 4, MATERIAL_AOBJ);
        put_data_u32(&mut data, SECOND_MAT_ANIM + 8, TEX_ANIM);
        put_data_u32(&mut data, DOBJ + 4, SECOND_DOBJ);
        put_data_u32(&mut data, DOBJ + 8, MOBJ);
        put_data_u32(&mut data, SECOND_DOBJ + 8, MOBJ);
        put_data_u32(&mut data, MOBJ + 8, TOBJ);
        put_data_u32(&mut data, MOBJ + 0x0c, MATERIAL);
        data[MATERIAL as usize + 4..MATERIAL as usize + 7].copy_from_slice(&[10, 20, 30]);
        put_data_f32(&mut data, MATERIAL + 0x0c, 0.75);
        put_data_u32(&mut data, TOBJ + 8, 7);
        put_data_f32(&mut data, TOBJ + 0x1c, 1.0);
        put_data_f32(&mut data, TOBJ + 0x20, 1.0);
        put_data_u32(&mut data, TOBJ + 0x4c, IMAGE_0);
        put_data_u32(&mut data, TOBJ + 0x58, TEV);
        data[TEV as usize + 16..TEV as usize + 24].copy_from_slice(&[1; 8]);
        put_data_u32(&mut data, TEX_ANIM + 4, 7);
        put_data_u32(&mut data, TEX_ANIM + 8, TEXTURE_AOBJ);
        put_data_u32(&mut data, TEX_ANIM + 0x0c, IMAGE_TABLE);
        put_data_u16(&mut data, TEX_ANIM + 0x14, 3);
        put_data_u32(&mut data, IMAGE_TABLE, IMAGE_0);
        put_data_u32(&mut data, IMAGE_TABLE + 8, IMAGE_2);

        let mut archive = vec![0; 32];
        archive.extend_from_slice(&data);
        let archive_len = archive.len() as u32;
        put_u32(&mut archive, 0, archive_len);
        put_u32(&mut archive, 4, data.len() as u32);
        let manifest = PresentationManifest {
            schema: PRESENTATION_MANIFEST_SCHEMA.into(),
            resource: ResourceSpec {
                id: RESOURCE_ID.into(),
                sha256: format!("{:x}", Sha256::digest(&archive)),
                data_section_file_offset: 32,
                data_section_size: data.len() as u32,
            },
            visual_offsets: VisualOffsetSpaces {
                joints: OffsetSpace::DataSection,
                materials: OffsetSpace::File,
                textures: OffsetSpace::File,
            },
            hierarchies: vec![HierarchySpec {
                id: "animated".into(),
                model_root: ANIMATED_MODEL,
                joint_animation_root: None,
                material_animation_root: Some(MAT_ANIM_JOINT),
                shape_animation_root: None,
            }],
            clips: Vec::new(),
        };
        Arc::new(manifest.bind_hsd_dat(&archive).unwrap())
    }

    fn scene_with_resources(path: &Path, resources: serde_json::Value) -> Scene {
        fs::write(
            path,
            serde_json::to_vec(&json!({
                "schema": "skirmish-visual-v1",
                "resources": resources,
                "meshes": []
            }))
            .unwrap(),
        )
        .unwrap();
        Scene::load(path).unwrap()
    }

    fn repeated_material_scene(path: &Path, hash: &str, material_offset: u32) -> Scene {
        let mesh = |name: &str, dobj_index: u16| {
            json!({
                "name": name,
                "joint": ANIMATED_MODEL,
                "resource_id": RESOURCE_ID,
                "dobj_index": dobj_index,
                "positions": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                "indices": [0, 1, 2],
                "material": {
                    "material_offset": material_offset + 32,
                    "textures": [{"tobj_offset": TOBJ + 32, "tobj_index": 0}]
                }
            })
        };
        fs::write(
            path,
            serde_json::to_vec(&json!({
                "schema": "skirmish-visual-v1",
                "resources": [{"id": RESOURCE_ID, "sha256": hash}],
                "joints": [{"name": "root", "offset": ANIMATED_MODEL}],
                "meshes": [mesh("first", 0), mesh("second", 1)]
            }))
            .unwrap(),
        )
        .unwrap();
        Scene::load(path).unwrap()
    }

    fn exact_joint_mesh(
        resource_id: VisualResourceId,
        visual_offset: u32,
    ) -> super::super::scene::Mesh {
        let mut mesh = Scene::demo().meshes.remove(0);
        mesh.joint = Some(visual_offset);
        mesh.source_occurrence = Some(VisualDObjOccurrence {
            owner_joint: VisualJointOccurrence {
                resource_id,
                visual_offset,
            },
            dobj_index: 0,
        });
        mesh
    }

    #[test]
    fn binds_only_the_exact_resource_and_normalizes_file_offsets() {
        let presentation = one_joint_presentation(OffsetSpace::File);
        let directory = tempfile::tempdir().unwrap();
        let hash = presentation.resource().sha256.clone();
        let mut scene = scene_with_resources(
            &directory.path().join("scene.json"),
            json!([
                {"id": RESOURCE_ID, "sha256": hash},
                {"id": "other.dat", "sha256": "1".repeat(64)}
            ]),
        );
        let target_resource = scene.resources[0].id().clone();
        let other_resource = scene.resources[1].id().clone();
        scene.meshes.push(exact_joint_mesh(
            other_resource,
            MODEL_ROOT + presentation.resource().data_section_file_offset,
        ));
        scene.meshes.push(exact_joint_mesh(
            target_resource.clone(),
            MODEL_ROOT + presentation.resource().data_section_file_offset,
        ));

        let binding =
            VisualPresentationBinding::bind(&scene, presentation.clone(), "root").unwrap();
        let source = &presentation.hierarchy("root").unwrap().joints()[0].source_id;
        assert_eq!(binding.hierarchy(), "root");
        assert_eq!(binding.presentation().resource(), presentation.resource());
        assert_eq!(
            binding.joint_occurrence(source),
            Some(&VisualJointOccurrence {
                resource_id: target_resource,
                visual_offset: MODEL_ROOT + 32,
            })
        );
    }

    #[test]
    fn binds_reused_materials_and_textures_by_full_occurrence_and_rejects_wrong_owner() {
        let presentation = repeated_material_presentation();
        let directory = tempfile::tempdir().unwrap();
        let scene = repeated_material_scene(
            &directory.path().join("exact.json"),
            &presentation.resource().sha256,
            MOBJ,
        );
        let binding =
            VisualPresentationBinding::bind(&scene, presentation.clone(), "animated").unwrap();
        let hierarchy = presentation.hierarchy("animated").unwrap();
        assert_eq!(hierarchy.materials().len(), 2);
        assert_eq!(hierarchy.textures().len(), 2);

        for material in hierarchy.materials() {
            let occurrence = binding
                .material_occurrence(&material.source_id)
                .expect("every animated MObj occurrence must bind");
            assert_eq!(occurrence.visual_offset.get(), MOBJ + 32);
            assert_eq!(
                occurrence.owner_dobj.dobj_index,
                material.identity.dobj_index.unwrap()
            );
        }
        for texture in hierarchy.textures() {
            let occurrence = binding
                .texture_occurrence(&texture.source_id)
                .expect("every animated TObj occurrence must bind");
            assert_eq!(occurrence.visual_offset.get(), TOBJ + 32);
            assert_eq!(
                occurrence.owner_material.owner_dobj.dobj_index,
                texture.identity.dobj_index.unwrap()
            );
            assert_eq!(
                occurrence.tobj_index,
                texture.identity.texture_index.unwrap()
            );
        }

        let wrong_owner = repeated_material_scene(
            &directory.path().join("wrong-owner.json"),
            &presentation.resource().sha256,
            MOBJ + 4,
        );
        assert!(matches!(
            VisualPresentationBinding::bind(&wrong_owner, presentation, "animated"),
            Err(VisualPresentationBindError::TextureOwnerMaterialMismatch {
                expected: MOBJ,
                actual,
                ..
            }) if actual == MOBJ + 4
        ));
    }

    #[test]
    fn rejects_missing_ambiguous_wrong_and_legacy_resource_inputs() {
        let presentation = one_joint_presentation(OffsetSpace::DataSection);
        let directory = tempfile::tempdir().unwrap();
        let missing = Scene::demo();
        assert!(matches!(
            VisualPresentationBinding::bind(&missing, presentation.clone(), "root"),
            Err(VisualPresentationBindError::MissingVisualResource { .. })
        ));

        let mut wrong_hash = scene_with_resources(
            &directory.path().join("wrong.json"),
            json!([{"id": RESOURCE_ID, "sha256": "0".repeat(64)}]),
        );
        wrong_hash.meshes.push(exact_joint_mesh(
            wrong_hash.resources[0].id().clone(),
            MODEL_ROOT,
        ));
        assert!(matches!(
            VisualPresentationBinding::bind(&wrong_hash, presentation.clone(), "root"),
            Err(VisualPresentationBindError::HashMismatch { .. })
        ));

        let mut ambiguous = scene_with_resources(
            &directory.path().join("ambiguous.json"),
            json!([{"id": RESOURCE_ID, "sha256": presentation.resource().sha256}]),
        );
        ambiguous.resources.push(ambiguous.resources[0].clone());
        assert!(matches!(
            VisualPresentationBinding::bind(&ambiguous, presentation.clone(), "root"),
            Err(VisualPresentationBindError::AmbiguousVisualResource { .. })
        ));

        let legacy = scene_with_resources(
            &directory.path().join("legacy.json"),
            json!([{"id": RESOURCE_ID, "sha256": presentation.resource().sha256}]),
        );
        assert!(matches!(
            VisualPresentationBinding::bind(&legacy, presentation.clone(), "root"),
            Err(VisualPresentationBindError::NoExactOccurrences { .. })
        ));
        assert!(matches!(
            VisualPresentationBinding::bind(&legacy, presentation, "missing"),
            Err(VisualPresentationBindError::MissingHierarchy { .. })
        ));
    }

    #[test]
    fn malformed_visual_offset_and_conflicting_occurrences_are_explicit() {
        let presentation = one_joint_presentation(OffsetSpace::File);
        let directory = tempfile::tempdir().unwrap();
        let mut scene = scene_with_resources(
            &directory.path().join("scene.json"),
            json!([{"id": RESOURCE_ID, "sha256": presentation.resource().sha256}]),
        );
        scene
            .meshes
            .push(exact_joint_mesh(scene.resources[0].id().clone(), 4));
        assert!(matches!(
            VisualPresentationBinding::bind(&scene, presentation, "root"),
            Err(VisualPresentationBindError::InvalidVisualOffset {
                kind: SourceObjectKind::Joint,
                visual_offset: 4,
                ..
            })
        ));

        let target = SourceTarget::Joint("joint".into());
        let mut map = HashMap::new();
        let first = VisualJointOccurrence {
            resource_id: VisualResourceId::for_test(RESOURCE_ID),
            visual_offset: 32,
        };
        let second = VisualJointOccurrence {
            resource_id: VisualResourceId::for_test(RESOURCE_ID),
            visual_offset: 36,
        };
        insert_unique(
            &mut map,
            SourceJointId::from("joint"),
            first,
            target.clone(),
        )
        .unwrap();
        assert!(matches!(
            insert_unique(&mut map, SourceJointId::from("joint"), second, target,),
            Err(VisualPresentationBindError::ConflictingOccurrence { .. })
        ));
    }

    #[test]
    fn occurrence_identity_keeps_owner_and_ordinals_for_each_kind() {
        let presentation = one_joint_presentation(OffsetSpace::DataSection);
        let joint = VisualJointOccurrence {
            resource_id: VisualResourceId::for_test(RESOURCE_ID),
            visual_offset: MODEL_ROOT,
        };
        let dobj = VisualDObjOccurrence {
            owner_joint: joint,
            dobj_index: 7,
        };
        let material = VisualMaterialOccurrence {
            owner_dobj: dobj,
            visual_offset: super::super::scene::MaterialSourceId::new(0x64),
        };
        let texture = VisualTextureOccurrence {
            owner_material: material.clone(),
            tobj_index: 3,
            visual_offset: super::super::scene::TextureSourceId::new(0x70),
        };
        assert_eq!(
            material_identity(presentation.as_ref(), &material).unwrap(),
            SourceBindingIdentity {
                kind: SourceObjectKind::Material,
                descriptor_offset: DataOffset::new(0x64),
                owner_joint_offset: DataOffset::new(MODEL_ROOT),
                dobj_index: Some(7),
                texture_index: None,
            }
        );
        assert_eq!(
            texture_identity(presentation.as_ref(), &texture).unwrap().0,
            SourceBindingIdentity {
                kind: SourceObjectKind::Texture,
                descriptor_offset: DataOffset::new(0x70),
                owner_joint_offset: DataOffset::new(MODEL_ROOT),
                dobj_index: Some(7),
                texture_index: Some(3),
            }
        );
    }
}
