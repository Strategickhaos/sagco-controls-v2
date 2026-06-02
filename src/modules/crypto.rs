use sha2::{Digest, Sha256};

/// Returns lowercase hex SHA256 of any byte slice.
pub fn seal(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    format!("{:x}", hash)
}

/// Reads a file from disk and returns its SHA256 hex string.
pub fn seal_file(path: &str) -> String {
    let bytes = std::fs::read(path).unwrap_or_default();
    seal(&bytes)
}
