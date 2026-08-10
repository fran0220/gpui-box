//! A bounded reader for a stated subset of glTF 2.0.
//!
//! A 3D document is the widest input this library takes: it is a container, a
//! JSON document, an index space, and a byte buffer, and every one of those is
//! a place where a file can ask a reader to allocate more than the reader has
//! or to read outside what it was given. So this reader is written the other
//! way round from a loader: it states what it accepts, refuses everything
//! else, and checks every bound *before* allocating for it.
//!
//! # What it accepts
//!
//! - Both containers: a `.gltf` JSON document, and a `.glb` binary container
//!   whose header declares version 2 and whose chunk lengths stay inside the
//!   bytes handed in.
//! - Buffers that are inside the file: the GLB binary chunk, and `data:` URIs
//!   with a `;base64,` payload. **Any other URI is refused**, because
//!   resolving one is I/O and this crate performs none — the same rule that
//!   makes `Markdown` name an image rather than fetch it.
//! - Triangle primitives with a `POSITION` accessor of `VEC3` `FLOAT`, with
//!   or without indices; indices may be unsigned byte, short, or int.
//! - Node hierarchy with either a 4×4 `matrix` or `translation`/`rotation`/
//!   `scale`, to a bounded depth.
//!
//! # What it refuses, rather than approximates
//!
//! Materials, textures, cameras, lights, animation, skins, morph targets,
//! sparse accessors, non-triangle primitives, and the `KHR_draco` family are
//! not read. Where ignoring one would change what the reader draws it is a
//! [`ModelDefect`]; where it only changes how the geometry is *shaded* it is
//! ignored, and [`ModelViewer`](crate::media::ModelViewer) draws untextured
//! geometry rather than pretending to a material it did not read.
//!
//! # Fail closed
//!
//! [`ModelBounds`] caps bytes, nodes, hierarchy depth, primitives, vertices,
//! and triangles. Every cap is checked while reading, so a document that
//! declares ten million vertices is refused at the accessor that says so
//! rather than after the allocation. A refusal names the limit, what the file
//! asked for, and what was allowed, so the caller can raise a bound on
//! purpose instead of guessing.

use gpui::SharedString;
use serde::Deserialize;

/// A magic number, a version, and the two chunk types of the GLB container.
const GLB_MAGIC: &[u8; 4] = b"glTF";
const GLB_HEADER: usize = 12;
const GLB_CHUNK_HEADER: usize = 8;
const GLB_CHUNK_JSON: u32 = 0x4E4F_534A;
const GLB_CHUNK_BIN: u32 = 0x004E_4942;

/// The glTF component types this reader knows.
const COMPONENT_UNSIGNED_BYTE: u32 = 5121;
const COMPONENT_UNSIGNED_SHORT: u32 = 5123;
const COMPONENT_UNSIGNED_INT: u32 = 5125;
const COMPONENT_FLOAT: u32 = 5126;

/// The only primitive mode that is a surface. 4 is `TRIANGLES`.
const MODE_TRIANGLES: u32 = 4;

/// Which cap a document ran into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelLimit {
    Bytes,
    Nodes,
    Depth,
    Primitives,
    Vertices,
    Triangles,
}

impl ModelLimit {
    /// The name a refusal publishes and a caller matches on.
    pub fn name(self) -> &'static str {
        match self {
            Self::Bytes => "bytes",
            Self::Nodes => "nodes",
            Self::Depth => "depth",
            Self::Primitives => "primitives",
            Self::Vertices => "vertices",
            Self::Triangles => "triangles",
        }
    }
}

/// Why a document is not one this reader will draw.
///
/// Each variant is a code rather than a sentence, because the sentence a
/// reader sees is the host's to write and this crate holds no English outside
/// [`crate::strings`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelDefect {
    /// The bytes are not JSON, or not a JSON object.
    NotJson,
    /// The GLB header, its declared length, or a chunk length is wrong.
    BadContainer,
    /// There is no `asset.version`, so this is not a glTF document.
    NotGltf,
    /// The document declares a glTF major version other than 2.
    UnsupportedVersion,
    /// A buffer points somewhere this crate would have to fetch.
    ExternalResource,
    /// A `data:` URI that is not base64, or base64 that will not decode.
    UnsupportedEncoding,
    /// A sparse accessor, which substitutes values this reader does not read.
    SparseAccessor,
    /// A primitive that is not a triangle list.
    UnsupportedPrimitive,
    /// An accessor component type this reader does not read.
    UnsupportedComponentType,
    /// An accessor whose element type is not the one the attribute requires.
    UnsupportedAccessorType,
    /// A primitive with no `POSITION`, which has no geometry to draw.
    MissingPositions,
    /// An index into accessors, buffer views, buffers, meshes, or nodes that
    /// the document does not contain.
    DanglingIndex,
    /// An accessor that reads past the end of the bytes it was given.
    TruncatedBuffer,
    /// An index that names a vertex the primitive does not have.
    IndexOutOfRange,
    /// A document that parsed and contains no triangle to draw.
    EmptyScene,
}

