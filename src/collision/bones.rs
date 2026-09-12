//! Headless Euler bone poses and bone-attached collision endpoints.
//!
//! `srt` translates HSD_MtxSRT, including parent-scale compensation. `concat`,
//! `transform_point`, and `transform_vector` preserve the scalar Dolphin C matrix
//! routines' arithmetic grouping. libm supplies portable sine/cosine; equality
//! with another math library is numerical, not a bitwise guarantee.
//!
//! The hierarchy implements HSD_JObjMakeMatrix's Euler/local-parent subset and
//! JOBJ_CLASSICAL_SCALE. Quaternion joints, animation sampling, RObj/IK constraints,
//! dirty flags, independent matrices, and AObj translation overrides are unported.
//! Callers provide a sampled pose. Collision radii use a caller-provided scalar;
//! matrix-dependent hurt/shield radii use the original narrow phase in
//! [`super::shield`].

extern crate alloc;
use alloc::vec::Vec;
use core::cell::RefCell;

/// Reusable scratch storage for [`Pose::evaluate`]'s topological walk
/// (`scales`/`visited`/`path`) plus a pool of `world` buffers recycled when a
/// [`Pose`] is dropped. Every call still runs the full topological
/// resolution and produces exactly the same values as a fresh, unpooled
/// evaluation -- this only recycles backing allocations across calls on the
/// same thread, on a fixed-size (4-deep) best-effort pool; a deeper nesting
/// or a cold thread simply allocates normally, same as before this existed.
struct Scratch {
    scales: Vec<Option<Vector>>,
    visited: Vec<u8>,
    path: Vec<usize>,
}

const POOL_DEPTH: usize = 4;

std::thread_local! {
    static SCRATCH_POOL: RefCell<Vec<Scratch>> = const { RefCell::new(Vec::new()) };
    static WORLD_POOL: RefCell<Vec<Vec<Matrix>>> = const { RefCell::new(Vec::new()) };
}

fn take_scratch() -> Scratch {
    SCRATCH_POOL
        .with(|pool| pool.borrow_mut().pop())
        .unwrap_or_else(|| Scratch {
            scales: Vec::new(),
            visited: Vec::new(),
            path: Vec::new(),
        })
}

fn return_scratch(mut scratch: Scratch) {
    scratch.scales.clear();
    scratch.visited.clear();
    scratch.path.clear();
    SCRATCH_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        if pool.len() < POOL_DEPTH {
            pool.push(scratch);
        }
    });
}

fn take_world(len: usize) -> Vec<Matrix> {
    let mut world = WORLD_POOL
        .with(|pool| pool.borrow_mut().pop())
        .unwrap_or_default();
    world.clear();
    world.resize(len, IDENTITY);
    world
}

fn return_world(mut world: Vec<Matrix>) {
    world.clear();
    WORLD_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        if pool.len() < POOL_DEPTH {
            pool.push(world);
        }
    });
}

pub type Vector = [f32; 3];
pub type Matrix = [[f32; 4]; 3];
pub const IDENTITY: Matrix = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalTransform {
    pub translation: Vector,
    /// Euler X/Y/Z angles in radians, using HSD's formula.
    pub rotation: Vector,
    pub scale: Vector,
}

