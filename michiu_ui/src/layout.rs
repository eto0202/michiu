use crate::{Context, EntityId};

/// テキストのサイズを仮計測する内部ロジック
pub(crate) fn measure_text_mock_internal(
    cx: &Context,
    id: EntityId,
    known_dimensions: taffy::Size<Option<f32>>,
    available_space: taffy::Size<taffy::AvailableSpace>,
) -> taffy::Size<f32> {
    let text = cx.text_contents.get(id).map(|s| s.as_ref()).unwrap_or("");
    let font_size = cx
        .visual_properties
        .get(id)
        .and_then(|v| v.font_size)
        .unwrap_or(16.0);

    // フェーズ3: 仮の計算 (1文字の幅 0.5em, 高さ 1.2em)
    let char_width = font_size * 0.5;
    let char_height = font_size * 1.2;
    let text_len = text.chars().count() as f32;

    let width = match available_space.width {
        taffy::AvailableSpace::Definite(w) => (text_len * char_width).min(w),
        _ => text_len * char_width,
    };

    let lines = if width > 0.0 {
        ((text_len * char_width) / width).ceil().max(1.0)
    } else {
        1.0
    };

    taffy::Size {
        width: known_dimensions.width.unwrap_or(width),
        height: known_dimensions.height.unwrap_or(lines * char_height),
    }
}
