use earthmesh_mesh::MeshState;

pub(crate) fn mesh_fingerprint(mesh: &MeshState) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    let mut feed = |value: u64| {
        for byte in value.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    };
    feed(mesh.vertex_count() as u64);
    feed(mesh.triangle_count() as u64);
    for site in mesh.active_vertex_slots() {
        let point = mesh.vertices()[site];
        feed(site as u64);
        feed(point.x.to_bits());
        feed(point.y.to_bits());
        feed(point.z.to_bits());
    }
    for face in mesh.active_triangle_slots() {
        feed(face as u64);
        for site in mesh.triangles()[face] {
            feed(site as u64);
        }
    }
    hash
}