impl Default for LocalTransform {
    fn default() -> Self {
        Self {
            translation: [0.0; 3],
            rotation: [0.0; 3],
            scale: [1.0; 3],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bone {
    pub parent: Option<usize>,
    pub local: LocalTransform,
    /// HSD JOBJ_CLASSICAL_SCALE: omit this bone's scale from the compensation
    /// scale passed to its descendants. The local matrix still uses its scale.
    pub classical_scale: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    world: Vec<Matrix>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoneError {
    ParentOutOfRange { bone: usize, parent: usize },
    Cycle { bone: usize },
    NonFiniteTransform { bone: usize },
    SingularParentScale { bone: usize },
    BoneOutOfRange { bone: usize },
    InvalidRoot,
    InvalidCapsule,
}

impl core::fmt::Display for BoneError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "bone pose error: {self:?}")
    }
}
impl core::error::Error for BoneError {}

impl Pose {
    /// Resolve any acyclic parent ordering, without recursive traversal. Every
    /// call evaluates the supplied pose completely; there is no hidden frame
    /// cache of *results* -- but the `world`/`scales`/`visited`/`path`
    /// scratch buffers are recycled through a thread-local pool (see
    /// `take_world`/`take_scratch` above) instead of freshly allocated each
    /// call, so this produces bit-for-bit identical output to the original
    /// always-allocate version, just with fewer `malloc`/`free` round trips.
    pub fn evaluate(bones: &[Bone]) -> Result<Self, BoneError> {
        let mut world = take_world(bones.len());
        let mut scratch = take_scratch();
        scratch.scales.clear();
        scratch.scales.resize(bones.len(), None);
        scratch.visited.clear();
        scratch.visited.resize(bones.len(), 0);
        for (i, bone) in bones.iter().enumerate() {
            if let Some(parent) = bone.parent
                && parent >= bones.len()
            {
                return Err(BoneError::ParentOutOfRange { bone: i, parent });
            }
            if bone
                .local
                .translation
                .into_iter()
                .chain(bone.local.rotation)
                .chain(bone.local.scale)
                .any(|value| !value.is_finite())
            {
                return Err(BoneError::NonFiniteTransform { bone: i });
            }
        }
        for index in 0..bones.len() {
            let mut next = Some(index);
            while let Some(i) = next {
                match scratch.visited[i] {
                    2 => break,
                    1 => return Err(BoneError::Cycle { bone: i }),
                    _ => {
                        scratch.visited[i] = 1;
                        scratch.path.push(i);
                        next = bones[i].parent;
                    }
                }
            }
            while let Some(i) = scratch.path.pop() {
                let bone = bones[i];
                let parent_scale = bone.parent.and_then(|parent| scratch.scales[parent]);
                if parent_scale.is_some_and(|scale| scale.contains(&0.0)) {
                    return Err(BoneError::SingularParentScale { bone: i });
                }
                scratch.scales[i] = if bone.classical_scale {
                    parent_scale
                } else {
                    Some(core::array::from_fn(|axis| {
                        bone.local.scale[axis] * parent_scale.map_or(1.0, |scale| scale[axis])
                    }))
                };
                let local = srt(bone.local, parent_scale);
                world[i] = bone
                    .parent
                    .map_or(local, |parent| concat(&world[parent], &local));
                if !finite(&world[i])
                    || scratch.scales[i].is_some_and(|scale| scale.iter().any(|x| !x.is_finite()))
                {
                    return Err(BoneError::NonFiniteTransform { bone: i });
                }
                scratch.visited[i] = 2;
            }
        }
        return_scratch(scratch);
        Ok(Self { world })
    }

    /// Apply external fighter position/facing after resolving the bone hierarchy.
    /// This root transform is not part of HSD's cumulative scale bookkeeping.
    pub fn evaluate_with_root(bones: &[Bone], root: &Matrix) -> Result<Self, BoneError> {
        if !finite(root) {
            return Err(BoneError::InvalidRoot);
        }
        let mut pose = Self::evaluate(bones)?;
        for (bone, matrix) in pose.world.iter_mut().enumerate() {
            *matrix = concat(root, matrix);
            if !finite(matrix) {
                return Err(BoneError::NonFiniteTransform { bone });
            }
        }
        Ok(pose)
    }

    pub fn world_matrices(&self) -> &[Matrix] {
        &self.world
    }

