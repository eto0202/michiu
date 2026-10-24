/// Flags that indicate an entities state, modifier, or whether it contains content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ComponentMask(pub u128);

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

    // 基本レイアウト関連のプロパティが1つでもあるか
    #[inline]
    #[must_use]
    pub(crate) fn has_basic_layout(&self) -> bool {
        self.has(Self::STYLE_BASIC_LAYOUT)
    }

    // Flexレイアウト関連のプロパティが1つでもあるか
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
        self.has_low(Self::INTERACTION_MASK_U64)
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
        self.has_queued_layout() || self.has_queued_render()
    }

    /// 下位64ビットを安全に取得する。
    /// キュー系、可視性、コア状態、主要ボックス、Flex、テキスト、インタラクションが含まれる。
    #[inline]
    #[must_use]
    #[expect(clippy::cast_possible_truncation)]
    pub(crate) fn lower_u64(&self) -> u64 {
        self.0 as u64
    }

    /// 下位64ビットのフラグに対する判定用ヘルパー
    #[inline]
    #[must_use]
    pub(crate) fn has_low(&self, flag: u64) -> bool {
        (self.lower_u64() & flag) != 0
    }

    // =========================================================================
    // 最優先ダーティ / キュー系 STATE
    // =========================================================================
    pub(crate) const STATE_QUEUED_LAYOUT: u128 = 1 << 0;
    pub(crate) const STATE_QUEUED_RENDER: u128 = 1 << 1;

    // =========================================================================
    // カリング・トランスフォーム累積用 STATE
    // =========================================================================
    pub(crate) const STATE_RENDER_VISIBLE: u128 = 1 << 2;
    pub(crate) const STATE_TRANSFORM_ACTIVE: u128 = 1 << 3;

    // =========================================================================
    // その他 STATE 系
    // =========================================================================
    pub(crate) const STATE_HOVERED: u128 = 1 << 4;
    pub(crate) const STATE_FOCUSED: u128 = 1 << 5;
    pub(crate) const STATE_FOCUSED_VISIBLE: u128 = 1 << 6;
    pub(crate) const STATE_PRESSED: u128 = 1 << 7;
    pub(crate) const STATE_ACTIVED: u128 = 1 << 8;
    pub(crate) const STATE_SELECTED: u128 = 1 << 9;
    pub(crate) const STATE_DISABLED: u128 = 1 << 10;
    pub(crate) const STATE_DRAGGED: u128 = 1 << 11;
    pub(crate) const STATE_DND_DRAGGING: u128 = 1 << 12;
    pub(crate) const STATE_DND_DRAG_IN: u128 = 1 << 13;
    pub(crate) const STATE_DND_DRAG_OVER: u128 = 1 << 14;

    // =========================================================================
    // COMP 系 (コンテンツ・エンティティ特性)
    // =========================================================================
    pub(crate) const COMP_TEXT_CONTENT: u128 = 1 << 15;
    pub(crate) const COMP_INPUT_CONTENT: u128 = 1 << 16;
    pub(crate) const COMP_EXTERNAL_VISUAL_CONTENT: u128 = 1 << 17;
    pub(crate) const COMP_EXTERNAL_TEXTURE_CONTENT: u128 = 1 << 18;
    pub(crate) const COMP_A11Y: u128 = 1 << 19;

    // =========================================================================
    // STYLE 系: コア・頻出ボックスモデル
    // =========================================================================
    pub(crate) const STYLE_DISPLAY: u128 = 1 << 20;
    pub(crate) const STYLE_BOX_SIZING: u128 = 1 << 21;
    pub(crate) const STYLE_SIZE: u128 = 1 << 22;
    pub(crate) const STYLE_MIN_SIZE: u128 = 1 << 23;
    pub(crate) const STYLE_MAX_SIZE: u128 = 1 << 24;
    pub(crate) const STYLE_MARGIN: u128 = 1 << 25;
    pub(crate) const STYLE_PADDING: u128 = 1 << 26;
    pub(crate) const STYLE_BORDER: u128 = 1 << 27;
    pub(crate) const STYLE_POSITION: u128 = 1 << 28;
    pub(crate) const STYLE_OVERFLOW: u128 = 1 << 29;

    // =========================================================================
    // STYLE 系: 配置・Flex レイアウト
    // =========================================================================
    pub(crate) const STYLE_FLEX_DIRECTION: u128 = 1 << 30;
    pub(crate) const STYLE_FLEX_WRAP: u128 = 1 << 31;
    pub(crate) const STYLE_FLEX_BASIS: u128 = 1 << 32;
    pub(crate) const STYLE_FLEX_GROW: u128 = 1 << 33;
    pub(crate) const STYLE_FLEX_SHRINK: u128 = 1 << 34;
    pub(crate) const STYLE_GAP: u128 = 1 << 35;
    pub(crate) const STYLE_ALIGN_ITEMS: u128 = 1 << 36;
    pub(crate) const STYLE_ALIGN_SELF: u128 = 1 << 37;
    pub(crate) const STYLE_JUSTIFY_ITEMS: u128 = 1 << 38;
    pub(crate) const STYLE_JUSTIFY_SELF: u128 = 1 << 39;
    pub(crate) const STYLE_ALIGN_CONTENT: u128 = 1 << 40;
    pub(crate) const STYLE_JUSTIFY_CONTENT: u128 = 1 << 41;

    // =========================================================================
    // STYLE 系: アニメーション / トランジション
    // =========================================================================
    pub(crate) const STYLE_ANIMATIONS: u128 = 1 << 42;
    pub(crate) const STYLE_TRANSITIONS: u128 = 1 << 43;

    // =========================================================================
    // STYLE 系: テキスト周り
    // =========================================================================
    pub(crate) const STYLE_TEXT_ALIGN: u128 = 1 << 44;
    pub(crate) const STYLE_TEXT_COLOR: u128 = 1 << 45;
    pub(crate) const STYLE_FONT_SIZE: u128 = 1 << 46;
    pub(crate) const STYLE_FONT_STYLE: u128 = 1 << 47;
    pub(crate) const STYLE_TEXT_SPANS: u128 = 1 << 48;
    pub(crate) const STYLE_AUTO_WRAP: u128 = 1 << 49;

    // =========================================================================
    // STYLE 系: インタラクション・イベント制御 (50..58)
    // =========================================================================
    pub(crate) const STYLE_CURSOR: u128 = 1 << 50;
    pub(crate) const STYLE_POINTER_EVENTS: u128 = 1 << 51;
    pub(crate) const STYLE_USER_SELECT: u128 = 1 << 52;
    pub(crate) const STYLE_FOCUSABLE: u128 = 1 << 53;
    pub(crate) const STYLE_PREVENT_FOCUS_STEAL: u128 = 1 << 54;
    pub(crate) const STYLE_PREVENT_FOCUS_STEAL_WITHIN: u128 = 1 << 55;
    pub(crate) const STYLE_INTERACTION_WITHIN: u128 = 1 << 56;
    pub(crate) const STYLE_INTERACTION_PARENT: u128 = 1 << 57;

    // =========================================================================
    // STYLE 系: ビジュアル装飾・描画
    // =========================================================================
    pub(crate) const STYLE_BG_COLOR: u128 = 1 << 58;
    pub(crate) const STYLE_BORDER_COLOR: u128 = 1 << 59;
    pub(crate) const STYLE_CORNER_RADIUS: u128 = 1 << 60;
    pub(crate) const STYLE_OUTLINE: u128 = 1 << 61;
    pub(crate) const STYLE_OPACITY: u128 = 1 << 62;
    pub(crate) const STYLE_BOX_SHADOW: u128 = 1 << 63;
    // -------------------------------------------------------------------------
    // ↑ ここまでが下位 64 bit
    // ↓ ここからが上位 64 bit
    // -------------------------------------------------------------------------
    pub(crate) const STYLE_BACKDROP: u128 = 1 << 64;
    pub(crate) const STYLE_Z_INDEX: u128 = 1 << 65;
    pub(crate) const STYLE_TRANSFORM: u128 = 1 << 66;
    pub(crate) const STYLE_TRANSFORM_INHERIT: u128 = 1 << 67;

    // =========================================================================
    // STYLE 系: 特殊・低頻度レイアウトプロパティ
    // =========================================================================
    pub(crate) const STYLE_ASPECT_RATIO: u128 = 1 << 68;
    pub(crate) const STYLE_ITEM_IS_TABLE: u128 = 1 << 69;
    pub(crate) const STYLE_ITEM_IS_REPLACED: u128 = 1 << 70;
    pub(crate) const STYLE_RESIZABLE: u128 = 1 << 71;
    pub(crate) const STYLE_DIRECTION: u128 = 1 << 72;
    pub(crate) const STYLE_INSET: u128 = 1 << 73;
    pub(crate) const STYLE_SCROLLBAR: u128 = 1 << 74;

    // =========================================================================
    // STYLE 系: その他（DnD設定など）
    // =========================================================================
    pub(crate) const STYLE_DND_DRAGGABLE: u128 = 1 << 75;
    pub(crate) const STYLE_DND_DROPPABLE: u128 = 1 << 76;

    // =========================================================================
    // 削除予定・レガシー (最後尾)
    // =========================================================================
    pub(crate) const STYLE_GRID_LAYOUT: u128 = 1 << 77;
    // =========================================================================
    // 複合フラグ
    // =========================================================================
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

    #[expect(clippy::cast_possible_truncation)]
    pub(crate) const INTERACTION_MASK_U64: u64 = { (Self::STYLE_INTERACTION_PROPERTY) as u64 };

    pub(crate) const STYLE_ACTIVE_INTERACTION_PROPERTY: u128 = Self::STATE_HOVERED
        | Self::STATE_FOCUSED
        | Self::STATE_PRESSED
        | Self::STATE_DRAGGED
        | Self::STATE_ACTIVED
        | Self::STATE_DND_DRAGGING
        | Self::STATE_FOCUSED_VISIBLE;
}

