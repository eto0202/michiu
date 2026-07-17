#![allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ComponentMask(pub u128);

/// アニメーションやトランジションを設定可能なプロパティの一覧
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyList {
    BackgroundColor,
    BorderColor,
    Opacity,
    Transform,
    CornerRadius,
    Width,
    Height,
    Size,
    BoxShadow,
    Resizable,
}

impl PropertyList {
    /// 内部的なビットフラグ（ComponentMask）にマッピング
    #[inline]
    pub(crate) fn to_mask_bit(self) -> u128 {
        match self {
            PropertyList::BackgroundColor => STYLE_BG_COLOR,
            PropertyList::BorderColor => STYLE_BORDER_COLOR,
            PropertyList::Opacity => STYLE_OPACITY,
            PropertyList::Transform => STYLE_TRANSFORM,
            PropertyList::CornerRadius => STYLE_CORNER_RADIUS,
            PropertyList::Width | PropertyList::Height | PropertyList::Size => STYLE_SIZE,
            PropertyList::BoxShadow => STYLE_BOX_SHADOW,
            PropertyList::Resizable => STYLE_RESIZABLE,
        }
    }
}

impl ComponentMask {
    #[inline]
    pub fn new(flag: u128) -> Self {
        Self(flag)
    }

    #[inline]
    pub fn merge(&mut self, other: ComponentMask) {
        self.0 |= other.0;
    }

    #[inline]
    pub fn has(&self, flag: u128) -> bool {
        (self.0 & flag) != 0
    }

    #[inline]
    pub fn set(&mut self, flag: u128) {
        self.0 |= flag;
    }

    #[inline]
    pub fn unset(&mut self, flag: u128) {
        self.0 &= !flag;
    }

    /// 基本レイアウト関連のプロパティが1つでもあるか
    #[inline]
    pub fn has_basic_layout(&self) -> bool {
        self.has(STYLE_BASIC_LAYOUT)
    }

    /// Flexレイアウト関連のプロパティが1つでもあるか
    #[inline]
    pub fn has_flex_layout(&self) -> bool {
        self.has(STYLE_FLEX_LAYOUT)
    }

    #[inline]
    pub fn has_grid_layout(&self) -> bool {
        self.has(STYLE_GRID_LAYOUT)
    }

    #[inline]
    pub fn has_visual_property(&self) -> bool {
        self.has(STYLE_VISUAL_PROPERTY)
    }

    #[inline]
    pub fn has_interaction_property(&self) -> bool {
        self.has(STYLE_INTERACTION_PROPERTY)
    }

    #[inline]
    pub fn has_active_interaction_property(&self) -> bool {
        self.has(STYLE_ACTIVE_INTERACTION_PROPERTY)
    }

    #[inline]
    pub fn has_image_content(&self) -> bool {
        self.has(COMP_IMAGE_CONTENT)
    }

    #[inline]
    pub fn has_movie_content(&self) -> bool {
        self.has(COMP_MOVIE_CONTENT)
    }
}

pub(crate) const STYLE_DISPLAY: u128 = 1 << 0;
pub(crate) const STYLE_ITEM_IS_TABLE: u128 = 1 << 1;
pub(crate) const STYLE_ITEM_IS_REPLACED: u128 = 1 << 2;
pub(crate) const STYLE_BOX_SIZING: u128 = 1 << 3;
pub(crate) const STYLE_DIRECTION: u128 = 1 << 4;
pub(crate) const STYLE_OVERFLOW: u128 = 1 << 5;
pub(crate) const STYLE_SCROLLBAR: u128 = 1 << 6;

pub(crate) const STYLE_POSITION: u128 = 1 << 9;
pub(crate) const STYLE_INSET: u128 = 1 << 10; // top, right, bottom, left
pub(crate) const STYLE_SIZE: u128 = 1 << 11; // width, height
pub(crate) const STYLE_MIN_SIZE: u128 = 1 << 12; // min_width, min_height
pub(crate) const STYLE_MAX_SIZE: u128 = 1 << 13; // max_width, max_height
pub(crate) const STYLE_ASPECT_RATIO: u128 = 1 << 14;
pub(crate) const STYLE_MARGIN: u128 = 1 << 15; // margin 4方向
pub(crate) const STYLE_PADDING: u128 = 1 << 16; // padding 4方向
pub(crate) const STYLE_BORDER: u128 = 1 << 17; // border 4方向 (太さのみ)

