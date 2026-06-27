#![allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ComponentMask(pub u64);

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
}

#[inline]
pub fn prop_bg_color() -> PropertyList {
    PropertyList::BackgroundColor
}

#[inline]
pub fn prop_border_color() -> PropertyList {
    PropertyList::BorderColor
}

#[inline]
pub fn prop_opacity() -> PropertyList {
    PropertyList::Opacity
}

#[inline]
pub fn prop_transform() -> PropertyList {
    PropertyList::Transform
}

#[inline]
pub fn prop_radius() -> PropertyList {
    PropertyList::CornerRadius
}

#[inline]
pub fn prop_width() -> PropertyList {
    PropertyList::Width
}

#[inline]
pub fn prop_height() -> PropertyList {
    PropertyList::Height
}

impl PropertyList {
    /// 内部的なビットフラグ（ComponentMask）にマッピング
    #[inline]
    pub(crate) fn to_mask_bit(self) -> u64 {
        match self {
            PropertyList::BackgroundColor => STYLE_BG_COLOR,
            PropertyList::BorderColor => STYLE_BORDER_COLOR,
            PropertyList::Opacity => STYLE_OPACITY,
            PropertyList::Transform => STYLE_TRANSFORM,
            PropertyList::CornerRadius => STYLE_CORNER_RADIUS,
            PropertyList::Width | PropertyList::Height => STYLE_SIZE,
        }
    }
}

impl ComponentMask {
    #[inline]
    pub fn new(flag: u64) -> Self {
        Self(flag)
    }

    #[inline]
    pub fn merge(&mut self, other: ComponentMask) {
        self.0 |= other.0;
    }

    #[inline]
    pub fn has(&self, flag: u64) -> bool {
        (self.0 & flag) != 0
    }

    #[inline]
    pub fn set(&mut self, flag: u64) {
        self.0 |= flag;
    }

