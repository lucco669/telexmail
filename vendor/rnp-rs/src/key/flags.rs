//! Bit-flag types for key operations.
//!
//! Each wraps a set of `RNP_*` C-side constants with a typed Rust newtype
//! and `BitOr` for combining.

use crate::ffi;

/// Flags for `Context::load_keys` / `save_keys`. Wraps the `RNP_LOAD_SAVE_*`
/// constants.
#[derive(Clone, Copy, Debug, Default)]
pub struct LoadSaveFlags(pub u32);

impl LoadSaveFlags {
    pub const PUBLIC: Self = Self(ffi::RNP_LOAD_SAVE_PUBLIC_KEYS as u32);
    pub const SECRET: Self = Self(ffi::RNP_LOAD_SAVE_SECRET_KEYS as u32);
    pub const PERMISSIVE: Self = Self(ffi::RNP_LOAD_SAVE_PERMISSIVE as u32);
    pub const SINGLE: Self = Self(ffi::RNP_LOAD_SAVE_SINGLE as u32);
    pub const BASE64: Self = Self(ffi::RNP_LOAD_SAVE_BASE64 as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for LoadSaveFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Flags for `Key::export`. Wraps the `RNP_KEY_EXPORT_*` constants.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExportFlags(pub u32);

impl ExportFlags {
    pub const ARMORED: Self = Self(ffi::RNP_KEY_EXPORT_ARMORED as u32);
    pub const PUBLIC: Self = Self(ffi::RNP_KEY_EXPORT_PUBLIC as u32);
    pub const SECRET: Self = Self(ffi::RNP_KEY_EXPORT_SECRET as u32);
    pub const SUBKEYS: Self = Self(ffi::RNP_KEY_EXPORT_SUBKEYS as u32);
    pub const BASE64: Self = Self(ffi::RNP_KEY_EXPORT_BASE64 as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for ExportFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Flags for `Key::remove`. Wraps the `RNP_KEY_REMOVE_*` constants.
#[derive(Clone, Copy, Debug, Default)]
pub struct RemoveFlags(pub u32);

impl RemoveFlags {
    pub const PUBLIC: Self = Self(ffi::RNP_KEY_REMOVE_PUBLIC as u32);
    pub const SECRET: Self = Self(ffi::RNP_KEY_REMOVE_SECRET as u32);
    pub const SUBKEYS: Self = Self(ffi::RNP_KEY_REMOVE_SUBKEYS as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for RemoveFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Flags for `Key::remove_signatures`. Wraps the `RNP_KEY_SIGNATURE_*`
/// selector constants.
#[derive(Clone, Copy, Debug, Default)]
pub struct RemoveSignaturesFlags(pub u32);

impl RemoveSignaturesFlags {
    pub const INVALID: Self = Self(ffi::RNP_KEY_SIGNATURE_INVALID as u32);
    pub const UNKNOWN_KEY: Self = Self(ffi::RNP_KEY_SIGNATURE_UNKNOWN_KEY as u32);
    pub const NON_SELF_SIG: Self = Self(ffi::RNP_KEY_SIGNATURE_NON_SELF_SIG as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for RemoveSignaturesFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Flags for `Context::unload_keys`. Wraps the `RNP_KEY_UNLOAD_*` constants.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnloadFlags(pub u32);

impl UnloadFlags {
    pub const PUBLIC: Self = Self(ffi::RNP_KEY_UNLOAD_PUBLIC as u32);
    pub const SECRET: Self = Self(ffi::RNP_KEY_UNLOAD_SECRET as u32);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for UnloadFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
