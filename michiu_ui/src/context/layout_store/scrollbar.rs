use crate::{
    ActiveInteractionStates, ActiveMasksSecondary, ActiveTransitionsSparse,
    BaseBasicLayoutsSecondary, BaseVisualPropertiesSecondary, BasicLayoutsSecondary,
    CapacityConfig, DEFAULT_BASIC, DEFAULT_FLEX, DebugStore, DirtyLayoutEntitiesVec,
    DirtyRenderEntitiesVec, Display, EntityId, FlexLayoutsSecondary, GridLayoutsSparse,
    InteractionPropertiesSecondary, LayoutPoint, LayoutSize, LayoutStore, Length, MichiuSoA,
    OutputStore, ParentsSecondary, Rect, RectsSecondary, RenderStore, ResolvedBasicSecondary,
    ResolvedFlexSecondary, ResolvedGridSparse, ScrollOffsetsSecondary, ScrollSizesSecondary,
    ScrollStore, Size, TaffyNodesSecondary, TaffyTreeEntityId, ThisStyle, Val,
    VisualPropertiesSecondary, WindowStore, define_sparse_secondary,
};
use slotmap::SparseSecondaryMap;
use smallvec::SmallVec;
use std::time::{Duration, Instant};

/// スクロールバーを表示する配置モード
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollbarMode {
    /// コンテンツの横/下にレイアウト領域を確保して配置（コンテンツが狭まる）
    Layout,
    /// コンテンツの最前面に重ねて配置（コンテンツ領域を侵食しない）
    #[default]
    Overlay,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollbarDisplay {
    None,
    Always,
    #[default]
    Auto,
    Transient,
}

/// スクロールコンテナが保持するスタイリング設定
#[derive(Debug, Clone)]
pub struct ScrollbarStyle {
    /// スクロールバーの太さ（縦スクロールバー時は幅、横スクロールバー時は高さ）
    pub width: f32,
    /// 表示
    pub display: ScrollbarDisplay,
    /// スクロールバーの表示モード
    pub mode: ScrollbarMode,
    /// 縦スクロールバー（Y軸）のトラック（レール）部分のスタイル
    pub v_track: Option<ThisStyle>,
    /// 縦スクロールバー（Y軸）のサム（つまみ）部分のスタイル
    pub v_thumb: Option<ThisStyle>,
    /// 横スクロールバー（X軸）のトラック（レール）部分のスタイル
    pub h_track: Option<ThisStyle>,
    /// 横スクロールバー（X軸）のサム（つまみ）部分のスタイル
    pub h_thumb: Option<ThisStyle>,
}

impl Default for ScrollbarStyle {
    fn default() -> Self {
        Self {
            width: 10.0,
            display: ScrollbarDisplay::Auto,
            mode: ScrollbarMode::Overlay,
            v_track: None,
            v_thumb: None,
            h_track: None,
            h_thumb: None,
        }
    }
}

impl ScrollbarStyle {
    #[must_use]
    pub fn new(width: f32) -> Self {
        Self {
            width,
            display: ScrollbarDisplay::Auto,
            mode: ScrollbarMode::Overlay,
            v_track: None,
            v_thumb: None,
            h_track: None,
            h_thumb: None,
        }
    }

    #[must_use]
    pub fn mode(mut self, mode: ScrollbarMode) -> Self {
        self.mode = mode;
        self
    }

    #[must_use]
    pub fn display(mut self, display: ScrollbarDisplay) -> Self {
        self.display = display;
        self
    }

    #[must_use]
    pub fn v_track(mut self, style: ThisStyle) -> Self {
        self.v_track = Some(style);
        self
    }

    #[must_use]
    pub fn v_thumb(mut self, style: ThisStyle) -> Self {
        self.v_thumb = Some(style);
        self
    }

    #[must_use]
    pub fn h_track(mut self, style: ThisStyle) -> Self {
        self.h_track = Some(style);
        self
    }

    #[must_use]
    pub fn h_thumb(mut self, style: ThisStyle) -> Self {
        self.h_thumb = Some(style);
        self
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScrollbarComponent {
    VThumb,
    HThumb,
    VTrack,
    HTrack,
}

#[derive(Debug, Clone, Default)]
pub struct ScrollBarState {
    pub style: ScrollbarStyle,

    // レイアウトツリーに動的挿入される Element の EntityId
    pub v_track_id: Option<EntityId>,
    pub v_thumb_id: Option<EntityId>,
    pub h_track_id: Option<EntityId>,
    pub h_thumb_id: Option<EntityId>,

    // ホバー・ドラッグのランタイム状態
    pub v_thumb_hovered: bool,
    pub v_thumb_dragged: bool,
    pub h_thumb_hovered: bool,
    pub h_thumb_dragged: bool,

    pub drag_start_mouse: LayoutPoint,
    pub drag_start_offset: LayoutPoint,

    // 一時表示（Transient）モードの表示制御用
    pub last_scroll_time: Option<std::time::Instant>,
}

define_sparse_secondary!(pub struct ScrollbarStylesSparse(ScrollBarState));

pub(crate) struct ScrollbarStore {
    pub(crate) bar_styles: ScrollbarStylesSparse,
}

impl Default for ScrollbarStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollbarStore {
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self {
            bar_styles: ScrollbarStylesSparse(SparseSecondaryMap::new()),
        }
    }

    #[inline]
    #[must_use]
    pub fn with_capacity(c: &CapacityConfig) -> Self {
        Self {
            bar_styles: ScrollbarStylesSparse(SparseSecondaryMap::with_capacity(c.bar_styles)),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.bar_styles.clear();
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.bar_styles.remove(id);
    }
}

impl ScrollbarStore {
    #[inline]
    pub(crate) fn get_scrollbar_dirty_ids(
        bar_styles: &mut ScrollbarStylesSparse,
    ) -> SmallVec<[EntityId; 4]> {
        let mut dirty_ids = SmallVec::<[EntityId; 4]>::new();

        for (id, state) in bar_styles.iter_mut() {
            if state.v_thumb_dragged || state.h_thumb_dragged {
                state.v_thumb_dragged = false;
                state.h_thumb_dragged = false;
                dirty_ids.push(id);
            }
        }
        dirty_ids
    }

    pub(crate) fn hit_decision_element_scrollbar(
        target_id: EntityId,
        pointer_pos: LayoutPoint,
        win_last_size: Option<LayoutSize>,
        evt_interaction_states: &mut ActiveInteractionStates,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        bar_styles: &mut ScrollbarStylesSparse,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparse,
        sc_offsets: &mut ScrollOffsetsSecondary,
        out_rects: &RectsSecondary,
        sc_sizes: &ScrollSizesSecondary,
        debug: &mut DebugStore,
    ) -> bool {
        let Some((c_id, component)) = bar_styles.iter().find_map(|(c_id, sb_state)| {
            if sb_state.v_thumb_id == Some(target_id) {
                Some((c_id, ScrollbarComponent::VThumb))
            } else if sb_state.h_thumb_id == Some(target_id) {
                Some((c_id, ScrollbarComponent::HThumb))
            } else if sb_state.v_track_id == Some(target_id) {
                Some((c_id, ScrollbarComponent::VTrack))
            } else if sb_state.h_track_id == Some(target_id) {
                Some((c_id, ScrollbarComponent::HTrack))
            } else {
                None
            }
        }) else {
            return false;
        };

        // 親スクロールコンテナ
        let sb_state = bar_styles.get(c_id).cloned().unwrap();
        let container_rect = *out_rects.at(c_id);
        let scroll_size = sc_sizes.get_or_default(c_id);
        let offset = sc_offsets.get_or_default(c_id);

        match component {
            ScrollbarComponent::VThumb | ScrollbarComponent::HThumb => {
                // サムをクリックした場合：ドラッグを開始
                if let Some(st) = bar_styles.get_mut(c_id) {
                    if component == ScrollbarComponent::VThumb {
                        st.v_thumb_dragged = true;
                    } else {
                        st.h_thumb_dragged = true;
                    }
                    st.drag_start_mouse = pointer_pos;
                    st.drag_start_offset = offset;
                }
                evt_interaction_states.pressed = Some(target_id);
                RenderStore::mark_render_dirty(target_id, topo_active_masks, rnd_dirty_entities);
            }
            ScrollbarComponent::VTrack | ScrollbarComponent::HTrack => {
                // レールをクリックした場合：ダイレクトジャンプスクロールを実行
                let is_vertical = component == ScrollbarComponent::VTrack;
                let thumb_id = if is_vertical {
                    sb_state.v_thumb_id
                } else {
                    sb_state.h_thumb_id
                };

                // ヒット先があるなら Some のはず
                let track_rect = *out_rects.at(target_id);
                let thumb_rect = thumb_id
                    .and_then(|i| out_rects.get(i).copied())
                    .unwrap_or_default();
                let visible_size = WindowStore::calc_visible_size(container_rect, win_last_size);

                // 縦・横の計算用パラメータ
                let (pointer_coord, track_coord, track_len, thumb_len, scroll_total, visible_total) =
                    if is_vertical {
                        (
                            pointer_pos.y,
                            track_rect.y,
                            track_rect.height,
                            thumb_rect.height,
                            scroll_size.height,
                            visible_size.height,
                        )
                    } else {
                        (
                            pointer_pos.x,
                            track_rect.x,
                            track_rect.width,
                            thumb_rect.width,
                            scroll_size.width,
                            visible_size.width,
                        )
                    };

                let relative_pos = pointer_coord - track_coord;
                let track_range = track_len - thumb_len;
                let scroll_ratio = if track_range > 0.0 {
                    ((relative_pos - thumb_len * 0.5) / track_range).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                let target_val = scroll_ratio * (scroll_total - visible_total);
                let (target_x, target_y) = if is_vertical {
                    (offset.x, target_val)
                } else {
                    (target_val, offset.y)
                };

                ScrollStore::scroll_to(
                    c_id,
                    target_x,
                    target_y,
                    win_last_size,
                    topo_active_masks,
                    topo_parents,
                    lay_dirty_entities,
                    lay_taffy_tree,
                    bar_styles,
                    lay_taffy_nodes,
                    lay_resolved_basic,
                    rnd_visual,
                    rnd_interaction,
                    rnd_active_transitions,
                    sc_offsets,
                    out_rects,
                    sc_sizes,
                    debug,
                );

                let new_offset = sc_offsets.get_or_default(c_id);
                if let Some(st) = bar_styles.get_mut(c_id) {
                    if is_vertical {
                        st.v_thumb_dragged = true;
                    } else {
                        st.h_thumb_dragged = true;
                    }
                    st.drag_start_mouse = pointer_pos;
                    st.drag_start_offset = new_offset;
                }

                evt_interaction_states.pressed = thumb_id;
                if let Some(tid) = thumb_id {
                    RenderStore::mark_render_dirty(tid, topo_active_masks, rnd_dirty_entities);
                }
            }
        }
        true
    }

    // 表示状態と不透明度の計算
    #[inline]
    fn calculate_visibility_opacity(
        display: ScrollbarDisplay,
        last_scroll_time: Option<Instant>,
        show_bar: bool,
    ) -> (bool, f32) {
        if !show_bar || display == ScrollbarDisplay::None {
            return (false, 1.0);
        }
        match display {
            ScrollbarDisplay::Always | ScrollbarDisplay::Auto => (true, 1.0),
            ScrollbarDisplay::Transient => {
                let Some(last) = last_scroll_time else {
                    return (false, 1.0);
                };

                let elapsed = last.elapsed();
                if elapsed < Duration::from_secs(1) {
                    (true, 1.0)
                } else if elapsed < Duration::from_millis(1500) {
                    let opacity = 1.0 - (elapsed.as_secs_f32() - 1.0) / 0.5;
                    (true, opacity)
                } else {
                    (false, 1.0)
                }
            }
            ScrollbarDisplay::None => (false, 1.0),
        }
    }

    // トラック有効長の計算
    #[inline]
    fn calculate_track_len(
        visible_dim: f32,
        border_start: f32,
        border_end: f32,
        show_other: bool,
        scrollbar_width: f32,
    ) -> f32 {
        let extra = if show_other { scrollbar_width } else { 0.0 };
        (visible_dim - border_start - border_end - extra).max(0.0)
    }

    // つまみの物理サイズと位置の計算
    #[inline]
    fn calculate_thumb_geometry(
        track_len: f32,
        visible_len: f32,
        scroll_len: f32,
        current_scroll_val: f32,
        scrollbar_width: f32,
        ext: &ExtractedThumb,
    ) -> (f32, f32, f32, f32) {
        let view_ratio = if scroll_len > 0.0 {
            (visible_len / scroll_len).min(1.0)
        } else {
            1.0
        };

        let calculated_len = ext.initial_len * view_ratio;
        let thumb_len = calculated_len
            .max(ext.min_len)
            .min(ext.max_len)
            .min(track_len);

        let scroll_ratio = if scroll_len > visible_len {
            (current_scroll_val / (scroll_len - visible_len)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let max_thumb_pos = (track_len - thumb_len - ext.margin_start - ext.margin_end).max(0.0);
        let thumb_main_pos = max_thumb_pos * scroll_ratio;

        let thumb_cross_len = if let Some(w) = ext.cross_size_override {
            w.min(scrollbar_width)
        } else {
            scrollbar_width
        };

        let thumb_cross_pos = if ext.pad_end > 0.0 {
            scrollbar_width - thumb_cross_len - ext.pad_end
        } else if ext.pad_start > 0.0 {
            ext.pad_start
        } else {
            (scrollbar_width - thumb_cross_len) * 0.5
        };

        (thumb_len, thumb_main_pos, thumb_cross_len, thumb_cross_pos)
    }

    pub(crate) fn sync_bar_styles(
        win_last_size: Option<LayoutSize>,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
        lay_resolved_basic: &mut ResolvedBasicSecondary,
        lay_resolved_flex: &mut ResolvedFlexSecondary,
        lay_resolved_grid: &mut ResolvedGridSparse,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSparse,
        bar_styles: &ScrollbarStylesSparse,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_base_visual: &mut BaseVisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparse,
        out_rects: &RectsSecondary,
        sc_offsets: &ScrollOffsetsSecondary,
        sc_sizes: &ScrollSizesSecondary,
        debug: &mut DebugStore,
    ) {
        let scrollbar_ids: Vec<EntityId> = bar_styles.keys().collect();

        for id in scrollbar_ids {
            let sb_state = bar_styles.get(id).cloned().unwrap();
            let rect = *out_rects.at(id);
            let scroll_size = sc_sizes.get_or_default(id);
            let current_scroll = sc_offsets.get_or_default(id);

            let basic = lay_resolved_basic.get_or(id, &DEFAULT_BASIC);
            let (border, padding) =
                LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

            let visible_size = WindowStore::calc_visible_size(rect, win_last_size);
            let content_size = OutputStore::calc_inner_content_size(visible_size, border, padding);

            let show_v = scroll_size.height > content_size.height;
            let show_h = scroll_size.width > content_size.width;

            // トラックデフォルト長の計算
            let track_h_default = ScrollbarStore::calculate_track_len(
                visible_size.height,
                border.top,
                border.bottom,
                show_h,
                sb_state.style.width,
            );
            let track_w = ScrollbarStore::calculate_track_len(
                visible_size.width,
                border.left,
                border.right,
                show_v,
                sb_state.style.width,
            );

            let mut ctx = ScrollbarSyncContext {
                topo_active_masks,
                topo_parents,
                lay_taffy_tree,
                lay_basic,
                lay_base_basic,
                lay_resolved_basic,
                lay_resolved_flex,
                lay_resolved_grid,
                lay_taffy_nodes,
                lay_flex,
                lay_grid,
                bar_styles,
                rnd_visual,
                rnd_base_visual,
                rnd_interaction,
                rnd_active_transitions,
                debug,
            };

            // 縦トラック (V-Track) の同期
            if let Some(v_track) = sb_state.v_track_id {
                let (visible, opacity) = ScrollbarStore::calculate_visibility_opacity(
                    sb_state.style.display,
                    sb_state.last_scroll_time,
                    show_v,
                );
                if visible {
                    let mut track_h = track_h_default;
                    if let Some(ref track_style) = sb_state.style.v_track
                        && let Val::Px(val) = track_style.inner.basic_layout.size.height
                    {
                        track_h = val;
                    }
                    ctx.update_el(
                        v_track,
                        Size::new(Val::Px(sb_state.style.width), Val::Px(track_h)),
                        Rect::new(Val::Px(0.0), Val::Px(0.0), Val::Auto, Val::Auto),
                        opacity,
                    );
                } else {
                    ctx.hide_el(v_track);
                }
            }

            // 縦つまみ (V-Thumb) の同期
            if let Some(v_thumb) = sb_state.v_thumb_id {
                let (visible, opacity) = ScrollbarStore::calculate_visibility_opacity(
                    sb_state.style.display,
                    sb_state.last_scroll_time,
                    show_v,
                );
                if visible {
                    let mut ext = ExtractedThumb {
                        initial_len: track_h_default,
                        min_len: 24.0,
                        max_len: track_h_default,
                        margin_start: 0.0,
                        margin_end: 0.0,
                        pad_start: 0.0,
                        pad_end: 0.0,
                        cross_size_override: None,
                    };

                    if let Some(ref thumb_style) = sb_state.style.v_thumb {
                        let layout = &thumb_style.inner.basic_layout;
                        if let Val::Px(val) = layout.size.height {
                            ext.initial_len = val;
                        }
                        if let Val::Px(val) = layout.min_size.height {
                            ext.min_len = val;
                        }
                        if let Val::Px(val) = layout.max_size.height {
                            ext.max_len = val;
                        }
                        if let Val::Px(val) = layout.margin.top {
                            ext.margin_start = val;
                        }
                        if let Val::Px(val) = layout.margin.bottom {
                            ext.margin_end = val;
                        }
                        if let Length::Px(val) = layout.padding.left {
                            ext.pad_start = val;
                        }
                        if let Length::Px(val) = layout.padding.right {
                            ext.pad_end = val;
                        }
                        if let Val::Px(val) = layout.size.width {
                            ext.cross_size_override = Some(val);
                        }
                    }

                    let (h, y, w, x) = ScrollbarStore::calculate_thumb_geometry(
                        track_h_default,
                        visible_size.height,
                        scroll_size.height,
                        current_scroll.y,
                        sb_state.style.width,
                        &ext,
                    );

                    ctx.update_el(
                        v_thumb,
                        Size::new(Val::Px(w), Val::Px(h)),
                        Rect::new(Val::Px(y), Val::Auto, Val::Auto, Val::Px(x)),
                        opacity,
                    );
                } else {
                    ctx.hide_el(v_thumb);
                }
            }

            // 横トラック (H-Track) の同期
            if let Some(h_track) = sb_state.h_track_id {
                let (visible, opacity) = ScrollbarStore::calculate_visibility_opacity(
                    sb_state.style.display,
                    sb_state.last_scroll_time,
                    show_h,
                );
                if visible {
                    ctx.update_el(
                        h_track,
                        Size::new(Val::Px(track_w), Val::Px(sb_state.style.width)),
                        Rect::new(Val::Auto, Val::Auto, Val::Px(0.0), Val::Px(0.0)),
                        opacity,
                    );
                } else {
                    ctx.hide_el(h_track);
                }
            }

            // 横つまみ (H-Thumb) の同期
            if let Some(h_thumb) = sb_state.h_thumb_id {
                let (visible, opacity) = ScrollbarStore::calculate_visibility_opacity(
                    sb_state.style.display,
                    sb_state.last_scroll_time,
                    show_h,
                );
                if visible {
                    let mut ext = ExtractedThumb {
                        initial_len: track_w,
                        min_len: 24.0,
                        max_len: track_w,
                        margin_start: 0.0,
                        margin_end: 0.0,
                        pad_start: 0.0,
                        pad_end: 0.0,
                        cross_size_override: None,
                    };

                    if let Some(ref thumb_style) = sb_state.style.h_thumb {
                        let layout = &thumb_style.inner.basic_layout;
                        if let Val::Px(val) = layout.size.width {
                            ext.initial_len = val;
                        }
                        if let Val::Px(val) = layout.min_size.width {
                            ext.min_len = val;
                        }
                        if let Val::Px(val) = layout.max_size.width {
                            ext.max_len = val;
                        }
                        if let Val::Px(val) = layout.margin.left {
                            ext.margin_start = val;
                        }
                        if let Val::Px(val) = layout.margin.right {
                            ext.margin_end = val;
                        }
                        if let Length::Px(val) = layout.padding.top {
                            ext.pad_start = val;
                        }
                        if let Length::Px(val) = layout.padding.bottom {
                            ext.pad_end = val;
                        }
                        if let Val::Px(val) = layout.size.height {
                            ext.cross_size_override = Some(val);
                        }
                    }

                    let (w, x, h, y) = ScrollbarStore::calculate_thumb_geometry(
                        track_w,
                        visible_size.width,
                        scroll_size.width,
                        current_scroll.x,
                        sb_state.style.width,
                        &ext,
                    );

                    ctx.update_el(
                        h_thumb,
                        Size::new(Val::Px(w), Val::Px(h)),
                        Rect::new(Val::Px(y), Val::Auto, Val::Auto, Val::Px(x)),
                        opacity,
                    );
                } else {
                    ctx.hide_el(h_thumb);
                }
            }
        }
    }
}

// つまみ（Thumb）計算用の入力パラメータ
pub(crate) struct ExtractedThumb {
    initial_len: f32,
    min_len: f32,
    max_len: f32,
    margin_start: f32,
    margin_end: f32,
    pad_start: f32,
    pad_end: f32,
    cross_size_override: Option<f32>,
}

pub(crate) struct ScrollbarSyncContext<'a> {
    pub topo_active_masks: &'a ActiveMasksSecondary,
    pub topo_parents: &'a ParentsSecondary,
    pub lay_taffy_tree: &'a mut TaffyTreeEntityId,
    pub lay_basic: &'a mut BasicLayoutsSecondary,
    pub lay_base_basic: &'a mut BaseBasicLayoutsSecondary,
    pub lay_resolved_basic: &'a mut ResolvedBasicSecondary,
    pub lay_resolved_flex: &'a mut ResolvedFlexSecondary,
    pub lay_resolved_grid: &'a mut ResolvedGridSparse,
    pub lay_taffy_nodes: &'a TaffyNodesSecondary,
    pub lay_flex: &'a FlexLayoutsSecondary,
    pub lay_grid: &'a GridLayoutsSparse,
    pub bar_styles: &'a ScrollbarStylesSparse,
    pub rnd_visual: &'a mut VisualPropertiesSecondary,
    pub rnd_base_visual: &'a mut BaseVisualPropertiesSecondary,
    pub rnd_interaction: &'a InteractionPropertiesSecondary,
    pub rnd_active_transitions: &'a ActiveTransitionsSparse,
    pub debug: &'a mut DebugStore,
}

impl ScrollbarSyncContext<'_> {
    #[inline]
    pub fn update_el(&mut self, el_id: EntityId, size: Size<Val>, rect: Rect<Val>, opacity: f32) {
        ScrollbarStore::update_scrollbar_element(
            el_id,
            size,
            rect,
            opacity,
            self.topo_active_masks,
            self.topo_parents,
            self.lay_taffy_tree,
            self.lay_basic,
            self.lay_base_basic,
            self.lay_resolved_basic,
            self.lay_resolved_flex,
            self.lay_resolved_grid,
            self.lay_taffy_nodes,
            self.lay_flex,
            self.lay_grid,
            self.bar_styles,
            self.rnd_visual,
            self.rnd_base_visual,
            self.rnd_interaction,
            self.rnd_active_transitions,
            self.debug,
        );
    }
    #[inline]
    pub fn hide_el(&mut self, el_id: EntityId) {
        ScrollbarStore::hide_scrollbar_element(
            el_id,
            self.lay_taffy_tree,
            self.lay_basic,
            self.lay_base_basic,
            self.lay_taffy_nodes,
        );
    }
}

impl ScrollbarStore {
    /// スクロールバー用要素のレイアウト情報を同期して更新。
    fn update_scrollbar_element_layout(
        id: EntityId,
        size: Size<Val>,
        inset: Rect<Val>,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
    ) {
        let layouts = [lay_basic.get_mut(id), lay_base_basic.get_mut(id)];

        for layout in layouts.into_iter().flatten() {
            layout.display = Display::Flex;
            layout.size = size;
            layout.inset = inset;
        }
    }

    /// スクロールバー用要素の不透明度（解決値と静的ベース値）を同時同期して更新します。
    #[inline]
    fn update_scrollbar_element_opacity(
        id: EntityId,
        opacity: f32,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_base_visual: &mut BaseVisualPropertiesSecondary,
    ) {
        let visuals = [rnd_visual.get_mut(id), rnd_base_visual.get_mut(id)];

        for vis in visuals.into_iter().flatten() {
            vis.opacity = Some(opacity);
        }
    }

    /// スクロールバー用要素（TrackやThumb）のレイアウト、不透明度、Taffyスタイルへの反映を一括して同期更新します。
    #[inline]
    fn update_scrollbar_element(
        id: EntityId,
        size: Size<Val>,
        inset: Rect<Val>,
        opacity: f32,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
        lay_resolved_basic: &mut ResolvedBasicSecondary,
        lay_resolved_flex: &mut ResolvedFlexSecondary,
        lay_resolved_grid: &mut ResolvedGridSparse,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSparse,
        bar_styles: &ScrollbarStylesSparse,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_base_visual: &mut BaseVisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparse,
        debug: &mut DebugStore,
    ) {
        ScrollbarStore::update_scrollbar_element_layout(
            id,
            size,
            inset,
            lay_taffy_tree,
            lay_basic,
            lay_base_basic,
            lay_taffy_nodes,
        );
        ScrollbarStore::update_scrollbar_element_opacity(id, opacity, rnd_visual, rnd_base_visual);

        // 変更されたスクロールバー要素のキャッシュを更新
        LayoutStore::update_resolved_active_layout_cache(
            id,
            topo_active_masks,
            topo_parents,
            lay_resolved_basic,
            lay_resolved_flex,
            lay_resolved_grid,
            lay_basic,
            lay_flex,
            lay_grid,
            rnd_visual,
            rnd_interaction,
            rnd_active_transitions,
            debug,
        );

        let basic = lay_resolved_basic.get_or(id, &DEFAULT_BASIC);
        let flex = lay_resolved_flex.get_or(id, &DEFAULT_FLEX);
        let grid = lay_resolved_grid.get(id); // Grid実装時用

        LayoutStore::set_taffy_style(
            id,
            &basic,
            &flex,
            grid,
            lay_taffy_tree,
            lay_taffy_nodes,
            bar_styles,
        );
    }

    /// スクロールバー用要素をレイアウト上から安全に隠します。
    #[inline]
    fn hide_scrollbar_element(
        id: EntityId,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
    ) {
        let layouts = [lay_basic.get_mut(id), lay_base_basic.get_mut(id)];

        for layout in layouts.into_iter().flatten() {
            layout.display = Display::None;
        }

        // 非表示パスで、Taffy側ノードスタイルを確実に Display::None にして同期する
        let node_id = *lay_taffy_nodes.at(id);
        lay_taffy_tree
            .set_style(
                node_id,
                taffy::Style {
                    display: taffy::Display::None,
                    ..Default::default()
                },
            )
            .unwrap();
    }
}