    #[inline]
    pub fn unset(&mut self, flag: u64) {
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

// セグメント 1: Taffy 基本レイアウトプロパティ (0..17ビット) - 計18個
pub(crate) const STYLE_DISPLAY: u64 = 1 << 0;
pub(crate) const STYLE_ITEM_IS_TABLE: u64 = 1 << 1;
pub(crate) const STYLE_ITEM_IS_REPLACED: u64 = 1 << 2;
pub(crate) const STYLE_BOX_SIZING: u64 = 1 << 3;
pub(crate) const STYLE_DIRECTION: u64 = 1 << 4;
pub(crate) const STYLE_OVERFLOW: u64 = 1 << 5;
pub(crate) const STYLE_SCROLLBAR_WIDTH: u64 = 1 << 6;
pub(crate) const STYLE_FLOAT: u64 = 1 << 7;
pub(crate) const STYLE_CLEAR: u64 = 1 << 8;
pub(crate) const STYLE_POSITION: u64 = 1 << 9;
pub(crate) const STYLE_INSET: u64 = 1 << 10; // top, right, bottom, left
pub(crate) const STYLE_SIZE: u64 = 1 << 11; // width, height
pub(crate) const STYLE_MIN_SIZE: u64 = 1 << 12; // min_width, min_height
pub(crate) const STYLE_MAX_SIZE: u64 = 1 << 13; // max_width, max_height
pub(crate) const STYLE_ASPECT_RATIO: u64 = 1 << 14;
pub(crate) const STYLE_MARGIN: u64 = 1 << 15; // margin 4方向
pub(crate) const STYLE_PADDING: u64 = 1 << 16; // padding 4方向
pub(crate) const STYLE_BORDER: u64 = 1 << 17; // border 4方向 (太さのみ)

// セグメント 2: Taffy Flexboxレイアウトプロパティ (18..30ビット) - 計13個
pub(crate) const STYLE_ALIGN_ITEMS: u64 = 1 << 18;
pub(crate) const STYLE_ALIGN_SELF: u64 = 1 << 19;
pub(crate) const STYLE_JUSTIFY_ITEMS: u64 = 1 << 20;
pub(crate) const STYLE_JUSTIFY_SELF: u64 = 1 << 21;
pub(crate) const STYLE_ALIGN_CONTENT: u64 = 1 << 22;
pub(crate) const STYLE_JUSTIFY_CONTENT: u64 = 1 << 23;
pub(crate) const STYLE_GAP: u64 = 1 << 24; // row_gap, column_gap
pub(crate) const STYLE_TEXT_ALIGN: u64 = 1 << 25;
pub(crate) const STYLE_FLEX_DIRECTION: u64 = 1 << 26;
pub(crate) const STYLE_FLEX_WRAP: u64 = 1 << 27;
pub(crate) const STYLE_FLEX_BASIS: u64 = 1 << 28;
pub(crate) const STYLE_FLEX_GROW: u64 = 1 << 29;
pub(crate) const STYLE_FLEX_SHRINK: u64 = 1 << 30;

// セグメント 3: 描画（ペイント）・ビジュアルプロパティ (31..42ビット) - 計12個
pub(crate) const STYLE_BG_COLOR: u64 = 1 << 31;
pub(crate) const STYLE_BORDER_COLOR: u64 = 1 << 32;
pub(crate) const STYLE_CORNER_RADIUS: u64 = 1 << 33; // top_left, top_right...
pub(crate) const STYLE_OPACITY: u64 = 1 << 34;
pub(crate) const STYLE_BOX_SHADOW: u64 = 1 << 35;
pub(crate) const STYLE_CLIP_PATH: u64 = 1 << 36;
pub(crate) const STYLE_TRANSFORM: u64 = 1 << 37; // 2D/3D 座標変換行列
pub(crate) const STYLE_Z_INDEX: u64 = 1 << 38;
pub(crate) const STYLE_CURSOR: u64 = 1 << 39;
pub(crate) const STYLE_FILTER: u64 = 1 << 40; // ぼかし(Blur)やグレースケール
pub(crate) const STYLE_TEXT_COLOR: u64 = 1 << 41;
pub(crate) const STYLE_FONT_SIZE: u64 = 1 << 42;

// セグメント 4: インタラクション・動的状態フラグ (43..47ビット) - 計5個
pub(crate) const STATE_HOVERED: u64 = 1 << 43;
pub(crate) const STATE_FOCUSED: u64 = 1 << 44;
pub(crate) const STATE_PRESSED: u64 = 1 << 45;
pub(crate) const STATE_DISABLED: u64 = 1 << 46;
pub(crate) const STATE_ACTIVED: u64 = 1 << 47;
pub(crate) const STATE_SELECTED: u64 = 1 << 48;
pub(crate) const STATE_DRAGGED: u64 = 1 << 49;

// セグメント 5: 可変長・コールドデータ拡張領域 (48..55ビット) - 計8個
// ※ Vec等を含む重い構造体。SparseSecondaryMap に実体を逃がす。
/// TaffyのGridレイアウト用の全プロパティ（grid_template_rows等：Vecを多数含む）
pub(crate) const STYLE_GRID_LAYOUT: u64 = 1 << 50;
/// 動的キーフレームアニメーションの定義シーケンス（Vec含む）
pub(crate) const STYLE_ANIMATIONS: u64 = 1 << 51;
/// RichText用の複数スパン情報やスタイリング（Vec含む）
pub(crate) const STYLE_TEXT_SPANS: u64 = 1 << 52;
/// 複雑な幾何学的クリッピング領域のポリゴンデータ（Vec含む）
pub(crate) const STYLE_CLIP_AREAS: u64 = 1 << 53;
/// 状態遷移時のトランジション定義（wgpuバッチパス）
pub(crate) const STYLE_TRANSITIONS: u64 = 1 << 54;
/// テキスト内容そのものを示す
pub(crate) const COMP_TEXT_CONTENT: u64 = 1 << 55;

pub(crate) const STATE_QUEUED_LAYOUT: u64 = 1 << 56;
pub(crate) const STATE_QUEUED_RENDER: u64 = 1 << 57;

pub(crate) const COMP_IMAGE_CONTENT: u64 = 1 << 58;
pub(crate) const COMP_MOVIE_CONTENT: u64 = 1 << 59;

pub(crate) const COMP_UIA_CONTENT: u64 = 1 << 60;

/// カスタムのCSS変数や開発者の動的プロパティ（HashMap等含む）
pub(crate) const STYLE_EXT_PROPERTIES: u64 = 1 << 61;

/// WebView2 のコンテンツを持っているか
pub const COMP_WEBVIEW_CONTENT: u64 = 1 << 62;
// セグメント 6: 完全予約領域
// あと1つしかにゃい．．．
pub(crate) const RESERVED_63: u64 = 1 << 63;

// 基本レイアウト一括判定マスク (STYLE_DISPLAY から STYLE_BORDER まで：ビット0..17)
/// 基本レイアウトの個別プロパティの「どれか1つでも有効化されているか」を判定するマスク。
/// (16進数表現：0x3FFFF)
pub(crate) const STYLE_BASIC_LAYOUT: u64 = STYLE_DISPLAY
    | STYLE_ITEM_IS_TABLE
    | STYLE_ITEM_IS_REPLACED
    | STYLE_BOX_SIZING
    | STYLE_DIRECTION
    | STYLE_OVERFLOW
    | STYLE_SCROLLBAR_WIDTH
    | STYLE_FLOAT
    | STYLE_CLEAR
    | STYLE_POSITION
    | STYLE_INSET
    | STYLE_SIZE
    | STYLE_MIN_SIZE
    | STYLE_MAX_SIZE
    | STYLE_ASPECT_RATIO
    | STYLE_MARGIN
    | STYLE_PADDING
    | STYLE_BORDER;

// Flexレイアウト一括判定マスク (STYLE_ALIGN_ITEMS から STYLE_FLEX_SHRINK まで：ビット18..30)
/// Flexboxレイアウトの個別プロパティの「どれか1つでも有効化されているか」を判定するマスク。
/// (16進数表現：0x7FFC0000)
pub(crate) const STYLE_FLEX_LAYOUT: u64 = STYLE_ALIGN_ITEMS
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
pub(crate) const STYLE_VISUAL_PROPERTY: u64 = STYLE_BG_COLOR
    | STYLE_BORDER_COLOR
    | STYLE_CORNER_RADIUS
    | STYLE_OPACITY
    | STYLE_BOX_SHADOW
    | STYLE_CLIP_PATH
    | STYLE_TRANSFORM
    | STYLE_Z_INDEX
    | STYLE_CURSOR
    | STYLE_FILTER
    | STYLE_TEXT_COLOR
    | STYLE_FONT_SIZE;

// インタラクションプロパティの一括判定用マスク（ビット43..49の論理和：16進数表現 0x3F80000000000）
pub(crate) const STYLE_INTERACTION_PROPERTY: u64 = STATE_HOVERED
    | STATE_FOCUSED
    | STATE_PRESSED
    | STATE_DISABLED
    | STATE_ACTIVED
    | STATE_SELECTED
    | STATE_DRAGGED;

pub(crate) const STYLE_ACTIVE_INTERACTION_PROPERTY: u64 =
    STATE_HOVERED | STATE_FOCUSED | STATE_PRESSED | STATE_DRAGGED | STATE_ACTIVED;
