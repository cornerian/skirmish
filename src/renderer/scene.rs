//! CPU assets for the native visual export. This is a Lambert preview, not GX emulation.
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CullMode {
    None,
    Front,
    Back,
    All,
}

/// Source MObj descriptor identity within one `skirmish-visual-v1` resource.
///
/// The integer is an offset in the source archive's data section. It remains
/// distinct from joints and texture stages so presentation adapters cannot
/// accidentally target every material owned by one joint. Resource and runtime
/// instance scope must be supplied by the future manifest/presentation bridge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct MaterialSourceId(u32);

impl MaterialSourceId {
    pub const fn new(offset: u32) -> Self {
        Self(offset)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Source TObj descriptor identity for the first texture stage retained by the preview.
///
/// Like [`MaterialSourceId`], this offset is only unique within its visual
/// resource; preserving all ordered stages belongs in the future manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct TextureSourceId(u32);

impl TextureSourceId {
    pub const fn new(offset: u32) -> Self {
        Self(offset)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Fixed HSD draw pass encoded in the high render-mode bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RenderModeClass {
    Opaque,
    TextureEdge,
    Translucent,
}

/// Validated source MObj render mode.
///
/// All flags are retained, while the mutually exclusive pass bits are checked
/// once at load time. `0x2000_0000` is not a valid HSD pass class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct RenderMode(u32);

impl RenderMode {
    const CLASS_MASK: u32 = 0x6000_0000;
    const CHANNEL_MODE_MASK: u32 = 0x3;
    const ALPHA_MODE_SHIFT: u32 = 13;

    pub const fn from_bits(bits: u32) -> Option<Self> {
        if bits & Self::CLASS_MASK == 0x2000_0000 {
            None
        } else {
            Some(Self(bits))
        }
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn class(self) -> RenderModeClass {
        match self.0 & Self::CLASS_MASK {
            0 => RenderModeClass::Opaque,
            0x4000_0000 => RenderModeClass::TextureEdge,
            0x6000_0000 => RenderModeClass::Translucent,
            _ => unreachable!(),
        }
    }

    /// Whether the HSD diffuse channel reads its value from vertex color.
    pub const fn uses_vertex_color(self) -> bool {
        self.diffuse_mode() & 0x2 != 0
    }

    /// Whether the HSD alpha channel reads its value from vertex alpha.
    pub const fn uses_vertex_alpha(self) -> bool {
        let encoded = (self.0 >> Self::ALPHA_MODE_SHIFT) & Self::CHANNEL_MODE_MASK;
        let mode = if encoded == 0 {
            self.diffuse_mode()
        } else {
            encoded
        };
        mode & 0x2 != 0
    }

    const fn diffuse_mode(self) -> u32 {
        let encoded = self.0 & Self::CHANNEL_MODE_MASK;
        if encoded == 0 { 1 } else { encoded }
    }
}

#[derive(Clone, Debug)]
pub struct Material {
    /// Exact source MObj descriptor when exported; absent for legacy/procedural scenes.
    pub source_id: Option<MaterialSourceId>,
    /// Exact source TObj descriptor for the previewed first texture stage.
    pub texture_source_id: Option<TextureSourceId>,
    /// Fixed source pass and the remaining original MObj render flags.
    pub render_mode: Option<RenderMode>,
    pub color: [f32; 4],
    pub texture: Option<usize>,
    pub cull_mode: CullMode,
}

#[derive(Clone, Debug)]
pub struct Mesh {
    pub name: String,
    /// Source JObj descriptor offset that owns this draw part.
    pub joint: Option<u32>,
    /// Stable exporter identity for repeated instances of the same source part.
    pub instance_id: Option<String>,
    pub vertices: Vec<Vertex>,
    /// Counterclockwise front faces, converted from the source's winding at load time.
    pub indices: Vec<u32>,
    pub material: Material,
    pub hidden: bool,
}

/// Serialized source-joint state retained for future instance and animation evaluation.
///
/// Matrices use the exporter's column-major layout. They are immutable resource
/// metadata for now; the preview renderer does not apply them to draw calls yet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JointPose {
    pub flags: u32,
    pub local: [f32; 16],
    pub world: [f32; 16],
    pub inverse_bind: [f32; 16],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Joint {
    pub name: String,
    pub offset: u32,
    pub parent: Option<u32>,
    /// Present when the visual export includes its complete serialized pose tuple.
    pub pose: Option<JointPose>,
}

#[derive(Clone, Debug)]
pub struct Texture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Straight-alpha sRGB bytes in top-to-bottom row order.
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub eye: [f32; 3],
    pub interest: [f32; 3],
    pub up: [f32; 3],
    pub vertical_fov_radians: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub source: Option<String>,
    pub joints: Vec<Joint>,
    pub meshes: Vec<Mesh>,
    pub textures: Vec<Texture>,
    pub warnings: Vec<String>,
    /// Authored scene camera when one is available; otherwise the renderer frames bounds.
    pub camera: Option<Camera>,
    /// Linear RGBA clear color used before scene draws.
    pub clear_color: [f32; 4],
}

#[derive(Deserialize)]
struct Document {
    schema: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    source_winding: Option<String>,
    meshes: Vec<RawMesh>,
    #[serde(default)]
    joints: Vec<RawJoint>,
    #[serde(default)]
    textures: Vec<RawTexture>,
    #[serde(default)]
    limitations: Vec<String>,
}

#[derive(Deserialize)]
struct RawMesh {
    name: String,
    #[serde(default)]
    joint: Option<u32>,
    #[serde(default)]
    instance_id: Option<String>,
    positions: Vec<[f32; 3]>,
    #[serde(default)]
    normals: Option<Vec<[f32; 3]>>,
    #[serde(default)]
    uv0: Option<Vec<[f32; 2]>>,
    #[serde(default)]
    colors0: Option<Vec<[f32; 4]>>,
    indices: Vec<u32>,
    #[serde(default)]
    line_indices: Vec<u32>,
    #[serde(default)]
    point_indices: Vec<u32>,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    color: Option<[f32; 4]>,
    #[serde(default)]
    material: Value,
    #[serde(default, alias = "cull")]
    cull_mode: Value,
}

#[derive(Deserialize)]
struct RawJoint {
    name: String,
    offset: u32,
    #[serde(default)]
    parent: Option<u32>,
    #[serde(default)]
    flags: Option<u32>,
    #[serde(default)]
    local: Option<[f32; 16]>,
    #[serde(default)]
    world: Option<[f32; 16]>,
    #[serde(default)]
    inverse_bind: Option<[f32; 16]>,
}

#[derive(Deserialize)]
struct RawTexture {
    id: String,
    path: String,
    width: u32,
    height: u32,
}

impl Scene {
    /// Read the native JSON export and only the PNGs used by its first texture stages.
    /// Relative PNG paths resolve against the scene directory; absolute paths are
    /// used as written, matching the asset viewer's composed scene exports.
    pub fn load(path: &Path) -> Result<Self> {
        Self::load_with_joint_roots(path, None)
    }

    /// Read only draw parts descended from the requested source JObj roots.
    ///
    /// Melee archives contain many independently instantiated public roots. The
    /// original scene code chooses roots by symbol; rendering every archive root
    /// at once is not equivalent. Offsets are the stable source identities carried
    /// by `skirmish-visual-v1` until symbol names are exported alongside them.
    pub fn load_joint_roots(path: &Path, roots: &[u32]) -> Result<Self> {
        ensure!(
            !roots.is_empty(),
            "at least one source joint root is required"
        );
        Self::load_with_joint_roots(path, Some(roots))
    }

    fn load_with_joint_roots(path: &Path, roots: Option<&[u32]>) -> Result<Self> {
        const MAX_JSON_BYTES: u64 = 128 * 1024 * 1024;
        let mut text = String::new();
        File::open(path)
            .with_context(|| format!("open scene {}", path.display()))?
            .take(MAX_JSON_BYTES + 1)
            .read_to_string(&mut text)?;
        ensure!(
            text.len() as u64 <= MAX_JSON_BYTES,
            "scene JSON exceeds 128 MiB"
        );
        let mut document: Document =
            serde_json::from_str(&text).context("decode visual scene JSON")?;
        ensure!(
            document.schema == "skirmish-visual-v1",
            "unsupported scene schema: {}",
            document.schema
        );
        let clockwise = match document.source_winding.as_deref() {
            None | Some("cw") => true,
            Some("ccw") => false,
            Some(other) => bail!("unsupported source_winding: {other}"),
        };
        let mut joints: Vec<_> = document
            .joints
            .drain(..)
            .map(load_joint)
            .collect::<Result<_>>()?;
        let parents = validate_joint_tree(&joints)?;
        for mesh in &document.meshes {
            ensure!(
                mesh.joint.is_none_or(|joint| parents.contains_key(&joint)),
                "{} references missing source joint {:?}",
                mesh.name,
                mesh.joint
            );
        }
        if let Some(roots) = roots {
            for &root in roots {
                ensure!(
                    parents.contains_key(&root),
                    "missing source joint root {root}"
                );
            }
            let selected: HashSet<_> = parents
                .keys()
                .copied()
                .filter(|&joint| descends_from(joint, roots, &parents))
                .collect();
            joints.retain(|joint| selected.contains(&joint.offset));
            document
                .meshes
                .retain(|mesh| mesh.joint.is_some_and(|joint| selected.contains(&joint)));
        }
        let folder = path.parent().unwrap_or(Path::new("."));
        let mut table = HashMap::new();
        for texture in document.textures {
            ensure!(
                !table.contains_key(&texture.id),
                "duplicate texture id: {}",
                texture.id
            );
            table.insert(texture.id.clone(), texture);
        }
        let mut loaded = HashMap::new();
        let mut scene = Self {
            source: document.source,
            joints,
            meshes: Vec::new(),
            textures: Vec::new(),
            warnings: document.limitations,
            camera: None,
            clear_color: [0.018, 0.025, 0.045, 1.0],
        };
        scene.warnings.push("Textured Lambert preview: GX TEV operations, alpha tests, custom blending/depth state, source lighting, animation and skinning are not reproduced.".into());
        for mut raw in document.meshes {
            let count = raw.positions.len();
            validate_attribute(&raw.positions, count, &raw.name, "positions")?;
            ensure!(
                count <= u32::MAX as usize,
                "{}: too many vertices",
                raw.name
            );
            if let Some(values) = &raw.normals {
                validate_attribute(values, count, &raw.name, "normals")?;
            }
            if let Some(values) = &raw.uv0 {
                validate_attribute(values, count, &raw.name, "uv0")?;
            }
            if let Some(values) = &raw.colors0 {
                validate_attribute(values, count, &raw.name, "colors0")?;
            }
            ensure!(
                raw.indices.len().is_multiple_of(3),
                "{}: triangle index count is not divisible by three",
                raw.name
            );
            ensure!(
                raw.line_indices.len().is_multiple_of(2),
                "{}: line index count is not divisible by two",
                raw.name
            );
            ensure!(
                raw.indices
                    .iter()
                    .chain(&raw.line_indices)
                    .chain(&raw.point_indices)
                    .all(|&i| (i as usize) < count),
                "{}: index out of bounds",
                raw.name
            );
            if !raw.line_indices.is_empty() || !raw.point_indices.is_empty() {
                scene.warnings.push(format!(
                    "{}: line and point primitives are omitted.",
                    raw.name
                ));
            }
            if clockwise {
                for triangle in raw.indices.as_chunks_mut::<3>().0 {
                    triangle.swap(1, 2);
                }
            }
            ensure!(
                raw.material.is_null() || raw.material.is_object(),
                "{}: material must be an object",
                raw.name
            );
            let material_source_id = optional_u32_field(&raw.material, "material_offset")
                .with_context(|| format!("{}: invalid material_offset", raw.name))?
                .map(MaterialSourceId::new);
            let render_mode = optional_u32_field(&raw.material, "render_mode")
                .with_context(|| format!("{}: invalid render_mode", raw.name))?
                .map(|bits| {
                    RenderMode::from_bits(bits).with_context(|| {
                        format!(
                            "{}: render_mode {bits:#010x} uses reserved pass bits 0x20000000",
                            raw.name
                        )
                    })
                })
                .transpose()?;
            let mut color = raw.color.unwrap_or([1.; 4]);
            if let Some(diffuse) = raw.material.get("diffuse").filter(|v| !v.is_null()) {
                color = serde_json::from_value(diffuse.clone())
                    .with_context(|| format!("{}: invalid diffuse color", raw.name))?;
            }
            if let Some(alpha) = raw.material.get("material_alpha").filter(|v| !v.is_null()) {
                color[3] = serde_json::from_value(alpha.clone())
                    .with_context(|| format!("{}: invalid material alpha", raw.name))?;
            }
            ensure!(
                color.iter().all(|v| v.is_finite()),
                "{}: nonfinite material color",
                raw.name
            );
            let cull = if raw.cull_mode.is_null() {
                raw.material
                    .get("cull_mode")
                    .or_else(|| raw.material.get("cull"))
                    .unwrap_or(&Value::Null)
            } else {
                &raw.cull_mode
            };
            let cull_mode =
                cull_mode(cull).with_context(|| format!("{}: invalid cull mode", raw.name))?;
            let (texture, texture_source_id) = if let Some(stages) =
                raw.material.get("textures").filter(|v| !v.is_null())
            {
                let stages = stages
                    .as_array()
                    .context("material textures must be an array")?;
                if stages.len() > 1 {
                    scene.warnings.push(format!(
                        "{}: only the first of {} texture stages is sampled.",
                        raw.name,
                        stages.len()
                    ));
                }
                if let Some(stage) = stages.first() {
                    ensure!(
                        stage.is_object(),
                        "{}: texture stage must be an object",
                        raw.name
                    );
                    let texture_source_id = optional_u32_field(stage, "tobj_offset")
                        .with_context(|| format!("{}: invalid first-stage tobj_offset", raw.name))?
                        .map(TextureSourceId::new);
                    scene.warnings.push(format!("{}: first texture uses UV0, repeat wrapping and linear filtering; GX texture transforms, coordinate generation, LOD and texture operations are approximated.", raw.name));
                    if ["wrap_s", "wrap_t"].iter().any(|key| {
                        stage
                            .get(*key)
                            .and_then(Value::as_u64)
                            .is_some_and(|v| v != 1)
                    }) {
                        scene.warnings.push(format!(
                            "{}: source texture wrapping differs from preview repeat wrapping.",
                            raw.name
                        ));
                    }
                    let id = stage
                        .get("texture_id")
                        .and_then(Value::as_str)
                        .or_else(|| stage.get("texture_ids")?.get(0)?.as_str());
                    let texture = match id.and_then(|id| table.get(id)) {
                        Some(source) => {
                            let index = if let Some(&index) = loaded.get(&source.id) {
                                index
                            } else {
                                let index = scene.textures.len();
                                scene.textures.push(load_texture(folder, source)?);
                                loaded.insert(source.id.clone(), index);
                                index
                            };
                            Some(index)
                        }
                        None => {
                            scene.warnings.push(format!(
                                "{}: unresolved first texture {}; diffuse color is used.",
                                raw.name,
                                id.unwrap_or("(no ID)")
                            ));
                            None
                        }
                    };
                    (texture, texture_source_id)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };
            for warning in raw
                .material
                .get("warnings")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                scene.warnings.push(format!("{}: {warning}", raw.name));
            }
            let normals = raw.normals.unwrap_or_else(|| {
                scene.warnings.push(format!(
                    "{}: missing normals were generated from triangles.",
                    raw.name
                ));
                generated_normals(&raw.positions, &raw.indices)
            });
            let explicit_use_color = raw
                .material
                .get("uses_vertex_color")
                .and_then(Value::as_bool);
            let explicit_use_alpha = raw
                .material
                .get("uses_vertex_alpha")
                .and_then(Value::as_bool);
            let (use_color, use_alpha) = if let Some(mode) = render_mode {
                let use_color = mode.uses_vertex_color();
                let use_alpha = mode.uses_vertex_alpha();
                ensure!(
                    explicit_use_color.is_none_or(|explicit| explicit == use_color),
                    "{}: uses_vertex_color contradicts render_mode",
                    raw.name
                );
                ensure!(
                    explicit_use_alpha.is_none_or(|explicit| explicit == use_alpha),
                    "{}: uses_vertex_alpha contradicts render_mode",
                    raw.name
                );
                (use_color, use_alpha)
            } else {
                (
                    explicit_use_color.unwrap_or(true),
                    explicit_use_alpha.unwrap_or(true),
                )
            };
            // GX vertex channels replace their material channels; the renderer multiplies
            // these normalized factors, so an active source vertex channel needs identity.
            let material_uses_vertex_color = render_mode
                .map(RenderMode::uses_vertex_color)
                .unwrap_or(explicit_use_color == Some(true));
            let material_uses_vertex_alpha = render_mode
                .map(RenderMode::uses_vertex_alpha)
                .unwrap_or(explicit_use_alpha == Some(true));
            if material_uses_vertex_color {
                color[..3].fill(1.);
            }
            if material_uses_vertex_alpha {
                color[3] = 1.;
            }
            let vertices = raw
                .positions
                .iter()
                .enumerate()
                .map(|(i, &position)| {
                    let mut color = raw.colors0.as_ref().map_or([1.; 4], |values| values[i]);
                    if !use_color {
                        color[..3].fill(1.);
                    }
                    if !use_alpha {
                        color[3] = 1.;
                    }
                    Vertex {
                        position,
                        normal: unit_normal(normals[i].map(f64::from)),
                        uv: raw.uv0.as_ref().map_or([0.; 2], |values| values[i]),
                        color,
                    }
                })
                .collect();
            scene.meshes.push(Mesh {
                name: raw.name,
                joint: raw.joint,
                instance_id: raw.instance_id,
                vertices,
                indices: raw.indices,
                material: Material {
                    source_id: material_source_id,
                    texture_source_id,
                    render_mode,
                    color,
                    texture,
                    cull_mode,
                },
                hidden: raw.hidden,
            });
        }
        Ok(scene)
    }

    /// Visible triangle bounds in source coordinates, suitable for automatic camera framing.
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let mut bounds: Option<([f32; 3], [f32; 3])> = None;
        for mesh in self
            .meshes
            .iter()
            .filter(|m| !m.hidden && m.material.cull_mode != CullMode::All)
        {
            for &index in &mesh.indices {
                let p = mesh.vertices[index as usize].position;
                let (min, max) = bounds.get_or_insert((p, p));
                for axis in 0..3 {
                    min[axis] = min[axis].min(p[axis]);
                    max[axis] = max[axis].max(p[axis]);
                }
            }
        }
        bounds
    }

    /// A colored checker cube and floor authored here; no external game assets are needed.
    pub fn demo() -> Self {
        let mut cube = Mesh {
            name: "checker cube".into(),
            joint: None,
            instance_id: None,
            vertices: Vec::new(),
            indices: Vec::new(),
            material: Material {
                source_id: None,
                texture_source_id: None,
                render_mode: None,
                color: [1.; 4],
                texture: Some(0),
                cull_mode: CullMode::Back,
            },
            hidden: false,
        };
        let faces = [
            (
                [0., 0., 1.],
                [[-1., 0., 1.], [1., 0., 1.], [1., 2., 1.], [-1., 2., 1.]],
                [0.25, 0.8, 1., 1.],
            ),
            (
                [0., 0., -1.],
                [[1., 0., -1.], [-1., 0., -1.], [-1., 2., -1.], [1., 2., -1.]],
                [0.9, 0.4, 0.65, 1.],
            ),
            (
                [1., 0., 0.],
                [[1., 0., 1.], [1., 0., -1.], [1., 2., -1.], [1., 2., 1.]],
                [1., 0.6, 0.25, 1.],
            ),
            (
                [-1., 0., 0.],
                [[-1., 0., -1.], [-1., 0., 1.], [-1., 2., 1.], [-1., 2., -1.]],
                [0.4, 0.85, 0.5, 1.],
            ),
            (
                [0., 1., 0.],
                [[-1., 2., 1.], [1., 2., 1.], [1., 2., -1.], [-1., 2., -1.]],
                [1., 0.85, 0.4, 1.],
            ),
            (
                [0., -1., 0.],
                [[-1., 0., -1.], [1., 0., -1.], [1., 0., 1.], [-1., 0., 1.]],
                [0.7, 0.5, 0.9, 1.],
            ),
        ];
        for (normal, positions, color) in faces {
            add_quad(&mut cube, positions, normal, color, 2.);
        }
        let mut floor = Mesh {
            name: "floor".into(),
            joint: None,
            instance_id: None,
            vertices: Vec::new(),
            indices: Vec::new(),
            material: Material {
                source_id: None,
                texture_source_id: None,
                render_mode: None,
                color: [0.2, 0.24, 0.32, 1.],
                texture: Some(0),
                cull_mode: CullMode::Back,
            },
            hidden: false,
        };
        add_quad(
            &mut floor,
            [
                [-4., -0.02, 4.],
                [4., -0.02, 4.],
                [4., -0.02, -4.],
                [-4., -0.02, -4.],
            ],
            [0., 1., 0.],
            [1.; 4],
            8.,
        );
        let mut rgba = Vec::new();
        for y in 0..8 {
            for x in 0..8 {
                let value = if (x / 2 + y / 2) % 2 == 0 { 255 } else { 165 };
                rgba.extend([value, value, value, 255]);
            }
        }
        Self {
            source: None,
            joints: Vec::new(),
            meshes: vec![cube, floor],
            textures: vec![Texture {
                name: "procedural checker".into(),
                width: 8,
                height: 8,
                rgba,
            }],
            warnings: Vec::new(),
            camera: None,
            clear_color: [0.018, 0.025, 0.045, 1.0],
        }
    }
}

fn load_joint(raw: RawJoint) -> Result<Joint> {
    let pose = match (raw.flags, raw.local, raw.world, raw.inverse_bind) {
        (None, None, None, None) => None,
        (Some(flags), Some(local), Some(world), Some(inverse_bind)) => {
            for (label, matrix) in [
                ("local", &local),
                ("world", &world),
                ("inverse_bind", &inverse_bind),
            ] {
                ensure!(
                    matrix.iter().all(|value| value.is_finite()),
                    "{}: nonfinite {label} joint matrix",
                    raw.name
                );
            }
            Some(JointPose {
                flags,
                local,
                world,
                inverse_bind,
            })
        }
        _ => bail!(
            "{}: joint pose metadata must provide flags, local, world, and inverse_bind together",
            raw.name
        ),
    };
    Ok(Joint {
        name: raw.name,
        offset: raw.offset,
        parent: raw.parent,
        pose,
    })
}

fn validate_joint_tree(joints: &[Joint]) -> Result<HashMap<u32, Option<u32>>> {
    let mut parents = HashMap::with_capacity(joints.len());
    for joint in joints {
        ensure!(
            parents.insert(joint.offset, joint.parent).is_none(),
            "duplicate source joint offset {}",
            joint.offset
        );
    }
    for joint in joints {
        if let Some(parent) = joint.parent {
            ensure!(
                parents.contains_key(&parent),
                "{} references missing parent joint {parent}",
                joint.name
            );
        }
        let mut seen = HashSet::new();
        let mut current = Some(joint.offset);
        while let Some(offset) = current {
            ensure!(seen.insert(offset), "cycle at source joint {offset}");
            current = parents[&offset];
        }
    }
    Ok(parents)
}

fn descends_from(joint: u32, roots: &[u32], parents: &HashMap<u32, Option<u32>>) -> bool {
    let mut current = Some(joint);
    while let Some(offset) = current {
        if roots.contains(&offset) {
            return true;
        }
        current = parents[&offset];
    }
    false
}

fn validate_attribute<const N: usize>(
    values: &[[f32; N]],
    count: usize,
    name: &str,
    attribute: &str,
) -> Result<()> {
    ensure!(
        values.len() == count,
        "{name}: {attribute} count {} differs from position count {count}",
        values.len()
    );
    ensure!(
        values.iter().flatten().all(|v| v.is_finite()),
        "{name}: nonfinite {attribute}"
    );
    Ok(())
}

fn optional_u32_field(value: &Value, field: &str) -> Result<Option<u32>> {
    let Some(value) = value.get(field).filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let integer = value
        .as_u64()
        .with_context(|| format!("{field} must be an unsigned integer"))?;
    Ok(Some(
        u32::try_from(integer).with_context(|| format!("{field} exceeds 32 bits"))?,
    ))
}

fn cull_mode(value: &Value) -> Result<CullMode> {
    Ok(match (value.as_str(), value.as_u64()) {
        _ if value.is_null() => CullMode::None,
        (Some("none"), _) | (_, Some(0)) => CullMode::None,
        (Some("front"), _) | (_, Some(1)) => CullMode::Front,
        (Some("back"), _) | (_, Some(2)) => CullMode::Back,
        (Some("all"), _) | (_, Some(3)) => CullMode::All,
        _ => bail!("unknown value {value}"),
    })
}

fn unit_normal(value: [f64; 3]) -> [f32; 3] {
    let length = value.iter().map(|v| v * v).sum::<f64>().sqrt();
    if length > 0. {
        value.map(|v| (v / length) as f32)
    } else {
        [0., 1., 0.]
    }
}

fn generated_normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut normals = vec![[0_f64; 3]; positions.len()];
    for triangle in indices.as_chunks::<3>().0 {
        let [a, b, c] =
            [triangle[0], triangle[1], triangle[2]].map(|i| positions[i as usize].map(f64::from));
        let u = std::array::from_fn::<_, 3, _>(|i| b[i] - a[i]);
        let v = std::array::from_fn::<_, 3, _>(|i| c[i] - a[i]);
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        for &index in triangle {
            for (sum, value) in normals[index as usize].iter_mut().zip(n) {
                *sum += value;
            }
        }
    }
    normals.into_iter().map(unit_normal).collect()
}

fn load_texture(folder: &Path, raw: &RawTexture) -> Result<Texture> {
    let path = folder.join(&raw.path);
    let mut decoder = png::Decoder::new(BufReader::new(
        File::open(&path).with_context(|| format!("open texture {}", path.display()))?,
    ));
    decoder.set_limits(png::Limits {
        bytes: 128 * 1024 * 1024,
    });
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .with_context(|| format!("decode PNG {}", path.display()))?;
    let info = reader.info();
    ensure!(
        info.width == raw.width && info.height == raw.height,
        "{}: PNG dimensions differ from scene metadata",
        raw.id
    );
    ensure!(
        raw.width > 0
            && raw.height > 0
            && u64::from(raw.width) * u64::from(raw.height) <= 16 * 1024 * 1024,
        "{}: texture exceeds 16 million pixels",
        raw.id
    );
    let mut data = vec![
        0;
        reader
            .output_buffer_size()
            .context("PNG output size overflow")?
    ];
    let output = reader.next_frame(&mut data)?;
    ensure!(
        output.width == raw.width && output.height == raw.height,
        "{}: partial PNG animation frames are unsupported",
        raw.id
    );
    let data = &data[..output.buffer_size()];
    let mut rgba = Vec::with_capacity(raw.width as usize * raw.height as usize * 4);
    match output.color_type {
        png::ColorType::Rgba => rgba.extend_from_slice(data),
        png::ColorType::Rgb => {
            for p in data.as_chunks::<3>().0 {
                rgba.extend([p[0], p[1], p[2], 255]);
            }
        }
        png::ColorType::Grayscale => {
            for &v in data {
                rgba.extend([v, v, v, 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for p in data.as_chunks::<2>().0 {
                rgba.extend([p[0], p[0], p[0], p[1]]);
            }
        }
        png::ColorType::Indexed => bail!("{}: PNG palette was not expanded", raw.id),
    }
    ensure!(
        rgba.len() == raw.width as usize * raw.height as usize * 4,
        "{}: invalid decoded RGBA byte count",
        raw.id
    );
    Ok(Texture {
        name: raw.id.clone(),
        width: raw.width,
        height: raw.height,
        rgba,
    })
}

fn add_quad(
    mesh: &mut Mesh,
    positions: [[f32; 3]; 4],
    normal: [f32; 3],
    color: [f32; 4],
    repeats: f32,
) {
    let start = mesh.vertices.len() as u32;
    for (position, uv) in
        positions
            .into_iter()
            .zip([[0., repeats], [repeats, repeats], [repeats, 0.], [0., 0.]])
    {
        mesh.vertices.push(Vertex {
            position,
            normal,
            uv,
            color,
        });
    }
    mesh.indices
        .extend([start, start + 1, start + 2, start, start + 2, start + 3]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::fs;

    fn document(material: Value) -> Value {
        json!({
            "schema": "skirmish-visual-v1",
            "meshes": [{
                "name": "triangle",
                "positions": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                "indices": [0, 1, 2],
                "material": material,
            }],
        })
    }

    fn load_document(root: &Path, document: &Value) -> Result<Scene> {
        let path = root.join("scene.json");
        fs::write(&path, serde_json::to_vec(document)?)?;
        Scene::load(&path)
    }

    #[test]
    fn visual_source_material_texture_and_render_mode_metadata_is_retained() {
        let directory = tempfile::tempdir().unwrap();
        let material_offset = 0x6_a180;
        let texture_offset = 0x6_a110;
        let render_mode = 0x6000_0019;
        let scene = load_document(
            directory.path(),
            &document(json!({
                "material_offset": material_offset,
                "render_mode": render_mode,
                "textures": [{"tobj_offset": texture_offset}],
            })),
        )
        .unwrap();

        let material = &scene.meshes[0].material;
        assert_eq!(
            material.source_id,
            Some(MaterialSourceId::new(material_offset))
        );
        assert_eq!(
            material.texture_source_id,
            Some(TextureSourceId::new(texture_offset))
        );
        let mode = material.render_mode.unwrap();
        assert_eq!(mode.bits(), render_mode);
        assert_eq!(mode.class(), RenderModeClass::Translucent);
        assert_eq!(material.texture, None, "identity survives a missing PNG");

        let legacy = load_document(directory.path(), &document(json!({}))).unwrap();
        assert_eq!(legacy.meshes[0].material.source_id, None);
        assert_eq!(legacy.meshes[0].material.texture_source_id, None);
        assert_eq!(legacy.meshes[0].material.render_mode, None);
    }

    #[test]
    fn render_mode_is_canonical_for_initial_vertex_channel_ownership() {
        let directory = tempfile::tempdir().unwrap();
        let cases = [
            (
                "vertex color, material alpha",
                0x0000_2002,
                [1.0, 1.0, 1.0, 0.5],
                [0.6, 0.7, 0.8, 1.0],
            ),
            (
                "material color, vertex alpha",
                0x0000_4001,
                [0.2, 0.3, 0.4, 1.0],
                [1.0, 1.0, 1.0, 0.9],
            ),
            (
                "vertex alpha inherits vertex diffuse mode",
                0x0000_0002,
                [1.0; 4],
                [0.6, 0.7, 0.8, 0.9],
            ),
        ];

        for (label, render_mode, expected_material, expected_vertex) in cases {
            let mut source = document(json!({
                "render_mode": render_mode,
                "diffuse": [0.2, 0.3, 0.4, 1.0],
                "material_alpha": 0.5,
            }));
            source["meshes"][0]["colors0"] = json!([
                [0.6, 0.7, 0.8, 0.9],
                [0.6, 0.7, 0.8, 0.9],
                [0.6, 0.7, 0.8, 0.9],
            ]);

            let scene = load_document(directory.path(), &source).unwrap();
            assert_eq!(scene.meshes[0].material.color, expected_material, "{label}");
            assert_eq!(
                scene.meshes[0].vertices[0].color, expected_vertex,
                "{label}"
            );
        }
    }

    #[test]
    fn malformed_visual_source_identity_and_render_mode_metadata_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let cases = [
            (
                "material type",
                json!({"material_offset": "430464"}),
                "material_offset must be an unsigned integer",
            ),
            (
                "material width",
                json!({"material_offset": u64::from(u32::MAX) + 1}),
                "material_offset exceeds 32 bits",
            ),
            (
                "reserved pass",
                json!({"render_mode": 0x2000_0000_u32}),
                "reserved pass bits",
            ),
            (
                "vertex color contradiction",
                json!({"render_mode": 0x0000_0002_u32, "uses_vertex_color": false}),
                "uses_vertex_color contradicts render_mode",
            ),
            (
                "vertex alpha contradiction",
                json!({"render_mode": 0x0000_4001_u32, "uses_vertex_alpha": false}),
                "uses_vertex_alpha contradicts render_mode",
            ),
            (
                "texture type",
                json!({"textures": [{"tobj_offset": -1}]}),
                "tobj_offset must be an unsigned integer",
            ),
            (
                "texture stage type",
                json!({"textures": [7]}),
                "texture stage must be an object",
            ),
        ];

        for (label, material, expected) in cases {
            let error = load_document(directory.path(), &document(material)).unwrap_err();
            assert!(
                format!("{error:#}").contains(expected),
                "unexpected {label} error: {error:#}"
            );
        }
    }

    #[test]
    fn render_mode_vertex_channel_sources_follow_hsd_fallbacks() {
        let expected_vertex_alpha = [
            [false, false, true, true],
            [false, false, true, true],
            [true, false, true, true],
            [true, false, true, true],
        ];

        for diffuse in 0_u32..4 {
            for alpha in 0_u32..4 {
                let mode = RenderMode::from_bits(diffuse | (alpha << 13)).unwrap();
                assert_eq!(
                    mode.uses_vertex_color(),
                    diffuse & 0x2 != 0,
                    "diffuse mode {diffuse}, alpha mode {alpha}"
                );
                assert_eq!(
                    mode.uses_vertex_alpha(),
                    expected_vertex_alpha[diffuse as usize][alpha as usize],
                    "diffuse mode {diffuse}, alpha mode {alpha}"
                );
            }
        }
    }
}
