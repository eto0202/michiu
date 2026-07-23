use crate::*;
use slotmap::{SecondaryMap, SparseSecondaryMap};
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};

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
    /// 描画（レンダー）ダーティ状態として登録された要素をすべてクリアします。
    pub fn clear_render_dirty(renders: &mut RenderStore, topology: &mut TopologyStore) {
        for id in renders.dirty_render_entities.drain(..) {
            if let Some(mask) = topology.active_masks.get_mut(id) {
                mask.unset(STATE_QUEUED_RENDER);
            }
        }
        renders.dirty_render_entities.clear();
    }

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

    /// 現在、アクティブに動いているトランジション（wgpuアニメーション）があるか判定します
    pub fn has_active_animations(
        renders: &RenderStore,
        events: &EventStore,
        layouts: &LayoutStore,
        contents: &ContentStore,
        has_drag_autoscroll: bool,
    ) -> bool {
        // トランジション（CSS transition）のアクティブ判定
        let has_transitions = !renders.active_transitions.is_empty()
            && renders
                .active_transitions
                .values()
                .any(|list| !list.is_empty());

        // キーフレームアニメーション（CSS animation）のアクティブ判定
        let has_keyframes = !renders.active_animations.is_empty()
            && renders
                .active_animations
                .values()
                .any(|list| !list.is_empty());

        // 3フォーカスされたインプットがあり、キャレット点滅が有効な間は描画ループを駆動
        let has_blinking_input = events
            .interaction_states
            .focused
            .and_then(|id| contents.input_contents.get(id))
            .map(|c| c.has_caret && c.is_blink)
            .unwrap_or(false);

        // 一時的表示スクロールバーのフェード進行中は描画更新ループを継続
        let has_active_transient_scrollbar = layouts.scrollbar_styles.values().any(|sb_state| {
            sb_state.style.display == ScrollbarDisplay::Transient
                && sb_state
                    .last_scroll_time
                    .map(|t| t.elapsed() < Duration::from_millis(1500))
                    .unwrap_or(false)
        });

        has_transitions
            || has_keyframes
            || has_blinking_input
            || has_active_transient_scrollbar
            || has_drag_autoscroll
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

    pub(crate) fn cascade_interaction_flag<'a>(
        id: EntityId,
        renders: &RenderStore,
        interaction: &'a InteractionStyles,
        focus_style_resolved: &'a Option<ThisStyle>,
    ) -> [(u128, &'a Option<ThisStyle>); 10] {
        [
            (STATE_FOCUSED, focus_style_resolved),
            (STATE_SELECTED, &interaction.selected),
            (STATE_ACTIVED, &interaction.actived),
            (STATE_HOVERED, &interaction.hovered),
            (STATE_PRESSED, &interaction.pressed),
            (STATE_DISABLED, &interaction.disabled),
            (STATE_DRAGGED, &interaction.dragged),
            (STATE_DRAGGING, &interaction.dragging),
            (STATE_DRAG_IN, &interaction.drag_in),
            (STATE_DRAG_OVER, &interaction.drag_over),
        ]
    }

    pub(crate) fn cascade_within_interaction_flag(
        id: EntityId,
        interaction: &InteractionStyles,
    ) -> [(u128, &Option<ThisStyle>); 8] {
        [
            (STATE_FOCUSED, &interaction.focused_within),
            (STATE_SELECTED, &interaction.selected_within),
            (STATE_ACTIVED, &interaction.actived_within),
            (STATE_HOVERED, &interaction.hovered_within),
            (STATE_PRESSED, &interaction.pressed_within),
            (STATE_DISABLED, &interaction.disabled_within),
            (STATE_DRAGGING, &interaction.dragged_within),
            (STATE_DRAG_IN, &interaction.hovered_within),
        ]
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

    #[inline]
    pub fn get_transform_and_origin(visual: &VisualProperty) -> ([[f32; 4]; 3], [f32; 2]) {
        let origin = visual
            .transform_origin
            .map(|p| [p.x, p.y])
            .unwrap_or([0.5, 0.5]);

        let full_transform = visual.transform.unwrap_or(IDENTITY_MATRIX);
        let packed_transform = [
            full_transform[0], // X軸基底
            full_transform[1], // Y軸基底
            full_transform[3], // 平行移動部
        ];
        (packed_transform, origin)
    }

    #[inline]
    pub(crate) fn get_outline_params(
        visual: &VisualProperty,
    ) -> (EdgeInsets, Color, EdgeInsets, [f32; 4]) {
        let o_width = visual.outline_width.unwrap_or(EdgeInsets::ZERO);
        let o_color = visual.outline_color.unwrap_or(Color::TRANSPARENT);
        let o_lengths = visual.outline_lengths.unwrap_or(EdgeInsets::px_all(1.0));
        let o_offset = visual.outline_offset.unwrap_or(0.0);
        let o_styles = visual.outline_styles.unwrap_or([BorderStyle::Solid; 4]);
        let o_aligns = visual
            .outline_alignments
            .unwrap_or([BorderAlignment::Start; 4]);

        let mut o_flags = 0u32;
        for idx in 0..4 {
            o_flags |= (o_styles[idx] as u32) << (idx * 4);
            o_flags |= (o_aligns[idx] as u32) << (idx * 4 + 2);
        }
        let outline_offset_and_flags = [o_offset, o_flags as f32, 0.0, 0.0];

        (o_width, o_color, o_lengths, outline_offset_and_flags)
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

impl TargetStyle {
    /// 指定された VisualProperty と ComponentMask を基に自身のスタイルをマージ。
    pub(crate) fn apply_visual_property(
        target: &mut TargetStyle,
        inner_vis: &VisualProperty,
        inner_mask: ComponentMask,
    ) {
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

impl Context {
    #[inline]
    pub(crate) fn get_basic_layout_mut(
        &mut self,
        id: EntityId,
        target: StyleTarget,
    ) -> Option<&mut BasicLayout> {
        RenderStore::get_basic_layout_mut(id, &mut self.renders, target)
    }

    #[inline]
    pub(crate) fn get_visual_property_mut(
        &mut self,
        id: EntityId,
        target: StyleTarget,
    ) -> Option<&mut VisualProperty> {
        RenderStore::get_visual_property_mut(id, &mut self.renders, target)
    }

    #[inline]
    pub(crate) fn get_flex_layout_mut(
        &mut self,
        id: EntityId,
        target: StyleTarget,
    ) -> Option<&mut FlexLayout> {
        RenderStore::get_flex_layout_mut(
            id,
            &mut self.renders,
            target,
            &mut self.layouts.flex_layouts,
        )
    }

    #[inline]
    pub fn get_transform_and_origin(&self, visual: &VisualProperty) -> ([[f32; 4]; 3], [f32; 2]) {
        RenderStore::get_transform_and_origin(visual)
    }

    #[inline]
    pub(crate) fn get_outline_params(
        &self,
        visual: &VisualProperty,
    ) -> (EdgeInsets, Color, EdgeInsets, [f32; 4]) {
        RenderStore::get_outline_params(visual)
    }

    #[inline]
    pub(crate) fn trigger_keyframe_animations_if_needed(&mut self, id: EntityId) {
        RenderStore::trigger_keyframe_animations_if_needed(id, &mut self.renders);
    }

    /// 指定された動的状態（例: STATE_HOVERED）に切り替わる際、
    /// その要素に割り当てられている状態スタイルがレイアウトの再計算を必要とするか判定します。
    #[inline]
    pub(crate) fn does_state_require_layout(&self, id: EntityId, state_flag: u128) -> bool {
        RenderStore::does_state_require_layout(id, &self.renders, state_flag)
    }

    #[inline]
    pub(crate) fn resolv_focus_style(
        &self,
        id: EntityId,
        active_mask: &ComponentMask,
    ) -> Option<ThisStyle> {
        RenderStore::resolv_focus_style(id, &self.renders, active_mask, &self.topology.parents)
    }

    #[inline]
    pub(crate) fn cascade_interaction(
        &self,
        id: EntityId,
        active_mask: ComponentMask,
        target: &mut TargetStyle,
        focus_style_resolved: Option<ThisStyle>,
    ) {
        if let Some(interaction) = self.renders.interaction_properties.get(id) {
            let cascade = RenderStore::cascade_interaction_flag(
                id,
                &self.renders,
                interaction,
                &focus_style_resolved,
            );

            for (state, style_opt) in cascade {
                if active_mask.has(state)
                    && let Some(style) = style_opt
                {
                    TargetStyle::apply_visual_property(
                        target,
                        &style.inner.visual_property,
                        style.inner.mask,
                    );
                }
            }
        }
    }

    #[inline]
    pub(crate) fn cascade_within_interaction(
        &self,
        id: EntityId,
        active_mask: ComponentMask,
        target: &mut TargetStyle,
    ) {
        if active_mask.has(STYLE_INTERACTION_WITHIN)
            && let Some(interaction) = self.renders.interaction_properties.get(id)
        {
            // 自身の mask にビットが立っている場合のみツリー再帰を走らせてマージ解決
            let cascade_within = RenderStore::cascade_within_interaction_flag(id, interaction);

            for (state, style_opt) in cascade_within {
                // 子孫要素のいずれかがこの state_flag を満たしているか
                if TopologyStore::has_descendant_with_state(id, &self.topology, state)
                    && let Some(style) = style_opt
                {
                    TargetStyle::apply_visual_property(
                        target,
                        &style.inner.visual_property,
                        style.inner.mask,
                    );
                }
            }

            // All（いずれかのインタラクションがあればON）の解決
            if let Some(ref style) = interaction.any_within
                && TopologyStore::has_descendant_with_any_active_state(id, &self.topology)
            {
                TargetStyle::apply_visual_property(
                    target,
                    &style.inner.visual_property,
                    style.inner.mask,
                );
            }
        }
    }

    #[inline]
    pub(crate) fn cascade_basic_layout(
        &self,
        id: EntityId,
        active_mask: ComponentMask,
        target_layout: &mut BasicLayout,
    ) {
        RenderStore::cascade_basic_layout(id, &self.renders, active_mask, target_layout);
    }

    /// 現在の描画用データを取得 (Copy可能なプリミティブのみ)
    #[inline]
    pub(crate) fn get_current_style(&self, id: EntityId) -> CurrentStyle {
        RenderStore::get_current_style(id, &self.renders)
    }

    /// 目標値を参照経由で構築
    #[inline]
    pub(crate) fn get_target_style(&self, id: EntityId) -> TargetStyle {
        RenderStore::get_target_style(id, &self.renders)
    }

    /// 現在ホバーされている要素から親ツリーを遡り、適用するべき物理的な CursorIcon を正確に解決します。
    #[inline]
    pub fn resolve_cursor(&self, hovered_id: EntityId) -> CursorIcon {
        RenderStore::resolve_cursor(hovered_id, &self.events, &self.renders, &self.topology)
    }

    /// 描画（レンダー）ダーティ状態として登録された要素をすべてクリアします。
    #[inline]
    pub fn clear_render_dirty(&mut self) {
        RenderStore::clear_render_dirty(&mut self.renders, &mut self.topology);
    }

    /// 現在、アクティブに動いているトランジション（wgpuアニメーション）があるか判定します
    pub fn has_active_animations(&self) -> bool {
        // ドラッグ選択中でポインタが可視境界外にある場合も継続
        let has_drag_autoscroll =
            OutputStore::is_drag_autoscroll_active(&self.events, &self.outputs, &self.renders);

        RenderStore::has_active_animations(
            &self.renders,
            &self.events,
            &self.layouts,
            &self.contents,
            has_drag_autoscroll,
        )
    }

    /// 対象の要素がキーボードフォーカス可能であるかを総合検証します
    pub(crate) fn is_keyboard_focusable(&self, id: EntityId) -> bool {
        // 生存確認、および無効化（Disabled）状態でないか検証
        if !self.topology.entities.contains_key(id) || self.is_disabled(id) {
            return false;
        }

        // 暗黙的または明示的にキーボードフォーカスを要求しているか
        let is_target = self.topology.active_masks[id].has(COMP_INPUT_CONTENT)
            || self.topology.active_masks[id].has(COMP_WEBVIEW_CONTENT)
            || (self.topology.active_masks[id].has(STYLE_FOCUSABLE)
                && self
                    .renders
                    .visual_properties
                    .get(id)
                    .and_then(|v| v.focusable)
                    .map(|f| match f {
                        Focusable::SelfStyle(trigger) | Focusable::Inherit(trigger) => {
                            trigger == FocusTrigger::Keyboard || trigger == FocusTrigger::Both
                        }
                        Focusable::None => false,
                    })
                    .unwrap_or(false));

        if !is_target {
            return false;
        }

        // 自分自身、および親先祖ツリーに非表示（Display::None）が1つも含まれていないか検証
        let mut curr = Some(id);
        while let Some(curr_id) = curr {
            if let Some(layout) = self.layouts.basic_layouts.get(curr_id)
                && layout.display == Display::None
            {
                return false;
            }
            curr = self.topology.parents.get(curr_id).copied().flatten();
        }

        true
    }

    /// 補間されたアニメーション値を SoA のアクティブプロパティへ安全に上書きします
    pub(crate) fn apply_animation_value(
        &mut self,
        id: EntityId,
        property: PropertyList,
        value: &TransitionValue,
    ) {
        if !self.renders.visual_properties.contains_key(id) {
            self.renders
                .visual_properties
                .insert(id, Default::default());
        }
        let v = self.renders.visual_properties.get_mut(id).unwrap();

        match *value {
            TransitionValue::Color(c) => {
                if property == PropertyList::BackgroundColor {
                    v.bg_color = Some(c);
                } else if property == PropertyList::BorderColor {
                    v.border_color = Some(c);
                }
            }
            TransitionValue::Opacity(o) => {
                v.opacity = Some(o);
            }
            TransitionValue::Transform(m) => {
                v.transform = Some(m);
            }
            TransitionValue::CornerRadius(cr) => {
                v.corner_radius = Some(cr);
            }
            TransitionValue::Width(w) => {
                if let Some(layout) = self.layouts.basic_layouts.get_mut(id) {
                    layout.size.width = Val::Px(w);
                }
                self.mark_layout_dirty(id); // レイアウト再計算を要求（スローパス）
            }
            TransitionValue::Height(h) => {
                if let Some(layout) = self.layouts.basic_layouts.get_mut(id) {
                    layout.size.height = Val::Px(h);
                }
                self.mark_layout_dirty(id);
            }
            TransitionValue::BoxShadow(shadow) => {
                v.shadow_params = Some(shadow);
                v.shadow_color = Some(shadow.color);
            }
        }
    }

    /// 毎フレームの描画前に呼び出され、すべてのアクティブなキーフレームアニメーションを 1 Tick 進めます
    pub fn tick_animations(&mut self) {
        let now = Instant::now();

        // 借用チェッカーを回避するため、一時的にマップを take して更新
        let mut active_map = std::mem::take(&mut self.renders.active_animations);
        let mut to_remove = Vec::new();

        for (id, animations) in active_map.iter_mut() {
            let mut i = 0;
            while i < animations.len() {
                let anim = &mut animations[i];
                let elapsed = now.duration_since(anim.start_time);
                let elapsed_secs = elapsed.as_secs_f32();
                let duration_secs = anim.duration.as_secs_f32();

                // 1. 現在の周回回数（ループインデックス）の算出
                let current_iteration = (elapsed_secs / duration_secs).floor() as u32;

                // ループ制限に達しているかチェック
                let is_finished = match anim.iteration_count {
                    PlaybackCount::Count(max_count) => current_iteration >= max_count,
                    PlaybackCount::Infinite => false,
                };

                if is_finished {
                    // ループ終了：目標の最終値（end_value）で固定してアニメーションを破棄
                    self.apply_animation_value(id, anim.property, &anim.end_value);
                    animations.remove(i);
                    continue;
                }

                // 2. 現在のループ内での正規化進行度 (0.0 ～ 1.0) の計算
                let local_time = elapsed_secs % duration_secs;
                let progress = if duration_secs > 0.0 {
                    (local_time / duration_secs).min(1.0)
                } else {
                    1.0
                };
                let eased_t = anim.curve.evaluate(progress);

                // 3. 値の補間
                let current_val = anim.start_value.lerp(&anim.end_value, eased_t);

                // 4. SoA へ補間された動的スタイル値を上書き書き戻し
                self.apply_animation_value(id, anim.property, &current_val);

                // レンダラーへ再描画要求（ファストパス）
                self.mark_render_dirty(id);

                i += 1;
            }

            if animations.is_empty() {
                to_remove.push(id);
            }
        }

        // 空になったエントリをクリーンアップ
        for id in to_remove {
            active_map.remove(id);
        }
        self.renders.active_animations = active_map;
    }

    /// 状態の変更を検知し、アニメーション（トランジション）が必要な箇所を自動的に開始・制御します。
    pub(crate) fn resolve_element_style_state(&mut self, id: EntityId, allow_transition: bool) {
        let active_mask = self.topology.active_masks[id];

        // ビジュアルプロパティ (bg_color, opacity等) の解決
        let has_base_visual = self.renders.base_visual_properties.contains_key(id);
        let has_active_visual = self.renders.visual_properties.contains_key(id);

        // 要素がホバーやプレス時の動的スタイルを登録しているか
        let has_interaction_styles = self.renders.interaction_properties.contains_key(id);

        // スタイルを一切持たない要素は、ヒープアロケーションを避けるため完全にスキップ
        // 静的なベース装飾がなくても、ホバースタイル等を持っていれば確実にカスケード解決を通す
        if has_base_visual || has_active_visual || has_interaction_styles {
            let current = self.get_current_style(id);
            let mut target = self.get_target_style(id);

            // 自身のフォーカススタイルが無い場合、親先祖要素が自身のために定義している focused スタイルを抽出
            let focus_style_resolved = self.resolv_focus_style(id, &active_mask);

            // 疑似クラス（Hovered等）のマージ
            self.cascade_interaction(id, active_mask, &mut target, focus_style_resolved);
            self.cascade_within_interaction(id, active_mask, &mut target);

            // プレースホルダー表示状態
            let mut is_placeholder_active = false;
            if let Some(contents) = self.contents.input_contents.get(id) {
                // 文字列が空、かつ IME 変換中でない場合はプレースホルダーと判定
                let has_no_ime = contents
                    .ime_state
                    .as_ref()
                    .map(|s| s.composition_text.is_empty())
                    .unwrap_or(true);
                if contents.text.0.get().is_empty() && has_no_ime {
                    is_placeholder_active = true;
                }
            }

            // 各プロパティの即時適用の変更を評価
            let target_bg_val = target.bg_color.unwrap_or(Color::TRANSPARENT);
            let bg_changed = current.bg_color != target_bg_val;

            let target_border_val = target.border_color.unwrap_or(Color::TRANSPARENT);
            let border_changed = current.border_color != target_border_val;

            let target_outline_width_val = target.outline_width.unwrap_or(EdgeInsets::ZERO);
            let outline_width_changed = current.outline_width != target_outline_width_val;

            let target_outline_color_val = target.outline_color.unwrap_or(Color::TRANSPARENT);
            let outline_color_changed = current.outline_color != target_outline_color_val;

            let target_outline_offset_val = target.outline_offset.unwrap_or(0.0);
            let outline_offset_changed =
                (current.outline_offset - target_outline_offset_val).abs() > 0.001;

            let target_opacity_val = target.opacity.unwrap_or(1.0);
            let opacity_changed = (current.opacity - target_opacity_val).abs() > 0.001;

            let target_transform_val = target.transform.unwrap_or(IDENTITY_MATRIX);
            let transform_changed = current.transform != target_transform_val;

            let target_radius_val = target.corner_radius.unwrap_or(CornerRadius::ZERO);
            let radius_changed = current.corner_radius != target_radius_val;

            let target_shadow_val = target.shadow_params.unwrap_or(BoxShadow::none());
            let shadow_changed = current.shadow_params != target_shadow_val;

            // トランジション判定 (変更がある場合のみトリガー)
            let mut bg_triggered = false;
            if allow_transition && bg_changed && has_active_visual {
                bg_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::BackgroundColor,
                    TransitionValue::Color(current.bg_color),
                    TransitionValue::Color(target_bg_val),
                );
            }

            let mut border_triggered = false;
            if border_changed && has_active_visual {
                border_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::BorderColor,
                    TransitionValue::Color(current.border_color),
                    TransitionValue::Color(target_border_val),
                );
            }

            let mut opacity_triggered = false;
            if opacity_changed && has_active_visual {
                opacity_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::Opacity,
                    TransitionValue::Opacity(current.opacity),
                    TransitionValue::Opacity(target_opacity_val),
                );
            }

            let mut transform_triggered = false;
            if transform_changed {
                transform_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::Transform,
                    TransitionValue::Transform(current.transform),
                    TransitionValue::Transform(target_transform_val),
                );
            }

            let mut radius_triggered = false;
            if radius_changed && has_active_visual {
                radius_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::CornerRadius,
                    TransitionValue::CornerRadius(current.corner_radius),
                    TransitionValue::CornerRadius(target_radius_val),
                );
            }

            let mut shadow_triggered = false;
            if shadow_changed && has_active_visual {
                shadow_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::BoxShadow,
                    TransitionValue::BoxShadow(current.shadow_params),
                    TransitionValue::BoxShadow(target_shadow_val),
                );
            }

            // アニメーションが起動した、または明示的にベースの描画プロパティがある場合のみ
            // 遅延評価（Lazy）でマップを確保し、書き込みを行う
            if bg_triggered
                || border_triggered
                || opacity_triggered
                || transform_triggered
                || radius_triggered
                || shadow_triggered
                || bg_changed
                || border_changed
                || opacity_changed
                || transform_changed
                || radius_changed
                || shadow_changed
                || outline_width_changed
                || outline_color_changed
                || outline_offset_changed
                || self.renders.base_visual_properties.contains_key(id)
            {
                if !self.renders.visual_properties.contains_key(id) {
                    self.renders
                        .visual_properties
                        .insert(id, Default::default());
                }
                let active_vis = self.renders.visual_properties.get_mut(id).unwrap();

                if !bg_triggered {
                    active_vis.bg_color = target.bg_color;
                }
                if !border_triggered {
                    active_vis.border_color = target.border_color;
                }
                if !opacity_triggered {
                    active_vis.opacity = target.opacity;
                }
                if !transform_triggered {
                    active_vis.transform = target.transform;
                }
                if !radius_triggered {
                    active_vis.corner_radius = target.corner_radius;
                }
                if is_placeholder_active {
                    // プレースホルダー時はフォーカスに関わらず、強制的に半透明の薄いグレー
                    active_vis.text_color = Some(Color::rgb_f32(0.5, 0.5, 0.5));
                } else {
                    active_vis.text_color = target.text_color; // 通常時、または疑似状態（Hover等）のテキストカラー
                }

                // 解決した影（target_shadow）をアクティブプロパティに代入
                // アニメーション非起動時のみ行うように修正
                if !shadow_triggered {
                    active_vis.shadow_params = target.shadow_params;
                    active_vis.shadow_color = target.shadow_color;
                }

                active_vis.border_lengths = target.border_lengths;
                active_vis.border_styles = target.border_styles;
                active_vis.border_alignments = target.border_alignments;

                active_vis.outline_width = target.outline_width;
                active_vis.outline_color = target.outline_color;
                active_vis.outline_lengths = target.outline_lengths;
                active_vis.outline_styles = target.outline_styles;
                active_vis.outline_alignments = target.outline_alignments;
                active_vis.outline_offset = target.outline_offset;

                // 解決された選択色をアクティブビジュアルに代入
                active_vis.select_bg_color = target.select_bg_color;
                active_vis.select_text_color = target.select_text_color;
                // 常に即時解決する静的プロパティ群
                active_vis.user_select = self
                    .renders
                    .base_visual_properties
                    .get(id)
                    .and_then(|v| v.user_select);

                active_vis.cursor = target.cursor;
                active_vis.resizable_cursor = target.resizable_cursor;

                // コールドプロパティの即時代入
                if let Some(target_vis) = self.renders.base_visual_properties.get(id) {
                    active_vis.z_index = target_vis.z_index;
                    active_vis.backdrop = target_vis.backdrop;
                    active_vis.font_size = target_vis.font_size;
                    active_vis.font_family = target_vis.font_family.clone();
                    active_vis.font_weight = target_vis.font_weight;
                    active_vis.font_style = target_vis.font_style;
                    active_vis.bg_gradient = target_vis.bg_gradient;
                    active_vis.pointer_events = target_vis.pointer_events;
                    active_vis.transitions = target_vis.transitions.clone();
                    active_vis.keyframe_animations = target_vis.keyframe_animations.clone();
                    active_vis.focusable = target_vis.focusable;
                }

                // 即時変更があったため、レンダラーへの転送 Dirty をマーク
                self.mark_render_dirty(id);
            }
        }

        //  (Width, Height) の解決
        let has_base_layout = self.renders.base_basic_layouts.contains_key(id);
        let has_active_layout = self.layouts.basic_layouts.contains_key(id);

        // レイアウト変更のない要素は完全にスキップ
        if has_base_layout || has_active_layout {
            let active_layout = self
                .layouts
                .basic_layouts
                .get(id)
                .cloned()
                .unwrap_or_default();

            // BasicLayout は heap allocation を持たないフラットな構造（Copy同等）なので
            // cloned() によるクローンは極めて低コスト
            let base_layout = self
                .renders
                .base_basic_layouts
                .get(id)
                .cloned()
                .unwrap_or_default();
            let mut target_layout = base_layout;

            self.cascade_basic_layout(id, active_mask, &mut target_layout);

            // 単位を親/ウィンドウアラインメントを考慮した物理ピクセル(f32)へ解決
            let target_w_px = self.resolve_val_to_px(id, target_layout.size.width, true);
            let current_w_px = self.resolve_val_to_px(id, active_layout.size.width, true);
            let target_h_px = self.resolve_val_to_px(id, target_layout.size.height, false);
            let current_h_px = self.resolve_val_to_px(id, active_layout.size.height, false);

            let mut width_triggered = false;
            let mut height_triggered = false;

            // Width または 一括 Size トランジション設定が定義されているか検証
            let can_trigger_width = self
                .renders
                .visual_properties
                .get(id)
                .map(|v| {
                    v.transitions.iter().any(|t| {
                        t.property_list == PropertyList::Width
                            || t.property_list == PropertyList::Size
                    })
                })
                .unwrap_or(false);

            if allow_transition
                && can_trigger_width
                && has_active_layout
                && let (Some(cw), Some(tw)) = (current_w_px, target_w_px)
                && (cw - tw).abs() > 0.01
            // 浮動小数点誤差を無視
            {
                width_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::Width,
                    TransitionValue::Width(cw),
                    TransitionValue::Width(tw),
                );
            }

            // Height または 一括 Size トランジション設定が定義されているか検証
            let can_trigger_height = self
                .renders
                .visual_properties
                .get(id)
                .map(|v| {
                    v.transitions.iter().any(|t| {
                        t.property_list == PropertyList::Height
                            || t.property_list == PropertyList::Size
                    })
                })
                .unwrap_or(false);

            if allow_transition
                && can_trigger_height
                && has_active_layout
                && let (Some(ch), Some(th)) = (current_h_px, target_h_px)
                && (ch - th).abs() > 0.01
            {
                height_triggered = self.trigger_transition_if_needed(
                    id,
                    PropertyList::Height,
                    TransitionValue::Height(ch),
                    TransitionValue::Height(th),
                );
            }

            // 遅延マウント
            if !self.layouts.basic_layouts.contains_key(id) {
                self.layouts.basic_layouts.insert(id, Default::default());
            }
            let active_layout_mut = self.layouts.basic_layouts.get_mut(id).unwrap();
            *active_layout_mut = target_layout;

            if width_triggered {
                active_layout_mut.size.width = Val::Px(current_w_px.unwrap());
            }
            if height_triggered {
                active_layout_mut.size.height = Val::Px(current_h_px.unwrap());
            }

            // 最終的に解決されたレイアウトを Taffy ツリーに即時同期させるため、
            // スタイル解決の末尾でレイアウトの Dirty マークを叩きます
            self.mark_layout_dirty(id);
        }

        // スタイル解決が完了した結果、自身に新しくキーフレームアニメーション定義が
        // 読み込まれていれば、自動的にそのアニメーションの再生を開始する
        self.trigger_keyframe_animations_if_needed(id);

        if let Some(effects) = self.reactive.element_effects.get(id) {
            let text_effects: Vec<EffectId> = effects
                .iter()
                .filter(|(cat, _)| *cat == EffectCategory::Text)
                .map(|(_, eff_id)| *eff_id)
                .collect();
            for eff_id in text_effects {
                crate::execute_effect(eff_id);
            }
        }
    }

    /// 毎フレームの描画前に呼び出され、すべてのアクティブなトランジションを 1 Tick 進めます
    pub fn tick_transitions(&mut self) {
        let now = Instant::now();

        // (1.0 / 120.0 秒 = 約 8,333,333 ナノ秒)
        const FRAME_TIME_120FPS: Duration = Duration::from_nanos(8_333_333);
        if let Some(last) = self.renders.last_tick_time
            && now.duration_since(last) < FRAME_TIME_120FPS
        {
            return;
        }

        // 実行制限を通過したため、基準時刻を更新して処理を継続
        self.renders.last_tick_time = Some(now);

        // 借用チェッカーを回避するため、一時的にマップを take して更新する
        let mut active_map = std::mem::take(&mut self.renders.active_transitions);

        // 完了して空になった要素のIDを記録する一時配列
        let mut to_remove = Vec::new();

        for (id, transitions) in active_map.iter_mut() {
            let mut i = 0;
            while i < transitions.len() {
                let t_state = &mut transitions[i];

                // start_time が None なら、このフレームの時刻 now を格納しその値を取り出す。
                let start_time = *t_state.start_time.get_or_insert(now);
                let elapsed = now.duration_since(start_time);

                // 進行度 (0.0 ～ 1.0)
                let progress = (elapsed.as_secs_f32() / t_state.duration.as_secs_f32()).min(1.0);
                let eased_t = t_state.curve.evaluate(progress);

                // Lerpによる新しい値の決定
                let current_val = t_state.start_value.lerp(&t_state.end_value, eased_t);

                // SoA（Context のアクティブなプロパティ）に補間された値を書き戻す
                match current_val {
                    TransitionValue::Color(c) => {
                        if let Some(v) = self.renders.visual_properties.get_mut(id) {
                            if t_state.property_list == PropertyList::BackgroundColor {
                                v.bg_color = Some(c);
                            } else if t_state.property_list == PropertyList::BorderColor {
                                v.border_color = Some(c);
                            }
                        }
                        self.mark_render_dirty(id);
                    }
                    TransitionValue::Opacity(o) => {
                        if let Some(v) = self.renders.visual_properties.get_mut(id) {
                            v.opacity = Some(o);
                        }
                        self.mark_render_dirty(id);
                    }
                    TransitionValue::Transform(m) => {
                        if let Some(v) = self.renders.visual_properties.get_mut(id) {
                            v.transform = Some(m);
                        }
                        self.mark_render_dirty(id);
                    }
                    TransitionValue::CornerRadius(cr) => {
                        if let Some(v) = self.renders.visual_properties.get_mut(id) {
                            v.corner_radius = Some(cr);
                        }
                        self.mark_render_dirty(id);
                    }
                    TransitionValue::Width(w) => {
                        if let Some(layout) = self.layouts.basic_layouts.get_mut(id) {
                            layout.size.width = Val::Px(w); // ピクセル値で上書き
                        }
                        self.mark_layout_dirty(id); // レイアウト再計算をマーク

                        // キャッシュを毎フレーム強制バイパスさせるためにマスクを再セット
                        if let Some(mask) = self.topology.active_masks.get_mut(id) {
                            mask.set(STATE_QUEUED_LAYOUT);
                        }
                    }
                    // 縦幅（Height）の毎フレームアニメーション補間
                    TransitionValue::Height(h) => {
                        if let Some(layout) = self.layouts.basic_layouts.get_mut(id) {
                            layout.size.height = Val::Px(h);
                        }
                        self.mark_layout_dirty(id);

                        if let Some(mask) = self.topology.active_masks.get_mut(id) {
                            mask.set(STATE_QUEUED_LAYOUT);
                        }
                    }
                    // 影（BoxShadow）の毎フレームの書き戻し処理
                    TransitionValue::BoxShadow(shadow) => {
                        if let Some(v) = self.renders.visual_properties.get_mut(id) {
                            v.shadow_params = Some(shadow);
                            v.shadow_color = Some(shadow.color);
                        }
                        self.mark_render_dirty(id);
                    }
                }

                // アニメーション完了判定
                if progress >= 1.0 {
                    transitions.remove(i);
                } else {
                    i += 1;
                }

                // トランジションが空になった要素をマーク
                if transitions.is_empty() {
                    to_remove.push(id);
                }
            }
        }

        // 空になったエントリをマップから完全削除（クリーンアップ）
        for id in to_remove {
            active_map.remove(id);
        }

        self.renders.active_transitions = active_map;
    }

    /// 必要に応じてトランジションを起動、または上書き（逆再生含む）します
    pub(crate) fn trigger_transition_if_needed(
        &mut self,
        id: EntityId,
        property_list: PropertyList,
        start_value: TransitionValue,
        end_value: TransitionValue,
    ) -> bool {
        // スタイルの再評価エフェクトの実行中であるか
        let is_style_evaluating = crate::signal::ACTIVE_EFFECT.with(|cell| {
            if let Some(effect_id) = cell.get() {
                // 現在走っているエフェクトがいずれかの要素の StyleCategory::Style のものであるか走査
                self.reactive.element_effects.values().any(|list| {
                    list.iter()
                        .any(|(cat, eff_id)| *eff_id == effect_id && *cat == EffectCategory::Style)
                })
            } else {
                false
            }
        });

        // スタイルエフェクト評価中であればトランジションの開始を完全拒否して値の即時書き換え
        if is_style_evaluating {
            return false;
        }

        // 1. その要素に、このプロパティに対するトランジション設定が定義されているか検証
        if let Some(visual) = self.renders.base_visual_properties.get(id) {
            // transitions ベクタの中から、一致する PropertyList を探す
            if let Some(t) = visual
                .transitions
                .iter()
                .find(|t| t.property_list == property_list || t.property_list == PropertyList::Size)
            {
                let now = Instant::now();
                if let Some(entry) = self.renders.active_transitions.entry(id) {
                    let active_list = entry.or_insert_with(Vec::new);

                    // 2. 割り込み処理の解決（すでに同じプロパティのアニメーションが走っているか）
                    let actual_start = if let Some(existing) = active_list
                        .iter_mut()
                        .find(|et| et.property_list == property_list)
                    {
                        // すでに同じ目的地に向かってアニメーション中の場合は、
                        // 割り込みを一切行わず、そのまま既存アニメーションを走らせる
                        if existing.end_value == end_value {
                            return true;
                        }

                        // すでに駆動中の場合は、その現在の補間位置をリアルタイム計算する
                        // ※ start_time が None の場合（登録されたが一度も tick されていない場合）は
                        // 経過時間 0 として進捗 progress を 0.0 にする
                        let elapsed = existing
                            .start_time
                            .map(|st| now.duration_since(st))
                            .unwrap_or(Duration::ZERO);
                        let progress =
                            (elapsed.as_secs_f32() / existing.duration.as_secs_f32()).min(1.0);
                        let eased_t = existing.curve.evaluate(progress);

                        // 中間位置の算出（これが新しいアニメーションの開始点になる）
                        let current_interposed_val =
                            existing.start_value.lerp(&existing.end_value, eased_t);

                        // 既存のアニメーション状態をリセットし、現在地点から新しい目標値（end_value）へ向かうように上書き
                        existing.start_time = None;
                        existing.start_value = current_interposed_val;
                        existing.end_value = end_value;
                        existing.duration = t.duration;
                        existing.curve = t.curve;

                        return true; // 既存のアニメーションを上書き更新したため即時復帰
                    } else {
                        // 新規開始の場合は、渡された現在の開始値をそのまま採用
                        start_value
                    };

                    // 3. 新規トランジションをアクティブリストに登録
                    active_list.push(ActiveTransition {
                        property_list,
                        start_time: None,
                        duration: t.duration,
                        curve: t.curve,
                        start_value: actual_start,
                        end_value,
                    });

                    return true; // トランジションを正常に起動
                }
            }
        }
        false // トランジション設定がなかったため、即時適用パスへ
    }
}
