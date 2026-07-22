use crate::*;
use slotmap::{SecondaryMap, SparseSecondaryMap};
use std::{collections::HashSet, sync::Arc, time::{Duration, Instant}};

/// CPU 側で現在再生中の動的なキーフレームアニメーションの状態
#[derive(Debug, Clone)]
pub(crate) struct ActiveAnimation {
    pub(crate) property: PropertyList,
    pub(crate) start_time: Instant,
    pub(crate) duration: Duration,
    pub(crate) iteration_count: PlaybackCount,
    pub(crate) curve: AnimationCurve,

    // 回転アニメーションなどのために、現在の周回（ループ）における開始ベース値と目標値を定義
    pub(crate) start_value: TransitionValue,
    pub(crate) end_value: TransitionValue,
}

pub struct RenderStore {
    pub(crate) visual_properties: SecondaryMap<EntityId, VisualProperty>,
    pub(crate) interaction_properties: SecondaryMap<EntityId, InteractionStyles>,
    pub(crate) base_basic_layouts: SecondaryMap<EntityId, BasicLayout>,
    pub(crate) base_visual_properties: SecondaryMap<EntityId, VisualProperty>,
    pub(crate) dirty_render_entities: Vec<EntityId>,
    pub(crate) active_transitions: SparseSecondaryMap<EntityId, Vec<ActiveTransition>>,
    pub(crate) active_animations: SparseSecondaryMap<EntityId, Vec<ActiveAnimation>>,
    pub(crate) active_webviews: HashSet<EntityId>,
    pub(crate) last_tick_time: Option<std::time::Instant>,
}

impl Default for RenderStore {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderStore {
    #[inline]
    pub fn new() -> Self {
        Self {
            visual_properties: SecondaryMap::new(),
            interaction_properties: SecondaryMap::new(),
            base_basic_layouts: SecondaryMap::new(),
            base_visual_properties: SecondaryMap::new(),
            dirty_render_entities: Vec::new(),
            active_transitions: SparseSecondaryMap::new(),
            active_animations: SparseSecondaryMap::new(),
            active_webviews: HashSet::new(),
            last_tick_time: None,
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.visual_properties.clear();
        self.interaction_properties.clear();
        self.base_basic_layouts.clear();
        self.base_visual_properties.clear();
        self.dirty_render_entities.clear();
        self.active_transitions.clear();
        self.active_animations.clear();
        self.active_webviews.clear();
        self.last_tick_time = None;
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.visual_properties.remove(id);
        self.interaction_properties.remove(id);
        self.base_basic_layouts.remove(id);
        self.base_visual_properties.remove(id);
        self.dirty_render_entities.retain(|&x| x != id);
        self.active_transitions.remove(id);
        self.active_animations.remove(id);
        self.active_webviews.remove(&id);
    }
}

impl RenderStore {
    pub(crate) fn get_basic_layout_mut(
        id: EntityId,
        renders: &mut RenderStore,
        target: StyleTarget,
    ) -> Option<&mut BasicLayout> {
        match target {
            StyleTarget::Base => renders.base_basic_layouts.get_mut(id),
            _ => {
                if !renders.interaction_properties.contains_key(id) {
                    renders
                        .interaction_properties
                        .insert(id, InteractionStyles::default());
                }
                let styles = renders.interaction_properties.get_mut(id).unwrap();
                let style_ref = styles.get_style_target_mut(target);
                Some(&mut Arc::make_mut(&mut style_ref.inner).basic_layout)
            }
        }
    }

    pub(crate) fn get_visual_property_mut(
        id: EntityId,
        renders: &mut RenderStore,
        target: StyleTarget,
    ) -> Option<&mut VisualProperty> {
        match target {
            StyleTarget::Base => renders.base_visual_properties.get_mut(id),
            _ => {
                if !renders.interaction_properties.contains_key(id) {
                    renders
                        .interaction_properties
                        .insert(id, InteractionStyles::default());
                }
                let styles = renders.interaction_properties.get_mut(id).unwrap();
                let style_ref = styles.get_style_target_mut(target);
                Some(&mut Arc::make_mut(&mut style_ref.inner).visual_property)
            }
        }
    }

