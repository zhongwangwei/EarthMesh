use std::error::Error;
use std::fmt;

pub const SOURCE_DEMAND_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

/// Fixed-size digest supplied by the caller's content hasher.
///
/// This crate deliberately owns canonicalization, not a hashing dependency.
/// Production callers should inject their existing SHA-256 implementation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DemandHash([u8; 32]);

impl DemandHash {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        use std::fmt::Write;

        let mut hex = String::with_capacity(64);
        for byte in self.0 {
            write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
        }
        hex
    }
}

impl fmt::Display for DemandHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DemandSupportKind {
    Anchor,
    Line,
    Area,
    Raster,
    StableCellIds,
}

impl DemandSupportKind {
    const fn tag(self) -> u8 {
        match self {
            Self::Anchor => 1,
            Self::Line => 2,
            Self::Area => 3,
            Self::Raster => 4,
            Self::StableCellIds => 5,
        }
    }
}

/// A geometry or raster support already normalized by the lowering layer.
///
/// `canonical_data` must contain only semantic content. File paths, process
/// IDs, timestamps, and traversal-order metadata have no fields in this
/// contract and must not be embedded in this byte string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemandSupport {
    kind: DemandSupportKind,
    canonical_data: Box<[u8]>,
}

impl DemandSupport {
    pub fn new(
        kind: DemandSupportKind,
        canonical_data: impl Into<Vec<u8>>,
    ) -> Result<Self, DemandSnapshotError> {
        let canonical_data = canonical_data.into();
        if canonical_data.is_empty() {
            return Err(DemandSnapshotError::EmptyCanonicalSupport);
        }
        Ok(Self {
            kind,
            canonical_data: canonical_data.into_boxed_slice(),
        })
    }

    pub const fn kind(&self) -> DemandSupportKind {
        self.kind
    }

