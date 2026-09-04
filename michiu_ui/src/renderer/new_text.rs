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

#[derive(Clone, Debug, Copy)]
pub(crate) struct NewTextCacheValue {
    pub(crate) uv_min: [f32; 2],
    pub(crate) uv_max: [f32; 2],
    pub(crate) offset_x: i32,
    pub(crate) offset_y: i32,
}

pub(crate) struct NewRendererView<'a> {
    pub(crate) render_data: &'a mut RenderData,
    pub(crate) atlas: &'a mut TextureAtlas,
    pub(crate) text_rasterizer: &'a TextRasterizer,
    pub(crate) text_cache: &'a mut FxHashMap<NewTextCacheKey, NewTextCacheValue>,
    pub(crate) queue: &'a wgpu::Queue,
}
