use std::{
    borrow::Cow,
    time::{Duration, Instant},
};

use crate::{
    ActiveMasksSecondary, Context, EntityId, ImageSource, InputContents, LayoutRect, MovieProperty,
    RenderStore, SystemStore, TextEngine, TextSpan, TopologyStore, VisualPropertiesSecondary,
    WebView2Contents,
};
use slotmap::{SecondaryMap, SparseSecondaryMap};

pub(crate) type TextContentsSparseSecondary = SparseSecondaryMap<EntityId, Cow<'static, str>>;
pub(crate) type TextSpansSparseSecondary = SparseSecondaryMap<EntityId, Vec<TextSpan>>;
pub(crate) type InputContentsSparseSecondary = SparseSecondaryMap<EntityId, InputContents>;
pub(crate) type ImageSourcesSparseSecondary = SparseSecondaryMap<EntityId, ImageSource>;
pub(crate) type MoviePropertiesSparseSecondary = SparseSecondaryMap<EntityId, MovieProperty>;
pub(crate) type WebviewContentsSparseSecondary = SparseSecondaryMap<EntityId, WebView2Contents>;

pub struct ContentStore {
    pub(crate) cont_text_contents: TextContentsSparseSecondary,
    pub(crate) cont_text_spans: TextSpansSparseSecondary,
    pub(crate) cont_input_contents: InputContentsSparseSecondary,
    pub(crate) cont_image_sources: ImageSourcesSparseSecondary,
    pub(crate) cont_movie_properties: MoviePropertiesSparseSecondary,
    pub(crate) cont_webview_contents: WebviewContentsSparseSecondary,
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
            cont_image_sources: SparseSecondaryMap::new(),
            cont_movie_properties: SparseSecondaryMap::new(),
            cont_webview_contents: SparseSecondaryMap::new(),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.cont_text_contents.clear();
        self.cont_text_spans.clear();
        self.cont_input_contents.clear();
        self.cont_image_sources.clear();
        self.cont_movie_properties.clear();
        self.cont_webview_contents.clear();
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.cont_text_contents.remove(id);
        self.cont_text_spans.remove(id);
        self.cont_input_contents.remove(id);
        self.cont_image_sources.remove(id);
        self.cont_movie_properties.remove(id);
        self.cont_webview_contents.remove(id);
    }
}

impl ContentStore {
    /// キャレットの点滅と描画を行うかを判定します
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

    /// テキストやインプットのサイズを DirectWrite を用いて計測し、Taffy 向けサイズを返します。
    pub(crate) fn measure_content(
        id: EntityId,
        known_dims: taffy::Size<Option<f32>>,
        sys_text_engine: &TextEngine,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_contents: &TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        topo_active_masks: &ActiveMasksSecondary,
        ren_visual: &VisualPropertiesSecondary,
    ) -> taffy::Size<f32> {
        let mask = topo_active_masks.get(id).copied().unwrap_or_default();

        // 入力かつキャッシュが既に存在する場合は即座にそのサイズを早期リターン
        if mask.has_input_content()
            && let Some(contents) = cont_input_contents.get(id)
            && let Some(layout_rect) = contents.last_layout
        {
            return taffy::Size {
                width: known_dims.width.unwrap_or(layout_rect.width),
                height: known_dims.height.unwrap_or(layout_rect.height),
            };
        }

        // テキストを持たない場合は、デフォルト値を早期リターン
        if !mask.has_text_content() {
            return taffy::Size {
                width: known_dims.width.unwrap_or(0.0),
                height: known_dims.height.unwrap_or(0.0),
            };
        }

        let text = cont_text_contents
            .get(id)
            .map_or("", std::convert::AsRef::as_ref);
        let (font_size, font_family, font_weight, font_style) =
            RenderStore::get_font_propery(id, ren_visual);
        let max_width = None;
        let spans = ContentStore::get_text_span(id, cont_text_spans);

        // DirectWrite を使用して正確なサイズを計測
        let size = sys_text_engine.measure_text(
            text,
            font_size,
            font_family,
            font_weight,
            font_style,
            max_width,
            spans,
        );

        // 計測した文字自体の正確なサイズをここでインプット要素にキャッシュする
        if mask.has_input_content()
            && let Some(contents) = cont_input_contents.get_mut(id)
        {
            contents.last_layout = Some(LayoutRect::new(0.0, 0.0, size.width, size.height));
        }

        // 文字のみのサイズを返す
        taffy::Size {
            width: known_dims.width.unwrap_or(size.width),
            height: known_dims.height.unwrap_or(size.height),
        }
    }
}