    pub fn canonical_data(&self) -> &[u8] {
        &self.canonical_data
    }

    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.u8(self.kind.tag());
        encoder.bytes(&self.canonical_data);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DemandStrength {
    Hard,
    Transition,
}

impl DemandStrength {
    const fn tag(self) -> u8 {
        match self {
            Self::Hard => 1,
            Self::Transition => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DemandSourceKind {
    Specified,
    Threshold,
    Landcover,
    Hydro,
    AutoRefine,
}

impl DemandSourceKind {
    const fn tag(self) -> u8 {
        match self {
            Self::Specified => 1,
            Self::Threshold => 2,
            Self::Landcover => 3,
            Self::Hydro => 4,
            Self::AutoRefine => 5,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemandComponent {
    component_id: DemandHash,
    content_hash: DemandHash,
    strength: DemandStrength,
    support: DemandSupport,
    domain_intersection: DemandSupport,
    requested_level: u8,
    target_cell_width_m_bits: u64,
    identity_canonical_bytes: Box<[u8]>,
    canonical_bytes: Box<[u8]>,
}

impl DemandComponent {
    pub fn new<H>(
        strength: DemandStrength,
        support: DemandSupport,
        domain_intersection: DemandSupport,
        requested_level: u8,
        target_cell_width_m: f64,
        hash: &H,
    ) -> Result<Self, DemandSnapshotError>
    where
        H: Fn(&[u8]) -> DemandHash,
    {
        if !target_cell_width_m.is_finite() || target_cell_width_m <= 0.0 {
            return Err(DemandSnapshotError::InvalidTargetCellWidth);
        }
        Ok(Self::build(
            strength,
            support,
            domain_intersection,
            requested_level,
            target_cell_width_m.to_bits(),
            hash,
        ))
    }

    fn build<H>(
        strength: DemandStrength,
        support: DemandSupport,
        domain_intersection: DemandSupport,
        requested_level: u8,
        target_cell_width_m_bits: u64,
        hash: &H,
    ) -> Self
    where
        H: Fn(&[u8]) -> DemandHash,
    {
        let mut identity = CanonicalEncoder::new(b"earthmesh-demand-component-id-v1");
        support.encode(&mut identity);
        domain_intersection.encode(&mut identity);
        let identity_canonical_bytes = identity.finish();
        let component_id = hash(&identity_canonical_bytes);

        let mut canonical = CanonicalEncoder::new(b"earthmesh-demand-component-v1");
        canonical.u32(SOURCE_DEMAND_SNAPSHOT_SCHEMA_VERSION);
        support.encode(&mut canonical);
        domain_intersection.encode(&mut canonical);
        canonical.u8(strength.tag());
        canonical.u8(requested_level);
        canonical.u64(target_cell_width_m_bits);
        let canonical_bytes = canonical.finish();
        let content_hash = hash(&canonical_bytes);

        Self {
            component_id,
            content_hash,
            strength,
            support,
            domain_intersection,
            requested_level,
            target_cell_width_m_bits,
            identity_canonical_bytes,
            canonical_bytes,
        }
    }

    fn rehash<H>(&self, hash: &H) -> Self
    where
        H: Fn(&[u8]) -> DemandHash,
    {
        Self::build(
            self.strength,
            self.support.clone(),
            self.domain_intersection.clone(),
            self.requested_level,
            self.target_cell_width_m_bits,
            hash,
        )
    }

    pub const fn component_id(&self) -> DemandHash {
        self.component_id
    }

    pub const fn content_hash(&self) -> DemandHash {
        self.content_hash
    }

    pub const fn strength(&self) -> DemandStrength {
        self.strength
    }

    pub const fn support(&self) -> &DemandSupport {
        &self.support
    }

    pub const fn domain_intersection(&self) -> &DemandSupport {
        &self.domain_intersection
    }

    pub const fn requested_level(&self) -> u8 {
        self.requested_level
    }

    pub fn target_cell_width_m(&self) -> f64 {
        f64::from_bits(self.target_cell_width_m_bits)
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemandSource {
    source_id: DemandHash,
    content_hash: DemandHash,
    kind: DemandSourceKind,
    canonical_descriptor: Box<[u8]>,
    components: Box<[DemandComponent]>,
    identity_canonical_bytes: Box<[u8]>,
    canonical_bytes: Box<[u8]>,
}

impl DemandSource {
    pub fn new<H>(
        kind: DemandSourceKind,
        canonical_descriptor: impl Into<Vec<u8>>,
        components: Vec<DemandComponent>,
        hash: &H,
    ) -> Result<Self, DemandSnapshotError>
    where
        H: Fn(&[u8]) -> DemandHash,
    {
        Self::build(
            kind,
            canonical_descriptor.into().into_boxed_slice(),
            components,
            hash,
        )
    }

    fn build<H>(
        kind: DemandSourceKind,
        canonical_descriptor: Box<[u8]>,
        components: Vec<DemandComponent>,
        hash: &H,
    ) -> Result<Self, DemandSnapshotError>
    where
        H: Fn(&[u8]) -> DemandHash,
    {
        if components.is_empty() {
            return Err(DemandSnapshotError::EmptySource);
        }

        let mut components = components
            .into_iter()
            .map(|component| component.rehash(hash))
            .collect::<Vec<_>>();
        components.sort_by(|left, right| {
            left.component_id.cmp(&right.component_id).then_with(|| {
                left.identity_canonical_bytes
                    .cmp(&right.identity_canonical_bytes)
            })
        });
        for pair in components.windows(2) {
            if pair[0].component_id == pair[1].component_id {
                if pair[0].identity_canonical_bytes == pair[1].identity_canonical_bytes {
                    return Err(DemandSnapshotError::DuplicateComponent {
                        component_id: pair[0].component_id,
                    });
                }
                return Err(DemandSnapshotError::HashCollision {
                    entity: "component",
                    hash: pair[0].component_id,
                });
            }
        }

        let mut identity = CanonicalEncoder::new(b"earthmesh-demand-source-id-v1");
        identity.u8(kind.tag());
        identity.bytes(&canonical_descriptor);
        let identity_canonical_bytes = identity.finish();
        let source_id = hash(&identity_canonical_bytes);

        let mut canonical = CanonicalEncoder::new(b"earthmesh-demand-source-v1");
        canonical.u32(SOURCE_DEMAND_SNAPSHOT_SCHEMA_VERSION);
        canonical.u8(kind.tag());
        canonical.bytes(&canonical_descriptor);
        canonical.u64(components.len() as u64);
        for component in &components {
            canonical.bytes(component.canonical_bytes());
        }
        let canonical_bytes = canonical.finish();
        let content_hash = hash(&canonical_bytes);

        Ok(Self {
            source_id,
            content_hash,
            kind,
            canonical_descriptor,
            components: components.into_boxed_slice(),
            identity_canonical_bytes,
            canonical_bytes,
        })
    }

    fn rehash<H>(&self, hash: &H) -> Result<Self, DemandSnapshotError>
    where
        H: Fn(&[u8]) -> DemandHash,
    {
        Self::build(
            self.kind,
            self.canonical_descriptor.clone(),
            self.components.to_vec(),
            hash,
        )
    }

    pub const fn source_id(&self) -> DemandHash {
        self.source_id
    }

    pub const fn content_hash(&self) -> DemandHash {
        self.content_hash
    }

    pub const fn kind(&self) -> DemandSourceKind {
        self.kind
    }

    pub fn canonical_descriptor(&self) -> &[u8] {
        &self.canonical_descriptor
    }

    pub fn components(&self) -> &[DemandComponent] {
        &self.components
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDemandSnapshot {
    project_hash: DemandHash,
    snapshot_hash: DemandHash,
    sources: Box<[DemandSource]>,
    canonical_bytes: Box<[u8]>,
}

impl SourceDemandSnapshot {
    pub fn new<H>(
        project_hash: DemandHash,
        sources: Vec<DemandSource>,
        hash: &H,
    ) -> Result<Self, DemandSnapshotError>
    where
        H: Fn(&[u8]) -> DemandHash,
    {
        let sources = normalize_sources(sources, hash)?;
        let mut canonical = CanonicalEncoder::new(b"earthmesh-source-demand-snapshot-v1");
        canonical.u32(SOURCE_DEMAND_SNAPSHOT_SCHEMA_VERSION);
        canonical.hash(project_hash);
        canonical.u64(sources.len() as u64);
        for source in &sources {
            canonical.bytes(source.canonical_bytes());
        }
        let canonical_bytes = canonical.finish();
        let snapshot_hash = hash(&canonical_bytes);
        Ok(Self {
            project_hash,
            snapshot_hash,
            sources: sources.into_boxed_slice(),
            canonical_bytes,
        })
    }

    pub const fn schema_version(&self) -> u32 {
        SOURCE_DEMAND_SNAPSHOT_SCHEMA_VERSION
    }

    pub const fn project_hash(&self) -> DemandHash {
        self.project_hash
    }

    pub const fn snapshot_hash(&self) -> DemandHash {
        self.snapshot_hash
    }

    pub fn sources(&self) -> &[DemandSource] {
        &self.sources
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemandEpoch {
    epoch_id: u32,
    parent_snapshot_hash: DemandHash,
    demand_hash: DemandHash,
    epoch_hash: DemandHash,
    new_sources: Box<[DemandSource]>,
    canonical_bytes: Box<[u8]>,
}

impl DemandEpoch {
    pub const fn epoch_id(&self) -> u32 {
        self.epoch_id
    }

    /// Base snapshot hash for epoch 1; previous epoch hash thereafter.
    pub const fn parent_snapshot_hash(&self) -> DemandHash {
        self.parent_snapshot_hash
    }

    /// Parent-independent hash used to reject repeated semantic demand.
    pub const fn demand_hash(&self) -> DemandHash {
        self.demand_hash
    }

    pub const fn epoch_hash(&self) -> DemandHash {
        self.epoch_hash
    }

    pub fn new_sources(&self) -> &[DemandSource] {
        &self.new_sources
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

/// Append-only AutoRefine history rooted at an immutable source snapshot.
#[derive(Debug, Eq, PartialEq)]
pub struct DemandEpochChain {
    snapshot: SourceDemandSnapshot,
    epochs: Vec<DemandEpoch>,
}

impl DemandEpochChain {
    pub fn new(snapshot: SourceDemandSnapshot) -> Self {
        Self {
            snapshot,
            epochs: Vec::new(),
        }
    }

    pub const fn snapshot(&self) -> &SourceDemandSnapshot {
        &self.snapshot
    }

    pub fn epochs(&self) -> &[DemandEpoch] {
        &self.epochs
    }

    pub fn tip_hash(&self) -> DemandHash {
        self.epochs
            .last()
            .map_or(self.snapshot.snapshot_hash, |epoch| epoch.epoch_hash)
    }

    pub fn append_epoch<H>(
        &mut self,
        new_sources: Vec<DemandSource>,
        hash: &H,
    ) -> Result<&DemandEpoch, DemandSnapshotError>
    where
        H: Fn(&[u8]) -> DemandHash,
    {
        if hash(self.snapshot.canonical_bytes()) != self.snapshot.snapshot_hash {
            return Err(DemandSnapshotError::InconsistentHasher);
        }
        if let Some(epoch) = self.epochs.last() {
            if hash(epoch.canonical_bytes()) != epoch.epoch_hash {
                return Err(DemandSnapshotError::InconsistentHasher);
            }
        }
        if new_sources.is_empty() {
            return Err(DemandSnapshotError::EmptyEpoch);
        }

        let new_sources = normalize_sources(new_sources, hash)?;
        let mut demand = CanonicalEncoder::new(b"earthmesh-demand-epoch-payload-v1");
        demand.u32(SOURCE_DEMAND_SNAPSHOT_SCHEMA_VERSION);
        demand.u64(new_sources.len() as u64);
        for source in &new_sources {
            demand.bytes(source.canonical_bytes());
        }
        let demand_canonical_bytes = demand.finish();
        let demand_hash = hash(&demand_canonical_bytes);
        if let Some(existing) = self
            .epochs
            .iter()
            .find(|epoch| epoch.demand_hash == demand_hash)
        {
            return Err(DemandSnapshotError::RepeatedDemandEpoch {
                demand_hash,
                existing_epoch_id: existing.epoch_id,
                existing_epoch_hash: existing.epoch_hash,
            });
        }

        let epoch_index = self
            .epochs
            .len()
            .checked_add(1)
            .ok_or(DemandSnapshotError::EpochIdOverflow)?;
        let epoch_id =
            u32::try_from(epoch_index).map_err(|_| DemandSnapshotError::EpochIdOverflow)?;
        let parent_snapshot_hash = self.tip_hash();
        let mut canonical = CanonicalEncoder::new(b"earthmesh-demand-epoch-v1");
        canonical.u32(SOURCE_DEMAND_SNAPSHOT_SCHEMA_VERSION);
        canonical.u32(epoch_id);
        canonical.hash(parent_snapshot_hash);
        canonical.hash(demand_hash);
        let canonical_bytes = canonical.finish();
        let epoch_hash = hash(&canonical_bytes);
        self.epochs.push(DemandEpoch {
            epoch_id,
            parent_snapshot_hash,
            demand_hash,
            epoch_hash,
            new_sources: new_sources.into_boxed_slice(),
            canonical_bytes,
        });
        Ok(self
            .epochs
            .last()
            .expect("an epoch was appended immediately above"))
    }
}

fn normalize_sources<H>(
    sources: Vec<DemandSource>,
    hash: &H,
) -> Result<Vec<DemandSource>, DemandSnapshotError>
where
    H: Fn(&[u8]) -> DemandHash,
{
    let mut sources = sources
        .into_iter()
        .map(|source| source.rehash(hash))
        .collect::<Result<Vec<_>, _>>()?;
    sources.sort_by(|left, right| {
        left.source_id.cmp(&right.source_id).then_with(|| {
            left.identity_canonical_bytes
                .cmp(&right.identity_canonical_bytes)
        })
    });
    for pair in sources.windows(2) {
        if pair[0].source_id == pair[1].source_id {
            if pair[0].identity_canonical_bytes == pair[1].identity_canonical_bytes {
                return Err(DemandSnapshotError::DuplicateSource {
                    source_id: pair[0].source_id,
                });
            }
            return Err(DemandSnapshotError::HashCollision {
                entity: "source",
                hash: pair[0].source_id,
            });
        }
    }
    Ok(sources)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DemandSnapshotError {
    EmptyCanonicalSupport,
    InvalidTargetCellWidth,
    EmptySource,
    EmptyEpoch,
    DuplicateComponent {
        component_id: DemandHash,
    },
    DuplicateSource {
        source_id: DemandHash,
    },
    HashCollision {
        entity: &'static str,
        hash: DemandHash,
    },
    InconsistentHasher,
    RepeatedDemandEpoch {
        demand_hash: DemandHash,
        existing_epoch_id: u32,
        existing_epoch_hash: DemandHash,
    },
    EpochIdOverflow,
}

impl fmt::Display for DemandSnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCanonicalSupport => {
                f.write_str("demand support canonical data must not be empty")
            }
            Self::InvalidTargetCellWidth => {
                f.write_str("target cell width must be positive and finite")
            }
            Self::EmptySource => f.write_str("a demand source must contain at least one component"),
            Self::EmptyEpoch => f.write_str("a demand epoch must add at least one source"),
            Self::DuplicateComponent { component_id } => {
                write!(f, "duplicate demand component {component_id}")
            }
            Self::DuplicateSource { source_id } => {
                write!(f, "duplicate demand source {source_id}")
            }
            Self::HashCollision { entity, hash } => {
                write!(f, "{entity} canonical hash collision at {hash}")
            }
            Self::InconsistentHasher => {
                f.write_str("the supplied demand hasher does not match the existing chain")
            }
            Self::RepeatedDemandEpoch {
                demand_hash,
                existing_epoch_id,
                ..
            } => write!(
                f,
                "repeated demand epoch payload {demand_hash} already exists at epoch {existing_epoch_id}"
            ),
            Self::EpochIdOverflow => f.write_str("demand epoch id exceeds u32"),
        }
    }
}

impl Error for DemandSnapshotError {}

struct CanonicalEncoder {
    bytes: Vec<u8>,
}

impl CanonicalEncoder {
    fn new(domain: &[u8]) -> Self {
        let mut encoder = Self { bytes: Vec::new() };
        encoder.bytes(b"earthmesh-canonical-v1");
        encoder.bytes(domain);
        encoder
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.u64(value.len() as u64);
        self.bytes.extend_from_slice(value);
    }

    fn hash(&mut self, value: DemandHash) {
        self.bytes.extend_from_slice(value.as_bytes());
    }

    fn finish(self) -> Box<[u8]> {
        self.bytes.into_boxed_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_hash(bytes: &[u8]) -> DemandHash {
        let mut lanes = [
            0xcbf2_9ce4_8422_2325_u64,
            0x8422_2325_cbf2_9ce4_u64,
            0x9e37_79b9_7f4a_7c15_u64,
            0x6a09_e667_f3bc_c909_u64,
        ];
        for (index, byte) in bytes.iter().copied().enumerate() {
            let lane = index & 3;
            lanes[lane] ^= u64::from(byte);
            lanes[lane] = lanes[lane].wrapping_mul(0x100_0000_01b3);
            lanes[lane] ^= (index as u64).rotate_left((lane * 11) as u32);
        }
        let mut digest = [0_u8; 32];
        for (index, lane) in lanes.into_iter().enumerate() {
            digest[index * 8..index * 8 + 8].copy_from_slice(&lane.to_le_bytes());
        }
        DemandHash::from_bytes(digest)
    }

    fn component(label: &[u8], level: u8) -> DemandComponent {
        DemandComponent::new(
            DemandStrength::Hard,
            DemandSupport::new(DemandSupportKind::Area, label.to_vec()).unwrap(),
            DemandSupport::new(
                DemandSupportKind::Area,
                [b"domain:".as_slice(), label].concat(),
            )
            .unwrap(),
            level,
            100_000.0 / f64::from(1_u32 << level),
            &test_hash,
        )
        .unwrap()
    }

    fn source(
        kind: DemandSourceKind,
        descriptor: &[u8],
        components: Vec<DemandComponent>,
    ) -> DemandSource {
        DemandSource::new(kind, descriptor.to_vec(), components, &test_hash).unwrap()
    }

    fn snapshot(sources: Vec<DemandSource>) -> SourceDemandSnapshot {
        SourceDemandSnapshot::new(test_hash(b"project"), sources, &test_hash).unwrap()
    }

    #[test]
    fn snapshot_hash_and_canonical_bytes_ignore_insertion_order() {
        let a = component(b"area-a", 1);
        let b = component(b"area-b", 2);
        let threshold_ab = source(
            DemandSourceKind::Threshold,
            b"field=slope;threshold=0.4",
            vec![a.clone(), b.clone()],
        );
        let threshold_ba = source(
            DemandSourceKind::Threshold,
            b"field=slope;threshold=0.4",
            vec![b, a],
        );
        let specified = source(
            DemandSourceKind::Specified,
            b"specified-circle",
            vec![component(b"circle", 1)],
        );

        let left = snapshot(vec![threshold_ab, specified.clone()]);
        let right = snapshot(vec![specified, threshold_ba]);

        assert_eq!(left.snapshot_hash(), right.snapshot_hash());
        assert_eq!(left.canonical_bytes(), right.canonical_bytes());
        assert!(left
            .sources()
            .windows(2)
            .all(|pair| pair[0].source_id() < pair[1].source_id()));
        for source in left.sources() {
            assert!(source
                .components()
                .windows(2)
                .all(|pair| pair[0].component_id() < pair[1].component_id()));
        }
    }

    #[test]
    fn semantic_changes_change_snapshot_hash() {
        let level_1 = snapshot(vec![source(
            DemandSourceKind::Specified,
            b"circle",
            vec![component(b"circle", 1)],
        )]);
        let level_2 = snapshot(vec![source(
            DemandSourceKind::Specified,
            b"circle",
            vec![component(b"circle", 2)],
        )]);

        assert_ne!(level_1.snapshot_hash(), level_2.snapshot_hash());
        assert_eq!(
            level_1.sources()[0].components()[0].component_id(),
            level_2.sources()[0].components()[0].component_id(),
            "component lineage is support-based while content commits the level"
        );
        assert_ne!(
            level_1.sources()[0].components()[0].content_hash(),
            level_2.sources()[0].components()[0].content_hash()
        );
    }

    #[test]
    fn epochs_are_append_only_chained_and_repeated_payloads_are_rejected() {
        let base = snapshot(vec![source(
            DemandSourceKind::Specified,
            b"base",
            vec![component(b"base", 1)],
        )]);
        let base_hash = base.snapshot_hash();
        let mut chain = DemandEpochChain::new(base);
        let first_source = source(
            DemandSourceKind::AutoRefine,
            b"violations=edge-cv:a",
            vec![component(b"repair-a", 2)],
        );
        let first = chain
            .append_epoch(vec![first_source.clone()], &test_hash)
            .unwrap()
            .clone();
        assert_eq!(first.epoch_id(), 1);
        assert_eq!(first.parent_snapshot_hash(), base_hash);

        let second = chain
            .append_epoch(
                vec![source(
                    DemandSourceKind::AutoRefine,
                    b"violations=edge-cv:b",
                    vec![component(b"repair-b", 3)],
                )],
                &test_hash,
            )
            .unwrap()
            .clone();
        assert_eq!(second.epoch_id(), 2);
        assert_eq!(second.parent_snapshot_hash(), first.epoch_hash());
        assert_eq!(chain.epochs()[0], first, "append must not rewrite epoch 1");

        let before = chain.epochs().to_vec();
        let error = chain
            .append_epoch(vec![first_source], &test_hash)
            .unwrap_err();
        assert!(matches!(
            error,
            DemandSnapshotError::RepeatedDemandEpoch {
                existing_epoch_id: 1,
                ..
            }
        ));
        assert_eq!(
            chain.epochs(),
            before,
            "rejection must leave the chain intact"
        );
    }

    #[test]
    fn epoch_hash_ignores_source_insertion_order() {
        let base = snapshot(Vec::new());
        let source_a = source(DemandSourceKind::AutoRefine, b"a", vec![component(b"a", 1)]);
        let source_b = source(DemandSourceKind::AutoRefine, b"b", vec![component(b"b", 2)]);
        let mut left = DemandEpochChain::new(base.clone());
        let mut right = DemandEpochChain::new(base);

        let left_epoch = left
            .append_epoch(vec![source_a.clone(), source_b.clone()], &test_hash)
            .unwrap();
        let right_epoch = right
            .append_epoch(vec![source_b, source_a], &test_hash)
            .unwrap();

        assert_eq!(left_epoch.demand_hash(), right_epoch.demand_hash());
        assert_eq!(left_epoch.epoch_hash(), right_epoch.epoch_hash());
        assert_eq!(left_epoch.canonical_bytes(), right_epoch.canonical_bytes());
    }

    #[test]
    fn chain_rejects_a_different_hash_function() {
        let base = snapshot(Vec::new());
        let mut chain = DemandEpochChain::new(base);
        let zero_hash = |_: &[u8]| DemandHash::from_bytes([0; 32]);

        let error = chain
            .append_epoch(
                vec![source(
                    DemandSourceKind::AutoRefine,
                    b"a",
                    vec![component(b"a", 1)],
                )],
                &zero_hash,
            )
            .unwrap_err();

        assert_eq!(error, DemandSnapshotError::InconsistentHasher);
        assert!(chain.epochs().is_empty());
    }

    #[test]
    fn duplicate_semantic_components_and_sources_are_rejected() {
        let duplicate_component = component(b"same", 1);
        let error = DemandSource::new(
            DemandSourceKind::Specified,
            b"source".to_vec(),
            vec![duplicate_component.clone(), duplicate_component],
            &test_hash,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            DemandSnapshotError::DuplicateComponent { .. }
        ));

        let duplicate_source = source(
            DemandSourceKind::Specified,
            b"source",
            vec![component(b"same", 1)],
        );
        let error = SourceDemandSnapshot::new(
            test_hash(b"project"),
            vec![duplicate_source.clone(), duplicate_source],
            &test_hash,
        )
        .unwrap_err();
        assert!(matches!(error, DemandSnapshotError::DuplicateSource { .. }));
    }
}