#[cfg(test)]
mod tests {
    use super::*;

    // 登録されている全フラグのリスト
    const ALL_FLAGS: &[u128] = &[
        ComponentMask::STATE_QUEUED_LAYOUT,
        ComponentMask::STATE_QUEUED_RENDER,
        ComponentMask::STATE_RENDER_VISIBLE,
        ComponentMask::STATE_TRANSFORM_ACTIVE,
        ComponentMask::STATE_HOVERED,
        ComponentMask::STATE_FOCUSED,
        ComponentMask::STATE_FOCUSED_VISIBLE,
        ComponentMask::STATE_PRESSED,
        ComponentMask::STATE_ACTIVED,
        ComponentMask::STATE_SELECTED,
        ComponentMask::STATE_DISABLED,
        ComponentMask::STATE_DRAGGED,
        ComponentMask::STATE_DND_DRAGGING,
        ComponentMask::STATE_DND_DRAG_IN,
        ComponentMask::STATE_DND_DRAG_OVER,
        ComponentMask::COMP_TEXT_CONTENT,
        ComponentMask::COMP_INPUT_CONTENT,
        ComponentMask::COMP_EXTERNAL_VISUAL_CONTENT,
        ComponentMask::COMP_EXTERNAL_TEXTURE_CONTENT,
        ComponentMask::COMP_A11Y,
        ComponentMask::STYLE_DISPLAY,
        ComponentMask::STYLE_BOX_SIZING,
        ComponentMask::STYLE_SIZE,
        ComponentMask::STYLE_MIN_SIZE,
        ComponentMask::STYLE_MAX_SIZE,
        ComponentMask::STYLE_MARGIN,
        ComponentMask::STYLE_PADDING,
        ComponentMask::STYLE_BORDER,
        ComponentMask::STYLE_POSITION,
        ComponentMask::STYLE_OVERFLOW,
        ComponentMask::STYLE_FLEX_DIRECTION,
        ComponentMask::STYLE_FLEX_WRAP,
        ComponentMask::STYLE_FLEX_BASIS,
        ComponentMask::STYLE_FLEX_GROW,
        ComponentMask::STYLE_FLEX_SHRINK,
        ComponentMask::STYLE_GAP,
        ComponentMask::STYLE_ALIGN_ITEMS,
        ComponentMask::STYLE_ALIGN_SELF,
        ComponentMask::STYLE_JUSTIFY_ITEMS,
        ComponentMask::STYLE_JUSTIFY_SELF,
        ComponentMask::STYLE_ALIGN_CONTENT,
        ComponentMask::STYLE_JUSTIFY_CONTENT,
        ComponentMask::STYLE_ANIMATIONS,
        ComponentMask::STYLE_TRANSITIONS,
        ComponentMask::STYLE_TEXT_ALIGN,
        ComponentMask::STYLE_TEXT_COLOR,
        ComponentMask::STYLE_FONT_SIZE,
        ComponentMask::STYLE_FONT_STYLE,
        ComponentMask::STYLE_TEXT_SPANS,
        ComponentMask::STYLE_AUTO_WRAP,
        ComponentMask::STYLE_CURSOR,
        ComponentMask::STYLE_POINTER_EVENTS,
        ComponentMask::STYLE_USER_SELECT,
        ComponentMask::STYLE_FOCUSABLE,
        ComponentMask::STYLE_PREVENT_FOCUS_STEAL,
        ComponentMask::STYLE_PREVENT_FOCUS_STEAL_WITHIN,
        ComponentMask::STYLE_INTERACTION_WITHIN,
        ComponentMask::STYLE_INTERACTION_PARENT,
        ComponentMask::STYLE_BG_COLOR,
        ComponentMask::STYLE_BORDER_COLOR,
        ComponentMask::STYLE_CORNER_RADIUS,
        ComponentMask::STYLE_OUTLINE,
        ComponentMask::STYLE_OPACITY,
        ComponentMask::STYLE_BOX_SHADOW,
        ComponentMask::STYLE_BACKDROP,
        ComponentMask::STYLE_Z_INDEX,
        ComponentMask::STYLE_TRANSFORM,
        ComponentMask::STYLE_TRANSFORM_INHERIT,
        ComponentMask::STYLE_ASPECT_RATIO,
        ComponentMask::STYLE_ITEM_IS_TABLE,
        ComponentMask::STYLE_ITEM_IS_REPLACED,
        ComponentMask::STYLE_RESIZABLE,
        ComponentMask::STYLE_DIRECTION,
        ComponentMask::STYLE_INSET,
        ComponentMask::STYLE_SCROLLBAR,
        ComponentMask::STYLE_DND_DRAGGABLE,
        ComponentMask::STYLE_DND_DROPPABLE,
        ComponentMask::STYLE_GRID_LAYOUT,
    ];

