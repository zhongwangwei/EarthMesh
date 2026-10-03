//! Recording an h-field pass, and replaying it.
//!
//! A global 12 km run reaches its failing pass after a quarter of an hour of
//! earlier passes. The pass itself is topological: which faces are selected
//! and which anchors must stay covered decide whether it builds, and the
//! coordinates do not. Each pass's record -- the mesh the first pass starts
//! from, the selection and the anchors -- goes to a sink the caller installs
//! (`set_pass_sink`); the algorithm writes nothing itself. The CLI's Method-C
//! adapter installs one that writes the records under
//! `EARTHMESH_METHOD_C_DUMP_DIR`, and the ignored test below rebuilds that mesh
//! and replays the passes.
//!
//! The mesh is recorded, not rebuilt from its NXP: the CLI reads its base back
//! from the gridinit gridfile, and that numbering is not `from_icosahedron`'s
//! -- a selection replayed on the latter is a scatter of unrelated faces.

use std::{fmt::Write as _, io, sync::OnceLock};

#[cfg(test)]
use std::{fs, path::Path};

use earthmesh_mesh::xyz_points_to_lonlat_degrees;

use super::{MethodCHfieldDemandCoverage, MethodCMesh};

const HEADER: &str = "earthmesh-method-c-pass v1";
const BASE_MAGIC: &[u8; 8] = b"EMCBASE1";

/// The file a pass record's base is kept in, beside `pass_NN.txt` and
/// `pass_NN_built.txt`, where the replay test reads them back.
pub const PASS_BASE_FILE: &str = "base.bin";

/// One pass, recorded for replay.
pub struct PassRecord {
    /// The pass's child level, which names its file.
    pub child_level: usize,
    /// Whether this is the selection the pass finally built with, after any
    /// dropped blocks, rather than the one it was asked for.
    pub built: bool,
    /// The mesh the pass starts from, as the replay reads it
    /// (`PASS_BASE_FILE`); every record carries it, a sink keeps the first.
    pub base: Vec<u8>,
    /// The selection and anchors, as the replay reads them.
    pub text: String,
}

/// Where pass records go: installed once per process by a caller that wants
/// them, and none by default.
pub type PassSink = Box<dyn Fn(&PassRecord) -> io::Result<()> + Send + Sync>;

static PASS_SINK: OnceLock<PassSink> = OnceLock::new();

/// Install the sink that receives every pass's record. Only the first
/// install takes; a later one is handed back.
pub fn set_pass_sink(sink: PassSink) -> Result<(), PassSink> {
    PASS_SINK.set(sink)
}

/// The gridfile tables `TriangularMesh::from_voronoi_gridfile_tables` takes:
/// one lon/lat per M id and one M triple per W id, placeholder rows included.
fn base_bytes(mesh: &MethodCMesh) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(24 + mesh.nmd * 16 + mesh.nwd * 12);
    bytes.extend_from_slice(BASE_MAGIC);
    bytes.extend_from_slice(&(mesh.nmd as u64).to_le_bytes());
    bytes.extend_from_slice(&(mesh.nwd as u64).to_le_bytes());
    for (im, point) in xyz_points_to_lonlat_degrees(&mesh.m_points[1..=mesh.nmd])
        .into_iter()
        .enumerate()
    {
        let (lon, lat) = if im == 0 {
            (0.0, 0.0)
        } else {
            (point.lon_degrees, point.lat_degrees)
        };
        bytes.extend_from_slice(&lon.to_le_bytes());
        bytes.extend_from_slice(&lat.to_le_bytes());
    }
    for face in &mesh.w_faces[1..=mesh.nwd] {
        for im in face.im {
            bytes.extend_from_slice(&(im as u32).to_le_bytes());
        }
    }
    bytes
}