    pub fn world_matrix(&self, bone: usize) -> Result<&Matrix, BoneError> {
        self.world
            .get(bone)
            .ok_or(BoneError::BoneOutOfRange { bone })
    }
}

impl Drop for Pose {
    /// Return this pose's `world` buffer to the thread-local pool instead of
    /// freeing it, so the next `Pose::evaluate` on this thread can reuse the
    /// allocation. Purely a memory-reuse optimization: nothing observable
    /// about `Pose` depends on `Drop` running (no other type holds a
    /// reference into `world` past this point).
    fn drop(&mut self) {
        return_world(core::mem::take(&mut self.world));
    }
}

/// Original HSD_MtxSRT. The optional scale is the parent's accumulated HSD scale.
/// Like C, this primitive retains IEEE behavior; Pose validates finite results.
pub fn srt(local: LocalTransform, parent_scale: Option<Vector>) -> Matrix {
    let [sx, sy, sz] = local.scale;
    let [rx, ry, rz] = local.rotation;
    let (sin_x, cos_x) = (crate::math::sinf(rx), crate::math::cosf(rx));
    let (sin_y, cos_y) = (crate::math::sinf(ry), crate::math::cosf(ry));
    let (sin_z, cos_z) = (crate::math::sinf(rz), crate::math::cosf(rz));
    let (x0, mut x1, mut x2) = (sx, sx, sx);
    let (mut y0, y1, mut y2) = (sy, sy, sy);
    let (mut z0, mut z1, z2) = (sz, sz, sz);
    if let Some([px, py, pz]) = parent_scale {
        // The original unsuffixed 1.0 literal performs double division first.
        let inv_x = (1.0_f64 / f64::from(px)) as f32;
        let inv_y = (1.0_f64 / f64::from(py)) as f32;
        let inv_z = (1.0_f64 / f64::from(pz)) as f32;
        y0 *= py * inv_x;
        z0 *= pz * inv_x;
        x1 *= px * inv_y;
        z1 *= pz * inv_y;
        x2 *= px * inv_z;
        y2 *= py * inv_z;
    }
    [
        [
            cos_z * (x0 * cos_y),
            y0 * ((cos_z * (sin_x * sin_y)) - (cos_x * sin_z)),
            z0 * ((cos_z * (cos_x * sin_y)) + (sin_x * sin_z)),
            local.translation[0],
        ],
        [
            sin_z * (x1 * cos_y),
            y1 * ((sin_z * (sin_x * sin_y)) + (cos_x * cos_z)),
            z1 * ((sin_z * (cos_x * sin_y)) - (sin_x * cos_z)),
            local.translation[1],
        ],
        [
            -x2 * sin_y,
            cos_y * (y2 * sin_x),
            cos_y * (z2 * cos_x),
            local.translation[2],
        ],
    ]
}

/// Original scalar C_MTXConcat: parent matrix multiplied by local matrix.
pub fn concat(a: &Matrix, b: &Matrix) -> Matrix {
    core::array::from_fn(|row| {
        core::array::from_fn(|column| {
            let xy = a[row][0] * b[0][column] + a[row][1] * b[1][column];
            let z = a[row][2] * b[2][column];
            if column == 3 {
                a[row][3] + (z + xy)
            } else {
                (0.0 + z) + xy
            }
        })
    })
}

/// Original scalar C_MTXMultVec, used for bone-local hit/hurt endpoints.
pub fn transform_point(matrix: &Matrix, point: Vector) -> Vector {
    core::array::from_fn(|row| {
        matrix[row][3]
            + (matrix[row][2] * point[2] + (matrix[row][0] * point[0] + matrix[row][1] * point[1]))
    })
}

/// Original scalar C_MTXMultVecSR; translation is deliberately omitted.
pub fn transform_vector(matrix: &Matrix, vector: Vector) -> Vector {
    core::array::from_fn(|row| {
        matrix[row][2] * vector[2] + (matrix[row][0] * vector[0] + matrix[row][1] * vector[1])
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoneCapsule {
    pub bone: usize,
    pub start: Vector,
    pub end: Vector,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldCapsule {
    pub start: Vector,
    pub end: Vector,
    pub radius: f32,
}

impl BoneCapsule {
    /// A spherical hitbox is the zero-length capsule case.
    pub fn sphere(bone: usize, center: Vector, radius: f32) -> Self {
        Self {
            bone,
            start: center,
            end: center,
            radius,
        }
    }

    /// Transform both bone-local endpoints, as the hurt-capsule update does.
    /// Radius scaling is explicit: HSD joint nonuniform scale is not guessed to
    /// be an isotropic radius. Use 1 for an already specified world-space radius.
    pub fn transform(&self, pose: &Pose, radius_scale: f32) -> Result<WorldCapsule, BoneError> {
        if !self.radius.is_finite()
            || self.radius < 0.0
            || !radius_scale.is_finite()
            || radius_scale < 0.0
            || self
                .start
                .into_iter()
                .chain(self.end)
                .any(|x| !x.is_finite())
        {
            return Err(BoneError::InvalidCapsule);
        }
        let matrix = pose.world_matrix(self.bone)?;
        let result = WorldCapsule {
            start: transform_point(matrix, self.start),
            end: transform_point(matrix, self.end),
            radius: self.radius * radius_scale,
        };
        if result
            .start
            .into_iter()
            .chain(result.end)
            .chain([result.radius])
            .any(|x| !x.is_finite())
        {
            return Err(BoneError::InvalidCapsule);
        }
        Ok(result)
    }
}

fn finite(matrix: &Matrix) -> bool {
    matrix.iter().flatten().all(|x| x.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchy_supports_out_of_order_parents_and_external_facing() {
        let bones = [
            Bone {
                parent: Some(1),
                local: LocalTransform {
                    translation: [2.0, 0.0, 0.0],
                    ..LocalTransform::default()
                },
                ..Bone::default()
            },
            Bone {
                local: LocalTransform {
                    translation: [3.0, 4.0, 0.0],
                    ..LocalTransform::default()
                },
                ..Bone::default()
            },
        ];
        let root = [
            [-1.0, 0.0, 0.0, 10.0],
            [0.0, 1.0, 0.0, 20.0],
            [0.0, 0.0, 1.0, 0.0],
        ];
        let pose = Pose::evaluate_with_root(&bones, &root).unwrap();
        assert_eq!(
            transform_point(pose.world_matrix(0).unwrap(), [0.0; 3]),
            [5.0, 24.0, 0.0]
        );
        let hit = BoneCapsule::sphere(0, [1.0, 0.0, 0.0], 0.5)
            .transform(&pose, 2.0)
            .unwrap();
        assert_eq!(
            hit,
            WorldCapsule {
                start: [4.0, 24.0, 0.0],
                end: [4.0, 24.0, 0.0],
                radius: 1.0
            }
        );
    }

    #[test]
    fn parent_scale_compensation_differs_from_classical_scaling() {
        let mut bones = [
            Bone {
                local: LocalTransform {
                    scale: [2.0, 3.0, 4.0],
                    ..LocalTransform::default()
                },
                ..Bone::default()
            },
            Bone {
                parent: Some(0),
                local: LocalTransform {
                    rotation: [0.0, 0.0, core::f32::consts::FRAC_PI_2],
                    ..LocalTransform::default()
                },
                ..Bone::default()
            },
        ];
        let hsd = Pose::evaluate(&bones).unwrap();
        bones[0].classical_scale = true;
        let classical = Pose::evaluate(&bones).unwrap();
        let hsd_x = transform_vector(hsd.world_matrix(1).unwrap(), [1.0, 0.0, 0.0]);
        let classical_x = transform_vector(classical.world_matrix(1).unwrap(), [1.0, 0.0, 0.0]);
        assert!((hsd_x[1] - 2.0).abs() < 0.000001);
        assert!((classical_x[1] - 3.0).abs() < 0.000001);
    }

    #[test]
    fn invalid_hierarchies_and_capsules_are_errors() {
        assert!(matches!(
            Pose::evaluate(&[Bone {
                parent: Some(0),
                ..Bone::default()
            }]),
            Err(BoneError::Cycle { .. })
        ));
        assert!(matches!(
            Pose::evaluate(&[Bone {
                parent: Some(7),
                ..Bone::default()
            }]),
            Err(BoneError::ParentOutOfRange { .. })
        ));
        let zero_scale = [
            Bone {
                local: LocalTransform {
                    scale: [0.0; 3],
                    ..LocalTransform::default()
                },
                ..Bone::default()
            },
            Bone {
                parent: Some(0),
                ..Bone::default()
            },
        ];
        assert!(matches!(
            Pose::evaluate(&zero_scale),
            Err(BoneError::SingularParentScale { .. })
        ));
        let pose = Pose::evaluate(&[Bone::default()]).unwrap();
        assert_eq!(
            BoneCapsule::sphere(0, [0.0; 3], -1.0).transform(&pose, 1.0),
            Err(BoneError::InvalidCapsule)
        );
        assert!(matches!(
            pose.world_matrix(8),
            Err(BoneError::BoneOutOfRange { .. })
        ));
    }
}
