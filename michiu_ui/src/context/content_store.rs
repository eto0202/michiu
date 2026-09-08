use crate::{
    ActiveMasksSecondary, CapacityConfig, EntityId, ExternalTexture, FlexLayout, InputContents, LayoutRect, MichiuSoA, MichiuString, RenderStore, TextBufferSparseSecondary, TextEngine, TextSpan, VisualPropertiesSecondary, WebView2Contents,
};
use slotmap::{SecondaryMap, SparseSecondaryMap};
use std::{
    borrow::Cow,
    sync::Arc,
    time::{Duration, Instant},
};
use windows::Win32::Graphics::DirectWrite::IDWriteTextLayout;

pub(crate) type TextContentsSparseSecondary = SparseSecondaryMap<EntityId, MichiuString>;
pub(crate) type TextSpansSparseSecondary = SparseSecondaryMap<EntityId, Vec<TextSpan>>;
pub(crate) type InputContentsSparseSecondary = SparseSecondaryMap<EntityId, InputContents>;
pub(crate) type ExternalTextureSparseSecondary =
    SparseSecondaryMap<EntityId, Arc<dyn ExternalTexture>>;
pub(crate) type WebviewContentsSparseSecondary = SparseSecondaryMap<EntityId, WebView2Contents>;

pub struct ContentStore {
    pub(crate) cont_text_contents: TextContentsSparseSecondary,
    pub(crate) cont_text_spans: TextSpansSparseSecondary,
    pub(crate) cont_input_contents: InputContentsSparseSecondary,
    pub(crate) cont_external_textures: ExternalTextureSparseSecondary,
    pub(crate) cont_webview_contents: WebviewContentsSparseSecondary,
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
            cont_text_contents: SparseSecondaryMap::new(),
            cont_text_spans: SparseSecondaryMap::new(),
            cont_input_contents: SparseSecondaryMap::new(),
            cont_external_textures: SparseSecondaryMap::new(),
            cont_webview_contents: SparseSecondaryMap::new(),
            cont_cut_text: None,
        }
    }

    #[inline]
    #[must_use]
    pub fn with_capacity(c: &CapacityConfig) -> Self {
        Self {
            cont_text_contents: SparseSecondaryMap::with_capacity(c.edit_selections),
            cont_text_spans: SparseSecondaryMap::with_capacity(c.cont_text_spans),
            cont_input_contents: SparseSecondaryMap::with_capacity(c.cont_input_contents),
            cont_external_textures: SparseSecondaryMap::with_capacity(c.cont_external_textures),
            cont_webview_contents: SparseSecondaryMap::with_capacity(c.cont_webview_contents),
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

    #[inline]
    pub(crate) fn get_text_span(
        id: EntityId,
        cont_text_spans: &TextSpansSparseSecondary,
    ) -> &[TextSpan] {
        cont_text_spans.get(id).map_or(&[], Vec::as_slice)
    }

    /// テキストやインプットのサイズを cosmic-text を用いて計測し、Taffy 向けサイズを返します。
    pub(crate) fn measure_content(
        id: EntityId,
        known_dims: taffy::Size<Option<f32>>,
        available_space: taffy::Size<taffy::AvailableSpace>,
        flex: &FlexLayout,
        sys_text_engine: &mut TextEngine,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_contents: &TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        topo_active_masks: &ActiveMasksSecondary,
        rnd_visual: &VisualPropertiesSecondary,
    ) -> taffy::Size<f32> {
        let mask = topo_active_masks.at(id);
        let has_input = mask.has_input_content();
        let has_text = mask.has_text_content();

        // キャッシュの取得
        let layout_rect = if has_input {
            cont_input_contents.get(id).and_then(|c| c.last_layout)
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

        let mut max_width = if auto_wrap {
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

        let Some(text) = cont_text_contents.get(id) else {
            return taffy::Size {
                width: known_dims.width.unwrap_or(0.0),
                height: known_dims.height.unwrap_or(0.0),
            };
        };

        let font = rnd_visual
            .get(id)
            .map(|v| v.font.clone())
            .unwrap_or_default();

        let spans = ContentStore::get_text_span(id, cont_text_spans);

        let size = sys_text_engine.measure_text(
            text,
            font,
            flex.text_align,
            max_width,
            Some(auto_wrap),
            spans,
        );

        // 計測した文字自体の正確なサイズをここでインプット要素にキャッシュする
        if has_input && let Some(contents) = cont_input_contents.get_mut(id) {
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
