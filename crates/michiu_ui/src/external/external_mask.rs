use std::sync::atomic::{AtomicU64, Ordering};

use crate::LayoutSize;

pub trait ExternalMask: Send + Sync {
    /// キャッシュキー（アイコンのパスやハッシュ値など）
    fn metadata(&self) -> ExternalMaskMetadata;

    /// レンダリング要求時に呼ばれる。
    /// 指定されたサイズに応じた A8 (R8Unorm) のバイト列と実ピクセルサイズを返す。
    /// ベクターであればここで CPU ラスタライズを行う
    fn resolve_mask(&self, width: u32, height: u32) -> MaskImage;
}

pub struct MaskImage {
    pub width: u32,
    pub height: u32,
    /// R8Unorm フォーマットのバイト配列
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExternalMaskMetadata {
    pub cache_key: u64,
    pub size: LayoutSize,
}

impl ExternalMaskMetadata {
    /// 通常はこちらを使用。IDはライブラリ側で一意なものが自動採番される。
    #[inline]
    #[must_use]
    pub fn new(size: LayoutSize) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self {
            cache_key: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            size,
        }
    }

    /// アセットのパスや文字列ハッシュなどを渡し、
    /// 別のインスタンス間でもアトラス領域を共有（重複ラスタライズを防止）したい場合に使用。
    #[inline]
    #[must_use]
    pub fn with_cache_key(mut self, key: u64) -> Self {
        self.cache_key = key;
        self
    }
}
