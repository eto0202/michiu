use crate::{LayoutSize, RenderData, TextCacheValue, TextRasterizer, TextSpan, TextureAtlas};
use cosmic_text::{
    Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight, Wrap,
};
use rustc_hash::FxHashMap;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct NewTextCacheKey {
    pub(crate) cache_key: CacheKey,
}
