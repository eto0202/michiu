use crate::{
    ActiveInteractionStates, ActiveMasksSecondary, BaseBasicLayoutsSecondary, BasicLayout,
    BasicLayoutsSecondary, ComponentMask, CursorIcon, DEFAULT_BASIC, DebugStore,
    DirtyLayoutEntitiesVec, DirtyRenderEntitiesVec, EntityId, LayoutPoint, LayoutRect, LayoutSize,
    LayoutStore, Length, MichiuSoA, OutputStore, ParentsSecondary, Position, Rect, RectsSecondary,
    TaffyNodesSecondary, TaffyTreeEntityId, TopologyStore, Val, VisualPropertiesSecondary,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeDirection {
    Top,
    Right,
    Bottom,
    Left,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone)]
pub struct ResizingState {
    pub entity_id: EntityId,
    pub direction: ResizeDirection,
    pub start_mouse_pos: LayoutPoint,
    pub start_rect: LayoutRect,
    pub start_inset: Rect<Val>,
}

pub type ActiveResizeHoverOption = Option<(EntityId, ResizeDirection)>;

pub(crate) struct ResizeStore {
    pub(crate) res_resizing_state: Option<ResizingState>,
    pub(crate) res_active_resize_hover: ActiveResizeHoverOption,
}

impl Default for ResizeStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ResizeStore {
    #[inline]
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            res_resizing_state: None,
            res_active_resize_hover: None,
        }
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.res_resizing_state = None;
        self.res_active_resize_hover = None;
    }
}

impl ResizeStore {
    /// リサイズ方向から対応するカーソル種別へ変換
    fn resize_direction_to_cursor(dir: ResizeDirection) -> CursorIcon {
        match dir {
            ResizeDirection::Top | ResizeDirection::Bottom => CursorIcon::ResizeNs(None),
            ResizeDirection::Left | ResizeDirection::Right => CursorIcon::ResizeEw(None),
            ResizeDirection::TopRight | ResizeDirection::BottomLeft => CursorIcon::ResizeNesw(None),
            ResizeDirection::TopLeft | ResizeDirection::BottomRight => CursorIcon::ResizeNwse(None),
        }
    }

    pub(crate) fn apply_resizable_cursor_style(
        id: EntityId,
        dir: ResizeDirection,
        rnd_visual: &mut VisualPropertiesSecondary,
    ) {
        let Some(vis) = rnd_visual.get_mut(id) else {
            return;
        };
        // 方向に対応する配列インデックス
        let idx = match dir {
            ResizeDirection::Top | ResizeDirection::Bottom => 0, // Ns
            ResizeDirection::Left | ResizeDirection::Right => 1, // Ew
            ResizeDirection::TopRight | ResizeDirection::BottomLeft => 2, // Nesw
            ResizeDirection::TopLeft | ResizeDirection::BottomRight => 3, // Nwse
        };

        // 独自指定があればそれを引き、なければデフォルトをフォールバックして解決
        let cursor = vis
            .resizable_cursor
            .and_then(|arr| arr[idx])
            .unwrap_or_else(|| ResizeStore::resize_direction_to_cursor(dir));

        vis.cursor = Some(cursor);
    }

    /// マウス位置と要素の境界・リサイズ許可フラグから、該当するリサイズ方向を算出するヘルパー
    fn detect_resize_direction(
        rect: LayoutRect,
        resizable: [bool; 4], // [top, right, bottom, left]
        pos: LayoutPoint,
        border: f32,
    ) -> Option<ResizeDirection> {
        let [t, r, b, l] = resizable;
        if !t && !r && !b && !l {
            return None;
        }

        // 境界線の外側（-border）から内側（+border）までのあそびの範囲を厳密に判定
        let on_t = t
            && (pos.y >= rect.y - border && pos.y <= rect.y + border)
            && (pos.x >= rect.x - border && pos.x <= rect.x + rect.width + border);

        let on_b = b
            && (pos.y >= rect.y + rect.height - border && pos.y <= rect.y + rect.height + border)
            && (pos.x >= rect.x - border && pos.x <= rect.x + rect.width + border);

        let on_l = l
            && (pos.x >= rect.x - border && pos.x <= rect.x + border)
            && (pos.y >= rect.y - border && pos.y <= rect.y + rect.height + border);

        let on_r = r
            && (pos.x >= rect.x + rect.width - border && pos.x <= rect.x + rect.width + border)
            && (pos.y >= rect.y - border && pos.y <= rect.y + rect.height + border);

        match (on_t, on_r, on_b, on_l) {
            (true, true, _, _) => Some(ResizeDirection::TopRight),
            (true, _, _, true) => Some(ResizeDirection::TopLeft),
            (_, true, true, _) => Some(ResizeDirection::BottomRight),
            (_, _, true, true) => Some(ResizeDirection::BottomLeft),
            (true, _, _, _) => Some(ResizeDirection::Top),
            (_, true, _, _) => Some(ResizeDirection::Right),
            (_, _, true, _) => Some(ResizeDirection::Bottom),
            (_, _, _, true) => Some(ResizeDirection::Left),
            _ => None,
        }
    }