    #[test]
    fn test_flags_are_unique_and_single_bit() {
        use std::collections::HashSet;

        let mut seen = HashSet::new();

        for &flag in ALL_FLAGS {
            // 各フラグが単一ビット（1 << n）であること
            assert_eq!(
                flag.count_ones(),
                1,
                "Flag 0x{flag:032x} must have exactly one bit set"
            );

            // 重複がないこと
            assert!(
                seen.insert(flag),
                "Duplicate bit detected: 0x{flag:032x} (bit #{})",
                flag.trailing_zeros()
            );
        }
    }

    #[test]
    fn test_interaction_flags_within_64_bits() {
        // 64bit以内に収めるフラグ群
        let target_flags = [
            ComponentMask::STYLE_CURSOR,
            ComponentMask::STYLE_POINTER_EVENTS,
            ComponentMask::STYLE_USER_SELECT,
            ComponentMask::STYLE_FOCUSABLE,
            ComponentMask::STYLE_PREVENT_FOCUS_STEAL,
            ComponentMask::STYLE_PREVENT_FOCUS_STEAL_WITHIN,
            ComponentMask::STYLE_INTERACTION_WITHIN,
            ComponentMask::STYLE_INTERACTION_PARENT,
        ];

        for &flag in &target_flags {
            // trailing_zeros が 0..=63 であること
            let bit_index = flag.trailing_zeros();
            assert!(
                bit_index < 64,
                "Flag 0x{flag:032x} (bit #{bit_index}) exceeds 64-bit boundary"
            );
        }
    }

