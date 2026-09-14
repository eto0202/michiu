use crate::{
    ActiveMasksSecondary, CapacityConfig, DebugStore, EntityId, ExternalTexture, FlexLayout,
    InputContents, LayoutRect, MichiuString, TextEngine, TextSpan, VisualPropertiesSecondary,
    WebView2Contents, define_sparse_secondary, soa::MichiuSoA,
};
use slotmap::SparseSecondaryMap;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

define_sparse_secondary!(pub struct TextContentsSparse(MichiuString));
define_sparse_secondary!(pub struct TextSpansSparse(Vec<TextSpan>));
define_sparse_secondary!(pub struct InputContentsSparse(InputContents));
define_sparse_secondary!(pub struct WebviewContentsSparse(WebView2Contents));

#[derive(Default, Clone, derive_more::Deref, derive_more::DerefMut, derive_more::IntoIterator)]
#[into_iterator(owned, ref, ref_mut)]
pub struct ExternalTextureSparse(pub SparseSecondaryMap<EntityId, Arc<dyn ExternalTexture>>);

impl MichiuSoA for ExternalTextureSparse {
    type Item = Arc<dyn ExternalTexture>;
    #[inline]
    fn get(&self, id: EntityId) -> Option<&Self::Item> {
        self.0.get(id)
    }
    #[inline]
    fn get_mut(&mut self, id: EntityId) -> Option<&mut Self::Item> {
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

pub struct ContentStore {
    pub(crate) cont_text_contents: TextContentsSparse,
    pub(crate) cont_text_spans: TextSpansSparse,
    pub(crate) cont_input_contents: InputContentsSparse,
    pub(crate) cont_external_textures: ExternalTextureSparse,
    pub(crate) cont_webview_contents: WebviewContentsSparse,
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
    pub fn new() -> Self {
        Self {
            cont_text_contents: TextContentsSparse(SparseSecondaryMap::new()),
            cont_text_spans: TextSpansSparse(SparseSecondaryMap::new()),
            cont_input_contents: InputContentsSparse(SparseSecondaryMap::new()),
            cont_external_textures: ExternalTextureSparse(SparseSecondaryMap::new()),
            cont_webview_contents: WebviewContentsSparse(SparseSecondaryMap::new()),
            cont_cut_text: None,
        }
    }

    #[inline]
    #[must_use]
    pub fn with_capacity(c: &CapacityConfig) -> Self {
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
            cont_webview_contents: WebviewContentsSparse(SparseSecondaryMap::with_capacity(
                c.cont_webview_contents,
            )),
            ..Default::default()
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.cont_text_contents.clear();
        self.cont_text_spans.clear();
        self.cont_input_contents.clear();
        self.cont_external_textures.clear();
        self.cont_webview_contents.clear();
        self.cont_cut_text = None;
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.cont_text_contents.remove(id);
        self.cont_text_spans.remove(id);
        self.cont_input_contents.remove(id);
        self.cont_external_textures.remove(id);
        self.cont_webview_contents.remove(id);
        self.cont_cut_text = None;
    }
}

impl ContentStore {
    /// キャレットの点滅と描画を行うかを判定
    pub(crate) fn should_show_caret(contents: &InputContents) -> bool {
        let now_instant = Instant::now();
        if let Some(last) = contents.last_interacted_time
            && now_instant.duration_since(last) < Duration::from_millis(300)
        {
            return true; // キー入力や移動の操作から 300ms 未満のときは常時表示
        }

        // 点滅しない場合はキャレットの有無をそのまま返す
        if !contents.is_blink {
            return contents.has_caret;
        }

        let freq = contents
            .blink_frequency
            .unwrap_or(Duration::from_millis(530))
            .as_millis();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        (now / freq).is_multiple_of(2)
    }

    /// テキストやインプットのサイズを cosmic-text を用いて計測し、Taffy 向けサイズを返します。
    pub(crate) fn measure_content(
        id: EntityId,
        known_dims: taffy::Size<Option<f32>>,
        available_space: taffy::Size<taffy::AvailableSpace>,
        flex: &FlexLayout,
        sys_text_engine: &mut TextEngine,
        cont_input_contents: &mut InputContentsSparse,
        cont_text_contents: &TextContentsSparse,
        cont_text_spans: &TextSpansSparse,
        topo_active_masks: &ActiveMasksSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        debug: &mut DebugStore,
    ) -> taffy::Size<f32> {
        let mask = topo_active_masks.at(id);
        let has_input = mask.has_input_content();
        let has_text = mask.has_text_content();

        // キャッシュの取得
        let layout_rect = if has_input {
            // has_input が true なら Some のはず
            cont_input_contents.at(id).last_layout
        } else {
            None
        };

        // キャッシュがなく、かつテキストも持たない場合
        if layout_rect.is_none() && !has_text {
            return taffy::Size {
                width: known_dims.width.unwrap_or(0.0),
                height: known_dims.height.unwrap_or(0.0),
            };
        }

        // 折り返し設定と最大幅
        let auto_wrap = rnd_visual
            .get(id)
            .and_then(|v| v.auto_wrap)
            .unwrap_or(false);

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
        if !auto_wrap && let Some(layout) = layout_rect {
            return taffy::Size {
                width: known_dims.width.unwrap_or(layout.width),
                height: known_dims.height.unwrap_or(layout.height),
            };
        }

        // キャッシュが存在し、かつ幅が変わっていない場合
        if let Some(layout) = layout_rect {
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

        let text = cont_text_contents.at(id);
        let font = rnd_visual
            .get(id)
            .map(|v| v.font.clone())
            .unwrap_or_default();

        let spans = cont_text_spans.get(id).map_or(&[][..], Vec::as_slice);

        let size = sys_text_engine.measure_text(
            text,
            font,
            flex.text_align,
            max_width,
            Some(auto_wrap),
            spans,
        );

        // 計測した文字自体の正確なサイズをここでインプット要素にキャッシュする
        if has_input {
            // has_input が true なら Some のはず
            let contents = cont_input_contents.at_mut(id);
            contents.last_layout = Some(LayoutRect::new(0.0, 0.0, size.width, size.height));
        }

        // 文字のみのサイズを返す
        taffy::Size {
            width: known_dims.width.unwrap_or(size.width),
            height: known_dims.height.unwrap_or(size.height),
        }
    }
}

#[cfg(test)]
mod tests;