    pub(crate) fn get_flex_layout_mut<'a>(
        id: EntityId,
        renders: &'a mut RenderStore,
        target: StyleTarget,
        flex_layouts: &'a mut SecondaryMap<EntityId, FlexLayout>,
    ) -> Option<&'a mut FlexLayout> {
        match target {
            StyleTarget::Base => flex_layouts.get_mut(id),
            _ => {
                if !renders.interaction_properties.contains_key(id) {
                    renders
                        .interaction_properties
                        .insert(id, InteractionStyles::default());
                }
                let styles = renders.interaction_properties.get_mut(id).unwrap();
                let style_ref = styles.get_style_target_mut(target);
                Some(&mut Arc::make_mut(&mut style_ref.inner).flex_layout)
            }
        }
    }

    /// スクロールバー用要素の不透明度（解決値と静的ベース値）を同時同期して更新します。
    #[inline]
    pub(crate) fn update_scrollbar_element_opacity(
        id: EntityId,
        renders: &mut RenderStore,
        opacity: f32,
    ) {
        if let Some(vis) = renders.visual_properties.get_mut(id) {
            vis.opacity = Some(opacity);
        }
        if let Some(vis) = renders.base_visual_properties.get_mut(id) {
            vis.opacity = Some(opacity);
        }
    }

    pub(crate) fn trigger_keyframe_animations_if_needed(id: EntityId, renders: &mut RenderStore) {
        if let Some(visual) = renders.visual_properties.get(id) {
            if visual.keyframe_animations.is_empty() {
                return;
            }

            let now = Instant::now();

            // 借用回避のため定義を一度クローン
            let anims = visual.keyframe_animations.clone();

            if !renders.active_animations.contains_key(id) {
                renders.active_animations.insert(id, Vec::new());
            }
            let active_list = renders.active_animations.get_mut(id).unwrap();

            for anim in anims {
                // すでに同じプロパティのアニメーションが駆動中なら重複起動をスルー
                if active_list.iter().any(|a| a.property == anim.property) {
                    continue;
                }

                // 初期値（開始値）と目標値（100%キーフレームに相当する値）を設定
                let (start_val, end_val) = match anim.property {
                    PropertyList::Transform => {
                        let start = TransitionValue::Transform(IDENTITY_MATRIX);
                        // Z軸を1周（2PI）回転させる行列を終点にする
                        let mut end_transform =
                            crate::Transform::new().rotate(std::f32::consts::PI * 2.0);
                        let end = TransitionValue::Transform(end_transform.matrix);
                        (start, end)
                    }
                    PropertyList::Opacity => {
                        (TransitionValue::Opacity(1.0), TransitionValue::Opacity(0.0)) // フェードアウト等
                    }
                    _ => continue, // TODO: 他プロパティも定義
                };

                active_list.push(ActiveAnimation {
                    property: anim.property,
                    start_time: now,
                    duration: anim.duration,
                    iteration_count: anim.iteration_count,
                    curve: anim.curve,
                    start_value: start_val,
                    end_value: end_val,
                });
            }
        }
    }

    /// 指定された動的状態（例: STATE_HOVERED）に切り替わる際、
    /// その要素に割り当てられている状態スタイルがレイアウトの再計算を必要とするか判定します。
    pub(crate) fn does_state_require_layout(
        id: EntityId,
        renders: &RenderStore,
        state_flag: u128,
    ) -> bool {
        if let Some(interaction) = renders.interaction_properties.get(id) {
            // 対象となる状態スタイルを取得
            let target_style = match state_flag {
                STATE_HOVERED => &interaction.hovered,
                STATE_FOCUSED => &interaction.focused,
                STATE_PRESSED => &interaction.pressed,
                STATE_DISABLED => &interaction.disabled,
                STATE_ACTIVED => &interaction.actived,
                STATE_SELECTED => &interaction.selected,
                STATE_DRAGGED => &interaction.dragged,
                STATE_DRAGGING => &interaction.dragging,
                STATE_DRAG_IN => &interaction.drag_in,
                STATE_DRAG_OVER => &interaction.drag_over,
                _ => &None,
            };

            // 指定された状態スタイルが存在する場合のみ、内部マスクを検証
            if let Some(style) = target_style {
                let mask = style.inner.mask;
                // 基本レイアウト、Flexレイアウト、またはGridレイアウト変更が含まれていれば true
                return mask.has_basic_layout() || mask.has_flex_layout() || mask.has_grid_layout();
            }
        }
        false
    }

