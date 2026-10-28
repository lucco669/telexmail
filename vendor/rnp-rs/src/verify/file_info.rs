//! [`FileInfo`] — file metadata from the literal-data packet.

/// File metadata from the literal-data packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfo {
    pub name: String,
    pub mtime: u32,
}
