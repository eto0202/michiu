use crate::{
    LayoutSize, RenderData, TextCacheKey, TextCacheValue, TextRasterizer, TextSpan, TextureAtlas,
};
use cosmic_text::{
    Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight, Wrap,
};
use rustc_hash::FxHashMap;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct NewTextCacheKey {
    pub(crate) cache_key: CacheKey,
}

pub(crate) struct NewRendererView<'a> {
    pub(crate) render_data: &'a mut RenderData,
    pub(crate) atlas: &'a mut TextureAtlas,
    pub(crate) text_rasterizer: &'a TextRasterizer,
    pub(crate) text_cache: &'a mut FxHashMap<NewTextCacheKey, TextCacheValue>,
    pub(crate) queue: &'a wgpu::Queue,
}