    pub(crate) fn resolv_focus_style(
        id: EntityId,
        renders: &RenderStore,
        active_mask: &ComponentMask,
        parents: &SecondaryMap<EntityId, Option<EntityId>>,
    ) -> Option<ThisStyle> {
        if active_mask.has(STATE_FOCUSED) {
            if let Some(interaction) = renders.interaction_properties.get(id)
                && let Some(ref self_f_style) = interaction.focused
            {
                Some(self_f_style.clone()) // 自身に明確な focused 指定があれば最優先
            } else {
                let focus_mode = renders
                    .visual_properties
                    .get(id)
                    .and_then(|v| v.focusable)
                    .unwrap_or(Focusable::None);

                if matches!(focus_mode, Focusable::Inherit(_)) {
                    // 親先祖を上に辿り、最初に focused 疑似スタイルを定義している要素のその設定をそのまま借用する
                    let mut curr = parents.get(id).copied().flatten();
                    let mut found_parent_focused_style = None;
                    while let Some(curr_id) = curr {
                        if let Some(parent_interaction) =
                            renders.interaction_properties.get(curr_id)
                            && let Some(ref parent_f_style) = parent_interaction.focused
                        {
                            found_parent_focused_style = Some(parent_f_style.clone());
                            break;
                        }
                        curr = parents.get(curr_id).copied().flatten();
                    }
                    found_parent_focused_style
                } else {
                    None
                }
            }
        } else {
            None
        }
    }

    pub(crate) fn cascade_interaction(
        id: EntityId,
        renders: &RenderStore,
        active_mask: ComponentMask,
        target: &mut TargetStyle,
        focus_style_resolved: Option<ThisStyle>,
    ) {
        if let Some(interaction) = renders.interaction_properties.get(id) {
            let cascade = [
                (STATE_FOCUSED, &focus_style_resolved),
                (STATE_SELECTED, &interaction.selected),
                (STATE_ACTIVED, &interaction.actived),
                (STATE_HOVERED, &interaction.hovered),
                (STATE_PRESSED, &interaction.pressed),
                (STATE_DISABLED, &interaction.disabled),
                (STATE_DRAGGED, &interaction.dragged),
                (STATE_DRAGGING, &interaction.dragging),
                (STATE_DRAG_IN, &interaction.drag_in),
                (STATE_DRAG_OVER, &interaction.drag_over),
            ];

            for (state, style_opt) in cascade {
                if active_mask.has(state)
                    && let Some(style) = style_opt
                {
                    let inner_vis = &style.inner.visual_property;
                    let inner_mask = style.inner.mask;

                    if inner_mask.has(STYLE_BG_COLOR) {
                        target.bg_color = inner_vis.bg_color;
                    }
                    if inner_mask.has(STYLE_BORDER_COLOR) {
                        target.border_color = inner_vis.border_color;
                    }
                    if inner_mask.has(STYLE_OPACITY) {
                        target.opacity = inner_vis.opacity;
                    }
                    if inner_mask.has(STYLE_TRANSFORM) {
                        target.transform = inner_vis.transform;
                    }
                    if inner_mask.has(STYLE_CORNER_RADIUS) {
                        target.corner_radius = inner_vis.corner_radius;
                    }
                    if inner_mask.has(STYLE_POINTER_EVENTS) {
                        target.pointer_events = inner_vis.pointer_events;
                    }
                    if inner_mask.has(STYLE_BOX_SHADOW) {
                        if inner_vis.shadow_params.is_some() {
                            target.shadow_params = inner_vis.shadow_params;
                        }
                        if inner_vis.shadow_color.is_some() {
                            target.shadow_color = inner_vis.shadow_color;
                        }
                    }
                    if inner_mask.has(STYLE_TEXT_COLOR) {
                        target.text_color = inner_vis.text_color;
                    }
                    if inner_mask.has(STYLE_USER_SELECT) {
                        if inner_vis.select_bg_color.is_some() {
                            target.select_bg_color = inner_vis.select_bg_color;
                        }
                        if inner_vis.select_text_color.is_some() {
                            target.select_text_color = inner_vis.select_text_color;
                        }
                    }
                    if inner_mask.has(STYLE_BORDER) {
                        if inner_vis.border_lengths.is_some() {
                            target.border_lengths = inner_vis.border_lengths;
                        }
                        if inner_vis.border_styles.is_some() {
                            target.border_styles = inner_vis.border_styles;
                        }
                        if inner_vis.border_alignments.is_some() {
                            target.border_alignments = inner_vis.border_alignments;
                        }
                    }
                    if inner_mask.has(STYLE_OUTLINE) {
                        if inner_vis.outline_width.is_some() {
                            target.outline_width = inner_vis.outline_width;
                        }
                        if inner_vis.outline_color.is_some() {
                            target.outline_color = inner_vis.outline_color;
                        }
                        if inner_vis.outline_lengths.is_some() {
                            target.outline_lengths = inner_vis.outline_lengths;
                        }
                        if inner_vis.outline_styles.is_some() {
                            target.outline_styles = inner_vis.outline_styles;
                        }
                        if inner_vis.outline_alignments.is_some() {
                            target.outline_alignments = inner_vis.outline_alignments;
                        }
                        if inner_vis.outline_offset.is_some() {
                            target.outline_offset = inner_vis.outline_offset;
                        }
                    }
                    if inner_mask.has(STYLE_CURSOR) {
                        target.cursor = inner_vis.cursor;
                    }
                    if inner_mask.has(STYLE_RESIZABLE) {
                        target.resizable_cursor = inner_vis.resizable_cursor;
                    }
                }
            }
        }
    }

