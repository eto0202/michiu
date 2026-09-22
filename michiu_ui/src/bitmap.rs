#![allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ComponentMask(pub u128);

/// Properties for which animations and transitions can be configured
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
    #[inline]
    pub(crate) fn to_mask_bit(self) -> u128 {
        match self {
            PropertyList::BackgroundColor => ComponentMask::STYLE_BG_COLOR,
            PropertyList::BorderColor => ComponentMask::STYLE_BORDER_COLOR,
            PropertyList::Opacity => ComponentMask::STYLE_OPACITY,
            PropertyList::Transform => ComponentMask::STYLE_TRANSFORM,
            PropertyList::CornerRadius => ComponentMask::STYLE_CORNER_RADIUS,
            PropertyList::Width | PropertyList::Height | PropertyList::Size => {
                ComponentMask::STYLE_SIZE
            }
            PropertyList::BoxShadow => ComponentMask::STYLE_BOX_SHADOW,
            PropertyList::Resizable => ComponentMask::STYLE_RESIZABLE,
        }
    }
}

impl ComponentMask {
    #[inline]
    #[must_use]
    pub(crate) fn new(flag: u128) -> Self {
        Self(flag)
    }

    #[inline]
    pub(crate) fn merge(&mut self, other: ComponentMask) {
        self.0 |= other.0;
    }

    #[inline]
    #[must_use]
    pub(crate) fn has(&self, flag: u128) -> bool {
        (self.0 & flag) != 0
    }

    #[inline]
    pub(crate) fn set(&mut self, flag: u128) {
        self.0 |= flag;
    }

    #[inline]
    pub(crate) fn unset(&mut self, flag: u128) {
        self.0 &= !flag;
    }

    /// 基本レイアウト関連のプロパティが1つでもあるか
    #[inline]
    #[must_use]
    pub(crate) fn has_basic_layout(&self) -> bool {
        self.has(Self::STYLE_BASIC_LAYOUT)
    }

    /// Flexレイアウト関連のプロパティが1つでもあるか
    #[inline]
    #[must_use]
    pub(crate) fn has_flex_layout(&self) -> bool {
        self.has(Self::STYLE_FLEX_LAYOUT)
    }

    #[inline]
    #[must_use]
    pub(crate) fn has_grid_layout(&self) -> bool {
        self.has(Self::STYLE_GRID_LAYOUT)
    }

    #[inline]
    #[must_use]
    pub(crate) fn has_visual_property(&self) -> bool {
        self.has(Self::STYLE_VISUAL_PROPERTY)
    }

    #[inline]
    #[must_use]
    pub(crate) fn has_interaction_property(&self) -> bool {
        self.has(Self::STYLE_INTERACTION_PROPERTY)
    }

    #[inline]
    #[must_use]
    pub(crate) fn has_active_interaction_property(&self) -> bool {
        self.has(Self::STYLE_ACTIVE_INTERACTION_PROPERTY)
    }

    #[inline]
    #[must_use]
    pub(crate) fn has_input_content(&self) -> bool {
        self.has(Self::COMP_INPUT_CONTENT)
    }

    #[inline]
    #[must_use]
    pub(crate) fn has_text_content(&self) -> bool {
        self.has(Self::COMP_TEXT_CONTENT)
    }

    #[inline]
    #[must_use]
    pub(crate) fn has_external_visual_content(&self) -> bool {
        self.has(ComponentMask::COMP_EXTERNAL_VISUAL_CONTENT)
    }

    #[inline]
    #[must_use]
    pub(crate) fn has_external_texture_content(&self) -> bool {
        self.has(ComponentMask::COMP_EXTERNAL_TEXTURE_CONTENT)
    }

    #[inline]
    #[must_use]
    pub(crate) fn has_image_content(&self) -> bool {
        self.has(Self::COMP_IMAGE_CONTENT)
    }

    #[inline]
    #[must_use]
    pub(crate) fn has_movie_content(&self) -> bool {
        self.has(Self::COMP_MOVIE_CONTENT)
    }