    #[track_caller]
    pub(crate) fn found_resize_hover(
        target_id: Option<EntityId>,
        logical_pos: LayoutPoint,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_basic: &BasicLayoutsSecondary,
        out_rects: &RectsSecondary,
        debug: &mut DebugStore,
    ) -> (Option<EntityId>, Option<(EntityId, ResizeDirection)>) {
        let mut current_id = target_id;
        let mut found_resize_hover = None;
        while let Some(id) = current_id {
            if topo_active_masks.at(id).has(ComponentMask::STYLE_RESIZABLE) {
                let rect = out_rects.find_or_default(id, debug);
                let resizable_flags = lay_basic.find(id).map_or([false; 4], |l| l.resizable);

                // 境界外周に 6.0px のあそびを持たせてヒット判定
                let detect_border = 6.0f32;
                let direction = ResizeStore::detect_resize_direction(
                    rect,
                    resizable_flags,
                    logical_pos,
                    detect_border,
                );
                if let Some(dir) = direction {
                    found_resize_hover = Some((id, dir));
                    break; // 最も前面寄りのリサイズ親要素を優先採用
                }
            }
            current_id = *topo_parents.at(id);
        }
        (current_id, found_resize_hover)
    }

    #[track_caller]
    pub(crate) fn state_pressed_resize_drag(
        id: EntityId,
        dir: ResizeDirection,
        evt_interaction_states: &mut ActiveInteractionStates,
        res_resizing_state: &mut Option<ResizingState>,
        evt_current_pointer_position: Option<LayoutPoint>,
        topo_parents: &ParentsSecondary,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
        out_rects: &RectsSecondary,
        debug: &mut DebugStore,
    ) {
        let rect = out_rects.find_or_default(id, debug);
        let (position, mut start_inset) = lay_basic
            .find(id)
            .map_or((Position::default(), BasicLayout::default().inset), |l| {
                (l.position, l.inset)
            });

        let resolve_length = |length: Length, ref_size: f32| match length {
            Length::Px(v) => v,
            Length::Percent(p) => ref_size * (p / 100.0),
        };

        // 親要素の矩形と、その左・上ボーダーの厚みを取得
        let parent_id = topo_parents.at(id);
        let (parent_rect, parent_border_left, parent_border_top) =
            parent_id.map_or((LayoutRect::ZERO, 0.0, 0.0), |p_id| {
                let p_rect = *out_rects.at(p_id);
                // ボーダー幅の抽出
                let (border_l, border_t) = lay_basic.find(p_id).map_or((0.0, 0.0), |l| {
                    let left = resolve_length(l.border.left, p_rect.width);
                    let top = resolve_length(l.border.top, p_rect.height);
                    (left, top)
                });
                (p_rect, border_l, border_t)
            });

        // 親コンテナのボーダー内側を基準点として物理相対位置を逆算
        let local_x = rect.x - (parent_rect.x + parent_border_left);
        let local_y = rect.y - (parent_rect.y + parent_border_top);

        // 絶対配置の場合、開始時に Top-Left 基準に完全に正規化
        if position == Position::Absolute {
            start_inset = Rect {
                top: Val::Px(local_y),
                right: Val::Auto,
                bottom: Val::Auto,
                left: Val::Px(local_x),
            };

            // SoA 側も、この Top-Left 座標で即時上書きアップデート
            let basic = lay_basic.get_mut(id);
            let base_basic = lay_base_basic.get_mut(id);
            for layout in [basic, base_basic].into_iter().flatten() {
                layout.inset = start_inset;
            }
        }

        let start_pos = evt_current_pointer_position.unwrap_or_default();

        *res_resizing_state = Some(ResizingState {
            entity_id: id,
            direction: dir,
            start_mouse_pos: start_pos,
            start_rect: rect,
            start_inset,
        });

        // リサイズ中の要素は pressed とマーク
        evt_interaction_states.pressed = Some(id);
    }