impl ModelDefect {
    /// The code a refusal publishes.
    pub fn name(self) -> &'static str {
        match self {
            Self::NotJson => "not-json",
            Self::BadContainer => "bad-container",
            Self::NotGltf => "not-gltf",
            Self::UnsupportedVersion => "unsupported-version",
            Self::ExternalResource => "external-resource",
            Self::UnsupportedEncoding => "unsupported-encoding",
            Self::SparseAccessor => "sparse-accessor",
            Self::UnsupportedPrimitive => "unsupported-primitive",
            Self::UnsupportedComponentType => "unsupported-component-type",
            Self::UnsupportedAccessorType => "unsupported-accessor-type",
            Self::MissingPositions => "missing-positions",
            Self::DanglingIndex => "dangling-index",
            Self::TruncatedBuffer => "truncated-buffer",
            Self::IndexOutOfRange => "index-out-of-range",
            Self::EmptyScene => "empty-scene",
        }
    }
}

/// Why a document was not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelError {
    /// The document asked for more than the caller allowed.
    TooLarge {
        limit: ModelLimit,
        /// What the document asked for.
        found: usize,
        /// What the caller allowed.
        allowed: usize,
    },
    /// The document is outside the subset this reader accepts.
    Rejected(ModelDefect),
}

impl ModelError {
    /// The name a semantic node publishes for the refusal itself.
    pub fn name(self) -> &'static str {
        match self {
            Self::TooLarge { .. } => "too-large",
            Self::Rejected(_) => "rejected",
        }
    }

    /// The code inside the refusal: a limit name or a defect code.
    pub fn code(self) -> &'static str {
        match self {
            Self::TooLarge { limit, .. } => limit.name(),
            Self::Rejected(defect) => defect.name(),
        }
    }
}

/// How much of a document this reader will take.
///
/// The defaults are chosen for what the viewer can draw at an interactive
/// frame rate on a laptop rather than for what a format allows: a model that
/// would take a second to paint is refused with a number the caller can raise
/// on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelBounds {
    pub max_bytes: usize,
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_primitives: usize,
    pub max_vertices: usize,
    pub max_triangles: usize,
}

impl Default for ModelBounds {
    fn default() -> Self {
        Self {
            max_bytes: 8 * 1024 * 1024,
            max_nodes: 1024,
            max_depth: 16,
            max_primitives: 128,
            max_vertices: 65_536,
            max_triangles: 24_576,
        }
    }
}

impl ModelBounds {
    pub fn max_bytes(mut self, bytes: usize) -> Self {
        self.max_bytes = bytes;
        self
    }

    pub fn max_nodes(mut self, nodes: usize) -> Self {
        self.max_nodes = nodes;
        self
    }

    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    pub fn max_primitives(mut self, primitives: usize) -> Self {
        self.max_primitives = primitives;
        self
    }

    pub fn max_vertices(mut self, vertices: usize) -> Self {
        self.max_vertices = vertices;
        self
    }

    pub fn max_triangles(mut self, triangles: usize) -> Self {
        self.max_triangles = triangles;
        self
    }

    fn check(self, limit: ModelLimit, found: usize) -> Result<(), ModelError> {
        let allowed = match limit {
            ModelLimit::Bytes => self.max_bytes,
            ModelLimit::Nodes => self.max_nodes,
            ModelLimit::Depth => self.max_depth,
            ModelLimit::Primitives => self.max_primitives,
            ModelLimit::Vertices => self.max_vertices,
            ModelLimit::Triangles => self.max_triangles,
        };
        if found > allowed {
            return Err(ModelError::TooLarge {
                limit,
                found,
                allowed,
            });
        }
        Ok(())
    }
}

/// An axis-aligned box around everything the reader accepted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelAabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl ModelAabb {
    pub fn centre(self) -> [f32; 3] {
        [
            (self.min[0] + self.max[0]) / 2.0,
            (self.min[1] + self.max[1]) / 2.0,
            (self.min[2] + self.max[2]) / 2.0,
        ]
    }

    /// The radius of the sphere around the box, which is what a camera fits
    /// against so that orbiting does not change the size of the model.
    pub fn radius(self) -> f32 {
        let half = [
            (self.max[0] - self.min[0]) / 2.0,
            (self.max[1] - self.min[1]) / 2.0,
            (self.max[2] - self.min[2]) / 2.0,
        ];
        (half[0] * half[0] + half[1] * half[1] + half[2] * half[2]).sqrt()
    }
}

