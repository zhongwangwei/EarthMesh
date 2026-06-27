mod discovery;

pub use discovery::{discover_mask_sources, MaskSourceDiscovery};
pub(crate) use discovery::{source_extension, unsupported_mask_source};