    pub(crate) fn cascade_within_interaction(
        id: EntityId,
        topology: &TopologyStore,
        renders: &RenderStore,
        active_mask: ComponentMask,
        target: &mut TargetStyle,
    ) {
        if active_mask.has(STYLE_INTERACTION_WITHIN)
            && let Some(interaction) = renders.interaction_properties.get(id)
        {
            // 自身の mask にビットが立っている場合のみツリー再帰を走らせてマージ解決
            let cascade_within = [
                (STATE_FOCUSED, &interaction.focused_within),
                (STATE_SELECTED, &interaction.selected_within),
                (STATE_ACTIVED, &interaction.actived_within),
                (STATE_HOVERED, &interaction.hovered_within),
                (STATE_PRESSED, &interaction.pressed_within),
                (STATE_DISABLED, &interaction.disabled_within),
                (STATE_DRAGGING, &interaction.dragged_within),
                (STATE_DRAG_IN, &interaction.hovered_within),
            ];

            for (state, style_opt) in cascade_within {
                // 子孫要素のいずれかがこの state_flag を満たしているか
                if TopologyStore::has_descendant_with_state(id, topology, state)
                    && let Some(style) = style_opt
                {
                    let inner_vis = &style.inner.visual_property;
                    let inner_mask = style.inner.mask;

                    if inner_mask.has(STYLE_BG_COLOR) {
                        target.bg_color = inner_vis.bg_color;
                    }
                    if inner_mask.has(STYLE_BORDER_COLOR) {
                        target.border_color = inner_vis.border_color;
                    }
                    if inner_mask.has(STYLE_OPACITY) {
                        target.opacity = inner_vis.opacity;
                    }
                    if inner_mask.has(STYLE_TRANSFORM) {
                        target.transform = inner_vis.transform;
                    }
                    if inner_mask.has(STYLE_CORNER_RADIUS) {
                        target.corner_radius = inner_vis.corner_radius;
                    }
                    if inner_mask.has(STYLE_POINTER_EVENTS) {
                        target.pointer_events = inner_vis.pointer_events;
                    }
                    if inner_mask.has(STYLE_BOX_SHADOW) {
                        if inner_vis.shadow_params.is_some() {
                            target.shadow_params = inner_vis.shadow_params;
                        }
                        if inner_vis.shadow_color.is_some() {
                            target.shadow_color = inner_vis.shadow_color;
                        }
                    }
                    if inner_mask.has(STYLE_TEXT_COLOR) {
                        target.text_color = inner_vis.text_color;
                    }
                    if inner_mask.has(STYLE_BORDER) {
                        if inner_vis.border_lengths.is_some() {
                            target.border_lengths = inner_vis.border_lengths;
                        }
                        if inner_vis.border_styles.is_some() {
                            target.border_styles = inner_vis.border_styles;
                        }
                        if inner_vis.border_alignments.is_some() {
                            target.border_alignments = inner_vis.border_alignments;
                        }
                    }
                    if inner_mask.has(STYLE_OUTLINE) {
                        if inner_vis.outline_width.is_some() {
                            target.outline_width = inner_vis.outline_width;
                        }
                        if inner_vis.outline_color.is_some() {
                            target.outline_color = inner_vis.outline_color;
                        }
                        if inner_vis.outline_lengths.is_some() {
                            target.outline_lengths = inner_vis.outline_lengths;
                        }
                        if inner_vis.outline_styles.is_some() {
                            target.outline_styles = inner_vis.outline_styles;
                        }
                        if inner_vis.outline_alignments.is_some() {
                            target.outline_alignments = inner_vis.outline_alignments;
                        }
                        if inner_vis.outline_offset.is_some() {
                            target.outline_offset = inner_vis.outline_offset;
                        }
                    }
                }
            }

            // All（いずれかのインタラクションがあればON）の解決
            if let Some(ref style) = interaction.any_within
                && TopologyStore::has_descendant_with_any_active_state(id, topology)
            {
                let inner_vis = &style.inner.visual_property;
                let inner_mask = style.inner.mask;

                if inner_mask.has(STYLE_BG_COLOR) {
                    target.bg_color = inner_vis.bg_color;
                }
                if inner_mask.has(STYLE_BORDER_COLOR) {
                    target.border_color = inner_vis.border_color;
                }
                if inner_mask.has(STYLE_OPACITY) {
                    target.opacity = inner_vis.opacity;
                }
                if inner_mask.has(STYLE_TRANSFORM) {
                    target.transform = inner_vis.transform;
                }
                if inner_mask.has(STYLE_CORNER_RADIUS) {
                    target.corner_radius = inner_vis.corner_radius;
                }
                if inner_mask.has(STYLE_POINTER_EVENTS) {
                    target.pointer_events = inner_vis.pointer_events;
                }
                if inner_mask.has(STYLE_BOX_SHADOW) {
                    if inner_vis.shadow_params.is_some() {
                        target.shadow_params = inner_vis.shadow_params;
                    }
                    if inner_vis.shadow_color.is_some() {
                        target.shadow_color = inner_vis.shadow_color;
                    }
                }
                if inner_mask.has(STYLE_TEXT_COLOR) {
                    target.text_color = inner_vis.text_color;
                }
                if inner_mask.has(STYLE_BORDER) {
                    if inner_vis.border_lengths.is_some() {
                        target.border_lengths = inner_vis.border_lengths;
                    }
                    if inner_vis.border_styles.is_some() {
                        target.border_styles = inner_vis.border_styles;
                    }
                    if inner_vis.border_alignments.is_some() {
                        target.border_alignments = inner_vis.border_alignments;
                    }
                }
                if inner_mask.has(STYLE_OUTLINE) {
                    if inner_vis.outline_width.is_some() {
                        target.outline_width = inner_vis.outline_width;
                    }
                    if inner_vis.outline_color.is_some() {
                        target.outline_color = inner_vis.outline_color;
                    }
                    if inner_vis.outline_lengths.is_some() {
                        target.outline_lengths = inner_vis.outline_lengths;
                    }
                    if inner_vis.outline_styles.is_some() {
                        target.outline_styles = inner_vis.outline_styles;
                    }
                    if inner_vis.outline_alignments.is_some() {
                        target.outline_alignments = inner_vis.outline_alignments;
                    }
                    if inner_vis.outline_offset.is_some() {
                        target.outline_offset = inner_vis.outline_offset;
                    }
                }
            }
        }
    }

