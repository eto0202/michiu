use crate::*;
use slotmap::{SecondaryMap, SparseSecondaryMap};
use std::{
    borrow::Cow,
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

pub(crate) type VisualPropertiesSecondary = SecondaryMap<EntityId, VisualProperty>;
pub(crate) type InteractionPropertiesSecondary = SecondaryMap<EntityId, InteractionStyles>;
pub(crate) type BaseVisualPropertiesSecondary = SecondaryMap<EntityId, VisualProperty>;
pub(crate) type DirtyRenderEntitiesVec = Vec<EntityId>;
pub(crate) type ActiveTransitionsSparseSecondary =
    SparseSecondaryMap<EntityId, Vec<ActiveTransition>>;
pub(crate) type ActiveAnimationsSparseSecondary =
    SparseSecondaryMap<EntityId, Vec<ActiveAnimation>>;
pub(crate) type ActiveWebviewsHashSet = HashSet<EntityId>;
pub(crate) type LastTickTimeOption = Option<Instant>;

pub struct RenderStore {
    pub(crate) visual_properties: VisualPropertiesSecondary,
    pub(crate) interaction_properties: InteractionPropertiesSecondary,
    pub(crate) base_visual_properties: BaseVisualPropertiesSecondary,
    pub(crate) dirty_render_entities: DirtyRenderEntitiesVec,
    pub(crate) active_transitions: ActiveTransitionsSparseSecondary,
    pub(crate) active_animations: ActiveAnimationsSparseSecondary,
    pub(crate) active_webviews: ActiveWebviewsHashSet,
    pub(crate) last_tick_time: LastTickTimeOption,
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
        self.base_visual_properties.remove(id);
        self.dirty_render_entities.retain(|&x| x != id);
        self.active_transitions.remove(id);
        self.active_animations.remove(id);
        self.active_webviews.remove(&id);
    }
}

impl RenderStore {
    #[inline]
    pub(crate) fn mark_render_dirty(
        id: EntityId,
        active_masks: &mut ActiveMasksSecondary,
        dirty_render_entities: &mut DirtyRenderEntitiesVec,
    ) {
        let Some(mask) = active_masks.get_mut(id) else {
            return;
        };
        if mask.has(STATE_QUEUED_RENDER) {
            return;
        }

        mask.set(STATE_QUEUED_RENDER);
        dirty_render_entities.push(id);
    }

    /// 描画（レンダー）ダーティ状態として登録された要素をすべてクリアします。
    pub(crate) fn clear_render_dirty(
        dirty_render_entities: &mut DirtyRenderEntitiesVec,
        active_masks: &mut ActiveMasksSecondary,
    ) {
        for id in dirty_render_entities.drain(..) {
            let Some(mask) = active_masks.get_mut(id) else {
                continue;
            };
            mask.unset(STATE_QUEUED_RENDER);
        }
        dirty_render_entities.clear();
    }

