use crate::{
    ActiveEntitiesVec, ActiveInteractionStates, ActiveMasksSecondary, ActiveTransition,
    AnimationCurve, BaseBasicLayoutsSecondary, BasicLayout, BasicLayoutsSecondary, BorderAlignment,
    BorderStyle, BoxShadow, CapacityConfig, ChildrenSecondary, ClipRectsSecondary, Color,
    ComponentMask, ContentStore, Context, CornerRadius, CursorIcon, DEFAULT_BASIC,
    DirtyLayoutEntitiesVec, Display, EdgeInsets, EffectCategory, EffectId, ElementEffectsSecondary,
    EntitiesSlot, EntityId, FlatDfsSequenceVec, FocusTrigger, Focusable, FontDate,
    GlobalCursorIcon, IDENTITY_MATRIX, InputContentsSparse, InteractionStyles, LayoutPoint,
    LayoutRect, LayoutSize, LayoutStore, MichiuSoA, OutputStore, ParentsSecondary, PlaybackCount,
    Point, PointerEvents, PropertyList, ReactiveStore, RectsSecondary, ScrollbarDisplay,
    ScrollbarStylesSecondary, StyleTarget, SystemStore, TaffyNodesSecondary, TaffyTreeEntityId,
    TextBufferSparse, ThisStyle, TopologyStore, TransitionValue, UserSelect, Val, VisualProperty,
    WindowStore, define_secondary, define_sparse_secondary, define_vec,
};
use rustc_hash::{FxBuildHasher, FxHashSet};
use slotmap::{SecondaryMap, SparseSecondaryMap};
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};

/// CPU 側で現在再生中の動的なキーフレームアニメーションの状態
#[derive(Debug, Clone)]
pub struct ActiveAnimation {
    pub property: PropertyList,
    pub start_time: Instant,
    pub duration: Duration,
    pub iteration_count: PlaybackCount,
    pub curve: AnimationCurve,

    // 回転アニメーションなどのために、現在の周回（ループ）における開始ベース値と目標値を定義
    pub start_value: TransitionValue,
    pub end_value: TransitionValue,
}

define_secondary!(pub struct VisualPropertiesSecondary(VisualProperty));
define_secondary!(pub struct BaseVisualPropertiesSecondary(VisualProperty));
define_secondary!(pub struct InteractionPropertiesSecondary(InteractionStyles));

define_sparse_secondary!(pub struct ActiveTransitionsSparse(Vec<ActiveTransition>));
define_sparse_secondary!(pub struct ActiveAnimationsSparse(Vec<ActiveAnimation>));

define_vec!(pub struct DirtyRenderEntitiesVec(EntityId));

pub type ActiveWebviewsHashSet = FxHashSet<EntityId>;

pub struct RenderStore {
    pub(crate) rnd_dirty_entities: DirtyRenderEntitiesVec,
    pub(crate) rnd_visual: VisualPropertiesSecondary,
    pub(crate) rnd_base_visual: BaseVisualPropertiesSecondary,
    pub(crate) rnd_interaction: InteractionPropertiesSecondary,
    pub(crate) rnd_active_transitions: ActiveTransitionsSparse,
    pub(crate) rnd_active_animations: ActiveAnimationsSparse,
    pub(crate) rnd_active_webviews: ActiveWebviewsHashSet,
    pub(crate) rnd_last_tick_time: Option<Instant>,
}

impl Default for RenderStore {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderStore {
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self {
            rnd_dirty_entities: DirtyRenderEntitiesVec(Vec::new()),
            rnd_visual: VisualPropertiesSecondary(SecondaryMap::new()),
            rnd_base_visual: BaseVisualPropertiesSecondary(SecondaryMap::new()),
            rnd_interaction: InteractionPropertiesSecondary(SecondaryMap::new()),
            rnd_active_transitions: ActiveTransitionsSparse(SparseSecondaryMap::new()),
            rnd_active_animations: ActiveAnimationsSparse(SparseSecondaryMap::new()),
            rnd_active_webviews: FxHashSet::default(),
            rnd_last_tick_time: None,
        }
    }

    #[inline]
    #[must_use]
    pub fn with_capacity(c: &CapacityConfig) -> Self {
        Self {
            rnd_dirty_entities: DirtyRenderEntitiesVec(Vec::with_capacity(c.rnd_dirty_entities)),
            rnd_visual: VisualPropertiesSecondary(SecondaryMap::with_capacity(c.rnd_visual)),
            rnd_base_visual: BaseVisualPropertiesSecondary(SecondaryMap::with_capacity(
                c.rnd_base_visual,
            )),
            rnd_interaction: InteractionPropertiesSecondary(SecondaryMap::with_capacity(
                c.rnd_interaction,
            )),
            rnd_active_transitions: ActiveTransitionsSparse(SparseSecondaryMap::with_capacity(
                c.rnd_active_transitions,
            )),
            rnd_active_animations: ActiveAnimationsSparse(SparseSecondaryMap::with_capacity(
                c.rnd_active_animations,
            )),
            rnd_active_webviews: FxHashSet::with_capacity_and_hasher(
                c.rnd_active_webviews,
                FxBuildHasher,
            ),
            ..Default::default()
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.rnd_dirty_entities.clear();
        self.rnd_visual.clear();
        self.rnd_base_visual.clear();
        self.rnd_interaction.clear();
        self.rnd_active_transitions.clear();
        self.rnd_active_animations.clear();
        self.rnd_active_webviews.clear();
        self.rnd_last_tick_time = None;
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.rnd_dirty_entities.retain(|&x| x != id);
        self.rnd_visual.remove(id);
        self.rnd_base_visual.remove(id);
        self.rnd_interaction.remove(id);
        self.rnd_active_transitions.remove(id);
        self.rnd_active_animations.remove(id);
        self.rnd_active_webviews.remove(&id);
    }
}

impl RenderStore {
    #[inline]
    pub(crate) fn mark_render_dirty(
        id: EntityId,
        topo_active_masks: &mut ActiveMasksSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
    ) {
        let mask = topo_active_masks.at_mut(id);

        if mask.has(ComponentMask::STATE_QUEUED_RENDER) {
            return;
        }
        mask.set(ComponentMask::STATE_QUEUED_RENDER);
        rnd_dirty_entities.push(id);
    }

