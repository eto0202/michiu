use crate::{
    ActiveMasksSecondary, BasicLayoutsSecondary, ComponentMask, Context, Display, EntitiesSlot,
    EntityId, MichiuSoA, ParentsSecondary, Pipeline, SystemStore, TextEditStore,
    VisualPropertiesSecondary, handle_on_blur, handle_on_focus,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum FocusTrigger {
    Mouse,
    Keyboard,
    #[default]
    Both,
}

/// 実際に発生したフォーカスイベントの物理入力ソース
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActiveFocusTrigger {
    Mouse,
    Keyboard,
}

/// フォーカスを受け入れる際の挙動およびスタイルの継承ポリシー
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Focusable {
    #[default]
    None, // フォーカス不可能
    SelfStyle(FocusTrigger), // フォーカス可能。ただし自身の focused スタイルのみを適用
    Inherit(FocusTrigger), // フォーカス可能。自身に focused スタイルが無い場合、親先祖の focused スタイルを自動継承
}

pub(crate) struct FocusStore {}

impl FocusStore {
    /// 指定要素またはその親階層において、フォーカスの略奪を防止すべきか判定
    #[inline]
    #[track_caller]
    pub(crate) fn should_prevent_focus_steal(cx: &Context, target_id: EntityId) -> bool {
        let mut curr = Some(target_id);
        while let Some(curr_id) = curr {
            let mask = cx.topology.topo_active_masks.at(curr_id);

            if mask.has(ComponentMask::STYLE_PREVENT_FOCUS_STEAL)
                && curr_id == target_id
                && cx
                    .renders
                    .rnd_visual
                    .find(curr_id)
                    .and_then(|v| v.prevent_focus_steal)
                    .unwrap_or(false)
            {
                return true;
            }

            if mask.has(ComponentMask::STYLE_PREVENT_FOCUS_STEAL_WITHIN)
                && cx
                    .renders
                    .rnd_visual
                    .find(curr_id)
                    .and_then(|v| v.prevent_focus_steal_within)
                    .unwrap_or(false)
            {
                return true;
            }

            curr = *cx.topology.topo_parents.at(curr_id);
        }
        false
    }

    pub(crate) fn restrict_focusable_element(
        id: EntityId,
        topo_active_masks: &ActiveMasksSecondary,
        rnd_visual: &VisualPropertiesSecondary,
    ) -> bool {
        let focusable = rnd_visual.find(id).and_then(|v| v.focusable).or_else(|| {
            let mask = topo_active_masks.at(id);
            if mask.has_input_content() || mask.has_external_visual_content() {
                Some(Focusable::Inherit(FocusTrigger::Both)) // 未指定時はキーボードフォーカス
            } else {
                None
            }
        });

        focusable.is_some_and(|f| match f {
            Focusable::SelfStyle(trigger) | Focusable::Inherit(trigger) => {
                trigger == FocusTrigger::Mouse || trigger == FocusTrigger::Both
            }
            Focusable::None => false,
        })
    }

    /// 入力トリガー源を考慮してフォーカス状態を更新します。
    #[inline]
    pub(crate) fn set_focused_by_trigger(
        cx: &mut Context,
        id: EntityId,
        focused: bool,
        trigger: ActiveFocusTrigger,
    ) {
        Pipeline::update_state(cx, id, ComponentMask::STATE_FOCUSED, focused);
        let show_visible = focused && (trigger == ActiveFocusTrigger::Keyboard);
        Pipeline::update_state(cx, id, ComponentMask::STATE_FOCUSED_VISIBLE, show_visible);
    }

    #[track_caller]
    pub(crate) fn auto_focus_switch_by_trigger(
        cx: &mut Context,
        id: EntityId,
        trigger: ActiveFocusTrigger,
    ) {
        // 同一要素をクリックした場合はフォーカス可視化の同期のみ
        if cx.events.evt_interaction_states.focused == Some(id) {
            FocusStore::set_focused_by_trigger(cx, id, true, trigger);
            return;
        }

        if let Some(old_focus_id) = cx.events.evt_interaction_states.focused {
            FocusStore::set_focused_by_trigger(cx, old_focus_id, false, trigger);

            // 古いフォーカス要素の選択範囲とハイライト矩形をクリア
            TextEditStore::clear_selection_highlight_rect(
                old_focus_id,
                &mut cx.topology.topo_active_masks,
                &mut cx.contents.cont_input_contents,
                &mut cx.contents.cont_text_spans,
                &mut cx.states.edit.edit_selections,
                &mut cx.states.edit.edit_selected_rects,
            );
            // 進行中の IME コンポジションを強制的に確定させ候補窓を閉じる
            SystemStore::force_complete_ime_composition();

            handle_on_blur(cx, old_focus_id);
        }

        // 新しいフォーカス可能要素にフォーカスを設定
        FocusStore::set_focused_by_trigger(cx, id, true, trigger);

        // 新しいフォーカス先が is_ime(false) の場合は IME 関連付けを解除
        let is_input = cx.topology.topo_active_masks.at(id).has_input_content();
        if is_input {
            // マスクがあるなら Some のはず
            let contents = cx.contents.cont_input_contents.at(id);
            SystemStore::unassociate_ime(contents, &mut cx.window.win_default_himc);
        } else {
            // インプット以外の場合は IME をデフォルト状態に戻す
            SystemStore::reset_ime_default_state(cx.window.win_default_himc.as_ref());
        }

        cx.events.evt_interaction_states.focused = Some(id);

        handle_on_focus(cx, id);
    }

    #[inline]
    pub(crate) fn handle_remove_focus(cx: &mut Context) {
        let Some(old_focus_id) = cx.events.evt_interaction_states.focused else {
            return;
        };

        // 先にフォーカス状態を解除しておく
        // コールバック内で再フォーカスされても上書きしないため
        cx.events.evt_interaction_states.focused = None;

        FocusStore::set_focused_by_trigger(cx, old_focus_id, false, ActiveFocusTrigger::Mouse);

        // 古いフォーカス要素の選択範囲とハイライト矩形をクリア
        TextEditStore::clear_selection_highlight_rect(
            old_focus_id,
            &mut cx.topology.topo_active_masks,
            &mut cx.contents.cont_input_contents,
            &mut cx.contents.cont_text_spans,
            &mut cx.states.edit.edit_selections,
            &mut cx.states.edit.edit_selected_rects,
        );
        // 進行中の IME コンポジションを強制的に確定させ候補窓を閉じる
        SystemStore::force_complete_ime_composition();

        // IME をデフォルトの有効化状態に戻す
        SystemStore::reset_ime_default_state(cx.window.win_default_himc.as_ref());

        handle_on_blur(cx, old_focus_id);
    }

    /// 対象の要素がキーボードフォーカス可能であるかを検証
    #[track_caller]
    pub(crate) fn is_keyboard_focusable(
        id: EntityId,
        topo_entities: &EntitiesSlot,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_basic: &BasicLayoutsSecondary,
        rnd_visual: &VisualPropertiesSecondary,
    ) -> bool {
        if !topo_entities.contains_key(id) {
            return false;
        }
        // 無効化（Disabled）状態でないか検証
        let mask = topo_active_masks.at(id);
        if mask.has(ComponentMask::STATE_DISABLED) {
            return false;
        }

        // 暗黙的または明示的にキーボードフォーカスを要求しているか
        let focusable = rnd_visual.find(id).and_then(|v| v.focusable);
        let is_target = match focusable {
            // 明示的にフォーカス設定がある場合
            Some(Focusable::SelfStyle(trigger) | Focusable::Inherit(trigger)) => {
                matches!(trigger, FocusTrigger::Keyboard | FocusTrigger::Both)
            }
            Some(Focusable::None) => false,
            // 設定がない場合の暗黙的なフォールバック（Input / Webview はデフォルトでフォーカス対象とする）
            None => mask.has_input_content() || mask.has_external_visual_content(),
        };

        if !is_target {
            return false;
        }

        // 自分自身、および親先祖ツリーに非表示（Display::None）が1つも含まれていないか検証
        let mut curr = Some(id);
        while let Some(curr_id) = curr {
            if let Some(layout) = lay_basic.find(curr_id)
                && layout.display == Display::None
            {
                return false;
            }
            curr = *topo_parents.at(curr_id);
        }
        true
    }
}