    #[test]
    fn test_basic_mask_operations() {
        let mut mask = ComponentMask::default();
        assert_eq!(mask.0, 0);

        // set & has
        mask.set(ComponentMask::STATE_QUEUED_LAYOUT);
        assert!(mask.has(ComponentMask::STATE_QUEUED_LAYOUT));
        assert!(!mask.has(ComponentMask::STATE_QUEUED_RENDER));

        // merge
        let other = ComponentMask::new(ComponentMask::STATE_QUEUED_RENDER);
        mask.merge(other);
        assert!(mask.has(ComponentMask::STATE_QUEUED_LAYOUT));
        assert!(mask.has(ComponentMask::STATE_QUEUED_RENDER));

        // unset
        mask.unset(ComponentMask::STATE_QUEUED_LAYOUT);
        assert!(!mask.has(ComponentMask::STATE_QUEUED_LAYOUT));
        assert!(mask.has(ComponentMask::STATE_QUEUED_RENDER));
    }

    #[test]
    fn test_lower_u64() {
        // 下位ビットのみ
        let low_mask = ComponentMask::new(ComponentMask::STATE_QUEUED_LAYOUT);
        assert_eq!(low_mask.lower_u64(), 1);

        // 64bit以上のビット（例: BACKDROP = 1 << 64）
        let high_mask = ComponentMask::new(ComponentMask::STYLE_BACKDROP);
        // lower_u64 は下位を切り捨てるので 0 になるはず
        assert_eq!(high_mask.lower_u64(), 0);

        // 混在
        let mixed =
            ComponentMask::new(ComponentMask::STATE_QUEUED_LAYOUT | ComponentMask::STYLE_BACKDROP);
        assert_eq!(mixed.lower_u64(), 1);
    }

    #[test]
    fn test_composite_masks() {
        // Flex レイアウト判定のチェック
        let mut mask = ComponentMask::default();
        assert!(!mask.has_flex_layout());

        mask.set(ComponentMask::STYLE_FLEX_GROW);
        assert!(mask.has_flex_layout());

        // 複合フラグ同士の独立性チェック
        assert!(!mask.has_basic_layout());

        // キューフラグの OR 判定
        let mut q_mask = ComponentMask::default();
        assert!(!q_mask.has_queued_layout_or_render());
        q_mask.set(ComponentMask::STATE_QUEUED_RENDER);
        assert!(q_mask.has_queued_layout_or_render());
    }
}