    #[track_caller]
    pub(crate) fn sync_resizing_drag(
        logical_pos: LayoutPoint,
        state: &ResizingState,
        win_last_size: Option<LayoutSize>,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_base_basic: &mut BaseBasicLayoutsSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        out_rects: &RectsSecondary,
        debug: &mut DebugStore,
    ) {
        let id = state.entity_id;
        let delta_x = logical_pos.x - state.start_mouse_pos.x;
        let delta_y = logical_pos.y - state.start_mouse_pos.y;

        let basic = lay_basic.find_or(id, &DEFAULT_BASIC, debug);

        let start_rect = state.start_rect;

        // 最小サイズ・最大クランプ値の解決
        let (min_w, max_w, min_h, max_h) = {
            let rect = *out_rects.at(id);
            let (border, padding) =
                LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

            // 枠線と余白を足した、物理的にこれ以上小さくできない限界サイズ
            let abs_min_w = border.left + border.right + padding.left + padding.right;
            let abs_min_h = border.top + border.bottom + padding.top + padding.bottom;

            // 指定値を物理ピクセルに解決する
            let mut resolve_val = |val: Val, is_width: bool, fallback: f32| match val {
                Val::Px(v) => v,
                Val::Percent(_) => OutputStore::val_to_px(
                    id,
                    val,
                    is_width,
                    win_last_size,
                    topo_parents,
                    out_rects,
                    debug,
                ),
                Val::Auto => fallback,
            };

            let user_min_w = resolve_val(basic.min_size.width, true, 0.0);
            let user_min_h = resolve_val(basic.min_size.height, false, 0.0);
            let user_max_w = resolve_val(basic.max_size.width, true, f32::MAX);
            let user_max_h = resolve_val(basic.max_size.height, false, f32::MAX);

            (
                abs_min_w.max(user_min_w).max(10.0), // 最低限 10px は維持
                user_max_w,
                abs_min_h.max(user_min_h).max(10.0),
                user_max_h,
            )
        };

        // ドラッグ方向に基づく係数マップ
        // 1.0 は引っ張り（サイズ増加）、-1.0 は押し込み（サイズ減少）
        let (h_factor, v_factor) = match state.direction {
            ResizeDirection::Left => (Some(-1.0), None),
            ResizeDirection::Right => (Some(1.0), None),
            ResizeDirection::Top => (None, Some(-1.0)),
            ResizeDirection::Bottom => (None, Some(1.0)),
            ResizeDirection::TopLeft => (Some(-1.0), Some(-1.0)),
            ResizeDirection::TopRight => (Some(1.0), Some(-1.0)),
            ResizeDirection::BottomLeft => (Some(-1.0), Some(1.0)),
            ResizeDirection::BottomRight => (Some(1.0), Some(1.0)),
        };

        let mut new_w = start_rect.width;
        let mut new_h = start_rect.height;
        let mut delta_inset_left = 0.0;
        let mut delta_inset_top = 0.0;

        if let Some(factor) = h_factor {
            new_w = (start_rect.width + delta_x * factor).clamp(min_w, max_w);
            // 絶対配置（Absolute）で、左側へサイズを伸ばした（縮めた）場合はインセットを同期補正
            if basic.position == Position::Absolute && factor < 0.0 {
                delta_inset_left = start_rect.width - new_w;
            }
        }

        if let Some(factor) = v_factor {
            new_h = (start_rect.height + delta_y * factor).clamp(min_h, max_h);
            // 絶対配置（Absolute）で、上側へサイズを伸ばした（縮めた）場合はインセットを同期補正
            if basic.position == Position::Absolute && factor < 0.0 {
                delta_inset_top = start_rect.height - new_h;
            }
        }

        let layouts = [lay_basic.get_mut(id), lay_base_basic.get_mut(id)];

        for layout in layouts.into_iter().flatten() {
            layout.size.width = Val::Px(new_w);
            layout.size.height = Val::Px(new_h);

            if layout.position != Position::Absolute {
                continue;
            }

            if let Val::Px(start_top) = state.start_inset.top {
                layout.inset.top = Val::Px(start_top + delta_inset_top);
            }
            if let Val::Px(start_left) = state.start_inset.left {
                layout.inset.left = Val::Px(start_left + delta_inset_left);
            }
            layout.inset.right = Val::Auto;
            layout.inset.bottom = Val::Auto;
        }
        TopologyStore::mark_dirty(
            id,
            topo_active_masks,
            topo_parents,
            lay_dirty_entities,
            lay_taffy_tree,
            lay_taffy_nodes,
            rnd_dirty_entities,
            debug,
        );
    }
}