pub(crate) const STYLE_ALIGN_ITEMS: u128 = 1 << 18;
pub(crate) const STYLE_ALIGN_SELF: u128 = 1 << 19;
pub(crate) const STYLE_JUSTIFY_ITEMS: u128 = 1 << 20;
pub(crate) const STYLE_JUSTIFY_SELF: u128 = 1 << 21;
pub(crate) const STYLE_ALIGN_CONTENT: u128 = 1 << 22;
pub(crate) const STYLE_JUSTIFY_CONTENT: u128 = 1 << 23;
pub(crate) const STYLE_GAP: u128 = 1 << 24; // row_gap, column_gap
pub(crate) const STYLE_TEXT_ALIGN: u128 = 1 << 25;
pub(crate) const STYLE_FLEX_DIRECTION: u128 = 1 << 26;
pub(crate) const STYLE_FLEX_WRAP: u128 = 1 << 27;
pub(crate) const STYLE_FLEX_BASIS: u128 = 1 << 28;
pub(crate) const STYLE_FLEX_GROW: u128 = 1 << 29;
pub(crate) const STYLE_FLEX_SHRINK: u128 = 1 << 30;

pub(crate) const STYLE_BG_COLOR: u128 = 1 << 31;
pub(crate) const STYLE_BORDER_COLOR: u128 = 1 << 32;
pub(crate) const STYLE_CORNER_RADIUS: u128 = 1 << 33; // top_left, top_right...
pub(crate) const STYLE_OPACITY: u128 = 1 << 34;
pub(crate) const STYLE_BOX_SHADOW: u128 = 1 << 35;
pub(crate) const STYLE_TRANSFORM: u128 = 1 << 37; // 2D/3D 座標変換行列
pub(crate) const STYLE_Z_INDEX: u128 = 1 << 38;
pub(crate) const STYLE_CURSOR: u128 = 1 << 39;
pub(crate) const STYLE_BACKDROP: u128 = 1 << 40;
pub(crate) const STYLE_TEXT_COLOR: u128 = 1 << 41;
pub(crate) const STYLE_FONT_SIZE: u128 = 1 << 42;

pub(crate) const STATE_HOVERED: u128 = 1 << 43;
pub(crate) const STATE_FOCUSED: u128 = 1 << 44;
pub(crate) const STATE_PRESSED: u128 = 1 << 45;
pub(crate) const STATE_DISABLED: u128 = 1 << 46;
pub(crate) const STATE_ACTIVED: u128 = 1 << 47;
pub(crate) const STATE_SELECTED: u128 = 1 << 48;
pub(crate) const STATE_DRAGGED: u128 = 1 << 49;
pub(crate) const STYLE_INTERACTION_WITHIN: u128 = 1 << 7; // 親に focus_within 等のスタイル定義が存在することを示す

// Vec等を含む重い構造体。SparseSecondaryMap に実体を逃がす。
/// TaffyのGridレイアウト用の全プロパティ（grid_template_rows等：Vecを多数含む）
pub(crate) const STYLE_GRID_LAYOUT: u128 = 1 << 50;
/// 動的キーフレームアニメーションの定義シーケンス（Vec含む）
pub(crate) const STYLE_ANIMATIONS: u128 = 1 << 51;
/// RichText用の複数スパン情報やスタイリング（Vec含む）
pub(crate) const STYLE_TEXT_SPANS: u128 = 1 << 52;
/// 複雑な幾何学的クリッピング領域のポリゴンデータ（Vec含む）
pub(crate) const STYLE_CLIP_AREAS: u128 = 1 << 53;
/// 状態遷移時のトランジション定義（wgpuバッチパス）
pub(crate) const STYLE_TRANSITIONS: u128 = 1 << 54;
/// テキスト内容そのものを示す
pub(crate) const COMP_TEXT_CONTENT: u128 = 1 << 55;
pub(crate) const COMP_INPUT_CONTENT: u128 = 1 << 36;

pub(crate) const STATE_QUEUED_LAYOUT: u128 = 1 << 56;
pub(crate) const STATE_QUEUED_RENDER: u128 = 1 << 57;

pub(crate) const COMP_IMAGE_CONTENT: u128 = 1 << 58;
pub(crate) const COMP_MOVIE_CONTENT: u128 = 1 << 59;

pub(crate) const COMP_UIA_CONTENT: u128 = 1 << 60;

/// カスタムのCSS変数や動的プロパティ（HashMap等含む）
pub(crate) const STYLE_EXT_PROPERTIES: u128 = 1 << 61;

/// WebView2 のコンテンツを持っているか
pub const COMP_WEBVIEW_CONTENT: u128 = 1 << 62;

