use crate::*;
use slotmap::{SecondaryMap, SparseSecondaryMap};
use std::{collections::HashSet, sync::Arc, time::Instant};

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
                    renders.interaction_properties
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
                    renders.interaction_properties
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
                // ※ ここでは例として「回転 (Transform)」の場合、0度から360度へ向かう値を算出します。
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
                    _ => continue, // 必要に応じて他プロパティも定義
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
}