    pub(crate) fn cascade_basic_layout(
        id: EntityId,
        renders: &RenderStore,
        active_mask: ComponentMask,
        target_layout: &mut BasicLayout,
    ) {
        if let Some(interaction) = renders.interaction_properties.get(id) {
            let cascade = [
                (STATE_FOCUSED, &interaction.focused),
                (STATE_SELECTED, &interaction.selected),
                (STATE_ACTIVED, &interaction.actived),
                (STATE_HOVERED, &interaction.hovered),
                (STATE_PRESSED, &interaction.pressed),
                (STATE_DISABLED, &interaction.disabled),
                (STATE_DRAGGED, &interaction.dragged),
                (STATE_DRAGGING, &interaction.dragging),
                (STATE_DRAG_IN, &interaction.drag_in),
                (STATE_DRAG_OVER, &interaction.drag_over),
            ];

            for (state, style_opt) in cascade {
                if active_mask.has(state)
                    && let Some(style) = style_opt
                {
                    target_layout.override_with(&style.inner.basic_layout, style.inner.mask);
                }
            }
        }
    }

    /// 現在ホバーされている要素から親ツリーを遡り、適用するべき物理的な CursorIcon を正確に解決します。
    pub fn resolve_cursor(
        hovered_id: EntityId,
        events: &EventStore,
        renders: &RenderStore,
        topology: &TopologyStore,
    ) -> CursorIcon {
        // 現在プレス中の要素（pressed）があればそれを最優先で探索の基点にする
        let start_id = events.interaction_states.pressed.unwrap_or(hovered_id);

        let mut curr = Some(start_id);
        let mut global_cursor = None;

        while let Some(id) = curr {
            let cursor_opt = renders
                .visual_properties
                .get(id)
                .and_then(|v| v.cursor)
                .or_else(|| {
                    renders
                        .base_visual_properties
                        .get(id)
                        .and_then(|v| v.cursor)
                });

            if let Some(cursor) = cursor_opt {
                match cursor {
                    // Global バリアントを見つけた場合、より具体的な個別カーソルが見つかっていない場合のみ記録
                    CursorIcon::Global(global_icon) => {
                        if global_cursor.is_none() {
                            global_cursor = Some(global_icon);
                        }
                    }
                    // 通常の個別カーソルが見つかった場合はこれが最優先なので即時採用
                    // 親の Global の影響を遮断してDefault()に戻したい場合は、子要素側で Default() がヒットするため即時解決
                    normal_cursor => {
                        return normal_cursor;
                    }
                }
            }
            curr = topology.parents.get(id).copied().flatten();
        }

        // 個別指定がなく、親のいずれかに Global カーソルが定義されていた場合はそれを採用
        if let Some(global) = global_cursor {
            match global {
                GlobalCursorIcon::Default(opt) => CursorIcon::Default(opt),
                GlobalCursorIcon::Pointer(opt) => CursorIcon::Pointer(opt),
                GlobalCursorIcon::Text(opt) => CursorIcon::Text(opt),
                GlobalCursorIcon::Grab(opt) => CursorIcon::Grab(opt),
                GlobalCursorIcon::Grabbing(opt) => CursorIcon::Grabbing(opt),
                GlobalCursorIcon::NotAllowed(opt) => CursorIcon::NotAllowed(opt),
                GlobalCursorIcon::ResizeNs(opt) => CursorIcon::ResizeNs(opt),
                GlobalCursorIcon::ResizeEw(opt) => CursorIcon::ResizeEw(opt),
                GlobalCursorIcon::ResizeNesw(opt) => CursorIcon::ResizeNesw(opt),
                GlobalCursorIcon::ResizeNwse(opt) => CursorIcon::ResizeNwse(opt),
            }
        } else {
            // 先祖に何の設定もない場合はデフォルトの矢印
            CursorIcon::Default(None)
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CurrentStyle {
    pub(crate) bg_color: Color,
    pub(crate) border_color: Color,
    pub(crate) outline_width: EdgeInsets,
    pub(crate) outline_color: Color,
    pub(crate) outline_offset: f32,
    pub(crate) opacity: f32,
    pub(crate) transform: [[f32; 4]; 4],
    pub(crate) corner_radius: CornerRadius,
    pub(crate) shadow_params: BoxShadow,
}
impl RenderStore {
    /// 現在の描画用データを取得 (Copy可能なプリミティブのみ)
    #[inline]
    pub(crate) fn get_current_style(id: EntityId, renders: &RenderStore) -> CurrentStyle {
        let bg_color = renders
            .visual_properties
            .get(id)
            .and_then(|v| v.bg_color)
            .unwrap_or(Color::TRANSPARENT);
        let border_color = renders
            .visual_properties
            .get(id)
            .and_then(|v| v.border_color)
            .unwrap_or(Color::TRANSPARENT);
        let outline_width = renders
            .visual_properties
            .get(id)
            .and_then(|v| v.outline_width)
            .unwrap_or(EdgeInsets::ZERO);
        let outline_color = renders
            .visual_properties
            .get(id)
            .and_then(|v| v.outline_color)
            .unwrap_or(Color::TRANSPARENT);
        let outline_offset = renders
            .visual_properties
            .get(id)
            .and_then(|v| v.outline_offset)
            .unwrap_or(0.0);
        let opacity = renders
            .visual_properties
            .get(id)
            .and_then(|v| v.opacity)
            .unwrap_or(1.0);
        let transform = renders
            .visual_properties
            .get(id)
            .and_then(|v| v.transform)
            .unwrap_or(IDENTITY_MATRIX);
        let corner_radius = renders
            .visual_properties
            .get(id)
            .and_then(|v| v.corner_radius)
            .unwrap_or(CornerRadius::ZERO);
        let shadow_params = renders
            .visual_properties
            .get(id)
            .and_then(|v| v.shadow_params)
            .unwrap_or(BoxShadow::none());

        CurrentStyle {
            bg_color,
            border_color,
            outline_width,
            outline_color,
            outline_offset,
            opacity,
            transform,
            corner_radius,
            shadow_params,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TargetStyle {
    pub(crate) pointer_events: Option<PointerEvents>,
    pub(crate) cursor: Option<CursorIcon>,
    pub(crate) resizable_cursor: Option<[Option<CursorIcon>; 4]>,
    pub(crate) bg_color: Option<Color>,
    pub(crate) border_color: Option<Color>,
    pub(crate) opacity: Option<f32>,
    pub(crate) transform: Option<[[f32; 4]; 4]>,
    pub(crate) corner_radius: Option<CornerRadius>,
    pub(crate) shadow_params: Option<BoxShadow>,
    pub(crate) shadow_color: Option<Color>,
    pub(crate) text_color: Option<Color>,
    pub(crate) select_bg_color: Option<Color>,
    pub(crate) select_text_color: Option<Color>,
    pub(crate) border_lengths: Option<EdgeInsets>,
    pub(crate) border_styles: Option<[BorderStyle; 4]>,
    pub(crate) border_alignments: Option<[BorderAlignment; 4]>,
    pub(crate) outline_width: Option<EdgeInsets>,
    pub(crate) outline_color: Option<Color>,
    pub(crate) outline_lengths: Option<EdgeInsets>,
    pub(crate) outline_styles: Option<[BorderStyle; 4]>,
    pub(crate) outline_alignments: Option<[BorderAlignment; 4]>,
    pub(crate) outline_offset: Option<f32>,
}

impl RenderStore {
    /// 目標値を参照経由で構築
    #[inline]
    pub(crate) fn get_target_style(id: EntityId, renders: &RenderStore) -> TargetStyle {
        let pointer_events = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.pointer_events);
        let cursor = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.cursor);
        let resizable_cursor = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.resizable_cursor);
        let bg_color = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.bg_color);
        let border_color = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.border_color);
        let opacity = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.opacity);
        let transform = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.transform);
        let corner_radius = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.corner_radius);
        let shadow_params = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.shadow_params);
        let shadow_color = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.shadow_color);
        let text_color = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.text_color);
        let select_bg_color = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.select_bg_color);
        let select_text_color = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.select_text_color);
        let border_lengths = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.border_lengths);
        let border_styles = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.border_styles);
        let border_alignments = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.border_alignments);
        let outline_width = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.outline_width);
        let outline_color = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.outline_color);
        let outline_lengths = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.outline_lengths);
        let outline_styles = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.outline_styles);
        let outline_alignments = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.outline_alignments);
        let outline_offset = renders
            .base_visual_properties
            .get(id)
            .and_then(|v| v.outline_offset);

        TargetStyle {
            pointer_events,
            cursor,
            resizable_cursor,
            bg_color,
            border_color,
            opacity,
            transform,
            corner_radius,
            shadow_params,
            shadow_color,
            text_color,
            select_bg_color,
            select_text_color,
            border_lengths,
            border_styles,
            border_alignments,
            outline_width,
            outline_color,
            outline_lengths,
            outline_styles,
            outline_alignments,
            outline_offset,
        }
    }
}