/// One primitive's triangles, already in the document's world space.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelMesh {
    name: SharedString,
    positions: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

impl ModelMesh {
    pub fn name(&self) -> &SharedString {
        &self.name
    }

    pub fn positions(&self) -> &[[f32; 3]] {
        &self.positions
    }

    /// Three indices per triangle, into [`positions`](Self::positions).
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// Everything the reader accepted out of one document.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelScene {
    meshes: Vec<ModelMesh>,
    vertices: usize,
    triangles: usize,
    aabb: ModelAabb,
}

impl ModelScene {
    /// Reads a glTF 2.0 document, refusing anything outside the accepted
    /// subset and anything past `bounds`.
    pub fn parse(bytes: &[u8], bounds: ModelBounds) -> Result<Self, ModelError> {
        bounds.check(ModelLimit::Bytes, bytes.len())?;
        let (json, binary) = split_container(bytes)?;
        let document: Document =
            serde_json::from_slice(json).map_err(|_| ModelError::Rejected(ModelDefect::NotJson))?;
        read(&document, binary, bounds)
    }

    pub fn meshes(&self) -> &[ModelMesh] {
        &self.meshes
    }

    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices
    }

    pub fn triangle_count(&self) -> usize {
        self.triangles
    }

    pub fn aabb(&self) -> ModelAabb {
        self.aabb
    }
}

// ---------------------------------------------------------------------------
// The container
// ---------------------------------------------------------------------------

