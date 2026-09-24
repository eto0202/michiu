use crate::{
    ActiveMasksSecondary, CapacityConfig, EntityId, ExternalTexture, ExternalVisual, InputContents,
    LayoutRect, MichiuString, TextLayoutSize, TextSpan, VisualPropertiesSecondary,
    define_sparse_secondary, soa::MichiuSoA,
};
use slotmap::SparseSecondaryMap;
use std::sync::Arc;

define_sparse_secondary!(pub struct TextContentsSparse(MichiuString));
define_sparse_secondary!(pub struct TextSpansSparse(Vec<TextSpan>));
define_sparse_secondary!(pub struct InputContentsSparse(InputContents));

impl InputContentsSparse {
    /// テキストやインプットのサイズを cosmic-text を用いて計測し、Taffy 向けサイズを返す。\n\
    /// F の第一引数は `auto_wrap`, 第二引数は Option<`max_width`> 。
    pub(crate) fn measure_content<F>(
        &mut self,
        id: EntityId,
        known_dims: taffy::Size<Option<f32>>,
        available_space: taffy::Size<taffy::AvailableSpace>,
        topo_active_masks: &ActiveMasksSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        measure_text: F,
    ) -> taffy::Size<f32>
    where
        F: FnOnce(bool, Option<f32>) -> TextLayoutSize,
    {
        let mask = topo_active_masks.at(id);
        let has_input = mask.has_input_content();
        let has_text = mask.has_text_content();

        // キャッシュの取得
        let last_layout = if has_input {
            // has_input が true なら Some のはず
            self.at(id).last_layout
        } else {
            None
        };

        // キャッシュがなく、かつテキストも持たない場合
        if last_layout.is_none() && !has_text {
            return taffy::Size {
                width: known_dims.width.unwrap_or(0.0),
                height: known_dims.height.unwrap_or(0.0),
            };
        }

        // 折り返し設定と最大幅
        let auto_wrap = rnd_visual.auto_wrap(id);

        let max_width = if auto_wrap {
            known_dims.width.or({
                if let taffy::AvailableSpace::Definite(w) = available_space.width {
                    Some(w)
                } else {
                    None
                }
            })
        } else {
            None
        };

        // 自動折り返しがない（1行入力、または折り返し無効の複数行）場合
        // 文字が変わらない限りサイズは絶対に変わらないので、前回のサイズを即座に返す
        if !auto_wrap && let Some(layout) = last_layout {
            return taffy::Size {
                width: known_dims.width.unwrap_or(layout.width),
                height: known_dims.height.unwrap_or(layout.height),
            };
        }

        // キャッシュが存在し、かつ幅が変わっていない場合
        if let Some(layout) = last_layout {
            // 制限幅が確定していない、または前回計測時と同じなら再利用
            if max_width.is_none() {
                return taffy::Size {
                    width: known_dims.width.unwrap_or(layout.width),
                    height: known_dims.height.unwrap_or(layout.height),
                };
            }
        }

        // キャッシュが無効（幅が変更された）で、かつテキストを持たない場合
        if !has_text {
            return taffy::Size {
                width: known_dims.width.unwrap_or(0.0),
                height: known_dims.height.unwrap_or(0.0),
            };
        }

        let size = measure_text(auto_wrap, max_width);

        // 計測した文字自体の正確なサイズをここでインプット要素にキャッシュする
        if has_input {
            // has_input が true なら Some のはず
            let contents = self.at_mut(id);
            contents.last_layout = Some(LayoutRect::new(0.0, 0.0, size.width, size.height));
        }

        // 文字のみのサイズを返す
        taffy::Size {
            width: known_dims.width.unwrap_or(size.width),
            height: known_dims.height.unwrap_or(size.height),
        }
    }
}

impl TextSpansSparse {
    pub(crate) fn span(&self, id: EntityId) -> &[TextSpan] {
        self.find(id).map_or(&[][..], Vec::as_slice)
    }
}

#[derive(Default, Clone, derive_more::Deref, derive_more::DerefMut, derive_more::IntoIterator)]
#[into_iterator(owned, ref, ref_mut)]
pub struct ExternalTextureSparse(pub SparseSecondaryMap<EntityId, Arc<dyn ExternalTexture>>);

impl MichiuSoA for ExternalTextureSparse {
    type Item = Arc<dyn ExternalTexture>;
    #[inline]
    fn find(&self, id: EntityId) -> Option<&Self::Item> {
        self.0.get(id)
    }
    #[inline]
    fn find_mut(&mut self, id: EntityId) -> Option<&mut Self::Item> {
        self.0.get_mut(id)
    }
}
impl std::fmt::Debug for ExternalTextureSparse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut map = f.debug_map();
        for (id, tex) in &self.0 {
            map.entry(
                &id,
                &format_args!(
                    "ExternalTexture {{ ptr: {:p}, meta: {:?} }}",
                    Arc::as_ptr(tex),
                    tex.metadata()
                ),
            );
        }
        map.finish()
    }
}