    pub(crate) fn get_visual_property_mut<'a>(
        id: EntityId,
        target: StyleTarget,
        base_visual_properties: &'a mut BaseVisualPropertiesSecondary,
        interaction_properties: &'a mut InteractionPropertiesSecondary,
    ) -> Option<&'a mut VisualProperty> {
        match target {
            StyleTarget::Base => base_visual_properties.get_mut(id),
            _ => {
                if !interaction_properties.contains_key(id) {
                    interaction_properties.insert(id, InteractionStyles::default());
                }
                let styles = interaction_properties.get_mut(id).unwrap();
                let style_ref = styles.get_style_target_mut(target);
                Some(&mut Arc::make_mut(&mut style_ref.inner).visual_property)
            }
        }
    }

    /// スクロールバー用要素の不透明度（解決値と静的ベース値）を同時同期して更新します。
    #[inline]
    pub(crate) fn update_scrollbar_element_opacity(
        id: EntityId,
        visual_properties: &mut VisualPropertiesSecondary,
        base_visual_properties: &mut BaseVisualPropertiesSecondary,
        opacity: f32,
    ) {
        let visuals = [
            visual_properties.get_mut(id),
            base_visual_properties.get_mut(id),
        ];

        for vis in visuals.into_iter().flatten() {
            vis.opacity = Some(opacity);
        }
    }

    pub(crate) fn trigger_keyframe_animations_if_needed(
        id: EntityId,
        visual_properties: &VisualPropertiesSecondary,
        active_animations: &mut ActiveAnimationsSparseSecondary,
    ) {
        let Some(visual) = visual_properties.get(id) else {
            return;
        };
        if visual.keyframe_animations.is_empty() {
            return;
        }

        let now = Instant::now();

        if !active_animations.contains_key(id) {
            active_animations.insert(id, Vec::new());
        }
        let active_list = active_animations.get_mut(id).unwrap();

        for anim in &visual.keyframe_animations {
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

    /// 現在、アクティブに動いているトランジションがあるか判定します
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn has_active_animations(
        interaction_states: &InteractionStates,
        current_pointer_position: &Option<LayoutPoint>,
        clip_rects: &ClipRectsSecondary,
        visual_properties: &VisualPropertiesSecondary,
        active_transitions: &ActiveTransitionsSparseSecondary,
        active_animations: &ActiveAnimationsSparseSecondary,
        input_contents: &InputContentsSparseSecondary,
        scrollbar_styles: &ScrollbarStylesSecondary,
    ) -> bool {
        // ドラッグ選択中でポインタが可視境界外にある場合も継続
        let has_drag_autoscroll = OutputStore::is_drag_autoscroll_active(
            interaction_states,
            current_pointer_position,
            clip_rects,
            visual_properties,
        );

        // トランジションのアクティブ判定
        let has_transitions = !active_transitions.is_empty()
            && active_transitions.values().any(|list| !list.is_empty());

        // キーフレームアニメーションのアクティブ判定
        let has_keyframes = !active_animations.is_empty()
            && active_animations.values().any(|list| !list.is_empty());

        // フォーカスされたインプットがあり、キャレット点滅が有効な間は描画ループを駆動
        let has_blinking_input = interaction_states
            .focused
            .and_then(|id| input_contents.get(id))
            .map(|c| c.has_caret && c.is_blink)
            .unwrap_or(false);

        // 一時的表示スクロールバーのフェード進行中は描画更新ループを継続
        let has_active_transient_scrollbar = scrollbar_styles.values().any(|sb_state| {
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

    /// 指定された動的状態に切り替わる際、
    /// その要素に割り当てられている状態スタイルがレイアウトの再計算を必要とするか判定。
    pub(crate) fn does_state_require_layout(
        id: EntityId,
        interaction_properties: &InteractionPropertiesSecondary,
        state_flag: u128,
    ) -> bool {
        let Some(interaction) = interaction_properties.get(id) else {
            return false;
        };

        // 対象となる状態スタイルを取得
        let target_style = match state_flag {
            STATE_HOVERED => &interaction.hovered,
            STATE_FOCUSED => &interaction.focused,
            STATE_FOCUSED_VISIBLE => &interaction.focused_visible,
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
        let Some(style) = target_style else {
            return false;
        };

        let mask = style.inner.mask;
        // 基本レイアウト、Flexレイアウト、またはGridレイアウト変更が含まれていれば true
        mask.has_basic_layout() || mask.has_flex_layout() || mask.has_grid_layout()
    }

    pub(crate) fn resolv_focus_style(
        id: EntityId,
        interaction_properties: &InteractionPropertiesSecondary,
        visual_properties: &VisualPropertiesSecondary,
        active_mask: &ComponentMask,
        parents: &SecondaryMap<EntityId, Option<EntityId>>,
        state_flag: u128,
    ) -> Option<ThisStyle> {
        if !active_mask.has(state_flag) {
            return None;
        }
        let self_style = interaction_properties.get(id).and_then(|interaction| {
            if state_flag == STATE_FOCUSED_VISIBLE {
                interaction.focused_visible.clone()
            } else {
                interaction.focused.clone()
            }
        });

        if let Some(style) = self_style {
            return Some(style);
        }

        let focus_mode = visual_properties
            .get(id)
            .and_then(|v| v.focusable)
            .unwrap_or_default();

        let is_trigger_match = match (state_flag, focus_mode) {
            (STATE_FOCUSED, Focusable::Inherit(_)) => true,
            (STATE_FOCUSED_VISIBLE, Focusable::Inherit(trigger)) => {
                trigger == FocusTrigger::Keyboard || trigger == FocusTrigger::Both
            }
            _ => false,
        };

        if !is_trigger_match {
            return None;
        }

        // 親先祖を上に辿り、最初に focused 疑似スタイルを定義している要素の設定をそのまま借用する
        let mut curr = parents.get(id).copied().flatten();
        while let Some(curr_id) = curr {
            if let Some(parent_interaction) = interaction_properties.get(curr_id) {
                let parent_style = if state_flag == STATE_FOCUSED_VISIBLE {
                    &parent_interaction.focused_visible
                } else {
                    &parent_interaction.focused
                };

                if let Some(p_style) = parent_style {
                    return Some(p_style.clone());
                }
            }
            curr = parents.get(curr_id).copied().flatten();
        }
        None
    }

    pub(crate) fn cascade_interaction_flag<'a>(
        id: EntityId,
        interaction: &'a InteractionStyles,
        focused_style_resolved: &'a Option<ThisStyle>,
        focused_visible_style_resolved: &'a Option<ThisStyle>,
    ) -> [(u128, &'a Option<ThisStyle>); 11] {
        [
            (STATE_FOCUSED, focused_style_resolved),
            (STATE_FOCUSED_VISIBLE, focused_visible_style_resolved),
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
    ) -> [(u128, &Option<ThisStyle>); 10] {
        [
            (STATE_FOCUSED, &interaction.focused_within),
            (STATE_FOCUSED_VISIBLE, &interaction.focused_visible_within),
            (STATE_SELECTED, &interaction.selected_within),
            (STATE_ACTIVED, &interaction.actived_within),
            (STATE_HOVERED, &interaction.hovered_within),
            (STATE_PRESSED, &interaction.pressed_within),
            (STATE_DISABLED, &interaction.disabled_within),
            (STATE_DRAGGED, &interaction.dragged_within),
            (STATE_DRAGGING, &interaction.dragged_within),
            (STATE_DRAG_IN, &interaction.hovered_within),
        ]
    }

    pub(crate) fn cascade_parent_interaction_flag(
        id: EntityId,
        interaction: &InteractionStyles,
    ) -> [(u128, &Option<ThisStyle>); 10] {
        [
            (STATE_FOCUSED, &interaction.focused_parent),
            (STATE_FOCUSED_VISIBLE, &interaction.focused_visible_parent),
            (STATE_SELECTED, &interaction.selected_parent),
            (STATE_ACTIVED, &interaction.actived_parent),
            (STATE_HOVERED, &interaction.hovered_parent),
            (STATE_PRESSED, &interaction.pressed_parent),
            (STATE_DISABLED, &interaction.disabled_parent),
            (STATE_DRAGGED, &interaction.dragged_parent),
            (STATE_DRAGGING, &interaction.dragged_parent),
            (STATE_DRAG_IN, &interaction.hovered_parent),
        ]
    }

    pub(crate) fn cascade_basic_layout(
        id: EntityId,
        interaction_properties: &InteractionPropertiesSecondary,
        active_mask: ComponentMask,
        target_layout: &mut BasicLayout,
    ) {
        let Some(interaction) = interaction_properties.get(id) else {
            return;
        };

        let cascade = [
            (STATE_FOCUSED, &interaction.focused),
            (STATE_FOCUSED_VISIBLE, &interaction.focused_visible),
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

    #[inline]
    pub(crate) fn cascade_interaction(
        id: EntityId,
        active_mask: ComponentMask,
        target: &mut TargetStyle,
        interaction_properties: &InteractionPropertiesSecondary,
        focused_style_resolved: Option<ThisStyle>,
        focused_visible_style_resolved: Option<ThisStyle>,
    ) {
        let Some(interaction) = interaction_properties.get(id) else {
            return;
        };

        let cascade = RenderStore::cascade_interaction_flag(
            id,
            interaction,
            &focused_style_resolved,
            &focused_visible_style_resolved,
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

    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub(crate) fn cascade_within_interaction(
        id: EntityId,
        active_mask: ComponentMask,
        target: &mut TargetStyle,
        interaction_properties: &InteractionPropertiesSecondary,
        entities: &EntitiesSlot,
        children: &ChildrenSecondary,
        active_masks: &ActiveMasksSecondary,
    ) {
        if !active_mask.has(STYLE_INTERACTION_WITHIN) {
            return;
        }

        let Some(interaction) = interaction_properties.get(id) else {
            return;
        };

        // 自身の mask にビットが立っている場合のみツリー再帰を走らせてマージ解決
        let cascade_within = RenderStore::cascade_within_interaction_flag(id, interaction);

        for (state, style_opt) in cascade_within {
            let Some(style) = style_opt else {
                continue;
            };

            // 子孫要素のいずれかがこの state_flag を満たしているか
            let with_state = TopologyStore::has_descendant_with_state(
                entities,
                children,
                active_masks,
                id,
                state,
            );

            if !with_state {
                continue;
            }

            TargetStyle::apply_visual_property(
                target,
                &style.inner.visual_property,
                style.inner.mask,
            );
        }

        // All（いずれかのインタラクションがあればON）の解決
        let Some(ref style) = interaction.any_within else {
            return;
        };

        let any_state = TopologyStore::has_descendant_with_state(
            entities,
            children,
            active_masks,
            id,
            STYLE_ACTIVE_INTERACTION_PROPERTY,
        );

        if !any_state {
            return;
        }

        TargetStyle::apply_visual_property(target, &style.inner.visual_property, style.inner.mask);
    }

    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub(crate) fn cascade_parent_interaction(
        id: EntityId,
        active_mask: ComponentMask,
        target: &mut TargetStyle,
        interaction_properties: &InteractionPropertiesSecondary,
        entities: &EntitiesSlot,
        parents: &ParentsSecondary,
        children: &ChildrenSecondary,
        active_masks: &ActiveMasksSecondary,
    ) {
        if active_mask.has(STYLE_INTERACTION_PARENT) {
            return;
        }
        let Some(interaction) = interaction_properties.get(id) else {
            return;
        };

        let cascade_parent = RenderStore::cascade_parent_interaction_flag(id, interaction);

        for (state, style_opt) in cascade_parent {
            let Some(style) = style_opt else {
                continue;
            };

            // 直近の親要素がこの state_flag を満たしているか
            let with_state =
                TopologyStore::has_parent_with_state(id, parents, entities, active_masks, state);

            if !with_state {
                continue;
            }

            TargetStyle::apply_visual_property(
                target,
                &style.inner.visual_property,
                style.inner.mask,
            );
        }

        let Some(ref style) = interaction.any_parent else {
            return;
        };
        let any_state = TopologyStore::has_parent_with_state(
            id,
            parents,
            entities,
            active_masks,
            STYLE_ACTIVE_INTERACTION_PROPERTY,
        );

        if !any_state {
            return;
        }

        TargetStyle::apply_visual_property(target, &style.inner.visual_property, style.inner.mask);
    }

    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub(crate) fn apply_interaction_cascades(
        id: EntityId,
        active_mask: ComponentMask,
        target: &mut TargetStyle,
        interaction_properties: &InteractionPropertiesSecondary,
        entities: &EntitiesSlot,
        parents: &ParentsSecondary,
        children: &ChildrenSecondary,
        active_masks: &ActiveMasksSecondary,
        focused_style_resolved: Option<ThisStyle>,
        focused_visible_style_resolved: Option<ThisStyle>,
    ) {
        RenderStore::cascade_interaction(
            id,
            active_mask,
            target,
            interaction_properties,
            focused_style_resolved,
            focused_visible_style_resolved,
        );
        RenderStore::cascade_parent_interaction(
            id,
            active_mask,
            target,
            interaction_properties,
            entities,
            parents,
            children,
            active_masks,
        );
        RenderStore::cascade_within_interaction(
            id,
            active_mask,
            target,
            interaction_properties,
            entities,
            children,
            active_masks,
        );
    }

    /// 対象の要素がキーボードフォーカス可能であるかを検証
    pub(crate) fn is_keyboard_focusable(
        id: EntityId,
        entities: &EntitiesSlot,
        active_masks: &ActiveMasksSecondary,
        parents: &ParentsSecondary,
        visual_properties: &VisualPropertiesSecondary,
        basic_layouts: &BasicLayoutsSecondary,
    ) -> bool {
        if !entities.contains_key(id) {
            return false;
        }
        // 無効化（Disabled）状態でないか検証
        let mask = active_masks.get(id).copied().unwrap_or_default();
        if mask.has(STATE_DISABLED) {
            return false;
        }

        // 暗黙的または明示的にキーボードフォーカスを要求しているか
        let focusable = visual_properties.get(id).and_then(|v| v.focusable);
        let is_target = match focusable {
            // 明示的にフォーカス設定がある場合
            Some(Focusable::SelfStyle(trigger) | Focusable::Inherit(trigger)) => {
                matches!(trigger, FocusTrigger::Keyboard | FocusTrigger::Both)
            }
            Some(Focusable::None) => false,
            // 設定がない場合の暗黙的なフォールバック（Input / Webview はデフォルトでフォーカス対象とする）
            None => mask.has(COMP_INPUT_CONTENT) || mask.has(COMP_WEBVIEW_CONTENT),
        };

        if !is_target {
            return false;
        }

        // 自分自身、および親先祖ツリーに非表示（Display::None）が1つも含まれていないか検証
        let mut curr = Some(id);
        while let Some(curr_id) = curr {
            if let Some(layout) = basic_layouts.get(curr_id)
                && layout.display == Display::None
            {
                return false;
            }
            curr = parents.get(curr_id).copied().flatten();
        }
        true
    }

    /// 補間されたアニメーション値を SoA のアクティブプロパティへ安全に上書きします
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply_animation_value(
        id: EntityId,
        property: PropertyList,
        value: &TransitionValue,
        visual_properties: &mut VisualPropertiesSecondary,
        basic_layouts: &mut BasicLayoutsSecondary,
        taffy_nodes: &TaffyNodesSecondary,
        taffy: &mut TaffyTreeEntityId,
        active_masks: &mut ActiveMasksSecondary,
        dirty_layout_entities: &mut DirtyLayoutEntitiesVec,
        parents: &ParentsSecondary,
    ) {
        if !visual_properties.contains_key(id) {
            visual_properties.insert(id, Default::default());
        }
        let v = visual_properties.get_mut(id).unwrap();

        // レイアウト変更が発生したか
        let mut is_layout_dirty = false;

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
                if let Some(layout) = basic_layouts.get_mut(id) {
                    layout.size.width = Val::Px(w);
                }
                is_layout_dirty = true;
            }
            TransitionValue::Height(h) => {
                if let Some(layout) = basic_layouts.get_mut(id) {
                    layout.size.height = Val::Px(h);
                }
                is_layout_dirty = true;
            }
            TransitionValue::BoxShadow(shadow) => {
                v.shadow_params = Some(shadow);
                v.shadow_color = Some(shadow.color);
            }
        }

        if is_layout_dirty {
            LayoutStore::mark_layout_dirty(
                id,
                taffy_nodes,
                taffy,
                active_masks,
                dirty_layout_entities,
                parents,
            );
        }
    }

    /// 状態の変更を検知しアニメーションが必要な箇所を自動的に開始・制御
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve_element_style_state(
        id: EntityId,
        allow_transition: bool,
        active_masks: &mut ActiveMasksSecondary,
        base_visual_properties: &BaseVisualPropertiesSecondary,
        interaction_properties: &InteractionPropertiesSecondary,
        visual_properties: &mut VisualPropertiesSecondary,
        parents: &ParentsSecondary,
        entities: &EntitiesSlot,
        children: &ChildrenSecondary,
        input_contents: &InputContentsSparseSecondary,
        element_effects: &ElementEffectsSecondary,
        active_transitions: &mut ActiveTransitionsSparseSecondary,
        active_animations: &mut ActiveAnimationsSparseSecondary,
        dirty_render_entities: &mut DirtyRenderEntitiesVec,
        basic_layouts: &mut BasicLayoutsSecondary,
        base_basic_layouts: &BaseBasicLayoutsSecondary,
        rects: &RectsSecondary,
        last_window_size: &Option<LayoutSize>,
        taffy_nodes: &TaffyNodesSecondary,
        taffy: &mut TaffyTreeEntityId,
        dirty_layout_entities: &mut DirtyLayoutEntitiesVec,
    ) {
        let active_mask = active_masks[id];

        RenderStore::resolve_visual_styles(
            id,
            allow_transition,
            active_mask,
            base_visual_properties,
            visual_properties,
            interaction_properties,
            entities,
            parents,
            children,
            active_masks,
            input_contents,
            element_effects,
            active_transitions,
            dirty_render_entities,
        );

        RenderStore::resolve_layout_styles(
            id,
            allow_transition,
            active_mask,
            basic_layouts,
            base_basic_layouts,
            interaction_properties,
            parents,
            taffy_nodes,
            taffy,
            active_masks,
            dirty_layout_entities,
            rects,
            last_window_size,
            visual_properties,
            base_visual_properties,
            element_effects,
            active_transitions,
        );

        // スタイル解決が完了した結果、自身に新しくキーフレームアニメーション定義が
        // 読み込まれていれば、自動的にそのアニメーションの再生を開始する
        RenderStore::trigger_keyframe_animations_if_needed(
            id,
            visual_properties,
            active_animations,
        );

        let Some(effects) = element_effects.get(id) else {
            return;
        };
        let text_effects: Vec<EffectId> = effects
            .iter()
            .filter(|(cat, _)| *cat == EffectCategory::Text)
            .map(|(_, eff_id)| *eff_id)
            .collect();
        for eff_id in text_effects {
            crate::execute_effect(eff_id);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_layout_styles(
        id: EntityId,
        allow_transition: bool,
        active_mask: ComponentMask,
        basic_layouts: &mut BasicLayoutsSecondary,
        base_basic_layouts: &BaseBasicLayoutsSecondary,
        interaction_properties: &InteractionPropertiesSecondary,
        parents: &ParentsSecondary,
        taffy_nodes: &TaffyNodesSecondary,
        taffy: &mut TaffyTreeEntityId,
        active_masks: &mut ActiveMasksSecondary,
        dirty_layout_entities: &mut DirtyLayoutEntitiesVec,
        rects: &RectsSecondary,
        last_window_size: &Option<LayoutSize>,
        visual_properties: &VisualPropertiesSecondary,
        base_visual_properties: &BaseVisualPropertiesSecondary,
        element_effects: &ElementEffectsSecondary,
        active_transitions: &mut ActiveTransitionsSparseSecondary,
    ) {
        let has_base_layout = base_basic_layouts.contains_key(id);
        let has_active_layout = basic_layouts.contains_key(id);

        if !has_base_layout && !has_active_layout {
            return;
        }

        let active_layout = basic_layouts.get(id).cloned().unwrap_or_default();
        let base_layout = base_basic_layouts.get(id).cloned().unwrap_or_default();
        let mut target_layout = base_layout;

        RenderStore::cascade_basic_layout(
            id,
            interaction_properties,
            active_mask,
            &mut target_layout,
        );

        let to_px = |val, is_width| {
            OutputStore::val_to_px(id, val, is_width, parents, rects, last_window_size)
        };

        let target_w_px = to_px(target_layout.size.width, true);
        let current_w_px = to_px(active_layout.size.width, true);
        let target_h_px = to_px(target_layout.size.height, false);
        let current_h_px = to_px(active_layout.size.height, false);

        let mut width_triggered = false;
        let mut height_triggered = false;

        let mut if_needed = |prop, start, end| {
            RenderStore::trigger_transition_if_needed(
                id,
                prop,
                start,
                end,
                element_effects,
                base_visual_properties,
                active_transitions,
            )
        };

        let can_trigger_width = visual_properties
            .get(id)
            .map(|v| {
                v.transitions.iter().any(|t| {
                    t.property_list == PropertyList::Width || t.property_list == PropertyList::Size
                })
            })
            .unwrap_or(false);

        if allow_transition
            && can_trigger_width
            && has_active_layout
            && let (Some(cw), Some(tw)) = (current_w_px, target_w_px)
            && (cw - tw).abs() > 0.01
        {
            width_triggered = if_needed(
                PropertyList::Width,
                TransitionValue::Width(cw),
                TransitionValue::Width(tw),
            );
        }

        let can_trigger_height = visual_properties
            .get(id)
            .map(|v| {
                v.transitions.iter().any(|t| {
                    t.property_list == PropertyList::Height || t.property_list == PropertyList::Size
                })
            })
            .unwrap_or(false);

        if allow_transition
            && can_trigger_height
            && has_active_layout
            && let (Some(ch), Some(th)) = (current_h_px, target_h_px)
            && (ch - th).abs() > 0.01
        {
            height_triggered = if_needed(
                PropertyList::Height,
                TransitionValue::Height(ch),
                TransitionValue::Height(th),
            );
        }

        if !basic_layouts.contains_key(id) {
            basic_layouts.insert(id, Default::default());
        }
        let active_layout_mut = basic_layouts.get_mut(id).unwrap();
        *active_layout_mut = target_layout;

        if width_triggered {
            active_layout_mut.size.width = Val::Px(current_w_px.unwrap());
        }
        if height_triggered {
            active_layout_mut.size.height = Val::Px(current_h_px.unwrap());
        }

        LayoutStore::mark_layout_dirty(
            id,
            taffy_nodes,
            taffy,
            active_masks,
            dirty_layout_entities,
            parents,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_visual_styles(
        id: EntityId,
        allow_transition: bool,
        active_mask: ComponentMask,
        base_visual_properties: &BaseVisualPropertiesSecondary,
        visual_properties: &mut VisualPropertiesSecondary,
        interaction_properties: &InteractionPropertiesSecondary,
        entities: &EntitiesSlot,
        parents: &ParentsSecondary,
        children: &ChildrenSecondary,
        active_masks: &mut ActiveMasksSecondary,
        input_contents: &InputContentsSparseSecondary,
        element_effects: &ElementEffectsSecondary,
        active_transitions: &mut ActiveTransitionsSparseSecondary,
        dirty_render_entities: &mut DirtyRenderEntitiesVec,
    ) {
        let has_base_visual = base_visual_properties.contains_key(id);
        let has_active_visual = visual_properties.contains_key(id);
        let has_interaction_styles = interaction_properties.contains_key(id);

        if !has_base_visual && !has_active_visual && !has_interaction_styles {
            return;
        }

        let current = RenderStore::get_current_style(id, visual_properties);
        let mut target = RenderStore::get_target_style(id, base_visual_properties);

        let resolv_focus = |flag| {
            RenderStore::resolv_focus_style(
                id,
                interaction_properties,
                visual_properties,
                &active_mask,
                parents,
                flag,
            )
        };

        let focused_style_resolved = resolv_focus(STATE_FOCUSED);
        let focused_visible_style_resolved = resolv_focus(STATE_FOCUSED_VISIBLE);

        RenderStore::apply_interaction_cascades(
            id,
            active_mask,
            &mut target,
            interaction_properties,
            entities,
            parents,
            children,
            active_masks,
            focused_style_resolved,
            focused_visible_style_resolved,
        );

        let mut is_placeholder_active = false;
        if let Some(contents) = input_contents.get(id) {
            let has_no_ime = contents
                .ime_state
                .as_ref()
                .map(|s| s.composition_text.is_empty())
                .unwrap_or(true);
            if contents.text.0.get().is_empty() && has_no_ime {
                is_placeholder_active = true;
            }
        }

        // 変更評価の計算
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

        let target_transform_origin_val = target.transform_origin.unwrap_or(Point::ORIGIN);
        let transform_origin_changed = current.transform_origin != target_transform_origin_val;

        let target_radius_val = target.corner_radius.unwrap_or(CornerRadius::ZERO);
        let radius_changed = current.corner_radius != target_radius_val;

        let target_shadow_val = target.shadow_params.unwrap_or(BoxShadow::none());
        let shadow_changed = current.shadow_params != target_shadow_val;

        // トランジション判定
        let mut if_needed = |prop, start, end| {
            RenderStore::trigger_transition_if_needed(
                id,
                prop,
                start,
                end,
                element_effects,
                base_visual_properties,
                active_transitions,
            )
        };

        let mut bg_triggered = false;
        if allow_transition && bg_changed && has_active_visual {
            bg_triggered = if_needed(
                PropertyList::BackgroundColor,
                TransitionValue::Color(current.bg_color),
                TransitionValue::Color(target_bg_val),
            );
        }

        let mut border_triggered = false;
        if border_changed && has_active_visual {
            border_triggered = if_needed(
                PropertyList::BorderColor,
                TransitionValue::Color(current.border_color),
                TransitionValue::Color(target_border_val),
            );
        }

        let mut opacity_triggered = false;
        if opacity_changed && has_active_visual {
            opacity_triggered = if_needed(
                PropertyList::Opacity,
                TransitionValue::Opacity(current.opacity),
                TransitionValue::Opacity(target_opacity_val),
            );
        }

        let mut transform_triggered = false;
        if transform_changed {
            transform_triggered = if_needed(
                PropertyList::Transform,
                TransitionValue::Transform(current.transform),
                TransitionValue::Transform(target_transform_val),
            );
        }

        let mut radius_triggered = false;
        if radius_changed && has_active_visual {
            radius_triggered = if_needed(
                PropertyList::CornerRadius,
                TransitionValue::CornerRadius(current.corner_radius),
                TransitionValue::CornerRadius(target_radius_val),
            );
        }

        let mut shadow_triggered = false;
        if shadow_changed && has_active_visual {
            shadow_triggered = if_needed(
                PropertyList::BoxShadow,
                TransitionValue::BoxShadow(current.shadow_params),
                TransitionValue::BoxShadow(target_shadow_val),
            );
        }

        // 更新の書き込み
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
            || base_visual_properties.contains_key(id)
        {
            if !visual_properties.contains_key(id) {
                visual_properties.insert(id, Default::default());
            }
            let active_vis = visual_properties.get_mut(id).unwrap();

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
                active_vis.transform_origin = target.transform_origin;
            }
            if !radius_triggered {
                active_vis.corner_radius = target.corner_radius;
            }

            if is_placeholder_active {
                active_vis.text_color = Some(Color::rgb_f32(0.5, 0.5, 0.5));
            } else {
                active_vis.text_color = target.text_color;
            }

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

            active_vis.select_bg_color = target.select_bg_color;
            active_vis.select_text_color = target.select_text_color;

            active_vis.cursor = target.cursor;
            active_vis.resizable_cursor = target.resizable_cursor;

            active_vis.font_size = target.font_size;
            active_vis.font_family = target.font_family.clone();
            active_vis.font_weight = target.font_weight;
            active_vis.font_style = target.font_style;

            active_vis.pointer_events = target.pointer_events;

            if let Some(target_vis) = base_visual_properties.get(id) {
                active_vis.z_index = target_vis.z_index;
                active_vis.backdrop = target_vis.backdrop;
                active_vis.bg_gradient = target_vis.bg_gradient;
                active_vis.transitions = target_vis.transitions.clone();
                active_vis.keyframe_animations = target_vis.keyframe_animations.clone();
                active_vis.focusable = target_vis.focusable;
                active_vis.prevent_focus_steal = target_vis.prevent_focus_steal;
                active_vis.prevent_focus_steal_within = target_vis.prevent_focus_steal_within;
                active_vis.transform_inherit = target.transform_inherit;
                active_vis.user_select = target_vis.user_select;
            }

            RenderStore::mark_render_dirty(id, active_masks, dirty_render_entities);
        }
    }

    /// 必要に応じてトランジションを起動、または上書き（逆再生含む）します
    pub(crate) fn trigger_transition_if_needed(
        id: EntityId,
        property_list: PropertyList,
        start_value: TransitionValue,
        end_value: TransitionValue,
        element_effects: &ElementEffectsSecondary,
        base_visual_properties: &BaseVisualPropertiesSecondary,
        active_transitions: &mut ActiveTransitionsSparseSecondary,
    ) -> bool {
        // スタイルの再評価エフェクトの実行中であるか
        let is_style_evaluating = crate::signal::ACTIVE_EFFECT.with(|cell| {
            if let Some(effect_id) = cell.get() {
                // 現在走っているエフェクトがいずれかの要素の StyleCategory::Style のものであるか走査
                element_effects.values().any(|list| {
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

        let Some(visual) = base_visual_properties.get(id) else {
            return false;
        };

        let Some(t) = visual
            .transitions
            .iter()
            .find(|t| t.property_list == property_list || t.property_list == PropertyList::Size)
        else {
            return false;
        };

        let Some(entry) = active_transitions.entry(id) else {
            return false;
        };

        let active_list = entry.or_insert_with(Vec::new);
        let now = Instant::now();

        // 割り込み処理の解決（すでに同じアニメーションが走っている場合）
        if let Some(existing) = active_list
            .iter_mut()
            .find(|et| et.property_list == property_list)
        {
            // 同一目的地なら何もしない
            if existing.end_value == end_value {
                return true;
            }

            // 中間補間位置を計算
            let elapsed = existing
                .start_time
                .map(|st| now.duration_since(st))
                .unwrap_or(Duration::ZERO);
            let progress = (elapsed.as_secs_f32() / existing.duration.as_secs_f32()).min(1.0);
            let eased_t = existing.curve.evaluate(progress);
            let current_interposed_val = existing.start_value.lerp(&existing.end_value, eased_t);

            // 既存状態を上書きしてリセット
            existing.start_time = None;
            existing.start_value = current_interposed_val;
            existing.end_value = end_value;
            existing.duration = t.duration;
            existing.curve = t.curve;

            return true; // 割り込み完了につき早期リターン
        }

        // 新規トランジション登録（割り込みが無かった場合のみここに到達）
        active_list.push(ActiveTransition {
            property_list,
            start_time: None,
            duration: t.duration,
            curve: t.curve,
            start_value, // 元の start_value
            end_value,
        });

        true
    }

    /// 現在ホバーされている要素から親ツリーを遡り、適用するべき物理的な CursorIcon を正確に解決します。
    pub(crate) fn resolve_cursor(
        hovered_id: EntityId,
        interaction_states: &InteractionStates,
        visual_properties: &VisualPropertiesSecondary,
        base_visual_properties: &BaseVisualPropertiesSecondary,
        parents: &ParentsSecondary,
    ) -> CursorIcon {
        // 現在プレス中の要素（pressed）があればそれを最優先で探索の基点にする
        let start_id = interaction_states.pressed.unwrap_or(hovered_id);

        let mut curr = Some(start_id);
        let mut global_cursor = None;

        while let Some(id) = curr {
            let cursor_opt = visual_properties
                .get(id)
                .and_then(|v| v.cursor)
                .or_else(|| base_visual_properties.get(id).and_then(|v| v.cursor));

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
            curr = parents.get(id).copied().flatten();
        }

        // 個別指定がなく、親のいずれかに Global カーソルが定義されていた場合はそれを採用
        let Some(global) = global_cursor else {
            // 先祖に何の設定もない場合はデフォルトの矢印
            return CursorIcon::Default(None);
        };
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
    }

    pub(crate) fn accumulate_transform_matrix(
        flat_dfs_sequence: &FlatDfsSequenceVec,
        visual_properties: &VisualPropertiesSecondary,
        parents: &ParentsSecondary,
        active_entities: &ActiveEntitiesVec,
    ) -> SecondaryMap<EntityId, [[f32; 4]; 4]> {
        let mut effective_transforms = SecondaryMap::with_capacity(active_entities.len());

        for &id in flat_dfs_sequence {
            let (self_transform, transform_inherit) = match visual_properties.get(id) {
                Some(v) => (
                    v.transform.unwrap_or(IDENTITY_MATRIX),
                    v.transform_inherit.unwrap_or(false),
                ),
                None => (IDENTITY_MATRIX, false),
            };

            let mut eff_transform = self_transform;

            if transform_inherit
                && let Some(parent_id) = parents.get(id).copied().flatten()
                && let Some(&parent_eff) = effective_transforms.get(parent_id)
            {
                // 親の累積トランスフォーム行列 * 自身のトランスフォーム行列 (Column-Major 順)
                eff_transform = OutputStore::mul_4x4(&parent_eff, &self_transform);
            }
            effective_transforms.insert(id, eff_transform);
        }
        effective_transforms
    }

    #[inline]
    pub(crate) fn get_transform_and_origin(
        id: EntityId,
        visual: &VisualProperty,
        transforms: &SecondaryMap<EntityId, [[f32; 4]; 4]>,
    ) -> ([[f32; 4]; 3], [f32; 2]) {
        let full_transform = transforms.get(id).copied().unwrap_or(IDENTITY_MATRIX);
        let packed_transform = [
            full_transform[0], // X軸基底
            full_transform[1], // Y軸基底
            full_transform[3], // 平行移動部
        ];
        let origin = visual
            .transform_origin
            .map(|p| [p.x, p.y])
            .unwrap_or([0.5, 0.5]);
        (packed_transform, origin)
    }

    #[inline]
    pub(crate) fn get_outline_params(
        visual: &VisualProperty,
    ) -> (EdgeInsets, Color, EdgeInsets, [f32; 4]) {
        let o_width = visual.outline_width.unwrap_or_default();
        let o_color = visual.outline_color.unwrap_or_default();
        let o_lengths = visual.outline_lengths.unwrap_or(EdgeInsets::px_all(1.0));
        let o_offset = visual.outline_offset.unwrap_or(0.0);
        let o_styles = visual.outline_styles.unwrap_or([BorderStyle::default(); 4]);
        let o_aligns = visual
            .outline_alignments
            .unwrap_or([BorderAlignment::default(); 4]);

        let mut o_flags = 0u32;
        // 1つの辺あたり4ビットを割り当て各辺の位置（idx * 4）へ配置
        // ビットレイアウト (u32, 下位16ビットを使用)
        //  15      12 11       8 7       4 3        0
        // +---------+---------+---------+---------+
        // |  Left   | Bottom  |  Right  |   Top   |  <-- 各4ビット (Edge)
        // +---------+---------+---------+---------+
        //   |_ Align (2bit)      |_ Align (2bit)
        //   |_ Style (2bit)      |_ Style (2bit)
        // う～ん、分からんｗ
        for (idx, (&style, &align)) in o_styles.iter().zip(o_aligns.iter()).enumerate() {
            // 将来列挙型が増えた際、隣のビットを汚染しないよう 2ビット（0b11）でマスク
            let style_bits = (style as u32) & 0b11; // 下位2ビット (0〜3)
            let align_bits = (align as u32) & 0b11; // 上位2ビット (0〜3)

            let edge_flags = style_bits | (align_bits << 2); // 4ビット分のデータ

            o_flags |= edge_flags << (idx * 4); // 対象の辺の位置（0, 4, 8, 12ビット目）
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
    pub(crate) transform_origin: Point<f32>,
    pub(crate) corner_radius: CornerRadius,
    pub(crate) shadow_params: BoxShadow,
}

impl Default for CurrentStyle {
    fn default() -> Self {
        CurrentStyle {
            bg_color: Color::TRANSPARENT,
            border_color: Color::TRANSPARENT,
            outline_width: EdgeInsets::ZERO,
            outline_color: Color::TRANSPARENT,
            outline_offset: 0.0,
            opacity: 1.0,
            transform: IDENTITY_MATRIX,
            transform_origin: Point::ORIGIN,
            corner_radius: CornerRadius::ZERO,
            shadow_params: BoxShadow::none(),
        }
    }
}
impl RenderStore {
    /// 現在の描画用データを取得 (Copy可能なプリミティブのみ)
    #[inline]
    pub(crate) fn get_current_style(
        id: EntityId,
        visual_properties: &VisualPropertiesSecondary,
    ) -> CurrentStyle {
        visual_properties
            .get(id)
            .map(|v| CurrentStyle {
                bg_color: v.bg_color.unwrap_or(Color::TRANSPARENT),
                border_color: v.border_color.unwrap_or(Color::TRANSPARENT),
                outline_width: v.outline_width.unwrap_or(EdgeInsets::ZERO),
                outline_color: v.outline_color.unwrap_or(Color::TRANSPARENT),
                outline_offset: v.outline_offset.unwrap_or(0.0),
                opacity: v.opacity.unwrap_or(1.0),
                transform: v.transform.unwrap_or(IDENTITY_MATRIX),
                transform_origin: v.transform_origin.unwrap_or(Point::ORIGIN),
                corner_radius: v.corner_radius.unwrap_or(CornerRadius::ZERO),
                shadow_params: v.shadow_params.unwrap_or(BoxShadow::none()),
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TargetStyle {
    pub(crate) pointer_events: Option<PointerEvents>,
    pub(crate) cursor: Option<CursorIcon>,
    pub(crate) resizable_cursor: Option<[Option<CursorIcon>; 4]>,
    pub(crate) bg_color: Option<Color>,
    pub(crate) border_color: Option<Color>,
    pub(crate) opacity: Option<f32>,
    pub(crate) transform: Option<[[f32; 4]; 4]>,
    pub(crate) transform_origin: Option<Point<f32>>,
    pub(crate) transform_inherit: Option<bool>,
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
    pub(crate) font_size: Option<f32>,
    pub(crate) font_family: Option<Cow<'static, str>>,
    pub(crate) font_weight: Option<u32>,
    pub(crate) font_style: Option<u32>,
}

impl RenderStore {
    /// 目標値を参照経由で構築
    #[inline]
    pub(crate) fn get_target_style(
        id: EntityId,
        base_visual_properties: &BaseVisualPropertiesSecondary,
    ) -> TargetStyle {
        base_visual_properties
            .get(id)
            .map(|v| TargetStyle {
                pointer_events: v.pointer_events,
                cursor: v.cursor,
                resizable_cursor: v.resizable_cursor,
                bg_color: v.bg_color,
                border_color: v.border_color,
                opacity: v.opacity,
                transform: v.transform,
                transform_origin: v.transform_origin,
                transform_inherit: v.transform_inherit,
                corner_radius: v.corner_radius,
                shadow_params: v.shadow_params,
                shadow_color: v.shadow_color,
                text_color: v.text_color,
                select_bg_color: v.select_bg_color,
                select_text_color: v.select_text_color,
                border_lengths: v.border_lengths,
                border_styles: v.border_styles,
                border_alignments: v.border_alignments,
                outline_width: v.outline_width,
                outline_color: v.outline_color,
                outline_lengths: v.outline_lengths,
                outline_styles: v.outline_styles,
                outline_alignments: v.outline_alignments,
                outline_offset: v.outline_offset,
                font_size: v.font_size,
                font_family: v.font_family.clone(),
                font_weight: v.font_weight,
                font_style: v.font_style,
            })
            .unwrap_or_default()
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
            target.transform_origin = inner_vis.transform_origin;
        }

        if inner_mask.has(STYLE_TRANSFORM_INHERIT) {
            target.transform_inherit = inner_vis.transform_inherit;
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
        if inner_mask.has(STYLE_FONT_SIZE) {
            target.font_size = inner_vis.font_size;
        }
        if inner_mask.has(STYLE_EXT_PROPERTIES) {
            if inner_vis.font_family.is_some() {
                target.font_family = inner_vis.font_family.clone();
            }
            if inner_vis.font_weight.is_some() {
                target.font_weight = inner_vis.font_weight;
            }
            if inner_vis.font_style.is_some() {
                target.font_style = inner_vis.font_style;
            }
        }
    }
}

impl Context {
    #[inline]
    pub(crate) fn mark_render_dirty(&mut self, id: EntityId) {
        let TopologyStore { active_masks, .. } = &mut self.topology;
        let RenderStore {
            dirty_render_entities,
            ..
        } = &mut self.renders;

        RenderStore::mark_render_dirty(id, active_masks, dirty_render_entities);
    }

    #[inline]
    pub(crate) fn get_visual_property_mut(
        &mut self,
        id: EntityId,
        target: StyleTarget,
    ) -> Option<&mut VisualProperty> {
        let RenderStore {
            base_visual_properties,
            interaction_properties,
            ..
        } = &mut self.renders;

        RenderStore::get_visual_property_mut(
            id,
            target,
            base_visual_properties,
            interaction_properties,
        )
    }

    #[inline]
    pub(crate) fn accumulate_transform_matrix(&self) -> SecondaryMap<EntityId, [[f32; 4]; 4]> {
        let TopologyStore {
            flat_dfs_sequence,
            parents,
            active_entities,
            ..
        } = &self.topology;
        let RenderStore {
            visual_properties, ..
        } = &self.renders;

        RenderStore::accumulate_transform_matrix(
            flat_dfs_sequence,
            visual_properties,
            parents,
            active_entities,
        )
    }

    #[inline]
    pub fn get_transform_and_origin(
        &self,
        id: EntityId,
        visual: &VisualProperty,
        transforms: &SecondaryMap<EntityId, [[f32; 4]; 4]>,
    ) -> ([[f32; 4]; 3], [f32; 2]) {
        RenderStore::get_transform_and_origin(id, visual, transforms)
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
        let RenderStore {
            visual_properties,
            active_animations,
            ..
        } = &mut self.renders;

        RenderStore::trigger_keyframe_animations_if_needed(
            id,
            visual_properties,
            active_animations,
        );
    }

    /// 指定された動的状態（例: STATE_HOVERED）に切り替わる際、
    /// その要素に割り当てられている状態スタイルがレイアウトの再計算を必要とするか判定します。
    #[inline]
    pub(crate) fn does_state_require_layout(&self, id: EntityId, state_flag: u128) -> bool {
        let RenderStore {
            interaction_properties,
            ..
        } = &self.renders;

        RenderStore::does_state_require_layout(id, interaction_properties, state_flag)
    }

    #[inline]
    pub(crate) fn resolv_focus_style(
        &self,
        id: EntityId,
        active_mask: &ComponentMask,
        state_flag: u128,
    ) -> Option<ThisStyle> {
        let RenderStore {
            interaction_properties,
            visual_properties,
            ..
        } = &self.renders;
        let TopologyStore {
            active_masks,
            parents,
            ..
        } = &self.topology;

        RenderStore::resolv_focus_style(
            id,
            interaction_properties,
            visual_properties,
            active_mask,
            parents,
            state_flag,
        )
    }

    #[inline]
    pub(crate) fn cascade_interaction(
        &self,
        id: EntityId,
        active_mask: ComponentMask,
        target: &mut TargetStyle,
        focused_style_resolved: Option<ThisStyle>,
        focused_visible_style_resolved: Option<ThisStyle>,
    ) {
        let RenderStore {
            interaction_properties,
            ..
        } = &self.renders;

        RenderStore::cascade_interaction(
            id,
            active_mask,
            target,
            interaction_properties,
            focused_style_resolved,
            focused_visible_style_resolved,
        );
    }

    #[inline]
    pub(crate) fn cascade_within_interaction(
        &self,
        id: EntityId,
        active_mask: ComponentMask,
        target: &mut TargetStyle,
    ) {
        let TopologyStore {
            entities,
            children,
            active_masks,
            ..
        } = &self.topology;
        let RenderStore {
            interaction_properties,
            ..
        } = &self.renders;

        RenderStore::cascade_within_interaction(
            id,
            active_mask,
            target,
            interaction_properties,
            entities,
            children,
            active_masks,
        );
    }

    #[inline]
    pub(crate) fn cascade_parent_interaction(
        &self,
        id: EntityId,
        active_mask: ComponentMask,
        target: &mut TargetStyle,
    ) {
        let TopologyStore {
            entities,
            parents,
            children,
            active_masks,
            ..
        } = &self.topology;
        let RenderStore {
            interaction_properties,
            ..
        } = &self.renders;

        RenderStore::cascade_parent_interaction(
            id,
            active_mask,
            target,
            interaction_properties,
            entities,
            parents,
            children,
            active_masks,
        );
    }

    #[inline]
    pub(crate) fn cascade_basic_layout(
        &self,
        id: EntityId,
        active_mask: ComponentMask,
        target_layout: &mut BasicLayout,
    ) {
        let RenderStore {
            interaction_properties,
            ..
        } = &self.renders;

        RenderStore::cascade_basic_layout(id, interaction_properties, active_mask, target_layout);
    }

    /// 現在の描画用データを取得 (Copy可能なプリミティブのみ)
    #[inline]
    pub(crate) fn get_current_style(&self, id: EntityId) -> CurrentStyle {
        let RenderStore {
            visual_properties, ..
        } = &self.renders;

        RenderStore::get_current_style(id, visual_properties)
    }

    /// 目標値を参照経由で構築
    #[inline]
    pub(crate) fn get_target_style(&self, id: EntityId) -> TargetStyle {
        let RenderStore {
            base_visual_properties,
            ..
        } = &self.renders;

        RenderStore::get_target_style(id, base_visual_properties)
    }

    /// 対象の要素がキーボードフォーカス可能であるかを総合検証します
    #[inline]
    pub(crate) fn is_keyboard_focusable(&self, id: EntityId) -> bool {
        let TopologyStore {
            entities,
            active_masks,
            parents,
            ..
        } = &self.topology;
        let RenderStore {
            visual_properties, ..
        } = &self.renders;
        let LayoutStore { basic_layouts, .. } = &self.layouts;

        RenderStore::is_keyboard_focusable(
            id,
            entities,
            active_masks,
            parents,
            visual_properties,
            basic_layouts,
        )
    }

    /// 補間されたアニメーション値を SoA のアクティブプロパティへ安全に上書きします
    #[inline]
    pub(crate) fn apply_animation_value(
        &mut self,
        id: EntityId,
        property: PropertyList,
        value: &TransitionValue,
    ) {
        let RenderStore {
            visual_properties, ..
        } = &mut self.renders;
        let TopologyStore {
            parents,
            active_masks,
            ..
        } = &mut self.topology;
        let LayoutStore {
            basic_layouts,
            taffy,
            taffy_nodes,
            dirty_layout_entities,
            ..
        } = &mut self.layouts;

        RenderStore::apply_animation_value(
            id,
            property,
            value,
            visual_properties,
            basic_layouts,
            taffy_nodes,
            taffy,
            active_masks,
            dirty_layout_entities,
            parents,
        );
    }

    /// 状態の変更を検知し、アニメーション（トランジション）が必要な箇所を自動的に開始・制御します。
    #[inline]
    pub(crate) fn resolve_element_style_state(&mut self, id: EntityId, allow_transition: bool) {
        let TopologyStore {
            entities,
            parents,
            children,
            active_masks,
            ..
        } = &mut self.topology;

        let LayoutStore {
            basic_layouts,
            base_basic_layouts,
            taffy_nodes,
            taffy,
            dirty_layout_entities,
            ..
        } = &mut self.layouts;

        let RenderStore {
            visual_properties,
            interaction_properties,
            base_visual_properties,
            dirty_render_entities,
            active_transitions,
            active_animations,
            ..
        } = &mut self.renders;

        let OutputStore { rects, .. } = &self.outputs;

        let ReactiveStore {
            element_effects, ..
        } = &self.reactive;

        let ContentStore { input_contents, .. } = &self.contents;

        let WindowStore {
            last_window_size, ..
        } = &self.window;

        RenderStore::resolve_element_style_state(
            id,
            allow_transition,
            active_masks,
            base_visual_properties,
            interaction_properties,
            visual_properties,
            parents,
            entities,
            children,
            input_contents,
            element_effects,
            active_transitions,
            active_animations,
            dirty_render_entities,
            basic_layouts,
            base_basic_layouts,
            rects,
            last_window_size,
            taffy_nodes,
            taffy,
            dirty_layout_entities,
        );
    }

    /// 必要に応じてトランジションを起動、または上書き（逆再生含む）します
    #[inline]
    pub(crate) fn trigger_transition_if_needed(
        &mut self,
        id: EntityId,
        property_list: PropertyList,
        start_value: TransitionValue,
        end_value: TransitionValue,
    ) -> bool {
        let RenderStore {
            base_visual_properties,
            active_transitions,
            ..
        } = &mut self.renders;
        let ReactiveStore {
            element_effects, ..
        } = &self.reactive;

        RenderStore::trigger_transition_if_needed(
            id,
            property_list,
            start_value,
            end_value,
            element_effects,
            base_visual_properties,
            active_transitions,
        )
    }
}