/// Splits the bytes into the JSON document and the binary chunk behind it.
///
/// A GLB whose declared total length, chunk length, or padding runs past what
/// was handed in is refused rather than clamped: a length that disagrees with
/// the file is exactly the shape of a document written to be read wrongly.
fn split_container(bytes: &[u8]) -> Result<(&[u8], &[u8]), ModelError> {
    if bytes.len() < 4 || &bytes[..4] != GLB_MAGIC {
        return Ok((bytes, &[]));
    }
    let bad = ModelError::Rejected(ModelDefect::BadContainer);
    if bytes.len() < GLB_HEADER {
        return Err(bad);
    }
    if read_u32(bytes, 4).ok_or(bad)? != 2 {
        return Err(ModelError::Rejected(ModelDefect::UnsupportedVersion));
    }
    let declared = read_u32(bytes, 8).ok_or(bad)? as usize;
    if declared > bytes.len() || declared < GLB_HEADER {
        return Err(bad);
    }

    let mut json: Option<&[u8]> = None;
    let mut binary: &[u8] = &[];
    let mut at = GLB_HEADER;
    while at + GLB_CHUNK_HEADER <= declared {
        let length = read_u32(bytes, at).ok_or(bad)? as usize;
        let kind = read_u32(bytes, at + 4).ok_or(bad)?;
        let start = at + GLB_CHUNK_HEADER;
        let end = start.checked_add(length).ok_or(bad)?;
        if end > declared {
            return Err(bad);
        }
        match kind {
            GLB_CHUNK_JSON if json.is_none() => json = Some(&bytes[start..end]),
            GLB_CHUNK_BIN if binary.is_empty() => binary = &bytes[start..end],
            // An unknown chunk type is skipped by the specification, which is
            // safe here because its length has already been bounded.
            _ => {}
        }
        // Chunks are padded to four bytes.
        at = end + (4 - end % 4) % 4;
    }
    Ok((json.ok_or(bad)?, binary))
}

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    let slice = bytes.get(at..at + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

// ---------------------------------------------------------------------------
// The document
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Document {
    #[serde(default)]
    asset: Asset,
    #[serde(default)]
    scene: Option<usize>,
    #[serde(default)]
    scenes: Vec<SceneNode>,
    #[serde(default)]
    nodes: Vec<Node>,
    #[serde(default)]
    meshes: Vec<Mesh>,
    #[serde(default)]
    accessors: Vec<Accessor>,
    #[serde(default)]
    buffer_views: Vec<BufferView>,
    #[serde(default)]
    buffers: Vec<Buffer>,
}

#[derive(Debug, Default, Deserialize)]
struct Asset {
    #[serde(default)]
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SceneNode {
    #[serde(default)]
    nodes: Vec<usize>,
}

#[derive(Debug, Deserialize)]
struct Node {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    mesh: Option<usize>,
    #[serde(default)]
    children: Vec<usize>,
    #[serde(default)]
    matrix: Option<[f32; 16]>,
    #[serde(default)]
    translation: Option<[f32; 3]>,
    #[serde(default)]
    rotation: Option<[f32; 4]>,
    #[serde(default)]
    scale: Option<[f32; 3]>,
}

#[derive(Debug, Deserialize)]
struct Mesh {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    primitives: Vec<Primitive>,
}

#[derive(Debug, Deserialize)]
struct Primitive {
    #[serde(default)]
    attributes: Attributes,
    #[serde(default)]
    indices: Option<usize>,
    #[serde(default)]
    mode: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
struct Attributes {
    #[serde(rename = "POSITION", default)]
    position: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Accessor {
    #[serde(default)]
    buffer_view: Option<usize>,
    #[serde(default)]
    byte_offset: usize,
    component_type: u32,
    count: usize,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    sparse: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BufferView {
    buffer: usize,
    #[serde(default)]
    byte_offset: usize,
    byte_length: usize,
    #[serde(default)]
    byte_stride: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct Buffer {
    #[serde(default)]
    uri: Option<String>,
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// A 4×4 transform in glTF's column-major order.
type Mat4 = [f32; 16];

const IDENTITY: Mat4 = [
    1.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    0.0, 0.0, 0.0, 1.0,
];

fn multiply(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [0.0; 16];
    for column in 0..4 {
        for row in 0..4 {
            let mut sum = 0.0;
            for step in 0..4 {
                sum += a[step * 4 + row] * b[column * 4 + step];
            }
            out[column * 4 + row] = sum;
        }
    }
    out
}

fn transform(matrix: &Mat4, point: [f32; 3]) -> [f32; 3] {
    let [x, y, z] = point;
    [
        matrix[0] * x + matrix[4] * y + matrix[8] * z + matrix[12],
        matrix[1] * x + matrix[5] * y + matrix[9] * z + matrix[13],
        matrix[2] * x + matrix[6] * y + matrix[10] * z + matrix[14],
    ]
}

/// The node's own transform: an explicit matrix, or the composed T·R·S.
fn local(node: &Node) -> Mat4 {
    if let Some(matrix) = node.matrix {
        return matrix;
    }
    let [tx, ty, tz] = node.translation.unwrap_or([0.0; 3]);
    let [sx, sy, sz] = node.scale.unwrap_or([1.0; 3]);
    let [x, y, z, w] = node.rotation.unwrap_or([0.0, 0.0, 0.0, 1.0]);
    // The quaternion is normalized here rather than trusted: an unnormalized
    // one in the file would otherwise scale the model as well as turn it.
    let length = (x * x + y * y + z * z + w * w).sqrt();
    let (x, y, z, w) = if length > f32::EPSILON {
        (x / length, y / length, z / length, w / length)
    } else {
        (0.0, 0.0, 0.0, 1.0)
    };
    let (xx, yy, zz) = (x * x, y * y, z * z);
    let (xy, xz, yz) = (x * y, x * z, y * z);
    let (wx, wy, wz) = (w * x, w * y, w * z);
    [
        (1.0 - 2.0 * (yy + zz)) * sx,
        (2.0 * (xy + wz)) * sx,
        (2.0 * (xz - wy)) * sx,
        0.0,
        (2.0 * (xy - wz)) * sy,
        (1.0 - 2.0 * (xx + zz)) * sy,
        (2.0 * (yz + wx)) * sy,
        0.0,
        (2.0 * (xz + wy)) * sz,
        (2.0 * (yz - wx)) * sz,
        (1.0 - 2.0 * (xx + yy)) * sz,
        0.0,
        tx,
        ty,
        tz,
        1.0,
    ]
}

/// What one traversal accumulates, so every cap is checked as it grows.
struct Reader<'a> {
    document: &'a Document,
    buffers: Vec<Vec<u8>>,
    binary: &'a [u8],
    bounds: ModelBounds,
    meshes: Vec<ModelMesh>,
    vertices: usize,
    triangles: usize,
    visited: usize,
    min: [f32; 3],
    max: [f32; 3],
}

fn read(document: &Document, binary: &[u8], bounds: ModelBounds) -> Result<ModelScene, ModelError> {
    let version = document
        .asset
        .version
        .as_deref()
        .ok_or(ModelError::Rejected(ModelDefect::NotGltf))?;
    if !version.starts_with('2') {
        return Err(ModelError::Rejected(ModelDefect::UnsupportedVersion));
    }
    bounds.check(ModelLimit::Nodes, document.nodes.len())?;

    let mut buffers = Vec::with_capacity(document.buffers.len());
    for buffer in &document.buffers {
        buffers.push(resolve(buffer, binary, bounds)?);
    }

    let mut reader = Reader {
        document,
        buffers,
        binary,
        bounds,
        meshes: Vec::new(),
        vertices: 0,
        triangles: 0,
        visited: 0,
        min: [f32::INFINITY; 3],
        max: [f32::NEG_INFINITY; 3],
    };

    let roots: Vec<usize> = match document.scenes.get(document.scene.unwrap_or(0)) {
        Some(scene) => scene.nodes.clone(),
        // A document with no scene list still has nodes, and drawing all of
        // them is what every reader does with one.
        None => (0..document.nodes.len()).collect(),
    };
    for root in roots {
        reader.walk(root, &IDENTITY, 0)?;
    }

    if reader.triangles == 0 {
        return Err(ModelError::Rejected(ModelDefect::EmptyScene));
    }
    Ok(ModelScene {
        meshes: reader.meshes,
        vertices: reader.vertices,
        triangles: reader.triangles,
        aabb: ModelAabb {
            min: reader.min,
            max: reader.max,
        },
    })
}

/// The bytes behind one buffer, or a refusal to go and get them.
fn resolve(buffer: &Buffer, binary: &[u8], bounds: ModelBounds) -> Result<Vec<u8>, ModelError> {
    let Some(uri) = buffer.uri.as_deref() else {
        return Ok(binary.to_vec());
    };
    let Some(rest) = uri.strip_prefix("data:") else {
        return Err(ModelError::Rejected(ModelDefect::ExternalResource));
    };
    let Some((_, payload)) = rest.split_once(";base64,") else {
        return Err(ModelError::Rejected(ModelDefect::UnsupportedEncoding));
    };
    // The decoded length is known from the encoded one, so an oversize buffer
    // is refused before it is decoded rather than after.
    bounds.check(ModelLimit::Bytes, payload.len() / 4 * 3)?;
    base64(payload).ok_or(ModelError::Rejected(ModelDefect::UnsupportedEncoding))
}

/// Standard base64 with optional padding, and nothing else.
fn base64(payload: &str) -> Option<Vec<u8>> {
    fn sextet(byte: u8) -> Option<u32> {
        match byte {
            b'A'..=b'Z' => Some(u32::from(byte - b'A')),
            b'a'..=b'z' => Some(u32::from(byte - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(byte - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let payload = payload.trim_end_matches('=');
    let mut out = Vec::with_capacity(payload.len() / 4 * 3);
    let mut accumulator = 0_u32;
    let mut bits = 0_u32;
    for byte in payload.bytes() {
        if byte.is_ascii_whitespace() {
            continue;
        }
        accumulator = (accumulator << 6) | sextet(byte)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((accumulator >> bits) & 0xFF) as u8);
        }
    }
    // Any bits left over must be the zero padding of the final group; a
    // non-zero remainder means the payload was truncated mid-byte.
    if accumulator & ((1 << bits) - 1) != 0 {
        return None;
    }
    Some(out)
}

impl Reader<'_> {
    fn walk(&mut self, index: usize, parent: &Mat4, depth: usize) -> Result<(), ModelError> {
        self.bounds.check(ModelLimit::Depth, depth)?;
        self.visited += 1;
        // A file may name the same node from two parents, and a malformed one
        // may name a node that reaches itself. The visit budget is what stops
        // either from becoming an unbounded walk.
        self.bounds.check(ModelLimit::Nodes, self.visited)?;

        let node = self
            .document
            .nodes
            .get(index)
            .ok_or(ModelError::Rejected(ModelDefect::DanglingIndex))?;
        let world = multiply(parent, &local(node));

        if let Some(mesh) = node.mesh {
            let mesh = self
                .document
                .meshes
                .get(mesh)
                .ok_or(ModelError::Rejected(ModelDefect::DanglingIndex))?;
            let name = mesh
                .name
                .clone()
                .or_else(|| node.name.clone())
                .unwrap_or_else(|| format!("mesh-{index}"));
            for primitive in &mesh.primitives {
                self.primitive(primitive, &world, &name)?;
            }
        }

        for child in &node.children {
            self.walk(*child, &world, depth + 1)?;
        }
        Ok(())
    }

    fn primitive(
        &mut self,
        primitive: &Primitive,
        world: &Mat4,
        name: &str,
    ) -> Result<(), ModelError> {
        if primitive.mode.unwrap_or(MODE_TRIANGLES) != MODE_TRIANGLES {
            return Err(ModelError::Rejected(ModelDefect::UnsupportedPrimitive));
        }
        let accessor = primitive
            .attributes
            .position
            .ok_or(ModelError::Rejected(ModelDefect::MissingPositions))?;
        self.bounds
            .check(ModelLimit::Primitives, self.meshes.len() + 1)?;

        let positions = self.positions(accessor, world)?;
        let indices = match primitive.indices {
            Some(accessor) => self.indices(accessor, positions.len())?,
            // An unindexed primitive is three consecutive vertices per face.
            None => (0..positions.len() as u32).collect(),
        };
        if indices.len() < 3 {
            return Ok(());
        }
        let indices: Vec<u32> = indices[..indices.len() - indices.len() % 3].to_vec();

        self.triangles += indices.len() / 3;
        self.bounds.check(ModelLimit::Triangles, self.triangles)?;
        self.meshes.push(ModelMesh {
            name: SharedString::from(name.to_owned()),
            positions,
            indices,
        });
        Ok(())
    }

    /// The bytes one accessor addresses, with every bound checked first.
    fn view(&self, accessor: &Accessor, size: usize) -> Result<(&[u8], usize), ModelError> {
        if accessor.sparse.is_some() {
            return Err(ModelError::Rejected(ModelDefect::SparseAccessor));
        }
        let truncated = ModelError::Rejected(ModelDefect::TruncatedBuffer);
        let dangling = ModelError::Rejected(ModelDefect::DanglingIndex);
        let view = accessor
            .buffer_view
            .and_then(|index| self.document.buffer_views.get(index))
            .ok_or(dangling)?;
        let buffer = self
            .buffers
            .get(view.buffer)
            .map(Vec::as_slice)
            .or(if view.buffer == 0 {
                Some(self.binary)
            } else {
                None
            })
            .ok_or(dangling)?;

        let start = view
            .byte_offset
            .checked_add(accessor.byte_offset)
            .ok_or(truncated)?;
        let stride = view.byte_stride.unwrap_or(size).max(size);
        // The last element only needs its own size, not a whole stride, which
        // is what a tightly packed final element in an interleaved view is.
        let span = stride
            .checked_mul(accessor.count.saturating_sub(1))
            .and_then(|span| span.checked_add(size))
            .ok_or(truncated)?;
        let end = start.checked_add(span).ok_or(truncated)?;
        if end > buffer.len() || view.byte_offset + view.byte_length > buffer.len() {
            return Err(truncated);
        }
        Ok((&buffer[start..end], stride))
    }

    fn positions(&mut self, index: usize, world: &Mat4) -> Result<Vec<[f32; 3]>, ModelError> {
        let accessor = self
            .document
            .accessors
            .get(index)
            .ok_or(ModelError::Rejected(ModelDefect::DanglingIndex))?;
        if accessor.kind != "VEC3" {
            return Err(ModelError::Rejected(ModelDefect::UnsupportedAccessorType));
        }
        if accessor.component_type != COMPONENT_FLOAT {
            return Err(ModelError::Rejected(ModelDefect::UnsupportedComponentType));
        }
        self.vertices += accessor.count;
        self.bounds.check(ModelLimit::Vertices, self.vertices)?;

        let (bytes, stride) = self.view(accessor, 12)?;
        let mut positions = Vec::with_capacity(accessor.count);
        for element in 0..accessor.count {
            let at = element * stride;
            let float = |offset: usize| {
                let slice = &bytes[at + offset..at + offset + 4];
                f32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]])
            };
            positions.push(transform(world, [float(0), float(4), float(8)]));
        }
        for point in &positions {
            for (axis, value) in point.iter().enumerate() {
                self.min[axis] = self.min[axis].min(*value);
                self.max[axis] = self.max[axis].max(*value);
            }
        }
        Ok(positions)
    }

    fn indices(&self, index: usize, vertices: usize) -> Result<Vec<u32>, ModelError> {
        let accessor = self
            .document
            .accessors
            .get(index)
            .ok_or(ModelError::Rejected(ModelDefect::DanglingIndex))?;
        if accessor.kind != "SCALAR" {
            return Err(ModelError::Rejected(ModelDefect::UnsupportedAccessorType));
        }
        let size = match accessor.component_type {
            COMPONENT_UNSIGNED_BYTE => 1,
            COMPONENT_UNSIGNED_SHORT => 2,
            COMPONENT_UNSIGNED_INT => 4,
            _ => return Err(ModelError::Rejected(ModelDefect::UnsupportedComponentType)),
        };
        self.bounds
            .check(ModelLimit::Triangles, self.triangles + accessor.count / 3)?;

        let (bytes, stride) = self.view(accessor, size)?;
        let mut indices = Vec::with_capacity(accessor.count);
        for element in 0..accessor.count {
            let at = element * stride;
            let value = match size {
                1 => u32::from(bytes[at]),
                2 => u32::from(u16::from_le_bytes([bytes[at], bytes[at + 1]])),
                _ => u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]),
            };
            if value as usize >= vertices {
                return Err(ModelError::Rejected(ModelDefect::IndexOutOfRange));
            }
            indices.push(value);
        }
        Ok(indices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One triangle, its positions in a base64 `data:` buffer.
    ///
    /// Written out rather than loaded, so the reader's tests need no file and
    /// no fixture directory.
    fn triangle() -> String {
        let positions: [f32; 9] = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let mut bytes = Vec::new();
        for value in positions {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        format!(
            r#"{{
              "asset": {{"version": "2.0"}},
              "scene": 0,
              "scenes": [{{"nodes": [0]}}],
              "nodes": [{{"mesh": 0}}],
              "meshes": [{{"name": "tri", "primitives": [{{"attributes": {{"POSITION": 0}}}}]}}],
              "accessors": [
                {{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"}}
              ],
              "bufferViews": [{{"buffer": 0, "byteOffset": 0, "byteLength": {length}}}],
              "buffers": [{{"uri": "data:application/octet-stream;base64,{payload}"}}]
            }}"#,
            length = bytes.len(),
            payload = encode(&bytes),
        )
    }

    /// The encoder the fixtures are written with. The reader has only a
    /// decoder, so this is a test's own tool rather than public surface.
    fn encode(bytes: &[u8]) -> String {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let mut block = [0_u8; 3];
            block[..chunk.len()].copy_from_slice(chunk);
            let packed =
                (u32::from(block[0]) << 16) | (u32::from(block[1]) << 8) | u32::from(block[2]);
            for step in 0..4 {
                if step <= chunk.len() {
                    let sextet = ((packed >> (18 - step * 6)) & 0x3F) as usize;
                    out.push(char::from(ALPHABET[sextet]));
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    #[test]
    fn a_triangle_reads_as_one_mesh_of_three_vertices() {
        let scene = ModelScene::parse(triangle().as_bytes(), ModelBounds::default())
            .expect("a triangle is inside the subset");
        assert_eq!(scene.mesh_count(), 1);
        assert_eq!(scene.vertex_count(), 3);
        assert_eq!(scene.triangle_count(), 1);
        assert_eq!(scene.meshes()[0].name().as_ref(), "tri");
        assert_eq!(scene.aabb().min, [0.0, 0.0, 0.0]);
        assert_eq!(scene.aabb().max, [1.0, 1.0, 0.0]);
    }

    #[test]
    fn a_base64_round_trip_holds_and_a_truncated_payload_does_not() {
        let bytes: Vec<u8> = (0..32).collect();
        assert_eq!(base64(&encode(&bytes)), Some(bytes));
        assert_eq!(
            base64("!!!!"),
            None,
            "a byte outside the alphabet is not data"
        );
    }

    #[test]
    fn a_document_larger_than_the_caller_allowed_is_refused_by_its_size() {
        let bounds = ModelBounds::default().max_bytes(16);
        assert_eq!(
            ModelScene::parse(triangle().as_bytes(), bounds),
            Err(ModelError::TooLarge {
                limit: ModelLimit::Bytes,
                found: triangle().len(),
                allowed: 16,
            })
        );
    }

    #[test]
    fn a_model_past_a_geometry_cap_is_refused_before_it_is_built() {
        let bounds = ModelBounds::default().max_vertices(2);
        assert_eq!(
            ModelScene::parse(triangle().as_bytes(), bounds),
            Err(ModelError::TooLarge {
                limit: ModelLimit::Vertices,
                found: 3,
                allowed: 2,
            })
        );
        let bounds = ModelBounds::default().max_triangles(0);
        assert!(matches!(
            ModelScene::parse(triangle().as_bytes(), bounds),
            Err(ModelError::TooLarge {
                limit: ModelLimit::Triangles,
                ..
            })
        ));
    }

    #[test]
    fn a_buffer_this_crate_would_have_to_fetch_is_refused() {
        let document = triangle().replace(
            "data:application/octet-stream;base64,",
            "https://example.invalid/scene.bin?",
        );
        assert_eq!(
            ModelScene::parse(document.as_bytes(), ModelBounds::default()),
            Err(ModelError::Rejected(ModelDefect::ExternalResource)),
            "resolving a URI is I/O, and this crate performs none"
        );
    }

    #[test]
    fn a_document_outside_the_subset_names_what_it_asked_for() {
        let lines = triangle().replace(r#""POSITION": 0}}"#, r#""POSITION": 0}, "mode": 1}"#);
        assert_eq!(
            ModelScene::parse(lines.as_bytes(), ModelBounds::default()),
            Err(ModelError::Rejected(ModelDefect::UnsupportedPrimitive))
        );

        let sparse = triangle().replace(
            r#""type": "VEC3"}"#,
            r#""type": "VEC3", "sparse": {"count": 1}}"#,
        );
        assert_eq!(
            ModelScene::parse(sparse.as_bytes(), ModelBounds::default()),
            Err(ModelError::Rejected(ModelDefect::SparseAccessor))
        );

        let doubles = triangle().replace(r#""componentType": 5126"#, r#""componentType": 5130"#);
        assert_eq!(
            ModelScene::parse(doubles.as_bytes(), ModelBounds::default()),
            Err(ModelError::Rejected(ModelDefect::UnsupportedComponentType))
        );
    }

    #[test]
    fn bytes_that_are_not_a_document_are_refused_as_such() {
        assert_eq!(
            ModelScene::parse(b"not a model", ModelBounds::default()),
            Err(ModelError::Rejected(ModelDefect::NotJson))
        );
        assert_eq!(
            ModelScene::parse(br#"{"nodes": []}"#, ModelBounds::default()),
            Err(ModelError::Rejected(ModelDefect::NotGltf))
        );
        assert_eq!(
            ModelScene::parse(br#"{"asset": {"version": "1.0"}}"#, ModelBounds::default()),
            Err(ModelError::Rejected(ModelDefect::UnsupportedVersion))
        );
    }

    #[test]
    fn an_accessor_that_reads_past_its_buffer_is_refused() {
        let overrun = triangle().replace(
            r#""count": 3, "type": "VEC3""#,
            r#""count": 9, "type": "VEC3""#,
        );
        assert_eq!(
            ModelScene::parse(overrun.as_bytes(), ModelBounds::default()),
            Err(ModelError::Rejected(ModelDefect::TruncatedBuffer))
        );
    }

    #[test]
    fn a_glb_container_carries_the_same_document() {
        let json = triangle();
        let mut padded = json.into_bytes();
        while !padded.len().is_multiple_of(4) {
            padded.push(b' ');
        }
        let mut glb = Vec::new();
        glb.extend_from_slice(GLB_MAGIC);
        glb.extend_from_slice(&2_u32.to_le_bytes());
        glb.extend_from_slice(
            &((GLB_HEADER + GLB_CHUNK_HEADER + padded.len()) as u32).to_le_bytes(),
        );
        glb.extend_from_slice(&(padded.len() as u32).to_le_bytes());
        glb.extend_from_slice(&GLB_CHUNK_JSON.to_le_bytes());
        glb.extend_from_slice(&padded);

        let scene =
            ModelScene::parse(&glb, ModelBounds::default()).expect("a GLB is inside the subset");
        assert_eq!(scene.triangle_count(), 1);

        // A declared length past what was handed in is the shape of a file
        // written to be read wrongly, so it is refused rather than clamped.
        let mut lying = glb.clone();
        lying[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            ModelScene::parse(&lying, ModelBounds::default()),
            Err(ModelError::Rejected(ModelDefect::BadContainer))
        );
    }

    #[test]
    fn a_node_that_reaches_itself_stops_at_the_visit_budget() {
        let looping = r#"{
          "asset": {"version": "2.0"},
          "scenes": [{"nodes": [0]}],
          "nodes": [{"children": [1]}, {"children": [0]}]
        }"#;
        assert!(matches!(
            ModelScene::parse(looping.as_bytes(), ModelBounds::default()),
            Err(ModelError::TooLarge {
                limit: ModelLimit::Depth | ModelLimit::Nodes,
                ..
            })
        ));
    }

    #[test]
    fn a_transform_places_the_geometry_the_node_moved() {
        let moved = triangle().replace(
            r#"{"mesh": 0}"#,
            r#"{"mesh": 0, "translation": [10.0, 0.0, 0.0]}"#,
        );
        let scene = ModelScene::parse(moved.as_bytes(), ModelBounds::default()).expect("read");
        assert_eq!(scene.aabb().min, [10.0, 0.0, 0.0]);
        assert_eq!(scene.aabb().max, [11.0, 1.0, 0.0]);
    }

    #[test]
    fn a_bounding_box_answers_a_centre_and_a_radius() {
        let aabb = ModelAabb {
            min: [-1.0, -1.0, -1.0],
            max: [1.0, 1.0, 1.0],
        };
        assert_eq!(aabb.centre(), [0.0, 0.0, 0.0]);
        assert!((aabb.radius() - 3.0_f32.sqrt()).abs() < 0.001);
    }
}