    #[inline]
    #[must_use]
    pub(crate) fn has_queued_layout(&self) -> bool {
        self.has(Self::STATE_QUEUED_LAYOUT)
    }

    #[inline]
    #[must_use]
    pub(crate) fn has_queued_render(&self) -> bool {
        self.has(Self::STATE_QUEUED_RENDER)
    }

    #[inline]
    #[must_use]
    pub(crate) fn has_queued_layout_or_render(&self) -> bool {
        self.has(Self::STATE_QUEUED_LAYOUT) || self.has(Self::STATE_QUEUED_RENDER)
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

    // `TaffyのGridレイアウト用の全プロパティ（grid_template_rows等：Vecを多数含む`）
    pub(crate) const STYLE_GRID_LAYOUT: u128 = 1 << 50;
    // 動的キーフレームアニメーションの定義シーケンス（Vec含む）
    pub(crate) const STYLE_ANIMATIONS: u128 = 1 << 51;
    // RichText用の複数スパン情報やスタイリング（Vec含む）
    pub(crate) const STYLE_TEXT_SPANS: u128 = 1 << 52;
    // 複雑な幾何学的クリッピング領域のポリゴンデータ（Vec含む）
    pub(crate) const STYLE_CLIP_AREAS: u128 = 1 << 53;
    // 状態遷移時のトランジション定義（wgpuバッチパス）
    pub(crate) const STYLE_TRANSITIONS: u128 = 1 << 54;
    // テキスト内容そのものを示す
    pub(crate) const COMP_TEXT_CONTENT: u128 = 1 << 55;
    pub(crate) const COMP_INPUT_CONTENT: u128 = 1 << 36;

    pub(crate) const STATE_QUEUED_LAYOUT: u128 = 1 << 56;
    pub(crate) const STATE_QUEUED_RENDER: u128 = 1 << 57;

    pub(crate) const COMP_IMAGE_CONTENT: u128 = 1 << 58;
    pub(crate) const COMP_MOVIE_CONTENT: u128 = 1 << 59;

    pub(crate) const COMP_UIA_CONTENT: u128 = 1 << 60;

    pub(crate) const STYLE_FONT_STYLE: u128 = 1 << 61;

    // 62 空き

    pub(crate) const STYLE_POINTER_EVENTS: u128 = 1 << 63;

    pub(crate) const STYLE_USER_SELECT: u128 = 1 << 8;

    pub(crate) const STYLE_RESIZABLE: u128 = 1 << 64;

    pub(crate) const STYLE_DND_DRAGGABLE: u128 = 1 << 65;
    pub(crate) const STYLE_DND_DROPPABLE: u128 = 1 << 66;

    pub(crate) const STATE_DND_DRAGGING: u128 = 1 << 67; // ドラッグ元の実体に当てる（Dragging）
    pub(crate) const STATE_DND_DRAG_IN: u128 = 1 << 68; // ドロップ受け入れ先に当てる（DragIn）
    pub(crate) const STATE_DND_DRAG_OVER: u128 = 1 << 69; // プレースホルダー自体に当てる（DragOver）

    pub(crate) const STYLE_FOCUSABLE: u128 = 1 << 70;
    pub(crate) const STYLE_OUTLINE: u128 = 1 << 71;

    pub(crate) const STYLE_INTERACTION_PARENT: u128 = 1 << 72;

    pub(crate) const STYLE_TRANSFORM_INHERIT: u128 = 1 << 73;

    pub(crate) const STATE_FOCUSED_VISIBLE: u128 = 1 << 74;

    pub(crate) const STYLE_PREVENT_FOCUS_STEAL: u128 = 1 << 75;
    pub(crate) const STYLE_PREVENT_FOCUS_STEAL_WITHIN: u128 = 1 << 76;

    pub(crate) const STYLE_AUTO_WRAP: u128 = 1 << 77;

    // 階層的カリング用
    pub(crate) const STATE_RENDER_VISIBLE: u128 = 1 << 78;
    // 累積トランスフォーム用
    pub(crate) const STATE_TRANSFORM_ACTIVE: u128 = 1 << 79;

    // 外部提供テクスチャが有効であることを示す
    pub(crate) const COMP_EXTERNAL_TEXTURE_CONTENT: u128 = 1 << 80;
    pub(crate) const COMP_EXTERNAL_VISUAL_CONTENT: u128 = 1 << 81;

    pub(crate) const STYLE_BASIC_LAYOUT: u128 = Self::STYLE_DISPLAY
        | Self::STYLE_ITEM_IS_TABLE
        | Self::STYLE_ITEM_IS_REPLACED
        | Self::STYLE_BOX_SIZING
        | Self::STYLE_DIRECTION
        | Self::STYLE_OVERFLOW
        | Self::STYLE_SCROLLBAR
        | Self::STYLE_POSITION
        | Self::STYLE_INSET
        | Self::STYLE_SIZE
        | Self::STYLE_MIN_SIZE
        | Self::STYLE_MAX_SIZE
        | Self::STYLE_ASPECT_RATIO
        | Self::STYLE_MARGIN
        | Self::STYLE_PADDING
        | Self::STYLE_BORDER
        | Self::STYLE_RESIZABLE;

    pub(crate) const STYLE_FLEX_LAYOUT: u128 = Self::STYLE_ALIGN_ITEMS
        | Self::STYLE_ALIGN_SELF
        | Self::STYLE_JUSTIFY_ITEMS
        | Self::STYLE_JUSTIFY_SELF
        | Self::STYLE_ALIGN_CONTENT
        | Self::STYLE_JUSTIFY_CONTENT
        | Self::STYLE_GAP
        | Self::STYLE_TEXT_ALIGN
        | Self::STYLE_FLEX_DIRECTION
        | Self::STYLE_FLEX_WRAP
        | Self::STYLE_FLEX_BASIS
        | Self::STYLE_FLEX_GROW
        | Self::STYLE_FLEX_SHRINK;

    pub(crate) const STYLE_VISUAL_PROPERTY: u128 = Self::STYLE_BG_COLOR
        | Self::STYLE_BORDER_COLOR
        | Self::STYLE_CORNER_RADIUS
        | Self::STYLE_OPACITY
        | Self::STYLE_BOX_SHADOW
        | Self::STYLE_TRANSFORM
        | Self::STYLE_Z_INDEX
        | Self::STYLE_CURSOR
        | Self::STYLE_BACKDROP
        | Self::STYLE_TEXT_COLOR
        | Self::STYLE_FONT_SIZE
        | Self::STYLE_FONT_STYLE
        | Self::STYLE_POINTER_EVENTS
        | Self::STYLE_USER_SELECT
        | Self::STYLE_DND_DRAGGABLE
        | Self::STYLE_DND_DROPPABLE
        | Self::STYLE_FOCUSABLE
        | Self::STYLE_OUTLINE
        | Self::STYLE_TRANSFORM_INHERIT
        | Self::STYLE_PREVENT_FOCUS_STEAL
        | Self::STYLE_PREVENT_FOCUS_STEAL_WITHIN
        | Self::STYLE_AUTO_WRAP;

    pub(crate) const STYLE_INTERACTION_PROPERTY: u128 = Self::STATE_HOVERED
        | Self::STATE_FOCUSED
        | Self::STATE_PRESSED
        | Self::STATE_DISABLED
        | Self::STATE_ACTIVED
        | Self::STATE_SELECTED
        | Self::STATE_DRAGGED
        | Self::STATE_DND_DRAGGING
        | Self::STATE_DND_DRAG_IN
        | Self::STATE_DND_DRAG_OVER
        | Self::STATE_FOCUSED_VISIBLE;

    pub(crate) const STYLE_ACTIVE_INTERACTION_PROPERTY: u128 = Self::STATE_HOVERED
        | Self::STATE_FOCUSED
        | Self::STATE_PRESSED
        | Self::STATE_DRAGGED
        | Self::STATE_ACTIVED
        | Self::STATE_DND_DRAGGING
        | Self::STATE_FOCUSED_VISIBLE;
}
