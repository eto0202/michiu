use std::sync::atomic::{AtomicU64, Ordering};

use crate::LayoutSize;

pub trait ExternalMask: Send + Sync {
    /// Cache keys (such as icon paths and hash values)
    fn metadata(&self) -> ExternalMaskMetadata;

    /// Called when a rendering request is made.
    /// Returns a byte array of A8 (R8Unorm) values corresponding to the specified size, along with the actual pixel size.
    /// If the input is a vector, CPU rasterization is performed here.
    fn resolve_mask(&self, width: u32, height: u32) -> MaskImage;
}

pub struct MaskImage {
    pub width: u32,
    pub height: u32,
    /// Byte array in R8Unorm format
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExternalMaskMetadata {
    pub cache_key: u64,
    pub size: LayoutSize,
}

impl ExternalMaskMetadata {
    /// This is typically what we use. The library automatically assigns a unique ID.
    #[inline]
    #[must_use]
    pub fn new(size: LayoutSize) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self {
            cache_key: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            size,
        }
    }

    /// Use this when you want to share atlas regions (to prevent duplicate rasterization) across different instances by passing asset paths, string hashes, and the like.
    #[inline]
    #[must_use]
    pub fn with_cache_key(mut self, key: u64) -> Self {
        self.cache_key = key;
        self
    }
}