#[derive(Default, Clone, derive_more::Deref, derive_more::DerefMut, derive_more::IntoIterator)]
#[into_iterator(owned, ref, ref_mut)]
pub struct ExternalVisualSparse(pub SparseSecondaryMap<EntityId, Arc<dyn ExternalVisual>>);

impl MichiuSoA for ExternalVisualSparse {
    type Item = Arc<dyn ExternalVisual>;
    #[inline]
    fn find(&self, id: EntityId) -> Option<&Self::Item> {
        self.0.get(id)
    }
    #[inline]
    fn find_mut(&mut self, id: EntityId) -> Option<&mut Self::Item> {
        self.0.get_mut(id)
    }
}
impl std::fmt::Debug for ExternalVisualSparse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut map = f.debug_map();
        for (id, tex) in &self.0 {
            map.entry(
                &id,
                &format_args!(
                    "ExternalVisual {{ ptr: {:p}, meta: {:?} }}",
                    Arc::as_ptr(tex),
                    tex.metadata()
                ),
            );
        }
        map.finish()
    }
}

pub struct ContentStore {
    pub(crate) cont_text_contents: TextContentsSparse,
    pub(crate) cont_text_spans: TextSpansSparse,
    pub(crate) cont_input_contents: InputContentsSparse,
    pub(crate) cont_external_textures: ExternalTextureSparse,
    pub(crate) cont_external_visual: ExternalVisualSparse,
    pub(crate) cont_cut_text: Option<MichiuString>,
}

impl Default for ContentStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentStore {
    #[inline]
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            cont_text_contents: TextContentsSparse(SparseSecondaryMap::new()),
            cont_text_spans: TextSpansSparse(SparseSecondaryMap::new()),
            cont_input_contents: InputContentsSparse(SparseSecondaryMap::new()),
            cont_external_textures: ExternalTextureSparse(SparseSecondaryMap::new()),
            cont_external_visual: ExternalVisualSparse(SparseSecondaryMap::new()),
            cont_cut_text: None,
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn with_capacity(c: &CapacityConfig) -> Self {
        Self {
            cont_text_contents: TextContentsSparse(SparseSecondaryMap::with_capacity(
                c.edit_selections,
            )),
            cont_text_spans: TextSpansSparse(SparseSecondaryMap::with_capacity(c.cont_text_spans)),
            cont_input_contents: InputContentsSparse(SparseSecondaryMap::with_capacity(
                c.cont_input_contents,
            )),
            cont_external_textures: ExternalTextureSparse(SparseSecondaryMap::with_capacity(
                c.cont_external_textures,
            )),
            cont_external_visual: ExternalVisualSparse(SparseSecondaryMap::with_capacity(
                c.cont_external_visual,
            )),
            ..Default::default()
        }
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.cont_text_contents.clear();
        self.cont_text_spans.clear();
        self.cont_input_contents.clear();
        self.cont_external_textures.clear();
        self.cont_external_visual.clear();
        self.cont_cut_text = None;
    }

    #[inline]
    pub(crate) fn despawn(&mut self, id: EntityId) {
        self.cont_text_contents.remove(id);
        self.cont_text_spans.remove(id);
        self.cont_input_contents.remove(id);
        self.cont_external_textures.remove(id);
        self.cont_external_visual.remove(id);
        self.cont_cut_text = None;
    }

    #[inline]
    pub fn cut_text_mut(&mut self) -> &mut Option<MichiuString> {
        &mut self.cont_cut_text
    }

    #[inline]
    pub fn external_textures_mut(&mut self) -> &mut ExternalTextureSparse {
        &mut self.cont_external_textures
    }

    #[inline]
    pub fn input_contents_mut(&mut self) -> &mut InputContentsSparse {
        &mut self.cont_input_contents
    }

    #[inline]
    pub fn cont_text_contents_mut(&mut self) -> &mut TextContentsSparse {
        &mut self.cont_text_contents
    }

    #[inline]
    pub fn text_spans_mut(&mut self) -> &mut TextSpansSparse {
        &mut self.cont_text_spans
    }

    #[inline]
    pub fn webview_contents_mut(&mut self) -> &mut ExternalVisualSparse {
        &mut self.cont_external_visual
    }
}

#[cfg(test)]
mod tests;