/// Hand a pass to the sink, if one is installed. `built`: the selection a
/// pass finally built with, after any dropped blocks, so a replay can reach
/// the next pass in one attempt.
pub(super) fn record_pass(
    mesh: &MethodCMesh,
    selected: &[bool],
    child_level: usize,
    max_mrows: usize,
    coverage: &MethodCHfieldDemandCoverage,
    built: bool,
) -> io::Result<()> {
    let Some(sink) = PASS_SINK.get() else {
        return Ok(());
    };
    let mut text = String::new();
    let faces: Vec<usize> = (0..selected.len()).filter(|&iw| selected[iw]).collect();
    let _ = writeln!(text, "{HEADER}");
    let _ = writeln!(
        text,
        "{} {} {} {child_level} {max_mrows}",
        mesh.nmd, mesh.nwd, mesh.nud
    );
    let _ = writeln!(
        text,
        "{} {} {} {}",
        coverage.requested_anchor_count,
        coverage.demanded_face_count,
        coverage.unmet_face_count,
        coverage.clipped_anchor_count
    );
    let _ = writeln!(text, "{}", join(&faces));
    for (im, faces) in &coverage.anchors {
        let _ = writeln!(text, "{im} {}", join(faces));
    }
    sink(&PassRecord {
        child_level,
        built,
        base: base_bytes(mesh),
        text,
    })
}

fn join(values: &[usize]) -> String {
    let mut line = String::with_capacity(values.len() * 8);
    for (k, value) in values.iter().enumerate() {
        if k > 0 {
            line.push(' ');
        }
        let _ = write!(line, "{value}");
    }
    line
}

#[cfg(test)]
pub(crate) fn read_base(path: &Path) -> io::Result<MethodCMesh> {
    use earthmesh_mesh::{LonLatDegrees, TriangularMesh};
    let invalid = |what: &str| io::Error::new(io::ErrorKind::InvalidData, what.to_string());
    let bytes = fs::read(path)?;
    if bytes.len() < 24 || &bytes[..8] != BASE_MAGIC {
        return Err(invalid("not a Method-C base dump"));
    }
    let word = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
    let (nmd, nwd) = (word(8) as usize, word(16) as usize);
    if bytes.len() != 24 + nmd * 16 + nwd * 12 {
        return Err(invalid("truncated Method-C base dump"));
    }
    let real = |at: usize| f64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
    let points = (0..nmd)
        .map(|row| LonLatDegrees::new(real(24 + row * 16), real(32 + row * 16)))
        .collect::<Vec<_>>();
    let faces_at = 24 + nmd * 16;
    let id = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    let faces = (0..nwd)
        .map(|row| {
            let at = faces_at + row * 12;
            [id(at), id(at + 4), id(at + 8)]
        })
        .collect::<Vec<_>>();
    let mut valence = vec![0usize; nmd];
    for face in faces.iter().skip(1) {
        for &im in face {
            valence[im - 1] += 1;
        }
    }
    TriangularMesh::from_voronoi_gridfile_tables(&points, &faces, &valence).map(MethodCMesh::new)
}

#[cfg(test)]
pub(crate) struct DumpedPass {
    pub(crate) nmd: usize,
    pub(crate) nwd: usize,
    pub(crate) nud: usize,
    pub(crate) child_level: usize,
    pub(crate) max_mrows: usize,
    pub(crate) selected: Vec<bool>,
    pub(crate) coverage: MethodCHfieldDemandCoverage,
}

