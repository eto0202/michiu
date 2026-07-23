use std::time::{Duration, Instant};

use crate::*;
use slotmap::{SecondaryMap, SparseSecondaryMap};

pub struct ContentStore {
    pub(crate) text_contents: SparseSecondaryMap<EntityId, std::borrow::Cow<'static, str>>,
    pub(crate) text_spans: SparseSecondaryMap<EntityId, Vec<TextSpan>>,
    pub(crate) input_contents: SparseSecondaryMap<EntityId, InputContents>,
    pub(crate) image_sources: SparseSecondaryMap<EntityId, ImageSource>,
    pub(crate) movie_properties: SparseSecondaryMap<EntityId, MovieProperty>,
    pub(crate) webview_contents: SparseSecondaryMap<EntityId, WebView2Contents>,
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

        if contents.is_blink {
            let freq = contents
                .blink_frequency
                .unwrap_or(Duration::from_millis(530))
                .as_millis();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            (now / freq).is_multiple_of(2)
        } else {
            contents.has_caret
        }
    }

    /// テキストやインプットのサイズを DirectWrite を用いて計測し、Taffy 向けサイズを返します。
    pub(crate) fn measure_content(
        id: EntityId,
        contents: &mut ContentStore,
        active_masks: &SecondaryMap<EntityId, ComponentMask>,
        visual_properties: &SecondaryMap<EntityId, VisualProperty>,
        text_engine: &TextEngine,
        known_dims: taffy::Size<Option<f32>>,
    ) -> taffy::Size<f32> {
        let mask = active_masks.get(id).copied().unwrap_or_default();

        if mask.has(COMP_INPUT_CONTENT)
            && let Some(contents) = contents.input_contents.get(id)
            && let Some(layout_rect) = contents.last_layout
        {
            return taffy::Size {
                width: known_dims.width.unwrap_or(layout_rect.width),
                height: known_dims.height.unwrap_or(layout_rect.height),
            };
        }

        if mask.has(COMP_TEXT_CONTENT) {
            let text = contents
                .text_contents
                .get(id)
                .map(|s| s.as_ref())
                .unwrap_or("");
            let (font_size, font_family, font_weight, font_style) = visual_properties
                .get(id)
                .map(|v| {
                    (
                        v.font_size.unwrap_or(16.0),
                        v.font_family.as_deref(),
                        v.font_weight,
                        v.font_style,
                    )
                })
                // もし該当要素に VisualProperty 自体がなければデフォルト値をあてる
                .unwrap_or((16.0, None, None, None));

            let max_width = None;

            let spans = contents
                .text_spans
                .get(id)
                .map(|s| s.as_slice())
                .unwrap_or(&[]);

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
            if mask.has(COMP_INPUT_CONTENT)
                && let Some(contents) = contents.input_contents.get_mut(id)
            {
                contents.last_layout = Some(LayoutRect::new(0.0, 0.0, size.width, size.height));
            }

            // 文字のみのサイズ
            return taffy::Size {
                width: known_dims.width.unwrap_or(size.width),
                height: known_dims.height.unwrap_or(size.height),
            };
        }

        // テキストも入力も持たない空の div 等の場合、
        // スタイルに割り当てられたサイズがあればそれを優先して返し、無ければ ZERO とする
        taffy::Size {
            width: known_dims.width.unwrap_or(0.0),
            height: known_dims.height.unwrap_or(0.0),
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
        visual_properties: &SecondaryMap<EntityId, VisualProperty>,
        known_dims: taffy::Size<Option<f32>>,
    ) -> taffy::Size<f32> {
        ContentStore::measure_content(
            id,
            &mut self.contents,
            &self.topology.active_masks,
            visual_properties,
            &self.system.text_engine,
            known_dims,
        )
    }
}
