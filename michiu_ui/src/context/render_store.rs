use crate::{
    ActiveEntitiesVec, ActiveMasksSecondary, ActiveTransition, AnimationCurve,
    BaseBasicLayoutsSecondary, BasicLayout, BasicLayoutsSecondary, BorderAlignment, BorderStyle,
    BoxShadow, ChildrenSecondary, ClipRectsSecondary, Color, ComponentMask, ContentStore, Context,
    CornerRadius, CursorIcon, DirtyLayoutEntitiesVec, Display, EdgeInsets, EffectCategory,
    EffectId, EffectiveTransformsSecondary, ElementEffectsSecondary, EntitiesSlot, EntityId,
    FlatDfsSequenceVec, FocusTrigger, Focusable, GlobalCursorIcon, IDENTITY_MATRIX,
    InputContentsSparseSecondary, InteractionStates, InteractionStyles, LayoutPoint, LayoutSize,
    LayoutStore, OutputStore, ParentsSecondary, PlaybackCount, Point, PointerEvents, PropertyList,
    ReactiveStore, RectsSecondary, STATE_ACTIVED, STATE_DISABLED, STATE_DND_DRAG_IN,
    STATE_DND_DRAG_OVER, STATE_DND_DRAGGING, STATE_DRAGGED, STATE_FOCUSED, STATE_FOCUSED_VISIBLE,
    STATE_HOVERED, STATE_PRESSED, STATE_QUEUED_LAYOUT, STATE_QUEUED_RENDER, STATE_SELECTED,
    STYLE_ACTIVE_INTERACTION_PROPERTY, STYLE_BG_COLOR, STYLE_BORDER, STYLE_BORDER_COLOR,
    STYLE_BOX_SHADOW, STYLE_CORNER_RADIUS, STYLE_CURSOR, STYLE_EXT_PROPERTIES, STYLE_FONT_SIZE,
    STYLE_INTERACTION_PARENT, STYLE_INTERACTION_WITHIN, STYLE_OPACITY, STYLE_OUTLINE,
    STYLE_POINTER_EVENTS, STYLE_RESIZABLE, STYLE_TEXT_COLOR, STYLE_TRANSFORM,
    STYLE_TRANSFORM_INHERIT, STYLE_USER_SELECT, ScrollbarDisplay, ScrollbarStylesSecondary,
    StyleTarget, TaffyNodesSecondary, TaffyTreeEntityId, ThisStyle, TopologyStore, TransitionValue,
    Val, VisualProperty, WindowStore,
};
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

#[allow(clippy::struct_field_names)]
pub struct RenderStore {
    pub(crate) ren_visual: VisualPropertiesSecondary,
    pub(crate) ren_interaction: InteractionPropertiesSecondary,
    pub(crate) ren_base_visual: BaseVisualPropertiesSecondary,
    pub(crate) ren_dirty_entities: DirtyRenderEntitiesVec,
    pub(crate) ren_active_transitions: ActiveTransitionsSparseSecondary,
    pub(crate) ren_active_animations: ActiveAnimationsSparseSecondary,
    pub(crate) ren_active_webviews: ActiveWebviewsHashSet,
    pub(crate) ren_last_tick_time: Option<Instant>,
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
            ren_visual: SecondaryMap::new(),
            ren_interaction: SecondaryMap::new(),
            ren_base_visual: SecondaryMap::new(),
            ren_dirty_entities: Vec::new(),
            ren_active_transitions: SparseSecondaryMap::new(),
            ren_active_animations: SparseSecondaryMap::new(),
            ren_active_webviews: HashSet::new(),
            ren_last_tick_time: None,
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.ren_visual.clear();
        self.ren_interaction.clear();
        self.ren_base_visual.clear();
        self.ren_dirty_entities.clear();
        self.ren_active_transitions.clear();
        self.ren_active_animations.clear();
        self.ren_active_webviews.clear();
        self.ren_last_tick_time = None;
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.ren_visual.remove(id);
        self.ren_interaction.remove(id);
        self.ren_base_visual.remove(id);
        self.ren_dirty_entities.retain(|&x| x != id);
        self.ren_active_transitions.remove(id);
        self.ren_active_animations.remove(id);
        self.ren_active_webviews.remove(&id);
    }
}

impl RenderStore {
    #[inline]
    pub(crate) fn mark_render_dirty(
        id: EntityId,
        topo_active_masks: &mut ActiveMasksSecondary,
        ren_dirty_entities: &mut DirtyRenderEntitiesVec,
    ) {
        let Some(mask) = topo_active_masks.get_mut(id) else {
            return;
        };
        if mask.has(STATE_QUEUED_RENDER) {
            return;
        }

        mask.set(STATE_QUEUED_RENDER);
        ren_dirty_entities.push(id);
    }