#[cfg(test)]
pub(crate) fn read_pass(path: &Path) -> io::Result<DumpedPass> {
    let invalid = |what: &str| io::Error::new(io::ErrorKind::InvalidData, what.to_string());
    let text = fs::read_to_string(path)?;
    let mut lines = text.lines();
    if lines.next() != Some(HEADER) {
        return Err(invalid("not a Method-C pass dump"));
    }
    let numbers = |line: Option<&str>| -> io::Result<Vec<usize>> {
        line.ok_or_else(|| invalid("truncated pass dump"))?
            .split_ascii_whitespace()
            .map(|word| word.parse().map_err(|_| invalid("bad number")))
            .collect()
    };
    let sizes = numbers(lines.next())?;
    let counts = numbers(lines.next())?;
    let faces = numbers(lines.next())?;
    let [nmd, nwd, nud, child_level, max_mrows] = sizes[..] else {
        return Err(invalid("bad size line"));
    };
    let [requested_anchor_count, demanded_face_count, unmet_face_count, clipped_anchor_count] =
        counts[..]
    else {
        return Err(invalid("bad count line"));
    };
    let mut selected = vec![false; nwd + 1];
    for iw in faces {
        selected[iw] = true;
    }
    let mut anchors = Vec::new();
    for line in lines {
        let mut values = numbers(Some(line))?;
        if values.is_empty() {
            continue;
        }
        let im = values.remove(0);
        anchors.push((im, values));
    }
    Ok(DumpedPass {
        nmd,
        nwd,
        nud,
        child_level,
        max_mrows,
        selected,
        coverage: MethodCHfieldDemandCoverage {
            anchors,
            requested_anchor_count,
            demanded_face_count,
            unmet_face_count,
            clipped_anchor_count,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MethodCHfieldSpawnDiagnostics;

    #[test]
    fn a_dumped_base_reads_back_with_the_same_faces_and_edges() {
        let dir = std::env::temp_dir().join(format!("method_c_base_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let original = MethodCMesh::from_icosahedron(12, 0, 1.0, 0.25).unwrap();
        fs::write(dir.join(PASS_BASE_FILE), base_bytes(&original)).unwrap();
        let first = read_base(&dir.join(PASS_BASE_FILE)).unwrap();
        // The W rows and their M triples are what the file says they are.
        assert_eq!(first.nwd, original.nwd);
        for iw in 2..=original.nwd {
            assert_eq!(first.w_faces[iw].im, original.w_faces[iw].im, "face {iw}");
        }
        // Read back again, nothing moves: the edges the replay derives are the
        // ones the run derived from the same tables.
        fs::write(dir.join(PASS_BASE_FILE), base_bytes(&first)).unwrap();
        let second = read_base(&dir.join(PASS_BASE_FILE)).unwrap();
        assert_eq!(second.u_edges, first.u_edges);
        assert_eq!(second.w_faces, first.w_faces);
        let _ = fs::remove_dir_all(&dir);
    }

    /// `EARTHMESH_METHOD_C_REPLAY_DIR=<dir> cargo test --release
    /// -p earthmesh_refine_method_c replay_dumped_passes -- --ignored --nocapture`
    #[test]
    #[ignore = "replays a pass dump named by EARTHMESH_METHOD_C_REPLAY_DIR"]
    fn replay_dumped_passes() {
        let dir = std::env::var_os("EARTHMESH_METHOD_C_REPLAY_DIR").expect("replay dir");
        let mut paths: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("pass_") && !name.contains("_built"))
            })
            .collect();
        paths.sort();
        let mut mesh = read_base(&Path::new(&dir).join(PASS_BASE_FILE)).expect("base mesh");
        let last = paths.len();
        for (k, path) in paths.into_iter().enumerate() {
            // Every pass but the last starts from what it built with, when the
            // run got that far: one attempt instead of the whole drop search.
            let built = path.with_file_name(
                path.file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replace(".txt", "_built.txt"),
            );
            let path = if k + 1 < last && built.exists() {
                built
            } else {
                path
            };
            let pass = read_pass(&path).unwrap();
            assert_eq!(
                (mesh.nmd, mesh.nwd, mesh.nud),
                (pass.nmd, pass.nwd, pass.nud),
                "{}: the replayed parent is not the dumped one",
                path.display()
            );
            let started = std::time::Instant::now();
            let mut diagnostics = MethodCHfieldSpawnDiagnostics::default();
            let faces = pass.selected.iter().filter(|&&face| face).count();
            let result = mesh.spawn_nest_pass_dropping_unbuildable_blocks(
                pass.selected,
                pass.child_level,
                pass.max_mrows,
                pass.coverage,
                &mut diagnostics,
            );
            eprintln!(
                "replay: pass {} ({faces} faces) in {:.1} s: {} ({diagnostics:?})",
                pass.child_level,
                started.elapsed().as_secs_f64(),
                match &result {
                    Ok(_) => "built".to_string(),
                    Err(error) => error.to_string(),
                }
            );
            mesh = result.unwrap();
        }
    }
}
