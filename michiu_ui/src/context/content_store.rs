use std::{
    borrow::Cow,
    time::{Duration, Instant},
};

use crate::*;
use slotmap::{SecondaryMap, SparseSecondaryMap};

pub(crate) type TextContentsSparseSecondary = SparseSecondaryMap<EntityId, Cow<'static, str>>;
pub(crate) type TextSpansSparseSecondary = SparseSecondaryMap<EntityId, Vec<TextSpan>>;
pub(crate) type InputContentsSparseSecondary = SparseSecondaryMap<EntityId, InputContents>;
pub(crate) type ImageSourcesSparseSecondary = SparseSecondaryMap<EntityId, ImageSource>;
pub(crate) type MoviePropertiesSparseSecondary = SparseSecondaryMap<EntityId, MovieProperty>;
pub(crate) type WebviewContentsSparseSecondary = SparseSecondaryMap<EntityId, WebView2Contents>;

pub struct ContentStore {
    pub(crate) text_contents: TextContentsSparseSecondary,
    pub(crate) text_spans: TextSpansSparseSecondary,
    pub(crate) input_contents: InputContentsSparseSecondary,
    pub(crate) image_sources: ImageSourcesSparseSecondary,
    pub(crate) movie_properties: MoviePropertiesSparseSecondary,
    pub(crate) webview_contents: WebviewContentsSparseSecondary,
}

impl Default for ContentStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentStore {
    #[inline]
    pub fn new() -> Self {
        Self {
            text_contents: SparseSecondaryMap::new(),
            text_spans: SparseSecondaryMap::new(),
            input_contents: SparseSecondaryMap::new(),
            image_sources: SparseSecondaryMap::new(),
            movie_properties: SparseSecondaryMap::new(),
            webview_contents: SparseSecondaryMap::new(),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.text_contents.clear();
        self.text_spans.clear();
        self.input_contents.clear();
        self.image_sources.clear();
        self.movie_properties.clear();
        self.webview_contents.clear();
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.text_contents.remove(id);
        self.text_spans.remove(id);
        self.input_contents.remove(id);
        self.image_sources.remove(id);
        self.movie_properties.remove(id);
        self.webview_contents.remove(id);
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
        text_spans: &TextSpansSparseSecondary,
    ) -> &[TextSpan] {
        text_spans.get(id).map(|s| s.as_slice()).unwrap_or(&[])
    }

    /// テキストやインプットのサイズを DirectWrite を用いて計測し、Taffy 向けサイズを返します。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn measure_content(
        id: EntityId,
        input_contents: &mut InputContentsSparseSecondary,
        text_contents: &TextContentsSparseSecondary,
        text_spans: &TextSpansSparseSecondary,
        active_masks: &ActiveMasksSecondary,
        visual_properties: &VisualPropertiesSecondary,
        text_engine: &TextEngine,
        known_dims: taffy::Size<Option<f32>>,
    ) -> taffy::Size<f32> {
        let mask = active_masks.get(id).copied().unwrap_or_default();

        // 入力かつキャッシュが既に存在する場合は即座にそのサイズを早期リターン
        if mask.has_input_content()
            && let Some(contents) = input_contents.get(id)
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

        let text = text_contents.get(id).map(|s| s.as_ref()).unwrap_or("");
        let (font_size, font_family, font_weight, font_style) =
            RenderStore::get_font_propery(id, visual_properties);
        let max_width = None;
        let spans = ContentStore::get_text_span(id, text_spans);

        // DirectWrite を使用して正確なサイズを計測
        let size = text_engine.measure_text(
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
            && let Some(contents) = input_contents.get_mut(id)
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

impl Context {
    #[inline]
    pub(crate) fn should_show_caret(&self, contents: &InputContents) -> bool {
        ContentStore::should_show_caret(contents)
    }

    /// テキストやインプットのサイズを DirectWrite を用いて計測し、Taffy 向けサイズを返します。
    #[inline]
    pub(crate) fn measure_content(
        &mut self,
        id: EntityId,
        known_dims: taffy::Size<Option<f32>>,
    ) -> taffy::Size<f32> {
        let TopologyStore { active_masks, .. } = &mut self.topology;
        let RenderStore {
            visual_properties, ..
        } = &self.renders;
        let ContentStore {
            input_contents,
            text_contents,
            text_spans,
            ..
        } = &mut self.contents;
        let SystemStore { text_engine, .. } = &mut self.system;

        ContentStore::measure_content(
            id,
            input_contents,
            text_contents,
            text_spans,
            active_masks,
            visual_properties,
            text_engine,
            known_dims,
        )
    }
}
