use std::fs;
use std::io;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

const CACHE_MAGIC: &[u8] = b"EARTHMESH_STAGE_CACHE_V1\0";

/// Lowercase hex SHA-256 of an in-memory byte slice.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// Streaming lowercase hex SHA-256 of a file's bytes.
///
/// Used to bind delivered artifacts to the exact inputs a run read, so the
/// whole file is not loaded into memory (the baseline meshes can be large).
pub fn file_sha256(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(hex(&hash.finalize()))
}

/// Build a stable SHA-256 key from ordered, length-delimited named inputs.
pub fn content_addressed_stage_key(stage: &str, parts: &[(&str, &[u8])]) -> String {
    let mut hash = Sha256::new();
    hash.update(b"earthmesh-stage-key-v1\0");
    hash_part(&mut hash, "stage", stage.as_bytes());
    for (name, bytes) in parts {
        hash_part(&mut hash, name, bytes);
    }
    hex(&hash.finalize())
}

fn hash_part(hash: &mut Sha256, name: &str, bytes: &[u8]) {
    hash.update((name.len() as u64).to_le_bytes());
    hash.update(name.as_bytes());
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Small content-addressed byte store. Corrupt entries are discarded and
/// treated as misses; writes use a same-directory temporary file plus rename.
#[derive(Clone, Debug)]
pub struct StageCache {
    root: PathBuf,
}

impl StageCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn load(&self, key: &str) -> io::Result<Option<Vec<u8>>> {
        let path = self.path(key)?;
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let header_len = CACHE_MAGIC.len() + 32;
        if bytes.len() < header_len || &bytes[..CACHE_MAGIC.len()] != CACHE_MAGIC {
            let _ = fs::remove_file(path);
            return Ok(None);
        }
        let expected = &bytes[CACHE_MAGIC.len()..header_len];
        let payload = &bytes[header_len..];
        if &Sha256::digest(payload)[..] != expected {
            let _ = fs::remove_file(path);
            return Ok(None);
        }
        Ok(Some(payload.to_vec()))
    }

    pub fn store(&self, key: &str, payload: &[u8]) -> io::Result<PathBuf> {
        let path = self.path(key)?;
        fs::create_dir_all(&self.root)?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temp = self
            .root
            .join(format!(".{key}.{}.{}.tmp", std::process::id(), nonce));
        let mut bytes = Vec::with_capacity(CACHE_MAGIC.len() + 32 + payload.len());
        bytes.extend_from_slice(CACHE_MAGIC);
        bytes.extend_from_slice(&Sha256::digest(payload));
        bytes.extend_from_slice(payload);
        fs::write(&temp, bytes)?;
        if let Err(error) = fs::rename(&temp, &path) {
            let _ = fs::remove_file(&temp);
            if !path.exists() {
                return Err(error);
            }
        }
        Ok(path)
    }

    fn path(&self, key: &str) -> io::Result<PathBuf> {
        if key.len() != 64 || !key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "stage cache key must be a 64-character hexadecimal SHA-256",
            ));
        }
        Ok(self.root.join(format!("{key}.bin")))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "earthmesh_stage_cache_{name}_{}",
            std::process::id()
        ))
    }

    #[test]
    fn key_is_ordered_length_delimited_and_content_sensitive() {
        let a = content_addressed_stage_key("hfield-v1", &[("a", b"bc"), ("d", b"e")]);
        let b = content_addressed_stage_key("hfield-v1", &[("ab", b"c"), ("d", b"e")]);
        let c = content_addressed_stage_key("hfield-v2", &[("a", b"bc"), ("d", b"e")]);
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn store_load_and_corruption_recovery() {
        let root = temp_root("roundtrip");
        let _ = fs::remove_dir_all(&root);
        let cache = StageCache::new(&root);
        let key = content_addressed_stage_key("test", &[("input", b"one")]);
        let path = cache.store(&key, b"payload").expect("store cache");
        assert_eq!(cache.load(&key).unwrap(), Some(b"payload".to_vec()));

        fs::write(&path, b"corrupt").unwrap();
        assert_eq!(cache.load(&key).unwrap(), None);
        assert!(!path.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sha256_known_answer_and_file_streaming_match() {
        // NIST FIPS 180-4 example: SHA-256("abc").
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(sha256_hex(b"abc"), expected);

        let root = temp_root("file_sha256");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("hash root");
        let path = root.join("payload.bin");
        // Larger than one 64 KiB read buffer to exercise the streaming loop.
        let mut payload = vec![0_u8; 200 * 1024];
        for (index, byte) in payload.iter_mut().enumerate() {
            *byte = (index % 251) as u8;
        }
        fs::write(&path, &payload).expect("write payload");
        assert_eq!(file_sha256(&path).expect("hash file"), sha256_hex(&payload));
        let _ = fs::remove_dir_all(root);
    }
}