    /// 描画（レンダー）ダーティ状態として登録された要素をすべてクリアします。
    pub(crate) fn clear_render_dirty(
        topo_active_masks: &mut ActiveMasksSecondary,
        ren_dirty_entities: &mut DirtyRenderEntitiesVec,
    ) {
        for id in ren_dirty_entities.drain(..) {
            let Some(mask) = topo_active_masks.get_mut(id) else {
                continue;
            };
            mask.unset(STATE_QUEUED_RENDER);
        }
        ren_dirty_entities.clear();
    }

    #[inline]
    pub(crate) fn get_font_propery(
        id: EntityId,
        ren_visual: &VisualPropertiesSecondary,
    ) -> (f32, Option<&str>, Option<u32>, Option<u32>) {
        ren_visual.get(id).map_or((16.0, None, None, None), |v| {
            (
                v.font_size.unwrap_or(16.0),
                v.font_family.as_deref(),
                v.font_weight,
                v.font_style,
            )
        })
    }

    pub(crate) fn get_visual_property_mut<'a>(
        id: EntityId,
        target: StyleTarget,
        ren_base_visual: &'a mut BaseVisualPropertiesSecondary,
        ren_interaction: &'a mut InteractionPropertiesSecondary,
    ) -> Option<&'a mut VisualProperty> {
        if target == StyleTarget::Base {
            ren_base_visual.get_mut(id)
        } else {
            if !ren_interaction.contains_key(id) {
                ren_interaction.insert(id, InteractionStyles::default());
            }
            let styles = ren_interaction.get_mut(id)?;
            let style_ref = styles.get_style_target_mut(target);
            Some(&mut Arc::make_mut(&mut style_ref.inner).visual_property)
        }
    }

    /// スクロールバー用要素の不透明度（解決値と静的ベース値）を同時同期して更新します。
    #[inline]
    pub(crate) fn update_scrollbar_element_opacity(
        id: EntityId,
        opacity: f32,
        ren_visual: &mut VisualPropertiesSecondary,
        ren_base_visual: &mut BaseVisualPropertiesSecondary,
    ) {
        let visuals = [ren_visual.get_mut(id), ren_base_visual.get_mut(id)];

        for vis in visuals.into_iter().flatten() {
            vis.opacity = Some(opacity);
        }
    }

    pub(crate) fn trigger_keyframe_animations_if_needed(
        id: EntityId,
        ren_active_animations: &mut ActiveAnimationsSparseSecondary,
        ren_visual: &VisualPropertiesSecondary,
    ) {
        let Some(visual) = ren_visual.get(id) else {
            return;
        };
        if visual.keyframe_animations.is_empty() {
            return;
        }

        let now = Instant::now();

        if !ren_active_animations.contains_key(id) {
            ren_active_animations.insert(id, Vec::new());
        }
        let active_list = ren_active_animations.get_mut(id).unwrap();

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
        evt_interaction_states: &InteractionStates,
        evt_current_pointer_position: Option<&LayoutPoint>,
        cont_input_contents: &InputContentsSparseSecondary,
        lay_scrollbar_styles: &ScrollbarStylesSecondary,
        ren_visual: &VisualPropertiesSecondary,
        ren_active_transitions: &ActiveTransitionsSparseSecondary,
        ren_active_animations: &ActiveAnimationsSparseSecondary,
        out_clip_rects: &ClipRectsSecondary,
    ) -> bool {
        // ドラッグ選択中でポインタが可視境界外にある場合も継続
        let has_drag_autoscroll = OutputStore::is_drag_autoscroll_active(
            evt_interaction_states,
            evt_current_pointer_position,
            ren_visual,
            out_clip_rects,
        );

        // トランジションのアクティブ判定
        let has_transitions = !ren_active_transitions.is_empty()
            && ren_active_transitions.values().any(|list| !list.is_empty());

        // キーフレームアニメーションのアクティブ判定
        let has_keyframes = !ren_active_animations.is_empty()
            && ren_active_animations.values().any(|list| !list.is_empty());

        // フォーカスされたインプットがあり、キャレット点滅が有効な間は描画ループを駆動
        let has_blinking_input = evt_interaction_states
            .focused
            .and_then(|id| cont_input_contents.get(id))
            .is_some_and(|c| c.has_caret && c.is_blink);

        // 一時的表示スクロールバーのフェード進行中は描画更新ループを継続
        let has_active_transient_scrollbar = lay_scrollbar_styles.values().any(|sb_state| {
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
        ren_interaction: &InteractionPropertiesSecondary,
    ) -> bool {
        let Some(interaction) = ren_interaction.get(id) else {
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
            STATE_DND_DRAGGING => &interaction.dragging,
            STATE_DND_DRAG_IN => &interaction.drag_in,
            STATE_DND_DRAG_OVER => &interaction.drag_over,
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
        active_mask: &ComponentMask,
        state_flag: u128,
        topo_parents: &SecondaryMap<EntityId, Option<EntityId>>,
        ren_visual: &VisualPropertiesSecondary,
        ren_interaction: &InteractionPropertiesSecondary,
    ) -> Option<ThisStyle> {
        if !active_mask.has(state_flag) {
            return None;
        }
        let self_style = ren_interaction.get(id).and_then(|interaction| {
            if state_flag == STATE_FOCUSED_VISIBLE {
                interaction.focused_visible.clone()
            } else {
                interaction.focused.clone()
            }
        });

        if let Some(style) = self_style {
            return Some(style);
        }

        let focus_mode = ren_visual
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
        let mut curr = topo_parents.get(id).copied().flatten();
        while let Some(curr_id) = curr {
            if let Some(parent_interaction) = ren_interaction.get(curr_id) {
                let parent_style = if state_flag == STATE_FOCUSED_VISIBLE {
                    &parent_interaction.focused_visible
                } else {
                    &parent_interaction.focused
                };

                if let Some(p_style) = parent_style {
                    return Some(p_style.clone());
                }
            }
            curr = topo_parents.get(curr_id).copied().flatten();
        }
        None
    }

    pub(crate) fn cascade_interaction_flag<'a>(
        id: EntityId,
        interaction: &'a InteractionStyles,
        focused_style_resolved: Option<&'a ThisStyle>,
        focused_visible_style_resolved: Option<&'a ThisStyle>,
    ) -> [(u128, Option<&'a ThisStyle>); 11] {
        [
            (STATE_FOCUSED, focused_style_resolved),
            (STATE_FOCUSED_VISIBLE, focused_visible_style_resolved),
            (STATE_SELECTED, interaction.selected.as_ref()),
            (STATE_ACTIVED, interaction.actived.as_ref()),
            (STATE_HOVERED, interaction.hovered.as_ref()),
            (STATE_PRESSED, interaction.pressed.as_ref()),
            (STATE_DISABLED, interaction.disabled.as_ref()),
            (STATE_DRAGGED, interaction.dragged.as_ref()),
            (STATE_DND_DRAGGING, interaction.dragging.as_ref()),
            (STATE_DND_DRAG_IN, interaction.drag_in.as_ref()),
            (STATE_DND_DRAG_OVER, interaction.drag_over.as_ref()),
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
            (STATE_DND_DRAGGING, &interaction.dragged_within),
            (STATE_DND_DRAG_IN, &interaction.hovered_within),
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
            (STATE_DND_DRAGGING, &interaction.dragged_parent),
            (STATE_DND_DRAG_IN, &interaction.hovered_parent),
        ]
    }

    pub(crate) fn cascade_basic_layout(
        id: EntityId,
        target_layout: &mut BasicLayout,
        active_mask: ComponentMask,
        ren_interaction: &InteractionPropertiesSecondary,
    ) {
        let Some(interaction) = ren_interaction.get(id) else {
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
            (STATE_DND_DRAGGING, &interaction.dragging),
            (STATE_DND_DRAG_IN, &interaction.drag_in),
            (STATE_DND_DRAG_OVER, &interaction.drag_over),
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
        active_mask: ComponentMask,
        focused_style_resolved: Option<ThisStyle>,
        focused_visible_style_resolved: Option<ThisStyle>,
        ren_interaction: &InteractionPropertiesSecondary,
    ) {
        let Some(interaction) = ren_interaction.get(id) else {
            return;
        };

        let cascade = RenderStore::cascade_interaction_flag(
            id,
            interaction,
            focused_style_resolved.as_ref(),
            focused_visible_style_resolved.as_ref(),
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
        target: &mut TargetStyle,
        active_mask: ComponentMask,
        topo_active_masks: &ActiveMasksSecondary,
        topo_entities: &EntitiesSlot,
        topo_children: &ChildrenSecondary,
        ren_interaction: &InteractionPropertiesSecondary,
    ) {
        if !active_mask.has(STYLE_INTERACTION_WITHIN) {
            return;
        }

        let Some(interaction) = ren_interaction.get(id) else {
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
                topo_entities,
                topo_children,
                topo_active_masks,
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
            topo_entities,
            topo_children,
            topo_active_masks,
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
        target: &mut TargetStyle,
        active_mask: ComponentMask,
        topo_active_masks: &ActiveMasksSecondary,
        topo_entities: &EntitiesSlot,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        ren_interaction: &InteractionPropertiesSecondary,
    ) {
        if active_mask.has(STYLE_INTERACTION_PARENT) {
            return;
        }
        let Some(interaction) = ren_interaction.get(id) else {
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
                topo_parents,
                topo_entities,
                topo_active_masks,
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

        let Some(ref style) = interaction.any_parent else {
            return;
        };
        let any_state = TopologyStore::has_parent_with_state(
            id,
            topo_parents,
            topo_entities,
            topo_active_masks,
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
        target: &mut TargetStyle,
        active_mask: ComponentMask,
        focused_style_resolved: Option<ThisStyle>,
        focused_visible_style_resolved: Option<ThisStyle>,
        topo_active_masks: &ActiveMasksSecondary,
        topo_entities: &EntitiesSlot,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        ren_interaction: &InteractionPropertiesSecondary,
    ) {
        RenderStore::cascade_interaction(
            id,
            target,
            active_mask,
            focused_style_resolved,
            focused_visible_style_resolved,
            ren_interaction,
        );
        RenderStore::cascade_parent_interaction(
            id,
            target,
            active_mask,
            topo_active_masks,
            topo_entities,
            topo_parents,
            topo_children,
            ren_interaction,
        );
        RenderStore::cascade_within_interaction(
            id,
            target,
            active_mask,
            topo_active_masks,
            topo_entities,
            topo_children,
            ren_interaction,
        );
    }

    /// 対象の要素がキーボードフォーカス可能であるかを検証
    pub(crate) fn is_keyboard_focusable(
        id: EntityId,
        topo_active_masks: &ActiveMasksSecondary,
        topo_entities: &EntitiesSlot,
        topo_parents: &ParentsSecondary,
        lay_basic: &BasicLayoutsSecondary,
        ren_visual: &VisualPropertiesSecondary,
    ) -> bool {
        if !topo_entities.contains_key(id) {
            return false;
        }
        // 無効化（Disabled）状態でないか検証
        let mask = topo_active_masks.get(id).copied().unwrap_or_default();
        if mask.has(STATE_DISABLED) {
            return false;
        }

        // 暗黙的または明示的にキーボードフォーカスを要求しているか
        let focusable = ren_visual.get(id).and_then(|v| v.focusable);
        let is_target = match focusable {
            // 明示的にフォーカス設定がある場合
            Some(Focusable::SelfStyle(trigger) | Focusable::Inherit(trigger)) => {
                matches!(trigger, FocusTrigger::Keyboard | FocusTrigger::Both)
            }
            Some(Focusable::None) => false,
            // 設定がない場合の暗黙的なフォールバック（Input / Webview はデフォルトでフォーカス対象とする）
            None => mask.has_input_content() || mask.has_webveiw2_content(),
        };

        if !is_target {
            return false;
        }

        // 自分自身、および親先祖ツリーに非表示（Display::None）が1つも含まれていないか検証
        let mut curr = Some(id);
        while let Some(curr_id) = curr {
            if let Some(layout) = lay_basic.get(curr_id)
                && layout.display == Display::None
            {
                return false;
            }
            curr = topo_parents.get(curr_id).copied().flatten();
        }
        true
    }

    /// 補間されたアニメーション値を `SoA` のアクティブプロパティへ安全に上書きします
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply_animation_value(
        id: EntityId,
        property: PropertyList,
        value: &TransitionValue,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_nodes: &TaffyNodesSecondary,
        ren_visual: &mut VisualPropertiesSecondary,
    ) {
        if !ren_visual.contains_key(id) {
            ren_visual.insert(id, VisualProperty::default());
        }
        let v = ren_visual.get_mut(id).unwrap();

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
                if let Some(layout) = lay_basic.get_mut(id) {
                    layout.size.width = Val::Px(w);
                }
                is_layout_dirty = true;
            }
            TransitionValue::Height(h) => {
                if let Some(layout) = lay_basic.get_mut(id) {
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
                topo_active_masks,
                topo_parents,
                lay_taffy,
                lay_dirty_entities,
                lay_taffy_nodes,
            );
        }
    }

    /// 状態の変更を検知しアニメーションが必要な箇所を自動的に開始・制御
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve_element_style_state(
        id: EntityId,
        allow_transition: bool,
        win_last_size: Option<&LayoutSize>,
        react_element_effects: &ElementEffectsSecondary,
        cont_input_contents: &InputContentsSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_entities: &EntitiesSlot,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_base_basic: &BaseBasicLayoutsSecondary,
        ren_visual: &mut VisualPropertiesSecondary,
        ren_dirty_entities: &mut DirtyRenderEntitiesVec,
        ren_active_transitions: &mut ActiveTransitionsSparseSecondary,
        ren_active_animations: &mut ActiveAnimationsSparseSecondary,
        ren_base_visual: &BaseVisualPropertiesSecondary,
        ren_interaction: &InteractionPropertiesSecondary,
        out_rects: &RectsSecondary,
    ) {
        let active_mask = topo_active_masks[id];

        RenderStore::resolve_visual_styles(
            id,
            allow_transition,
            active_mask,
            react_element_effects,
            cont_input_contents,
            topo_active_masks,
            topo_entities,
            topo_parents,
            topo_children,
            ren_visual,
            ren_dirty_entities,
            ren_active_transitions,
            ren_base_visual,
            ren_interaction,
        );

        RenderStore::resolve_layout_styles(
            id,
            allow_transition,
            active_mask,
            win_last_size,
            react_element_effects,
            topo_active_masks,
            topo_parents,
            lay_taffy,
            lay_basic,
            lay_dirty_entities,
            lay_taffy_nodes,
            lay_base_basic,
            ren_active_transitions,
            ren_visual,
            ren_base_visual,
            ren_interaction,
            out_rects,
        );

        // スタイル解決が完了した結果、自身に新しくキーフレームアニメーション定義が
        // 読み込まれていれば、自動的にそのアニメーションの再生を開始する
        RenderStore::trigger_keyframe_animations_if_needed(id, ren_active_animations, ren_visual);

        let Some(react_effects) = react_element_effects.get(id) else {
            return;
        };
        let text_effects: Vec<EffectId> = react_effects
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
        win_last_size: Option<&LayoutSize>,
        react_element_effects: &ElementEffectsSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_base_basic: &BaseBasicLayoutsSecondary,
        ren_active_transitions: &mut ActiveTransitionsSparseSecondary,
        ren_visual: &VisualPropertiesSecondary,
        ren_base_visual: &BaseVisualPropertiesSecondary,
        ren_interaction: &InteractionPropertiesSecondary,
        out_rects: &RectsSecondary,
    ) {
        let has_base_layout = lay_base_basic.contains_key(id);
        let has_active_layout = lay_basic.contains_key(id);

        if !has_base_layout && !has_active_layout {
            return;
        }

        let active_layout = lay_basic.get(id).copied().unwrap_or_default();
        let base_layout = lay_base_basic.get(id).copied().unwrap_or_default();
        let mut target_layout = base_layout;

        RenderStore::cascade_basic_layout(id, &mut target_layout, active_mask, ren_interaction);

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
                ren_active_transitions,
                ren_base_visual,
            )
        };

        let can_trigger_width = ren_visual.get(id).is_some_and(|v| {
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

        let can_trigger_height = ren_visual.get(id).is_some_and(|v| {
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
        let active_layout_mut = lay_basic.get_mut(id).unwrap();
        *active_layout_mut = target_layout;

        if width_triggered {
            active_layout_mut.size.width = Val::Px(current_w.unwrap());
        }
        if height_triggered {
            active_layout_mut.size.height = Val::Px(current_h.unwrap());
        }

        LayoutStore::mark_layout_dirty(
            id,
            topo_active_masks,
            topo_parents,
            lay_taffy,
            lay_dirty_entities,
            lay_taffy_nodes,
        );
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn resolve_visual_styles(
        id: EntityId,
        allow_transition: bool,
        active_mask: ComponentMask,
        react_element_effects: &ElementEffectsSecondary,
        cont_input_contents: &InputContentsSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_entities: &EntitiesSlot,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        ren_visual: &mut VisualPropertiesSecondary,
        ren_dirty_entities: &mut DirtyRenderEntitiesVec,
        ren_active_transitions: &mut ActiveTransitionsSparseSecondary,
        ren_base_visual: &BaseVisualPropertiesSecondary,
        ren_interaction: &InteractionPropertiesSecondary,
    ) {
        let has_base_visual = ren_base_visual.contains_key(id);
        let has_active_visual = ren_visual.contains_key(id);
        let has_interaction_styles = ren_interaction.contains_key(id);

        if !has_base_visual && !has_active_visual && !has_interaction_styles {
            return;
        }

        let current = RenderStore::get_current_style(id, ren_visual);
        let mut target = RenderStore::get_target_style(id, ren_base_visual);

        let resolv_focus = |flag| {
            RenderStore::resolv_focus_style(
                id,
                &active_mask,
                flag,
                topo_parents,
                ren_visual,
                ren_interaction,
            )
        };

        let focused_style_resolved = resolv_focus(STATE_FOCUSED);
        let focused_visible_style_resolved = resolv_focus(STATE_FOCUSED_VISIBLE);

        RenderStore::apply_interaction_cascades(
            id,
            &mut target,
            active_mask,
            focused_style_resolved,
            focused_visible_style_resolved,
            topo_active_masks,
            topo_entities,
            topo_parents,
            topo_children,
            ren_interaction,
        );

        let mut is_placeholder_active = false;
        if let Some(contents) = cont_input_contents.get(id) {
            let has_no_ime = contents
                .ime_state
                .as_ref()
                .is_none_or(|s| s.composition_text.is_empty());
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
                react_element_effects,
                ren_active_transitions,
                ren_base_visual,
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
            || ren_base_visual.contains_key(id)
        {
            if !ren_visual.contains_key(id) {
                ren_visual.insert(id, VisualProperty::default());
            }
            let active_vis = ren_visual.get_mut(id).unwrap();

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
            active_vis.font_family.clone_from(&target.font_family);
            active_vis.font_weight = target.font_weight;
            active_vis.font_style = target.font_style;

            active_vis.pointer_events = target.pointer_events;

            if let Some(target_vis) = ren_base_visual.get(id) {
                active_vis.z_index = target_vis.z_index;
                active_vis.backdrop = target_vis.backdrop;
                active_vis.bg_gradient = target_vis.bg_gradient;
                active_vis.transitions.clone_from(&target_vis.transitions);
                active_vis
                    .keyframe_animations
                    .clone_from(&target_vis.keyframe_animations);
                active_vis.focusable = target_vis.focusable;
                active_vis.prevent_focus_steal = target_vis.prevent_focus_steal;
                active_vis.prevent_focus_steal_within = target_vis.prevent_focus_steal_within;
                active_vis.transform_inherit = target.transform_inherit;
                active_vis.user_select = target_vis.user_select;
            }

            RenderStore::mark_render_dirty(id, topo_active_masks, ren_dirty_entities);
        }
    }

    /// 必要に応じてトランジションを起動、または上書き（逆再生含む）します
    pub(crate) fn trigger_transition_if_needed(
        id: EntityId,
        property_list: PropertyList,
        start_value: TransitionValue,
        end_value: TransitionValue,
        react_element_effects: &ElementEffectsSecondary,
        ren_active_transitions: &mut ActiveTransitionsSparseSecondary,
        ren_base_visual: &BaseVisualPropertiesSecondary,
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

        let Some(visual) = ren_base_visual.get(id) else {
            return false;
        };

        let Some(t) = visual
            .transitions
            .iter()
            .find(|t| t.property_list == property_list || t.property_list == PropertyList::Size)
        else {
            return false;
        };

        let Some(entry) = ren_active_transitions.entry(id) else {
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
        evt_interaction_states: &InteractionStates,
        topo_parents: &ParentsSecondary,
        ren_visual: &VisualPropertiesSecondary,
        ren_base_visual: &BaseVisualPropertiesSecondary,
    ) -> CursorIcon {
        // 現在プレス中の要素（pressed）があればそれを最優先で探索の基点にする
        let start_id = evt_interaction_states.pressed.unwrap_or(hovered_id);

        let mut curr = Some(start_id);
        let mut global_cursor = None;

        while let Some(id) = curr {
            let cursor_opt = ren_visual
                .get(id)
                .and_then(|v| v.cursor)
                .or_else(|| ren_base_visual.get(id).and_then(|v| v.cursor));

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
            curr = topo_parents.get(id).copied().flatten();
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

    /// 各要素の実効トランスフォーム行列を累積計算
    pub(crate) fn accumulate_transform_matrix(
        topo_effective_transforms: &mut EffectiveTransformsSecondary,
        topo_active_entities: &ActiveEntitiesVec,
        topo_parents: &ParentsSecondary,
        topo_flat_dfs_sequence: &FlatDfsSequenceVec,
        ren_visual: &VisualPropertiesSecondary,
    ) {
        topo_effective_transforms.clear();

        for &id in topo_flat_dfs_sequence {
            let (self_transform, transform_inherit) = match ren_visual.get(id) {
                Some(v) => (
                    v.transform.unwrap_or(IDENTITY_MATRIX),
                    v.transform_inherit.unwrap_or(false),
                ),
                None => (IDENTITY_MATRIX, false),
            };

            let mut eff_transform = self_transform;

            if transform_inherit
                && let Some(parent_id) = topo_parents.get(id).copied().flatten()
                && let Some(&parent_eff) = topo_effective_transforms.get(parent_id)
            {
                // 親の累積トランスフォーム行列 * 自身のトランスフォーム行列 (Column-Major 順)
                eff_transform = OutputStore::mul_4x4(&parent_eff, &self_transform);
            }
            topo_effective_transforms.insert(id, eff_transform);
        }
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
        let origin = visual.transform_origin.map_or([0.5, 0.5], |p| [p.x, p.y]);
        (packed_transform, origin)
    }

    #[allow(clippy::cast_precision_loss)]
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
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(crate) fn tick_animations(
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_nodes: &TaffyNodesSecondary,
        ren_visual: &mut VisualPropertiesSecondary,
        ren_dirty_entities: &mut DirtyRenderEntitiesVec,
        ren_active_animations: &mut ActiveAnimationsSparseSecondary,
    ) {
        let now = Instant::now();

        ren_active_animations.retain(|id, animations| {
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
                        lay_taffy,
                        lay_basic,
                        lay_dirty_entities,
                        lay_taffy_nodes,
                        ren_visual,
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
                    lay_taffy,
                    lay_basic,
                    lay_dirty_entities,
                    lay_taffy_nodes,
                    ren_visual,
                );

                RenderStore::mark_render_dirty(id, topo_active_masks, ren_dirty_entities);
                true // 継続して保持
            });

            // // アニメーションが空の要素はマップごと削除
            !animations.is_empty()
        });
    }

    /// 毎フレームの描画前に呼び出され、すべてのアクティブなトランジションを 1 Tick 進めます
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(crate) fn tick_transitions(
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_basic: &mut BasicLayoutsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_nodes: &TaffyNodesSecondary,
        ren_visual: &mut VisualPropertiesSecondary,
        ren_dirty_entities: &mut DirtyRenderEntitiesVec,
        ren_active_transitions: &mut ActiveTransitionsSparseSecondary,
        ren_last_tick_time: &mut Option<Instant>,
    ) {
        const FRAME_TIME_120FPS: Duration = Duration::from_nanos(8_333_333);
        let now = Instant::now();

        // (1.0 / 120.0 秒 = 約 8,333,333 ナノ秒)
        if let Some(last) = ren_last_tick_time
            && now.duration_since(*last) < FRAME_TIME_120FPS
        {
            return;
        }

        // 実行制限を通過したため、基準時刻を更新して処理を継続
        *ren_last_tick_time = Some(now);
        ren_active_transitions.retain(|id, transitions| {
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
                        if let Some(v) = ren_visual.get_mut(id) {
                            if t_state.property_list == PropertyList::BackgroundColor {
                                v.bg_color = Some(c);
                            } else if t_state.property_list == PropertyList::BorderColor {
                                v.border_color = Some(c);
                            }
                        }
                        RenderStore::mark_render_dirty(id, topo_active_masks, ren_dirty_entities);
                    }
                    TransitionValue::Opacity(o) => {
                        if let Some(v) = ren_visual.get_mut(id) {
                            v.opacity = Some(o);
                        }
                        RenderStore::mark_render_dirty(id, topo_active_masks, ren_dirty_entities);
                    }
                    TransitionValue::Transform(m) => {
                        if let Some(v) = ren_visual.get_mut(id) {
                            v.transform = Some(m);
                        }
                        RenderStore::mark_render_dirty(id, topo_active_masks, ren_dirty_entities);
                    }
                    TransitionValue::CornerRadius(cr) => {
                        if let Some(v) = ren_visual.get_mut(id) {
                            v.corner_radius = Some(cr);
                        }
                        RenderStore::mark_render_dirty(id, topo_active_masks, ren_dirty_entities);
                    }
                    TransitionValue::Width(w) => {
                        if let Some(layout) = lay_basic.get_mut(id) {
                            layout.size.width = Val::Px(w); // ピクセル値で上書き
                        }
                        LayoutStore::mark_layout_dirty(
                            id,
                            topo_active_masks,
                            topo_parents,
                            lay_taffy,
                            lay_dirty_entities,
                            lay_taffy_nodes,
                        );

                        // キャッシュを毎フレーム強制バイパスさせるためにマスクを再セット
                        if let Some(mask) = topo_active_masks.get_mut(id) {
                            mask.set(STATE_QUEUED_LAYOUT);
                        }
                    }
                    // 縦幅（Height）の毎フレームアニメーション補間
                    TransitionValue::Height(h) => {
                        if let Some(layout) = lay_basic.get_mut(id) {
                            layout.size.height = Val::Px(h);
                        }
                        LayoutStore::mark_layout_dirty(
                            id,
                            topo_active_masks,
                            topo_parents,
                            lay_taffy,
                            lay_dirty_entities,
                            lay_taffy_nodes,
                        );

                        if let Some(mask) = topo_active_masks.get_mut(id) {
                            mask.set(STATE_QUEUED_LAYOUT);
                        }
                    }
                    // 影（BoxShadow）の毎フレームの書き戻し処理
                    TransitionValue::BoxShadow(shadow) => {
                        if let Some(v) = ren_visual.get_mut(id) {
                            v.shadow_params = Some(shadow);
                            v.shadow_color = Some(shadow.color);
                        }
                        RenderStore::mark_render_dirty(id, topo_active_masks, ren_dirty_entities);
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
        ren_visual: &VisualPropertiesSecondary,
    ) -> CurrentStyle {
        ren_visual
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
        ren_base_visual: &BaseVisualPropertiesSecondary,
    ) -> TargetStyle {
        ren_base_visual
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
    /// 指定された `VisualProperty` と `ComponentMask` を基に自身のスタイルをマージ。
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
                target.font_family.clone_from(&inner_vis.font_family);
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
        let TopologyStore {
            topo_active_masks, ..
        } = &mut self.topology;
        let RenderStore {
            ren_dirty_entities, ..
        } = &mut self.renders;

        RenderStore::mark_render_dirty(id, topo_active_masks, ren_dirty_entities);
    }

    #[inline]
    pub(crate) fn get_visual_property_mut(
        &mut self,
        id: EntityId,
        target: StyleTarget,
    ) -> Option<&mut VisualProperty> {
        let RenderStore {
            ren_base_visual,
            ren_interaction,
            ..
        } = &mut self.renders;

        RenderStore::get_visual_property_mut(id, target, ren_base_visual, ren_interaction)
    }

    /// 対象の要素がキーボードフォーカス可能であるかを総合検証します
    #[inline]
    pub(crate) fn is_keyboard_focusable(&self, id: EntityId) -> bool {
        let TopologyStore {
            topo_entities: topo_entities,
            topo_active_masks,
            topo_parents,
            ..
        } = &self.topology;
        let RenderStore { ren_visual, .. } = &self.renders;
        let LayoutStore { lay_basic, .. } = &self.layouts;

        RenderStore::is_keyboard_focusable(
            id,
            topo_active_masks,
            topo_entities,
            topo_parents,
            lay_basic,
            ren_visual,
        )
    }

    /// 状態の変更を検知し、アニメーション（トランジション）が必要な箇所を自動的に開始・制御します。
    #[inline]
    pub(crate) fn resolve_element_style_state(&mut self, id: EntityId, allow_transition: bool) {
        let TopologyStore {
            topo_entities,
            topo_parents,
            topo_children,
            topo_active_masks,
            ..
        } = &mut self.topology;

        let LayoutStore {
            lay_basic,
            lay_base_basic,
            lay_taffy_nodes,
            lay_taffy,
            lay_dirty_entities,
            ..
        } = &mut self.layouts;

        let RenderStore {
            ren_visual,
            ren_interaction,
            ren_base_visual,
            ren_dirty_entities,
            ren_active_transitions,
            ren_active_animations,
            ..
        } = &mut self.renders;

        let OutputStore { out_rects, .. } = &self.outputs;

        let ReactiveStore {
            react_element_effects,
            ..
        } = &self.reactive;

        let ContentStore {
            cont_input_contents,
            ..
        } = &self.contents;

        let WindowStore { win_last_size, .. } = &self.window;

        RenderStore::resolve_element_style_state(
            id,
            allow_transition,
            win_last_size.as_ref(),
            react_element_effects,
            cont_input_contents,
            topo_active_masks,
            topo_entities,
            topo_parents,
            topo_children,
            lay_taffy,
            lay_basic,
            lay_dirty_entities,
            lay_taffy_nodes,
            lay_base_basic,
            ren_visual,
            ren_dirty_entities,
            ren_active_transitions,
            ren_active_animations,
            ren_base_visual,
            ren_interaction,
            out_rects,
        );
    }
}