    /// 描画（レンダー）ダーティ状態として登録された要素をすべてクリアします。
    #[inline]
    pub(crate) fn clear_render_dirty(
        topo_active_masks: &mut ActiveMasksSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
    ) {
        for id in rnd_dirty_entities.drain(..) {
            // 過去に詰まれたIDのため、破棄されて死んでいる可能性がある
            let Some(mask) = topo_active_masks.get_mut(id) else {
                continue;
            };
            mask.unset(ComponentMask::STATE_QUEUED_RENDER);
        }
        rnd_dirty_entities.clear();
    }

    #[inline]
    pub(crate) fn get_visual_property_mut<'a>(
        id: EntityId,
        target: StyleTarget,
        rnd_base_visual: &'a mut BaseVisualPropertiesSecondary,
        rnd_interaction: &'a mut InteractionPropertiesSecondary,
    ) -> &'a mut VisualProperty {
        if target == StyleTarget::Base {
            rnd_base_visual.at_mut(id)
        } else {
            if !rnd_interaction.contains_key(id) {
                rnd_interaction.insert(id, InteractionStyles::default());
            }
            // 上で入れたばっかだから Some のはず
            let styles = rnd_interaction.at_mut(id);
            let style_ref = styles.get_style_target_mut(target);
            &mut Arc::make_mut(&mut style_ref.inner).visual_property
        }
    }

    /// 現在、アクティブに動いているトランジションがあるか判定します
    pub(crate) fn has_active_frame(
        evt_interaction_states: &ActiveInteractionStates,
        evt_current_pointer_position: Option<&LayoutPoint>,
        cont_input_contents: &InputContentsSparse,
        bar_styles: &ScrollbarStylesSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparse,
        rnd_active_animations: &ActiveAnimationsSparse,
        out_clip_rects: &ClipRectsSecondary,
    ) -> bool {
        // ドラッグ選択中でポインタが可視境界外にある場合も継続
        let has_drag_autoscroll = OutputStore::is_drag_autoscroll_active(
            evt_interaction_states,
            evt_current_pointer_position,
            rnd_visual,
            out_clip_rects,
        );

        // トランジションのアクティブ判定
        let has_transitions = !rnd_active_transitions.is_empty()
            && rnd_active_transitions.values().any(|list| !list.is_empty());

        // キーフレームアニメーションのアクティブ判定
        let has_keyframes = !rnd_active_animations.is_empty()
            && rnd_active_animations.values().any(|list| !list.is_empty());

        // フォーカスされたインプットがあり、キャレット点滅が有効な間は描画ループを駆動
        let has_blinking_input = evt_interaction_states
            .focused
            .and_then(|id| cont_input_contents.get(id))
            .is_some_and(|c| c.has_caret && c.is_blink);

        // 一時的表示スクロールバーのフェード進行中は描画更新ループを継続
        let has_active_transient_scrollbar = bar_styles.values().any(|sb_state| {
            sb_state.style.display == ScrollbarDisplay::Transient
                && sb_state
                    .last_scroll_time
                    .is_some_and(|t| t.elapsed() < Duration::from_millis(1500))
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
        state_flag: u128,
        rnd_interaction: &InteractionPropertiesSecondary,
    ) -> bool {
        let Some(interaction) = rnd_interaction.get(id) else {
            return false;
        };

        // 対象となる状態スタイルを取得
        let target_style = match state_flag {
            ComponentMask::STATE_HOVERED => &interaction.hovered,
            ComponentMask::STATE_FOCUSED => &interaction.focused,
            ComponentMask::STATE_FOCUSED_VISIBLE => &interaction.focused_visible,
            ComponentMask::STATE_PRESSED => &interaction.pressed,
            ComponentMask::STATE_DISABLED => &interaction.disabled,
            ComponentMask::STATE_ACTIVED => &interaction.actived,
            ComponentMask::STATE_SELECTED => &interaction.selected,
            ComponentMask::STATE_DRAGGED => &interaction.dragged,
            ComponentMask::STATE_DND_DRAGGING => &interaction.dragging,
            ComponentMask::STATE_DND_DRAG_IN => &interaction.drag_in,
            ComponentMask::STATE_DND_DRAG_OVER => &interaction.drag_over,
            _ => &None,
        };

        // 指定された状態スタイルが存在する場合のみ、内部マスクを検証
        let Some(style) = target_style else {
            return false;
        };

        let mask = style.inner.mask;
        // 基本レイアウト、Flexレイアウト、またはGridレイアウト変更が含まれていれば true
        mask.has_basic_layout()
            || mask.has_flex_layout()
            || mask.has_grid_layout()
            || mask.has(ComponentMask::STYLE_FONT_SIZE)
            || mask.has(ComponentMask::STYLE_AUTO_WRAP)
    }

    pub(crate) fn resolv_focus_style(
        id: EntityId,
        active_mask: &ComponentMask,
        state_flag: u128,
        topo_parents: &ParentsSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
    ) -> Option<ThisStyle> {
        if !active_mask.has(state_flag) {
            return None;
        }
        let self_style = rnd_interaction.get(id).and_then(|interaction| {
            if state_flag == ComponentMask::STATE_FOCUSED_VISIBLE {
                interaction.focused_visible.clone()
            } else {
                interaction.focused.clone()
            }
        });

        if let Some(style) = self_style {
            return Some(style);
        }

        let focus_mode = rnd_visual
            .get(id)
            .and_then(|v| v.focusable)
            .unwrap_or_default();

        let is_trigger_match = match (state_flag, focus_mode) {
            (ComponentMask::STATE_FOCUSED, Focusable::Inherit(_)) => true,
            (ComponentMask::STATE_FOCUSED_VISIBLE, Focusable::Inherit(trigger)) => {
                trigger == FocusTrigger::Keyboard || trigger == FocusTrigger::Both
            }
            _ => false,
        };

        if !is_trigger_match {
            return None;
        }

        // 親先祖を上に辿り、最初に focused 疑似スタイルを定義している要素の設定をそのまま借用する
        let mut curr = *topo_parents.at(id);
        while let Some(curr_id) = curr {
            if let Some(parent_interaction) = rnd_interaction.get(curr_id) {
                let parent_style = if state_flag == ComponentMask::STATE_FOCUSED_VISIBLE {
                    &parent_interaction.focused_visible
                } else {
                    &parent_interaction.focused
                };

                if let Some(p_style) = parent_style {
                    return Some(p_style.clone());
                }
            }
            curr = *topo_parents.at(curr_id);
        }
        None
    }

    #[inline]
    pub(crate) fn cascade_interaction_flag<'a>(
        id: EntityId,
        interaction: &'a InteractionStyles,
        focused_style_resolved: Option<&'a ThisStyle>,
        focused_visible_style_resolved: Option<&'a ThisStyle>,
    ) -> [(u128, Option<&'a ThisStyle>); 11] {
        [
            (ComponentMask::STATE_FOCUSED, focused_style_resolved),
            (
                ComponentMask::STATE_FOCUSED_VISIBLE,
                focused_visible_style_resolved,
            ),
            (ComponentMask::STATE_SELECTED, interaction.selected.as_ref()),
            (ComponentMask::STATE_ACTIVED, interaction.actived.as_ref()),
            (ComponentMask::STATE_HOVERED, interaction.hovered.as_ref()),
            (ComponentMask::STATE_PRESSED, interaction.pressed.as_ref()),
            (ComponentMask::STATE_DISABLED, interaction.disabled.as_ref()),
            (ComponentMask::STATE_DRAGGED, interaction.dragged.as_ref()),
            (
                ComponentMask::STATE_DND_DRAGGING,
                interaction.dragging.as_ref(),
            ),
            (
                ComponentMask::STATE_DND_DRAG_IN,
                interaction.drag_in.as_ref(),
            ),
            (
                ComponentMask::STATE_DND_DRAG_OVER,
                interaction.drag_over.as_ref(),
            ),
        ]
    }

    #[inline]
    pub(crate) fn cascade_within_interaction_flag(
        id: EntityId,
        interaction: &InteractionStyles,
    ) -> [(u128, &Option<ThisStyle>); 10] {
        [
            (ComponentMask::STATE_FOCUSED, &interaction.focused_within),
            (
                ComponentMask::STATE_FOCUSED_VISIBLE,
                &interaction.focused_visible_within,
            ),
            (ComponentMask::STATE_SELECTED, &interaction.selected_within),
            (ComponentMask::STATE_ACTIVED, &interaction.actived_within),
            (ComponentMask::STATE_HOVERED, &interaction.hovered_within),
            (ComponentMask::STATE_PRESSED, &interaction.pressed_within),
            (ComponentMask::STATE_DISABLED, &interaction.disabled_within),
            (ComponentMask::STATE_DRAGGED, &interaction.dragged_within),
            (
                ComponentMask::STATE_DND_DRAGGING,
                &interaction.dragged_within,
            ),
            (
                ComponentMask::STATE_DND_DRAG_IN,
                &interaction.hovered_within,
            ),
        ]
    }

    #[inline]
    pub(crate) fn cascade_parent_interaction_flag(
        id: EntityId,
        interaction: &InteractionStyles,
    ) -> [(u128, &Option<ThisStyle>); 10] {
        [
            (ComponentMask::STATE_FOCUSED, &interaction.focused_parent),
            (
                ComponentMask::STATE_FOCUSED_VISIBLE,
                &interaction.focused_visible_parent,
            ),
            (ComponentMask::STATE_SELECTED, &interaction.selected_parent),
            (ComponentMask::STATE_ACTIVED, &interaction.actived_parent),
            (ComponentMask::STATE_HOVERED, &interaction.hovered_parent),
            (ComponentMask::STATE_PRESSED, &interaction.pressed_parent),
            (ComponentMask::STATE_DISABLED, &interaction.disabled_parent),
            (ComponentMask::STATE_DRAGGED, &interaction.dragged_parent),
            (
                ComponentMask::STATE_DND_DRAGGING,
                &interaction.dragged_parent,
            ),
            (
                ComponentMask::STATE_DND_DRAG_IN,
                &interaction.hovered_parent,
            ),
        ]
    }

    #[inline]
    pub(crate) fn cascade_basic_layout(
        id: EntityId,
        target_layout: &mut BasicLayout,
        active_mask: &ComponentMask,
        rnd_interaction: &InteractionPropertiesSecondary,
    ) {
        let Some(interaction) = rnd_interaction.get(id) else {
            return;
        };

        let cascade = [
            (ComponentMask::STATE_FOCUSED, &interaction.focused),
            (
                ComponentMask::STATE_FOCUSED_VISIBLE,
                &interaction.focused_visible,
            ),
            (ComponentMask::STATE_SELECTED, &interaction.selected),
            (ComponentMask::STATE_ACTIVED, &interaction.actived),
            (ComponentMask::STATE_HOVERED, &interaction.hovered),
            (ComponentMask::STATE_PRESSED, &interaction.pressed),
            (ComponentMask::STATE_DISABLED, &interaction.disabled),
            (ComponentMask::STATE_DRAGGED, &interaction.dragged),
            (ComponentMask::STATE_DND_DRAGGING, &interaction.dragging),
            (ComponentMask::STATE_DND_DRAG_IN, &interaction.drag_in),
            (ComponentMask::STATE_DND_DRAG_OVER, &interaction.drag_over),
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
        target: &mut TargetStyle,
        active_mask: &ComponentMask,
        focused_style_resolved: Option<&ThisStyle>,
        focused_visible_style_resolved: Option<&ThisStyle>,
        rnd_interaction: &InteractionPropertiesSecondary,
    ) {
        let Some(interaction) = rnd_interaction.get(id) else {
            return;
        };

        let cascade = RenderStore::cascade_interaction_flag(
            id,
            interaction,
            focused_style_resolved,
            focused_visible_style_resolved,
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

    #[inline]
    pub(crate) fn cascade_within_interaction(
        id: EntityId,
        target: &mut TargetStyle,
        active_mask: &ComponentMask,
        topo_entities: &EntitiesSlot,
        topo_active_masks: &ActiveMasksSecondary,
        topo_children: &ChildrenSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
    ) {
        if !active_mask.has(ComponentMask::STYLE_INTERACTION_WITHIN) {
            return;
        }

        let Some(interaction) = rnd_interaction.get(id) else {
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
                id,
                state,
                topo_entities,
                topo_active_masks,
                topo_children,
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
            id,
            ComponentMask::STYLE_ACTIVE_INTERACTION_PROPERTY,
            topo_entities,
            topo_active_masks,
            topo_children,
        );

        if !any_state {
            return;
        }

        TargetStyle::apply_visual_property(target, &style.inner.visual_property, style.inner.mask);
    }

    #[inline]
    pub(crate) fn cascade_parent_interaction(
        id: EntityId,
        target: &mut TargetStyle,
        active_mask: &ComponentMask,
        topo_entities: &EntitiesSlot,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
    ) {
        if !active_mask.has(ComponentMask::STYLE_INTERACTION_PARENT) {
            return;
        }
        let Some(interaction) = rnd_interaction.get(id) else {
            return;
        };

        let cascade_parent = RenderStore::cascade_parent_interaction_flag(id, interaction);

        for (state, style_opt) in cascade_parent {
            let Some(style) = style_opt else {
                continue;
            };

            // 直近の親要素がこの state_flag を満たしているか
            let with_state = TopologyStore::has_parent_with_state(
                id,
                state,
                topo_entities,
                topo_active_masks,
                topo_parents,
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

        let Some(ref style) = interaction.any_parent else {
            return;
        };
        let any_state = TopologyStore::has_parent_with_state(
            id,
            ComponentMask::STYLE_ACTIVE_INTERACTION_PROPERTY,
            topo_entities,
            topo_active_masks,
            topo_parents,
        );

        if !any_state {
            return;
        }

        TargetStyle::apply_visual_property(target, &style.inner.visual_property, style.inner.mask);
    }

    #[inline]
    pub(crate) fn apply_interaction_cascades(
        id: EntityId,
        target: &mut TargetStyle,
        active_mask: &ComponentMask,
        focused_style_resolved: Option<&ThisStyle>,
        focused_visible_style_resolved: Option<&ThisStyle>,
        topo_entities: &EntitiesSlot,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
    ) {
        RenderStore::cascade_interaction(
            id,
            target,
            active_mask,
            focused_style_resolved,
            focused_visible_style_resolved,
            rnd_interaction,
        );
        RenderStore::cascade_parent_interaction(
            id,
            target,
            active_mask,
            topo_entities,
            topo_active_masks,
            topo_parents,
            topo_children,
            rnd_interaction,
        );
        RenderStore::cascade_within_interaction(
            id,
            target,
            active_mask,
            topo_entities,
            topo_active_masks,
            topo_children,
            rnd_interaction,
        );
    }

    /// 補間されたアニメーション値を `SoA` のアクティブプロパティへ安全に上書きします
    pub(crate) fn apply_animation_value(
        id: EntityId,
        property: PropertyList,
        value: &TransitionValue,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        rnd_visual: &mut VisualPropertiesSecondary,
    ) {
        if !rnd_visual.contains_key(id) {
            rnd_visual.insert(id, VisualProperty::default());
        }
        let v = rnd_visual.at_mut(id);

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
                // 書き込み先は絶対にあるはず
                let layout = lay_basic.at_mut(id);
                layout.size.width = Val::Px(w);
                is_layout_dirty = true;
            }
            TransitionValue::Height(h) => {
                // 書き込み先は絶対にあるはず
                let layout = lay_basic.at_mut(id);
                layout.size.height = Val::Px(h);
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
                topo_active_masks,
                topo_parents,
                lay_dirty_entities,
                lay_taffy_tree,
                lay_taffy_nodes,
            );
        }
    }

    fn trigger_keyframe_animations_if_needed(
        id: EntityId,
        rnd_active_animations: &mut ActiveAnimationsSparse,
        rnd_visual: &VisualPropertiesSecondary,
    ) {
        let Some(visual) = rnd_visual.get(id) else {
            return;
        };
        if visual.keyframe_animations.is_empty() {
            return;
        }

        let now = Instant::now();

        if !rnd_active_animations.contains_key(id) {
            rnd_active_animations.insert(id, Vec::new());
        }
        // 上で入れたばっかなので Some のはず
        let active_list = rnd_active_animations.at_mut(id);

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

    /// 状態の変更を検知しアニメーションが必要な箇所を自動的に開始・制御
    pub(crate) fn resolve_element_style_state(
        id: EntityId,
        allow_transition: bool,
        win_last_size: Option<LayoutSize>,
        sys_text_buffers: &TextBufferSparse,
        react_element_effects: &ElementEffectsSecondary,
        cont_input_contents: &InputContentsSparse,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_is_sort_dirty: &mut bool,
        topo_entities: &EntitiesSlot,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_base_basic: &BaseBasicLayoutsSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_active_transitions: &mut ActiveTransitionsSparse,
        rnd_active_animations: &mut ActiveAnimationsSparse,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        out_rects: &RectsSecondary,
    ) {
        RenderStore::resolve_visual_styles(
            id,
            allow_transition,
            sys_text_buffers,
            react_element_effects,
            cont_input_contents,
            topo_active_masks,
            topo_is_sort_dirty,
            topo_entities,
            topo_parents,
            topo_children,
            lay_dirty_entities,
            lay_taffy_tree,
            lay_taffy_nodes,
            rnd_dirty_entities,
            rnd_visual,
            rnd_active_transitions,
            rnd_base_visual,
            rnd_interaction,
        );

        RenderStore::resolve_layout_styles(
            id,
            allow_transition,
            win_last_size,
            react_element_effects,
            topo_active_masks,
            topo_parents,
            lay_dirty_entities,
            lay_taffy_tree,
            lay_basic,
            lay_taffy_nodes,
            lay_base_basic,
            rnd_active_transitions,
            rnd_visual,
            rnd_base_visual,
            rnd_interaction,
            out_rects,
        );

        // スタイル解決が完了した結果、自身に新しくキーフレームアニメーション定義が
        // 読み込まれていれば、自動的にそのアニメーションの再生を開始する
        RenderStore::trigger_keyframe_animations_if_needed(id, rnd_active_animations, rnd_visual);
    }

    fn resolve_layout_styles(
        id: EntityId,
        allow_transition: bool,
        win_last_size: Option<LayoutSize>,
        react_element_effects: &ElementEffectsSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_base_basic: &BaseBasicLayoutsSecondary,
        rnd_active_transitions: &mut ActiveTransitionsSparse,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        out_rects: &RectsSecondary,
    ) {
        let active_mask = topo_active_masks.at(id);
        let has_base_layout = lay_base_basic.contains_key(id);
        let has_active_layout = lay_basic.contains_key(id);

        if !has_base_layout && !has_active_layout {
            return;
        }

        let active_layout = lay_basic.get_or(id, &DEFAULT_BASIC);
        let base_layout = lay_base_basic.get_or_default(id);
        let mut target_layout = base_layout;

        RenderStore::cascade_basic_layout(id, &mut target_layout, active_mask, rnd_interaction);

        let to_px = |val, is_width| {
            OutputStore::val_to_px(id, val, is_width, win_last_size, topo_parents, out_rects)
        };

        let target_w = to_px(target_layout.size.width, true);
        let current_w = to_px(active_layout.size.width, true);
        let target_h = to_px(target_layout.size.height, false);
        let current_h = to_px(active_layout.size.height, false);

        let mut width_triggered = false;
        let mut height_triggered = false;

        let mut if_needed = |prop, start, end| {
            RenderStore::trigger_transition_if_needed(
                id,
                prop,
                start,
                end,
                react_element_effects,
                rnd_active_transitions,
                rnd_base_visual,
            )
        };

        let can_trigger_width = rnd_visual.get(id).is_some_and(|v| {
            v.transitions.iter().any(|t| {
                t.property_list == PropertyList::Width || t.property_list == PropertyList::Size
            })
        });

        if allow_transition
            && can_trigger_width
            && has_active_layout
            && let (Some(cw), Some(tw)) = (current_w, target_w)
            && (cw - tw).abs() > 0.01
        {
            width_triggered = if_needed(
                PropertyList::Width,
                TransitionValue::Width(cw),
                TransitionValue::Width(tw),
            );
        }

        let can_trigger_height = rnd_visual.get(id).is_some_and(|v| {
            v.transitions.iter().any(|t| {
                t.property_list == PropertyList::Height || t.property_list == PropertyList::Size
            })
        });

        if allow_transition
            && can_trigger_height
            && has_active_layout
            && let (Some(ch), Some(th)) = (current_h, target_h)
            && (ch - th).abs() > 0.01
        {
            height_triggered = if_needed(
                PropertyList::Height,
                TransitionValue::Height(ch),
                TransitionValue::Height(th),
            );
        }

        if !lay_basic.contains_key(id) {
            lay_basic.insert(id, BasicLayout::default());
        }
        let active_layout_mut = lay_basic.at_mut(id);

        // 解決後の target_layout と 現在の active_layout が異なっているか
        let is_layout_changed = *active_layout_mut != target_layout;

        *active_layout_mut = target_layout;

        if width_triggered {
            active_layout_mut.size.width = Val::Px(current_w.unwrap());
        }
        if height_triggered {
            active_layout_mut.size.height = Val::Px(current_h.unwrap());
        }

        if is_layout_changed {
            LayoutStore::mark_layout_dirty(
                id,
                topo_active_masks,
                topo_parents,
                lay_dirty_entities,
                lay_taffy_tree,
                lay_taffy_nodes,
            );
        }
    }

    fn resolve_visual_styles(
        id: EntityId,
        allow_transition: bool,
        sys_text_buffers: &TextBufferSparse,
        react_element_effects: &ElementEffectsSecondary,
        cont_input_contents: &InputContentsSparse,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_is_sort_dirty: &mut bool,
        topo_entities: &EntitiesSlot,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_active_transitions: &mut ActiveTransitionsSparse,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
    ) {
        let active_mask = topo_active_masks.at(id);
        let has_base_visual = rnd_base_visual.contains_key(id);
        let has_active_visual = rnd_visual.contains_key(id);
        let has_interaction_styles = rnd_interaction.contains_key(id);

        if !has_base_visual && !has_active_visual && !has_interaction_styles {
            return;
        }

        let current = RenderStore::get_current_style(id, rnd_visual);
        let mut target = RenderStore::get_target_style(id, rnd_base_visual);

        let resolv_focus = |flag| {
            RenderStore::resolv_focus_style(
                id,
                active_mask,
                flag,
                topo_parents,
                rnd_visual,
                rnd_interaction,
            )
        };

        let focused_style_resolved = resolv_focus(ComponentMask::STATE_FOCUSED);
        let focused_visible_style_resolved = resolv_focus(ComponentMask::STATE_FOCUSED_VISIBLE);

        RenderStore::apply_interaction_cascades(
            id,
            &mut target,
            active_mask,
            focused_style_resolved.as_ref(),
            focused_visible_style_resolved.as_ref(),
            topo_entities,
            topo_active_masks,
            topo_parents,
            topo_children,
            rnd_interaction,
        );

        let mut is_placeholder_active = false;
        if let Some(contents) = cont_input_contents.get(id) {
            let has_no_ime = contents
                .ime_state
                .as_ref()
                .is_none_or(|s| s.composition_text.is_empty());
            if contents.text_empty() && has_no_ime {
                is_placeholder_active = true;
            }
        }

        // 変更評価の計算
        let target_bg_val = target.bg_color.unwrap_or_default();
        let bg_changed = current.bg_color != target_bg_val;

        let target_border_val = target.border_color.unwrap_or_default();
        let border_changed = current.border_color != target_border_val;

        let target_outline_width_val = target.outline_width.unwrap_or_default();
        let outline_width_changed = current.outline_width != target_outline_width_val;

        let target_outline_color_val = target.outline_color.unwrap_or_default();
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

        let target_radius_val = target.corner_radius.unwrap_or_default();
        let radius_changed = current.corner_radius != target_radius_val;

        let target_shadow_val = target.shadow_params.unwrap_or_default();
        let shadow_changed = current.shadow_params != target_shadow_val;

        let target_text_color = target.text_color.unwrap_or(Color::WHITE);
        let text_color_changed = current.text_color != target_text_color;

        let target_font_size = target.font.size.unwrap_or(16.0);
        let target_font_family = target.font.family.clone();
        let target_font_weight = target.font.weight.unwrap_or(400);
        let target_font_style = target.font.style.unwrap_or(0);
        let target_auto_wrap = target.auto_wrap.unwrap_or(false);

        let font_changed = (current.font_size - target_font_size).abs() >= 0.01
            || current.font_family != target_font_family
            || current.font_weight != target_font_weight
            || current.font_style != target_font_style
            || current.auto_wrap != target_auto_wrap;

        let target_pointer_events = target.pointer_events.unwrap_or_default();
        let pointer_events_changed = current.pointer_events != target_pointer_events;

        // トランジション判定
        let mut if_needed = |prop, start, end| {
            RenderStore::trigger_transition_if_needed(
                id,
                prop,
                start,
                end,
                react_element_effects,
                rnd_active_transitions,
                rnd_base_visual,
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
            || text_color_changed
            || font_changed
            || pointer_events_changed
            || rnd_base_visual.contains_key(id)
        {
            if !rnd_visual.contains_key(id) {
                rnd_visual.insert(id, VisualProperty::default());
            }
            let active_vis = rnd_visual.at_mut(id);

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

            active_vis.font.clone_from(&target.font);
            active_vis.auto_wrap = target.auto_wrap;

            active_vis.pointer_events = target.pointer_events;

            if let Some(base_vis) = rnd_base_visual.get(id) {
                active_vis.z_index = base_vis.z_index;
                active_vis.backdrop = base_vis.backdrop;
                active_vis.bg_gradient = base_vis.bg_gradient;
                active_vis.transitions.clone_from(&base_vis.transitions);
                active_vis
                    .keyframe_animations
                    .clone_from(&base_vis.keyframe_animations);
                active_vis.focusable = base_vis.focusable;
                active_vis.prevent_focus_steal = base_vis.prevent_focus_steal;
                active_vis.prevent_focus_steal_within = base_vis.prevent_focus_steal_within;
                active_vis.transform_inherit = target.transform_inherit;
                active_vis.user_select = base_vis.user_select;
            }

            if font_changed {
                SystemStore::clear_layout_cache(id, sys_text_buffers);
                LayoutStore::mark_layout_dirty(
                    id,
                    topo_active_masks,
                    topo_parents,
                    lay_dirty_entities,
                    lay_taffy_tree,
                    lay_taffy_nodes,
                );
            }

            if transform_changed || transform_origin_changed {
                *topo_is_sort_dirty = true;
            }

            RenderStore::mark_render_dirty(id, topo_active_masks, rnd_dirty_entities);
        }
    }

    /// 必要に応じてトランジションを起動、または上書き（逆再生含む）します
    pub(crate) fn trigger_transition_if_needed(
        id: EntityId,
        property_list: PropertyList,
        start_value: TransitionValue,
        end_value: TransitionValue,
        react_element_effects: &ElementEffectsSecondary,
        rnd_active_transitions: &mut ActiveTransitionsSparse,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
    ) -> bool {
        // スタイルの再評価エフェクトの実行中であるか
        let is_style_evaluating = crate::signal::ACTIVE_EFFECT.with(|cell| {
            if let Some(effect_id) = cell.get() {
                // 現在走っているエフェクトがいずれかの要素の StyleCategory::Style のものであるか走査
                react_element_effects.values().any(|list| {
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

        let Some(visual) = rnd_base_visual.get(id) else {
            return false;
        };

        let Some(t) = visual
            .transitions
            .iter()
            .find(|t| t.property_list == property_list || t.property_list == PropertyList::Size)
        else {
            return false;
        };

        let Some(entry) = rnd_active_transitions.entry(id) else {
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
                .map_or(Duration::ZERO, |st| now.duration_since(st));
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

    /// 現在ホバーされている要素から親ツリーを遡り、適用するべき物理的な `CursorIcon` を正確に解決します。
    pub(crate) fn resolve_cursor(
        hovered_id: EntityId,
        evt_interaction_states: &ActiveInteractionStates,
        topo_parents: &ParentsSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
    ) -> CursorIcon {
        // 現在プレス中の要素（pressed）があればそれを最優先で探索の基点にする
        let start_id = evt_interaction_states.pressed.unwrap_or(hovered_id);

        let mut curr = Some(start_id);
        let mut global_cursor = None;

        while let Some(id) = curr {
            let cursor_opt = rnd_visual
                .get(id)
                .and_then(|v| v.cursor)
                .or_else(|| rnd_base_visual.get(id).and_then(|v| v.cursor));

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
            curr = *topo_parents.at(id);
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

    /// 指定された要素の実効トランスフォームを親に遡りながら動的に解決
    pub(crate) fn resolve_effective_transform(
        id: EntityId,
        topo_parents: &ParentsSecondary,
        rnd_visual: &VisualPropertiesSecondary,
    ) -> [[f32; 4]; 4] {
        let mut curr = Some(id);
        let mut result = IDENTITY_MATRIX;

        while let Some(curr_id) = curr {
            let (self_transform, transform_inherit) = match rnd_visual.get(curr_id) {
                Some(v) => (
                    v.transform.unwrap_or(IDENTITY_MATRIX),
                    v.transform_inherit.unwrap_or(false),
                ),
                None => (IDENTITY_MATRIX, false),
            };

            // 子から親へ遡るため、親の行列を左から掛けていくことで、
            // Column-Major におけるカスケード順序（親の累積 * 自身）と数学的に一致
            result = OutputStore::mul_4x4(&self_transform, &result);

            // 継承設定があり親が存在する場合のみ上に遡る
            if transform_inherit {
                curr = *topo_parents.at(curr_id);
            } else {
                break;
            }
        }
        result
    }

    #[inline]
    pub(crate) fn get_transform_and_origin(
        id: EntityId,
        visual: &VisualProperty,
        full_transform: [[f32; 4]; 4],
    ) -> ([[f32; 4]; 3], [f32; 2]) {
        let packed_transform = [
            full_transform[0], // X軸基底
            full_transform[1], // Y軸基底
            full_transform[3], // 平行移動部
        ];
        let origin = visual.transform_origin.map_or([0.5, 0.5], |p| [p.x, p.y]);
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

    /// 毎フレームの描画前に呼び出され、すべてのアクティブなキーフレームアニメーションを 1 Tick 進めます
    pub(crate) fn tick_animations(
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_is_sort_dirty: &mut bool,
        topo_parents: &ParentsSecondary,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_active_animations: &mut ActiveAnimationsSparse,
    ) {
        let now = Instant::now();

        rnd_active_animations.retain(|id, animations| {
            animations.retain_mut(|anim| {
                let elapsed = now.duration_since(anim.start_time);
                let elapsed_secs = elapsed.as_secs_f32();
                let duration_secs = anim.duration.as_secs_f32();

                // 現在の周回回数
                let current_iteration = (elapsed_secs / duration_secs).floor() as u32;

                // ループ制限に達しているかチェック
                let is_finished = match anim.iteration_count {
                    PlaybackCount::Count(max_count) => current_iteration >= max_count,
                    PlaybackCount::Infinite => false,
                };

                if is_finished {
                    // ループ終了：目標の最終値で固定してアニメーションを破棄
                    RenderStore::apply_animation_value(
                        id,
                        anim.property,
                        &anim.end_value,
                        topo_active_masks,
                        topo_parents,
                        lay_dirty_entities,
                        lay_taffy_tree,
                        lay_basic,
                        lay_taffy_nodes,
                        rnd_visual,
                    );
                    return false;
                }

                // 現在のループ内での正規化進行度
                let local_time = elapsed_secs % duration_secs;
                let progress = if duration_secs > 0.0 {
                    (local_time / duration_secs).min(1.0)
                } else {
                    1.0
                };
                let eased_t = anim.curve.evaluate(progress);

                // 値の補間
                let current_val = anim.start_value.lerp(&anim.end_value, eased_t);

                // 補間された動的スタイル値を書き戻し
                RenderStore::apply_animation_value(
                    id,
                    anim.property,
                    &current_val,
                    topo_active_masks,
                    topo_parents,
                    lay_dirty_entities,
                    lay_taffy_tree,
                    lay_basic,
                    lay_taffy_nodes,
                    rnd_visual,
                );

                if anim.property == PropertyList::Transform {
                    *topo_is_sort_dirty = true;
                }

                RenderStore::mark_render_dirty(id, topo_active_masks, rnd_dirty_entities);
                true // 継続して保持
            });

            // // アニメーションが空の要素はマップごと削除
            !animations.is_empty()
        });
    }

    /// 毎フレームの描画前に呼び出され、すべてのアクティブなトランジションを 1 Tick 進めます
    pub(crate) fn tick_transitions(
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_is_sort_dirty: &mut bool,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_active_transitions: &mut ActiveTransitionsSparse,
        rnd_last_tick_time: &mut Option<Instant>,
    ) {
        const FRAME_TIME_120FPS: Duration = Duration::from_nanos(8_333_333);
        let now = Instant::now();

        // (1.0 / 120.0 秒 = 約 8,333,333 ナノ秒)
        if let Some(last) = rnd_last_tick_time
            && now.duration_since(*last) < FRAME_TIME_120FPS
        {
            return;
        }

        // 実行制限を通過したため、基準時刻を更新して処理を継続
        *rnd_last_tick_time = Some(now);
        rnd_active_transitions.retain(|id, transitions| {
            transitions.retain_mut(|t_state| {
                // start_time が None なら、このフレームの時刻 now を格納しその値を取り出す。
                let start_time = *t_state.start_time.get_or_insert(now);
                let elapsed = now.duration_since(start_time);

                // 進行度 (0.0 ～ 1.0)
                let progress = (elapsed.as_secs_f32() / t_state.duration.as_secs_f32()).min(1.0);
                let eased_t = t_state.curve.evaluate(progress);

                // Lerpによる新しい値の決定
                let current_val = t_state.start_value.lerp(&t_state.end_value, eased_t);

                // 補間された値を書き戻す
                match current_val {
                    TransitionValue::Color(c) => {
                        if let Some(v) = rnd_visual.get_mut(id) {
                            if t_state.property_list == PropertyList::BackgroundColor {
                                v.bg_color = Some(c);
                            } else if t_state.property_list == PropertyList::BorderColor {
                                v.border_color = Some(c);
                            }
                        }
                        RenderStore::mark_render_dirty(id, topo_active_masks, rnd_dirty_entities);
                    }
                    TransitionValue::Opacity(o) => {
                        if let Some(v) = rnd_visual.get_mut(id) {
                            v.opacity = Some(o);
                        }
                        RenderStore::mark_render_dirty(id, topo_active_masks, rnd_dirty_entities);
                    }
                    TransitionValue::Transform(m) => {
                        if let Some(v) = rnd_visual.get_mut(id) {
                            v.transform = Some(m);
                        }
                        // トランスフォームが実際に動いているためソート（カリング判定）を汚染
                        *topo_is_sort_dirty = true;
                        RenderStore::mark_render_dirty(id, topo_active_masks, rnd_dirty_entities);
                    }
                    TransitionValue::CornerRadius(cr) => {
                        if let Some(v) = rnd_visual.get_mut(id) {
                            v.corner_radius = Some(cr);
                        }
                        RenderStore::mark_render_dirty(id, topo_active_masks, rnd_dirty_entities);
                    }
                    TransitionValue::Width(w) => {
                        let layout = lay_basic.at_mut(id);
                        layout.size.width = Val::Px(w); // ピクセル値で上書き
                        LayoutStore::mark_layout_dirty(
                            id,
                            topo_active_masks,
                            topo_parents,
                            lay_dirty_entities,
                            lay_taffy_tree,
                            lay_taffy_nodes,
                        );
                    }
                    // 縦幅（Height）の毎フレームアニメーション補間
                    TransitionValue::Height(h) => {
                        let layout = lay_basic.at_mut(id);
                        layout.size.height = Val::Px(h);
                        LayoutStore::mark_layout_dirty(
                            id,
                            topo_active_masks,
                            topo_parents,
                            lay_dirty_entities,
                            lay_taffy_tree,
                            lay_taffy_nodes,
                        );
                    }
                    // 影（BoxShadow）の毎フレームの書き戻し処理
                    TransitionValue::BoxShadow(shadow) => {
                        if let Some(v) = rnd_visual.get_mut(id) {
                            v.shadow_params = Some(shadow);
                            v.shadow_color = Some(shadow.color);
                        }
                        RenderStore::mark_render_dirty(id, topo_active_masks, rnd_dirty_entities);
                    }
                }

                // アニメーション完了判定 (1.0未満なら継続=true, 1.0に達したら削除=false)
                progress < 1.0
            });

            // トランジションが空になった要素はマップごと削除
            !transitions.is_empty()
        });
    }
}

#[derive(Debug, Clone)]
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
    pub(crate) text_color: Color,
    pub(crate) font_size: f32,
    pub(crate) font_family: Option<Cow<'static, str>>,
    pub(crate) font_weight: u32,
    pub(crate) font_style: u32,
    pub(crate) auto_wrap: bool,
    pub(crate) pointer_events: PointerEvents,
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
            text_color: Color::WHITE,
            font_size: 16.0,
            font_family: None,
            font_weight: 400,
            font_style: 0,
            auto_wrap: false,
            pointer_events: PointerEvents::default(),
        }
    }
}
impl RenderStore {
    /// 現在の描画用データを取得 (Copy可能なプリミティブのみ)
    #[inline]
    pub(crate) fn get_current_style(
        id: EntityId,
        rnd_visual: &VisualPropertiesSecondary,
    ) -> CurrentStyle {
        rnd_visual
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
                text_color: v.text_color.unwrap_or(Color::WHITE),
                font_size: v.font.size.unwrap_or(16.0),
                font_family: v.font.family.clone(),
                font_weight: v.font.weight.unwrap_or(400),
                font_style: v.font.style.unwrap_or(0),
                auto_wrap: v.auto_wrap.unwrap_or(false),
                pointer_events: v.pointer_events.unwrap_or_default(),
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
    pub(crate) font: FontDate,
    pub(crate) auto_wrap: Option<bool>,
}

impl RenderStore {
    /// 目標値を参照経由で構築
    #[inline]
    pub(crate) fn get_target_style(
        id: EntityId,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
    ) -> TargetStyle {
        rnd_base_visual
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
                font: v.font.clone(),
                auto_wrap: v.auto_wrap,
            })
            .unwrap_or_default()
    }
}

impl TargetStyle {
    /// 指定された `VisualProperty` と `ComponentMask` を基に自身のスタイルをマージ。
    pub(crate) fn apply_visual_property(
        target: &mut TargetStyle,
        inner_vis: &VisualProperty,
        inner_mask: ComponentMask,
    ) {
        if inner_mask.has(ComponentMask::STYLE_BG_COLOR) {
            target.bg_color = inner_vis.bg_color;
        }
        if inner_mask.has(ComponentMask::STYLE_BORDER_COLOR) {
            target.border_color = inner_vis.border_color;
        }
        if inner_mask.has(ComponentMask::STYLE_OPACITY) {
            target.opacity = inner_vis.opacity;
        }
        if inner_mask.has(ComponentMask::STYLE_TRANSFORM) {
            target.transform = inner_vis.transform;
            target.transform_origin = inner_vis.transform_origin;
        }

        if inner_mask.has(ComponentMask::STYLE_TRANSFORM_INHERIT) {
            target.transform_inherit = inner_vis.transform_inherit;
        }
        if inner_mask.has(ComponentMask::STYLE_CORNER_RADIUS) {
            target.corner_radius = inner_vis.corner_radius;
        }
        if inner_mask.has(ComponentMask::STYLE_POINTER_EVENTS) {
            target.pointer_events = inner_vis.pointer_events;
        }
        if inner_mask.has(ComponentMask::STYLE_BOX_SHADOW) {
            if inner_vis.shadow_params.is_some() {
                target.shadow_params = inner_vis.shadow_params;
            }
            if inner_vis.shadow_color.is_some() {
                target.shadow_color = inner_vis.shadow_color;
            }
        }
        if inner_mask.has(ComponentMask::STYLE_TEXT_COLOR) {
            target.text_color = inner_vis.text_color;
        }
        if inner_mask.has(ComponentMask::STYLE_USER_SELECT) {
            if inner_vis.select_bg_color.is_some() {
                target.select_bg_color = inner_vis.select_bg_color;
            }
            if inner_vis.select_text_color.is_some() {
                target.select_text_color = inner_vis.select_text_color;
            }
        }
        if inner_mask.has(ComponentMask::STYLE_BORDER) {
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
        if inner_mask.has(ComponentMask::STYLE_OUTLINE) {
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
        if inner_mask.has(ComponentMask::STYLE_CURSOR) {
            target.cursor = inner_vis.cursor;
        }
        if inner_mask.has(ComponentMask::STYLE_RESIZABLE) {
            target.resizable_cursor = inner_vis.resizable_cursor;
        }
        if inner_mask.has(ComponentMask::STYLE_FONT_SIZE) {
            target.font.size = inner_vis.font.size;
        }
        if inner_mask.has(ComponentMask::STYLE_FONT_STYLE) {
            if inner_vis.font.family.is_some() {
                target.font.family.clone_from(&inner_vis.font.family);
            }
            if inner_vis.font.weight.is_some() {
                target.font.weight = inner_vis.font.weight;
            }
            if inner_vis.font.style.is_some() {
                target.font.style = inner_vis.font.style;
            }
        }
        if inner_mask.has(ComponentMask::STYLE_AUTO_WRAP) {
            target.auto_wrap = inner_vis.auto_wrap;
        }
    }
}

impl Context {
    #[inline]
    pub(crate) fn mark_render_dirty(&mut self, id: EntityId) {
        RenderStore::mark_render_dirty(
            id,
            &mut self.topology.topo_active_masks,
            &mut self.renders.rnd_dirty_entities,
        );
    }

    #[inline]
    pub(crate) fn get_visual_property_mut(
        &mut self,
        id: EntityId,
        target: StyleTarget,
    ) -> &mut VisualProperty {
        RenderStore::get_visual_property_mut(
            id,
            target,
            &mut self.renders.rnd_base_visual,
            &mut self.renders.rnd_interaction,
        )
    }

    /// 状態の変更を検知し、アニメーション（トランジション）が必要な箇所を自動的に開始・制御します。
    #[inline]
    pub(crate) fn resolve_element_style_state(&mut self, id: EntityId, allow_transition: bool) {
        RenderStore::resolve_element_style_state(
            id,
            allow_transition,
            self.window.win_last_size,
            &self.system.sys_text_buffers,
            &self.reactive.react_element_effects,
            &self.contents.cont_input_contents,
            &mut self.topology.topo_active_masks,
            &mut self.topology.topo_is_sort_dirty,
            &self.topology.topo_entities,
            &self.topology.topo_parents,
            &self.topology.topo_children,
            &mut self.layouts.lay_dirty_entities,
            &mut self.layouts.lay_taffy_tree,
            &mut self.layouts.lay_basic,
            &self.layouts.lay_taffy_nodes,
            &self.layouts.lay_base_basic,
            &mut self.renders.rnd_dirty_entities,
            &mut self.renders.rnd_visual,
            &mut self.renders.rnd_active_transitions,
            &mut self.renders.rnd_active_animations,
            &self.renders.rnd_base_visual,
            &self.renders.rnd_interaction,
            &self.outputs.out_rects,
        );
    }
}

#[cfg(test)]
mod tests;