pub(crate) const STYLE_POINTER_EVENTS: u128 = 1 << 63;

pub(crate) const STYLE_USER_SELECT: u128 = 1 << 8;

pub(crate) const STYLE_RESIZABLE: u128 = 1 << 64;

pub(crate) const STYLE_DRAGGABLE: u128 = 1 << 65;
pub(crate) const STYLE_DROPPABLE: u128 = 1 << 66;

pub(crate) const STATE_DRAGGING: u128 = 1 << 67; // ドラッグ元の実体に当てる（Dragging）
pub(crate) const STATE_DRAG_IN: u128 = 1 << 68; // ドロップ受け入れ先に当てる（DragIn）
pub(crate) const STATE_DRAG_OVER: u128 = 1 << 69; // プレースホルダー自体に当てる（DragOver）

pub(crate) const STYLE_FOCUSABLE: u128 = 1 << 70;
pub(crate) const STYLE_OUTLINE: u128 = 1 << 71;

// 基本レイアウト一括判定マスク (STYLE_DISPLAY から STYLE_BORDER まで：ビット0..17)
/// 基本レイアウトの個別プロパティの「どれか1つでも有効化されているか」を判定するマスク。
/// (16進数表現：0x3FFFF)
pub(crate) const STYLE_BASIC_LAYOUT: u128 = STYLE_DISPLAY
    | STYLE_ITEM_IS_TABLE
    | STYLE_ITEM_IS_REPLACED
    | STYLE_BOX_SIZING
    | STYLE_DIRECTION
    | STYLE_OVERFLOW
    | STYLE_SCROLLBAR
    | STYLE_POSITION
    | STYLE_INSET
    | STYLE_SIZE
    | STYLE_MIN_SIZE
    | STYLE_MAX_SIZE
    | STYLE_ASPECT_RATIO
    | STYLE_MARGIN
    | STYLE_PADDING
    | STYLE_BORDER
    | STYLE_RESIZABLE;

// Flexレイアウト一括判定マスク (STYLE_ALIGN_ITEMS から STYLE_FLEX_SHRINK まで：ビット18..30)
/// Flexboxレイアウトの個別プロパティの「どれか1つでも有効化されているか」を判定するマスク。
/// (16進数表現：0x7FFC0000)
pub(crate) const STYLE_FLEX_LAYOUT: u128 = STYLE_ALIGN_ITEMS
    | STYLE_ALIGN_SELF
    | STYLE_JUSTIFY_ITEMS
    | STYLE_JUSTIFY_SELF
    | STYLE_ALIGN_CONTENT
    | STYLE_JUSTIFY_CONTENT
    | STYLE_GAP
    | STYLE_TEXT_ALIGN
    | STYLE_FLEX_DIRECTION
    | STYLE_FLEX_WRAP
    | STYLE_FLEX_BASIS
    | STYLE_FLEX_GROW
    | STYLE_FLEX_SHRINK;

// ビジュアルプロパティの一括判定用マスク（ビット31..42の論理和：16進数表現 0x7FF80000000）
pub(crate) const STYLE_VISUAL_PROPERTY: u128 = STYLE_BG_COLOR
    | STYLE_BORDER_COLOR
    | STYLE_CORNER_RADIUS
    | STYLE_OPACITY
    | STYLE_BOX_SHADOW
    | STYLE_TRANSFORM
    | STYLE_Z_INDEX
    | STYLE_CURSOR
    | STYLE_BACKDROP
    | STYLE_TEXT_COLOR
    | STYLE_FONT_SIZE
    | STYLE_EXT_PROPERTIES
    | STYLE_POINTER_EVENTS
    | STYLE_USER_SELECT
    | STYLE_DRAGGABLE
    | STYLE_DROPPABLE
    | STYLE_FOCUSABLE
    | STYLE_OUTLINE;

// インタラクションプロパティの一括判定用マスク（ビット43..49の論理和：16進数表現 0x3F80000000000）
pub(crate) const STYLE_INTERACTION_PROPERTY: u128 = STATE_HOVERED
    | STATE_FOCUSED
    | STATE_PRESSED
    | STATE_DISABLED
    | STATE_ACTIVED
    | STATE_SELECTED
    | STATE_DRAGGED
    | STATE_DRAGGING
    | STATE_DRAG_IN
    | STATE_DRAG_OVER;

pub(crate) const STYLE_ACTIVE_INTERACTION_PROPERTY: u128 =
    STATE_HOVERED | STATE_FOCUSED | STATE_PRESSED | STATE_DRAGGED | STATE_ACTIVED | STATE_DRAGGING;
