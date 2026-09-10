use crate::{
    ActiveEntitiesVec, ActiveFocusTrigger, ActiveMasksSecondary, BaseBasicLayoutsSecondary,
    BaseVisualPropertiesSecondary, BasicLayout, BasicLayoutsSecondary, BatchType, BoxSizing,
    ChildrenSecondary, ClipRectsSecondary, Color, ComponentMask, ContentStore, Context,
    CornerRadius, DEFAULT_BASIC, DEFAULT_FLEX, DirtyLayoutEntitiesVec, DrawBatch, EdgeInsets,
    EffectId, ElementState, EntityId, EventStore, ExternalTextureAlphaMode, ExternalTextureSparse,
    ExtractedThumb, FlatDfsSequenceVec, FlexLayout, FocusStore, GridLayout, IDENTITY_MATRIX,
    ImeState, InputContents, InputContentsSparse, InputOp, LayoutPoint, LayoutRect, LayoutSize,
    LayoutStore, Length, MichiuSoA, Modifiers, MouseButton, OutputStore, ParentsSecondary,
    PointerEvents, PrevClipRectsSecondary, PrevRectsSecondary, QuadInstance, RangeExt,
    ReactiveStore, RectsSecondary, RenderData, RenderStore, RendererView, ResolvedBasicSecondary,
    ResolvedFlexSecondary, ResolvedGridSparse, ScrollBarState, ScrollOffsetsSecondary, ScrollStore,
    ScrollbarStore, ScrollbarStylesSecondary, Size, StrikethroughStyle, SystemStore,
    TaffyNodesSecondary, TaffyTreeEntityId, TextBufferSparse, TextCacheKey,
    TextContentsSparse, TextEditStore, TextEngine, TextSpan, TextSpansSparse, TopologyStore,
    UnderlineStyle, Val, VirtualKey, VisualPropertiesSecondary, VisualProperty, WindowStore,
    bind_context, execute_effect, handle_on_active, handle_on_char_input, handle_on_disable,
    handle_on_file_dropped, handle_on_ime, handle_on_select, with_context,
};
use cosmic_text::Buffer;
use slotmap::SparseSecondaryMap;
use std::{borrow::Cow, collections::HashSet, ops::Range, path::PathBuf};
use windows::Win32::Graphics::DirectWrite::{DWRITE_HIT_TEST_METRICS, IDWriteTextLayout};

#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum UserAction {
    PointerMove(LayoutPoint),
    PointerButton {
        button: MouseButton,
        state: ElementState,
        modifiers: Modifiers,
    },
    PointerDoubleClick {
        modifiers: Modifiers,
    },
    MouseWheel {
        scroll_x: f32,
        scroll_y: f32,
    },
    KeyboardKey {
        key: VirtualKey,
        state: ElementState,
        modifiers: Modifiers,
    },
    Character(char),
    Ime(ImeState),
    FileDropped(Vec<PathBuf>),
    Paste(Cow<'static, str>),
    Cut,
    Undo,
    Redo,
}

#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum TickType {
    All,
    Transition,
    Animation,
    TransitionAndAnimation,
    AutoScroll,
}

#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum StateFlag {
    Hovered,
    Focused,
    FocusedVisible,
    Pressed,
    Disabled,
    Actived,
    Selected,
    Dragged,
    DndDragging,
    DndDragIn,
    DndDragOver,
}

pub struct Pipeline;

impl Pipeline {
    #[inline]
    pub(crate) fn inject_user_action(cx: &mut Context, action: UserAction) {
        let _context_guard = bind_context(cx);

        match action {
            UserAction::PointerMove(layout_point) => {
                EventStore::inject_pointer_move(cx, layout_point);
            }
            UserAction::PointerButton {
                button,
                state,
                modifiers,
            } => EventStore::inject_pointer_button(cx, button, state, modifiers),
            UserAction::PointerDoubleClick { modifiers } => {
                EventStore::inject_pointer_double_click(cx, modifiers);
            }
            UserAction::MouseWheel { scroll_x, scroll_y } => {
                EventStore::inject_mouse_wheel(cx, scroll_x, scroll_y);
            }
            UserAction::KeyboardKey {
                key,
                state,
                modifiers,
            } => EventStore::inject_keyboard_key(cx, key, state, modifiers),
            UserAction::Character(c) => {
                let Some(focused_id) = cx.events.evt_interaction_states.focused else {
                    return;
                };
                handle_on_char_input(cx, focused_id, c);
            }
            UserAction::Ime(ime_state) => {
                let Some(focused_id) = cx.events.evt_interaction_states.focused else {
                    return;
                };
                handle_on_ime(cx, focused_id, ime_state);
            }
            UserAction::FileDropped(path_bufs) => {
                let Some(target_id) = cx.events.evt_interaction_states.hovered else {
                    return;
                };
                handle_on_file_dropped(cx, target_id, path_bufs);
            }
            UserAction::Paste(text) => EventStore::inject_paste(cx, &text.into()),
            UserAction::Cut => {
                cx.contents.cont_cut_text = EventStore::inject_cut(cx);
            }
            UserAction::Undo => EventStore::inject_undo(cx),
            UserAction::Redo => EventStore::inject_redo(cx),
        }
    }

    /// 各インタラクション状態（ステート）を更新し、レイアウト変更を伴うか自動的に判別して Dirty フラグを制御する共通ヘルパー
    #[inline]
    pub(crate) fn update_state(cx: &mut Context, id: EntityId, state_flag: u128, actived: bool) {
        let mut was_active = false;
        let mut state_changed = false;

        let mask = cx.topology.topo_active_masks.at_mut(id); // 絶対に生きてるはず

        was_active = mask.has(state_flag);
        if was_active == actived {
            return;
        }

        state_changed = true;

        if actived {
            mask.set(state_flag);
        } else {
            mask.unset(state_flag);
        }

        let mut resolve_element = |cx: &mut Context, id: EntityId| {
            RenderStore::resolve_element_style_state(
                id,
                true,
                cx.window.win_last_size,
                &cx.system.sys_text_buffers,
                &cx.reactive.react_element_effects,
                &cx.contents.cont_input_contents,
                &mut cx.topology.topo_active_masks,
                &mut cx.topology.topo_is_sort_dirty,
                &cx.topology.topo_entities,
                &cx.topology.topo_parents,
                &cx.topology.topo_children,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_taffy_tree,
                &mut cx.layouts.lay_basic,
                &cx.layouts.lay_taffy_nodes,
                &cx.layouts.lay_base_basic,
                &mut cx.renders.rnd_dirty_entities,
                &mut cx.renders.rnd_visual,
                &mut cx.renders.rnd_active_transitions,
                &mut cx.renders.rnd_active_animations,
                &cx.renders.rnd_base_visual,
                &cx.renders.rnd_interaction,
                &cx.outputs.out_rects,
            );

            // この要素のアクティブレイアウトキャッシュを差分更新
            LayoutStore::update_resolved_active_layout_cache(
                id,
                &cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_resolved_basic,
                &mut cx.layouts.lay_resolved_flex,
                &mut cx.layouts.lay_resolved_grid,
                &cx.layouts.lay_basic,
                &cx.layouts.lay_flex,
                &cx.layouts.lay_grid,
                &cx.renders.rnd_visual,
                &cx.renders.rnd_interaction,
                &cx.renders.rnd_active_transitions,
            );
        };

        let mark_dirty = |cx: &mut Context, id: EntityId| {
            if RenderStore::does_state_require_layout(id, state_flag, &cx.renders.rnd_interaction) {
                LayoutStore::mark_layout_dirty(
                    id,
                    &mut cx.topology.topo_active_masks,
                    &cx.topology.topo_parents,
                    &mut cx.layouts.lay_dirty_entities,
                    &mut cx.layouts.lay_taffy_tree,
                    &cx.layouts.lay_taffy_nodes,
                );
            }
            RenderStore::mark_render_dirty(
                id,
                &mut cx.topology.topo_active_masks,
                &mut cx.renders.rnd_dirty_entities,
            );
        };

        // 状態変化の発生時に即座に動的なスタイルを解決する
        resolve_element(cx, id);

        // 親から子方向へのスタイル解決の伝播
        for child_id in cx.topology.topo_children.at(id).clone() {
            let has_parent = cx
                .topology
                .topo_active_masks
                .at(child_id)
                .has(ComponentMask::STYLE_INTERACTION_PARENT);

            if has_parent {
                resolve_element(cx, child_id);
                mark_dirty(cx, child_id);
            }
        }

        // STYLE_INTERACTION_WITHIN マスク判定による親先祖の早期バイパス
        let mut curr = id;
        while let Some(parent_id) = *cx.topology.topo_parents.at(curr) {
            if cx.topology.topo_entities.contains_key(parent_id) {
                let has_within = cx
                    .topology
                    .topo_active_masks
                    .at(parent_id)
                    .has(ComponentMask::STYLE_INTERACTION_WITHIN);

                // 先祖要素が within スタイルを持っている場合のみそのスタイル評価を実行
                if has_within {
                    resolve_element(cx, parent_id);
                    mark_dirty(cx, parent_id);
                }
            }
            curr = parent_id;
        }

        // 状態変化による本要素のレイアウト汚染チェック
        mark_dirty(cx, id);

        if !state_changed {
            return;
        }

        // 残りの状態遷移イベントの解決
        if actived {
            match state_flag {
                ComponentMask::STATE_DISABLED => handle_on_disable(cx, id),
                ComponentMask::STATE_ACTIVED => handle_on_active(cx, id),
                ComponentMask::STATE_SELECTED => handle_on_select(cx, id),

                _ => {}
            }
        }
    }

    #[inline]
    pub(crate) fn set_states(cx: &mut Context, id: EntityId, flag: &StateFlag, actived: bool) {
        match flag {
            StateFlag::Hovered => {
                Pipeline::update_state(cx, id, ComponentMask::STATE_HOVERED, actived);
            }
            StateFlag::Focused => {
                FocusStore::set_focused_by_trigger(cx, id, actived, ActiveFocusTrigger::Mouse);
            }
            StateFlag::FocusedVisible => {
                FocusStore::set_focused_by_trigger(cx, id, actived, ActiveFocusTrigger::Keyboard);
            }
            StateFlag::Pressed => {
                Pipeline::update_state(cx, id, ComponentMask::STATE_PRESSED, actived);
            }
            StateFlag::Disabled => {
                Pipeline::update_state(cx, id, ComponentMask::STATE_DISABLED, actived);
            }
            StateFlag::Actived => {
                Pipeline::update_state(cx, id, ComponentMask::STATE_ACTIVED, actived);
            }
            StateFlag::Selected => {
                Pipeline::update_state(cx, id, ComponentMask::STATE_SELECTED, actived);
            }
            StateFlag::Dragged => {
                Pipeline::update_state(cx, id, ComponentMask::STATE_DRAGGED, actived);
            }
            StateFlag::DndDragging => {
                Pipeline::update_state(cx, id, ComponentMask::STATE_DND_DRAGGING, actived);
            }
            StateFlag::DndDragIn => {
                Pipeline::update_state(cx, id, ComponentMask::STATE_DND_DRAG_IN, actived);
            }
            StateFlag::DndDragOver => {
                Pipeline::update_state(cx, id, ComponentMask::STATE_DND_DRAG_OVER, actived);
            }
        }
    }

    #[inline]
    pub(crate) fn sync_layout_and_render(
        cx: &mut Context,
        root: EntityId,
        window_size: LayoutSize,
    ) {
        let _context_guard = bind_context(cx);

        /// トポロジーが完全に完成したビルド完了後、または同期直前に、溜めてある初回評価を一挙に実行
        ReactiveStore::evaluate_pending_element_effects(
            &mut cx.reactive.react_pending_element_effects,
            &cx.reactive.react_effects,
        );

        // ウィンドウサイズの変更検知
        let window_resized = cx.window.win_last_size.replace(window_size) != Some(window_size);

        // 構造変更がなく、スタイル変更（レイアウト変更要求）もなく、ウィンドウサイズも変わっていないなら、
        // すべてスキップして早期リターン。
        if cx.layouts.lay_dirty_entities.is_empty()
            && !cx.topology.topo_is_structure_dirty
            && !window_resized
            && !cx.outputs.out_rects.is_empty()
        {
            return;
        }

        // 実際にレイアウト再計算が発生する時だけマーク。それ以外はキャッシュされたカリング結果を使用。
        // スクロール時はレイアウト汚染が発生する
        // トランスフォームはtick_の方でフラグを立てている
        cx.topology.topo_is_sort_dirty = true;

        // DFSツリーシーケンスの再構築
        if cx.topology.topo_is_structure_dirty {
            TopologyStore::rebuild_dfs_sequence(
                root,
                &mut cx.topology.topo_flat_dfs_sequence,
                &mut cx.topology.topo_is_structure_dirty,
                &cx.topology.topo_children,
            );
        }

        // 全アクティブ要素のアクティブレイアウトを一度に解決してキャッシュ
        cx.layouts.lay_resolved_basic.clear();
        cx.layouts.lay_resolved_flex.clear();
        cx.layouts.lay_resolved_grid.clear();

        for &id in &cx.topology.topo_flat_dfs_sequence {
            LayoutStore::update_resolved_active_layout_cache(
                id,
                &cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_resolved_basic,
                &mut cx.layouts.lay_resolved_flex,
                &mut cx.layouts.lay_resolved_grid,
                &cx.layouts.lay_basic,
                &cx.layouts.lay_flex,
                &cx.layouts.lay_grid,
                &cx.renders.rnd_visual,
                &cx.renders.rnd_interaction,
                &cx.renders.rnd_active_transitions,
            );
        }

        // 全スクロールバー関連IDを一括抽出
        let scrollbar_el_ids = Pipeline::scrollbar_el_ids(&cx.layouts.scrollbar.bar_styles);

        // Taffy永続ツリーへの差分同期
        Pipeline::sync_dirty_styles_to_taffy(
            &scrollbar_el_ids,
            &mut cx.layouts.lay_taffy_tree,
            &cx.layouts.lay_dirty_entities,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_resolved_basic,
            &cx.layouts.lay_resolved_flex,
            &cx.layouts.lay_resolved_grid,
            &cx.layouts.scrollbar.bar_styles,
        );

        // Taffy 1回目レイアウト計算
        let root_node = *cx.layouts.lay_taffy_nodes.at(root);
        // 計測関数をクロージャとして定義
        let measure_func = |known_dims: taffy::Size<Option<f32>>,
                            available_space: taffy::Size<taffy::AvailableSpace>,
                            _node_id: taffy::NodeId,
                            context: Option<&mut EntityId>,
                            _style: &taffy::Style|
         -> taffy::Size<f32> {
            // 幅と高さの両方がすでにスタイル（known_dims）として解決されている場合はそれを最優先する
            if let (Some(w), Some(h)) = (known_dims.width, known_dims.height) {
                return taffy::Size {
                    width: w,
                    height: h,
                };
            }

            // テキスト内容を持っているかチェック
            // クロージャの外側の Context は直接キャプチャできないため、
            //  一時的に bind_context されているスレッドローカル経由で取得
            context.as_deref().copied().map_or(taffy::Size::ZERO, |id| {
                let flex = cx
                    .layouts
                    .lay_resolved_flex
                    .get(id)
                    .copied()
                    .unwrap_or_default();

                ContentStore::measure_content(
                    id,
                    known_dims,
                    available_space,
                    &flex,
                    &mut cx.system.sys_text_engine,
                    &mut cx.contents.cont_input_contents,
                    &cx.contents.cont_text_contents,
                    &cx.contents.cont_text_spans,
                    &cx.topology.topo_active_masks,
                    &cx.renders.rnd_visual,
                )
            })
        };

        cx.layouts
            .lay_taffy_tree
            .compute_layout_with_measure(
                root_node,
                taffy::Size {
                    width: taffy::AvailableSpace::Definite(window_size.width),
                    height: taffy::AvailableSpace::Definite(window_size.height),
                },
                measure_func,
            )
            .unwrap();

        // ダブルバッファをスワップし、1回目の出力座標を決定
        // scroll_size を正しく算出するため、スワップおよび一旦コンテンツの out_rects のみを確定
        std::mem::swap(
            &mut cx.outputs.out_rects.0,
            &mut cx.outputs.out_prev_rects.0,
        );
        std::mem::swap(
            &mut cx.outputs.out_clip_rects.0,
            &mut cx.outputs.out_prev_clip_rects.0,
        );
        cx.outputs.out_rects.clear();
        cx.outputs.out_clip_rects.clear();

        Pipeline::resolve_first_pass_rects(
            &scrollbar_el_ids,
            window_size,
            window_resized,
            &mut cx.contents.cont_input_contents,
            &mut cx.topology.topo_active_entities,
            &cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &cx.topology.topo_flat_dfs_sequence,
            &cx.layouts.lay_taffy_tree,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_basic,
            &mut cx.outputs.out_rects,
            &mut cx.outputs.out_clip_rects,
            &cx.outputs.out_prev_rects,
            &cx.outputs.out_prev_clip_rects,
            &cx.states.scroll.sc_offsets,
        );

        // 全スクロールコンテナの scroll_size を事前計算
        cx.states.scroll.sc_sizes.clear();
        for &id in &cx.topology.topo_flat_dfs_sequence {
            if cx
                .topology
                .topo_active_masks
                .at(id)
                .has(ComponentMask::STYLE_OVERFLOW)
            {
                let size = ScrollStore::get_scroll_size(
                    id,
                    &mut cx.system.sys_text_engine,
                    &cx.system.sys_text_buffers,
                    &cx.contents.cont_text_contents,
                    &cx.contents.cont_text_spans,
                    &cx.contents.cont_input_contents,
                    &cx.topology.topo_active_masks,
                    &cx.topology.topo_parents,
                    &cx.topology.topo_children,
                    &cx.layouts.lay_resolved_basic,
                    &cx.layouts.lay_resolved_flex,
                    &cx.layouts.scrollbar.bar_styles,
                    &cx.renders.rnd_visual,
                    &cx.renders.rnd_interaction,
                    &cx.renders.rnd_active_transitions,
                    &cx.outputs.out_rects,
                    &cx.states.scroll.sc_offsets,
                );
                cx.states.scroll.sc_sizes.insert(id, size);
            }
        }

        // スクロールバー要素（Track & Thumb）のサイズ・配置・不透明度を一括同期更新
        ScrollbarStore::sync_bar_styles(
            cx.window.win_last_size,
            &cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &mut cx.layouts.lay_taffy_tree,
            &mut cx.layouts.lay_basic,
            &mut cx.layouts.lay_base_basic,
            &mut cx.layouts.lay_resolved_basic,
            &mut cx.layouts.lay_resolved_flex,
            &mut cx.layouts.lay_resolved_grid,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_flex,
            &cx.layouts.lay_grid,
            &cx.layouts.scrollbar.bar_styles,
            &mut cx.renders.rnd_visual,
            &mut cx.renders.rnd_base_visual,
            &cx.renders.rnd_interaction,
            &cx.renders.rnd_active_transitions,
            &cx.outputs.out_rects,
            &cx.states.scroll.sc_offsets,
            &cx.states.scroll.sc_sizes,
        );

        // Taffy の 2回目レイアウト計算（スクロールバー配置確定後）
        let root_node = *cx.layouts.lay_taffy_nodes.at(root);
        cx.layouts
            .lay_taffy_tree
            .compute_layout_with_measure(
                root_node,
                taffy::Size {
                    width: taffy::AvailableSpace::Definite(window_size.width),
                    height: taffy::AvailableSpace::Definite(window_size.height),
                },
                |known_dims: taffy::Size<Option<f32>>,
                 _available_space: taffy::Size<taffy::AvailableSpace>,
                 _node_id: taffy::NodeId,
                 context: Option<&mut EntityId>,
                 _style: &taffy::Style|
                 -> taffy::Size<f32> {
                    context.as_deref().copied().map_or(taffy::Size::ZERO, |id| {
                        let is_input = cx.topology.topo_active_masks.at(id).has_input_content();
                        if is_input {
                            // マスクがあるなら Some のはず
                            let contents = cx.contents.cont_input_contents.at(id);
                            if let Some(layout_rect) = contents.last_layout {
                                return taffy::Size {
                                    width: known_dims.width.unwrap_or(layout_rect.width),
                                    height: known_dims.height.unwrap_or(layout_rect.height),
                                };
                            }
                        }

                        // 2回目パスはキャッシュサイズを即時引き出して高速マッピング
                        cx.outputs
                            .out_rects
                            .get(id)
                            .map_or(taffy::Size::ZERO, |rect| taffy::Size {
                                width: known_dims.width.unwrap_or(rect.width),
                                height: known_dims.height.unwrap_or(rect.height),
                            })
                    })
                },
            )
            .unwrap();

        // スクロールバーも加えた、最終的な出力座標の決定
        Pipeline::resolve_final_pass_rects(
            window_size,
            &mut cx.contents.cont_input_contents,
            &mut cx.topology.topo_active_entities,
            &cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &cx.topology.topo_flat_dfs_sequence,
            &cx.layouts.lay_taffy_tree,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_basic,
            &mut cx.outputs.out_rects,
            &mut cx.outputs.out_clip_rects,
            &cx.states.scroll.sc_offsets,
        );

        // リサイズ追従に伴い、インプットのキャレット・選択ハイライトを同期
        for &id in &cx.topology.topo_flat_dfs_sequence {
            let has_input = cx.topology.topo_active_masks.at(id).has_input_content();
            let is_focused = cx.events.evt_interaction_states.focused == Some(id);
            // フォーカスを得ている入力要素のみ、レイアウト確定後にキャレット・スクロールを同期
            if has_input && is_focused {
                TextEditStore::update_input_caret_position(
                    id,
                    cx.window.win_scale_factor,
                    cx.window.win_last_size,
                    &mut cx.system.sys_text_engine,
                    &cx.system.sys_text_buffers,
                    &mut cx.contents.cont_text_contents,
                    &mut cx.contents.cont_input_contents,
                    &cx.contents.cont_text_spans,
                    &mut cx.topology.topo_active_masks,
                    &cx.topology.topo_parents,
                    &mut cx.layouts.lay_dirty_entities,
                    &mut cx.layouts.lay_taffy_tree,
                    &mut cx.layouts.scrollbar.bar_styles,
                    &cx.layouts.lay_taffy_nodes,
                    &cx.layouts.lay_resolved_basic,
                    &cx.layouts.lay_resolved_flex,
                    &cx.layouts.lay_resolved_grid,
                    &mut cx.renders.rnd_visual,
                    &cx.renders.rnd_base_visual,
                    &cx.renders.rnd_interaction,
                    &cx.renders.rnd_active_transitions,
                    &mut cx.states.scroll.sc_offsets,
                    &mut cx.states.edit.edit_selections,
                    &cx.outputs.out_rects,
                    &cx.states.scroll.sc_sizes,
                );
            }
        }

        // 全アクティブコンテナのスクロールオフセット自動クランプ同期
        for &id in &cx.topology.topo_flat_dfs_sequence {
            let Some(current) = cx.states.scroll.sc_offsets.get(id).copied() else {
                continue;
            };
            // 枠サイズの変更など、現在のスクロール位置からはみ出していれば自動クランプ調整
            ScrollStore::scroll_to(
                id,
                current.x,
                current.y,
                cx.window.win_last_size,
                &mut cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_taffy_tree,
                &mut cx.layouts.scrollbar.bar_styles,
                &cx.layouts.lay_taffy_nodes,
                &cx.layouts.lay_resolved_basic,
                &cx.renders.rnd_visual,
                &cx.renders.rnd_interaction,
                &cx.renders.rnd_active_transitions,
                &mut cx.states.scroll.sc_offsets,
                &cx.outputs.out_rects,
                &cx.states.scroll.sc_sizes,
            );
        }

        // 全ての座標確定と絶対クリップ範囲の同期が完了した最末尾で、
        // 一括して Dirty フラグの完全クリアおよびキューリストのリセットを実行
        LayoutStore::clear_layout_dirty(
            &mut cx.topology.topo_active_masks,
            &mut cx.layouts.lay_dirty_entities,
        );
    }

    #[inline]
    pub(crate) fn tick_system_frame(cx: &mut Context, tick: &TickType) {
        let _context_guard = bind_context(cx);

        if *tick == TickType::Transition
            || *tick == TickType::TransitionAndAnimation
            || *tick == TickType::All
        {
            RenderStore::tick_transitions(
                &mut cx.topology.topo_active_masks,
                &mut cx.topology.topo_is_sort_dirty,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_taffy_tree,
                &mut cx.layouts.lay_basic,
                &cx.layouts.lay_taffy_nodes,
                &mut cx.renders.rnd_dirty_entities,
                &mut cx.renders.rnd_visual,
                &mut cx.renders.rnd_active_transitions,
                &mut cx.renders.rnd_last_tick_time,
            );
        }
        if *tick == TickType::Animation
            || *tick == TickType::TransitionAndAnimation
            || *tick == TickType::All
        {
            RenderStore::tick_animations(
                &mut cx.topology.topo_active_masks,
                &mut cx.topology.topo_is_sort_dirty,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_taffy_tree,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_basic,
                &cx.layouts.lay_taffy_nodes,
                &mut cx.renders.rnd_dirty_entities,
                &mut cx.renders.rnd_visual,
                &mut cx.renders.rnd_active_animations,
            );
        }
        if *tick == TickType::AutoScroll || *tick == TickType::All {
            let Some(id) = cx.events.evt_interaction_states.pressed else {
                return;
            };
            let (autoscroll_occurred, active_pos) = ScrollStore::autoscroll_occurred(
                id,
                cx.window.win_last_size,
                cx.events.evt_current_pointer_position,
                &mut cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_taffy_tree,
                &mut cx.layouts.scrollbar.bar_styles,
                &cx.layouts.lay_taffy_nodes,
                &cx.layouts.lay_resolved_basic,
                &cx.renders.rnd_visual,
                &cx.renders.rnd_interaction,
                &cx.renders.rnd_active_transitions,
                &mut cx.states.scroll.sc_offsets,
                &cx.outputs.out_rects,
                &cx.outputs.out_clip_rects,
                &cx.states.scroll.sc_sizes,
            );

            if autoscroll_occurred && let Some(pos) = active_pos {
                // スクロールによりテキストが流れたため、
                // 現在のポインタ座標で仮想的にポインタ移動を再トリガーし、
                // 選択文字インデックスおよびキャレット位置を同期
                EventStore::inject_pointer_move(cx, pos);
                RenderStore::mark_render_dirty(
                    id,
                    &mut cx.topology.topo_active_masks,
                    &mut cx.renders.rnd_dirty_entities,
                );
            }
        }
    }

    #[inline]
    pub(crate) fn collect_render_data(cx: &mut Context, view: &mut RendererView) {
        let default_visual = VisualProperty::default();
        TopologyStore::prepare_sorted_entities(
            cx.window.win_last_size,
            &mut cx.topology.topo_active_masks,
            &mut cx.topology.topo_dfs_indices,
            &mut cx.topology.topo_effective_z_indices,
            &mut cx.topology.topo_sorted_entities,
            &mut cx.topology.topo_sort_cache,
            &mut cx.topology.topo_is_sort_dirty,
            &cx.topology.topo_active_entities,
            &cx.topology.topo_parents,
            &cx.topology.topo_flat_dfs_sequence,
            &cx.renders.rnd_visual,
            &mut cx.outputs.out_clip_rects,
            &cx.outputs.out_rects,
        );

        let mut force_full_scan = false;
        for &id in &*cx.topology.topo_sorted_entities {
            let engines = cx.system.sys_text_buffers.borrow();
            let Some(engine) = engines.get(id) else {
                continue;
            };

            let mask = cx.topology.topo_active_masks.at(id);
            let is_dirty_text = mask.has_text_content() && mask.has_queued_layout_or_render();

            if is_dirty_text {
                let cleared = Pipeline::scan_and_register_element_glyphs(
                    id,
                    view,
                    engine,
                    &default_visual,
                    &mut cx.system.sys_text_engine,
                    cx.window.win_scale_factor,
                    &cx.topology.topo_active_masks,
                    &cx.contents.cont_text_contents,
                    &cx.contents.cont_text_spans,
                    &cx.contents.cont_input_contents,
                    &cx.renders.rnd_visual,
                );

                if cleared {
                    // このスキャン中にアトラスのクリアが起きたため再構築が必要
                    force_full_scan = true;
                }
            }
        }
        if force_full_scan {
            for &id in &*cx.topology.topo_sorted_entities {
                let engines = cx.system.sys_text_buffers.borrow();
                let Some(engine) = engines.get(id) else {
                    continue;
                };

                let has_text_content = cx.topology.topo_active_masks.at(id).has_text_content();

                if has_text_content {
                    let _ = Pipeline::scan_and_register_element_glyphs(
                        id,
                        view,
                        engine,
                        &default_visual,
                        &mut cx.system.sys_text_engine,
                        cx.window.win_scale_factor,
                        &cx.topology.topo_active_masks,
                        &cx.contents.cont_text_contents,
                        &cx.contents.cont_text_spans,
                        &cx.contents.cont_input_contents,
                        &cx.renders.rnd_visual,
                    );
                }
            }
        }

        view.render_data.clear();
        let mut last_flushed_offset = 0;
        let mut current_batch_type = BatchType::Normal;
        let mut last_clip = None;
        for &id in &*cx.topology.topo_sorted_entities {
            let rect = *cx.outputs.out_rects.at(id);
            if rect.width <= 0.0 || rect.height <= 0.0 {
                continue;
            }
            let clip = *cx.outputs.out_clip_rects.at(id);

            let basic = cx.layouts.lay_resolved_basic.get_or(id, &DEFAULT_BASIC);
            let flex = cx.layouts.lay_resolved_flex.get_or(id, &DEFAULT_FLEX);
            let grid = cx
                .layouts
                .lay_resolved_grid
                .get(id)
                .cloned()
                .unwrap_or_default();
            let visual = cx.renders.rnd_visual.get_or(id, &default_visual);

            let (border, padding) =
                LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);
            let scroll = cx.states.scroll.sc_offsets.get_or_default(id);

            // トランスフォームブランチの場合のみその場で累積を解決
            // それ以外は IDENTITY_MATRIX
            let eff_transform = if cx
                .topology
                .topo_active_masks
                .at(id)
                .has(ComponentMask::STATE_TRANSFORM_ACTIVE)
            {
                RenderStore::resolve_effective_transform(
                    id,
                    &cx.topology.topo_parents,
                    &cx.renders.rnd_visual,
                )
            } else {
                IDENTITY_MATRIX
            };

            let params = CommonParameters::new(id, rect, basic, visual, eff_transform);

            let is_webview = cx.topology.topo_active_masks.at(id).has_webveiw2_content();
            // コントローラーがまだ初期化されていない場合は通常通り背景を描画し透過を防止
            let is_webview_ready = is_webview && cx.renders.rnd_active_webviews.contains(&id);
            // WebView (アクティブ) の個別処理
            if is_webview_ready {
                // 溜まっている通常のバッチがあれば一旦フラッシュ
                Pipeline::flush_batch(
                    &mut view.render_data.batches,
                    view.render_data.instances.len(),
                    &mut last_flushed_offset,
                    last_clip.unwrap_or_default(),
                    current_batch_type,
                );

                Pipeline::push_punchout_instance(id, view.render_data, &params);
                // くり抜き用のバッチとして即座にフラッシュ
                Pipeline::flush_batch(
                    &mut view.render_data.batches,
                    view.render_data.instances.len(),
                    &mut last_flushed_offset,
                    clip,
                    BatchType::Punchout,
                );

                Pipeline::push_front_instance(id, view.render_data, &params);

                current_batch_type = BatchType::Normal;
                last_clip = Some(clip);
                continue;
            }

            // WebView (非アクティブ・静止キャッシュ) の処理
            let is_webview_static = is_webview && !is_webview_ready;
            if is_webview_static {
                // 一般UIインスタンスがあれば強制フラッシュ
                Pipeline::flush_batch(
                    &mut view.render_data.batches,
                    view.render_data.instances.len(),
                    &mut last_flushed_offset,
                    last_clip.unwrap_or_default(),
                    current_batch_type,
                );

                Pipeline::push_static_instance(id, view.render_data, &params);
                Pipeline::flush_batch(
                    &mut view.render_data.batches,
                    view.render_data.instances.len(),
                    &mut last_flushed_offset,
                    clip,
                    BatchType::Normal,
                );

                // 前面インスタンス
                Pipeline::push_static_front_instance(id, view.render_data, &params);
                Pipeline::flush_batch(
                    &mut view.render_data.batches,
                    view.render_data.instances.len(),
                    &mut last_flushed_offset,
                    clip,
                    BatchType::Normal,
                );

                last_clip = Some(clip);
                continue;
            }

            // 外部テクスチャ
            let is_external_texture = cx
                .topology
                .topo_active_masks
                .at(id)
                .has(ComponentMask::COMP_EXTERNAL_TEXTURE_CONTENT);
            if is_external_texture {
                // 既存UIインスタンスをフラッシュ
                Pipeline::flush_batch(
                    &mut view.render_data.batches,
                    view.render_data.instances.len(),
                    &mut last_flushed_offset,
                    last_clip.unwrap_or_default(),
                    current_batch_type,
                );

                Pipeline::push_external_texture_instance(
                    id,
                    view.render_data,
                    &params,
                    &cx.contents.cont_external_textures,
                );
                // テクスチャが固有に切り替わるため独立してバッチをフラッシュ
                Pipeline::flush_batch(
                    &mut view.render_data.batches,
                    view.render_data.instances.len(),
                    &mut last_flushed_offset,
                    clip,
                    BatchType::Normal,
                );

                // 前面インスタンス
                Pipeline::push_static_front_instance(id, view.render_data, &params);
                Pipeline::flush_batch(
                    &mut view.render_data.batches,
                    view.render_data.instances.len(),
                    &mut last_flushed_offset,
                    clip,
                    BatchType::Normal,
                );

                last_clip = Some(clip);
                continue;
            }

            // 一般要素
            if let Some(prev_clip) = last_clip {
                if clip != prev_clip {
                    Pipeline::flush_batch(
                        &mut view.render_data.batches,
                        view.render_data.instances.len(),
                        &mut last_flushed_offset,
                        prev_clip,
                        current_batch_type,
                    );
                    last_clip = Some(clip);
                }
            } else {
                last_clip = Some(clip);
            }

            // 選択ハイライト背景
            if let Some(sel_rects) = cx.states.edit.edit_selected_rects.get(id)
                && let Some(buffer) = SystemStore::get_or_create_layout(
                    id,
                    &mut cx.system.sys_text_engine,
                    &cx.system.sys_text_buffers,
                    &cx.contents.cont_text_contents,
                    &cx.contents.cont_text_spans,
                    &cx.layouts.lay_resolved_basic,
                    &cx.layouts.lay_resolved_flex,
                    &cx.renders.rnd_visual,
                    &cx.outputs.out_rects,
                )
            {
                let align_offset = Pipeline::text_size_to_align_offset(
                    id,
                    &params,
                    &buffer,
                    border,
                    padding,
                    flex,
                    &cx.system.sys_text_engine,
                    &cx.contents.cont_input_contents,
                );

                Pipeline::push_selection_highlight_instances(
                    id,
                    view.render_data,
                    &params,
                    align_offset,
                    border,
                    padding,
                    scroll,
                    sel_rects,
                    visual,
                );
            }

            // 背景色とテキスト
            let is_text = cx.topology.topo_active_masks.at(id).has_text_content();
            let has_bg = visual.bg_color.is_some()
                || visual.bg_gradient.is_some()
                || visual.border_color.is_some()
                || visual.shadow_params.is_some();

            if has_bg {
                Pipeline::push_background_instance(id, view.render_data, &params, visual);
            }

            if is_text
                && let Some(buffer) = SystemStore::get_or_create_layout(
                    id,
                    &mut cx.system.sys_text_engine,
                    &cx.system.sys_text_buffers,
                    &cx.contents.cont_text_contents,
                    &cx.contents.cont_text_spans,
                    &cx.layouts.lay_resolved_basic,
                    &cx.layouts.lay_resolved_flex,
                    &cx.renders.rnd_visual,
                    &cx.outputs.out_rects,
                )
            {
                let align_offset = Pipeline::text_size_to_align_offset(
                    id,
                    &params,
                    &buffer,
                    border,
                    padding,
                    flex,
                    &cx.system.sys_text_engine,
                    &cx.contents.cont_input_contents,
                );

                let spans = cx
                    .contents
                    .cont_text_spans
                    .get(id)
                    .map_or(&[][..], Vec::as_slice);
                let resolved_color =
                    Pipeline::resolve_text_color(id, visual, &cx.contents.cont_input_contents);

                Pipeline::push_text_background_instances(
                    id,
                    view.render_data,
                    &params,
                    &buffer,
                    spans,
                    align_offset,
                    border,
                    padding,
                    scroll,
                );

                Pipeline::push_text_metric_instances(
                    id,
                    view,
                    &params,
                    &buffer,
                    resolved_color,
                    border,
                    padding,
                    scroll,
                    align_offset,
                    cx.window.win_scale_factor,
                    &mut cx.system.sys_text_engine,
                );

                Pipeline::push_text_front_instances(
                    id,
                    view.render_data,
                    &params,
                    spans,
                    &buffer,
                    border,
                    padding,
                    scroll,
                    align_offset,
                    resolved_color,
                );
            }

            // 通常要素（テキスト以外）の背景マウントは親ループですでに has_bg が完了しているため不要
            // 背景指定がない場合でもボーダー単体描画等が必要な場合はフォールバック
            if !is_text && !has_bg {
                Pipeline::push_fallback_border_instance(id, view.render_data, &params);
            }

            // インプット要素のキャレット
            let is_input = cx.topology.topo_active_masks.at(id).has_input_content();
            let is_focused = cx.events.evt_interaction_states.focused == Some(id);

            if is_input && is_focused {
                Pipeline::push_caret_instance(
                    id,
                    view.render_data,
                    &params,
                    border,
                    padding,
                    scroll,
                    flex,
                    visual,
                    cx.window.win_scale_factor,
                    &cx.contents.cont_input_contents,
                    &cx.renders.rnd_base_visual,
                );
            }
        }
        Pipeline::flush_batch(
            &mut view.render_data.batches,
            view.render_data.instances.len(),
            &mut last_flushed_offset,
            last_clip.unwrap_or_default(),
            current_batch_type,
        );
    }
}

struct CommonParameters {
    rect: LayoutRect,
    transform: [[f32; 4]; 3],
    corner_radius: CornerRadius,
    border_width: EdgeInsets,
    border_color: Color,
    transform_origin: [f32; 2],
    border_lengths: EdgeInsets,
    outline_width: EdgeInsets,
    outline_color: Color,
    outline_lengths: EdgeInsets,
    outline_offset_and_flags: [f32; 4],
    opacity: f32,
    box_sizing_val: f32,
}

impl CommonParameters {
    #[inline]
    fn new(
        id: EntityId,
        rect: LayoutRect,
        basic: &BasicLayout,
        visual: &VisualProperty,
        eff_transform: [[f32; 4]; 4],
    ) -> Self {
        let (transform, transform_origin) =
            RenderStore::get_transform_and_origin(id, visual, eff_transform);
        let (outline_width, outline_color, outline_lengths, outline_offset_and_flags) =
            RenderStore::get_outline_params(visual);
        let corner_radius = visual.corner_radius.unwrap_or_default();
        let border_width = EdgeInsets {
            top: basic.border.top.into(),
            right: basic.border.right.into(),
            bottom: basic.border.bottom.into(),
            left: basic.border.left.into(),
        };
        let border_color = visual.border_color.unwrap_or_default();
        let border_lengths = visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0));
        let opacity = visual.opacity.unwrap_or(1.0);
        let box_sizing_val = match basic.box_sizing {
            BoxSizing::BorderBox => 0.0f32,
            BoxSizing::ContentBox => 1.0f32,
        };

        Self {
            rect,
            transform,
            corner_radius,
            border_width,
            border_color,
            transform_origin,
            border_lengths,
            outline_width,
            outline_color,
            outline_lengths,
            outline_offset_and_flags,
            opacity,
            box_sizing_val,
        }
    }
}

impl Pipeline {
    // 全スクロールバー関連IDを一括抽出
    #[inline]
    fn scrollbar_el_ids(
        bar_styles: &SparseSecondaryMap<EntityId, ScrollBarState>,
    ) -> HashSet<EntityId> {
        bar_styles
            .values()
            .flat_map(|sb_state| {
                [
                    sb_state.v_track_id,
                    sb_state.v_thumb_id,
                    sb_state.h_track_id,
                    sb_state.h_thumb_id,
                ]
                .into_iter()
                .flatten()
            })
            .collect()
    }

    /// Taffy永続ツリーへのスタイル差分同期
    fn sync_dirty_styles_to_taffy(
        scrollbar_el_ids: &HashSet<EntityId>,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_dirty_entities: &DirtyLayoutEntitiesVec,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparse,
        bar_styles: &ScrollbarStylesSecondary,
    ) {
        for &id in lay_dirty_entities {
            if scrollbar_el_ids.contains(&id) {
                continue;
            }

            let basic = lay_resolved_basic.get_or(id, &DEFAULT_BASIC);
            let flex = lay_resolved_flex.get_or(id, &DEFAULT_FLEX);
            let grid = lay_resolved_grid.get(id).cloned();

            // トランジション（アニメーション）中プロパティの現在値による上書き
            // 削除：resolve_active_layouts の段階でアニメーション中のサイズが正しく反映されたレイアウト）が返ってくるため
            // if let Some(active_list) = rnd_active_transitions.get(id) {}

            let taffy_style =
                LayoutStore::resolve_taffy_style(id, &basic, &flex, grid.as_ref(), bar_styles);

            let taffy_node = *lay_taffy_nodes.at(id);
            lay_taffy_tree.set_style(taffy_node, taffy_style).unwrap();
        }
    }

    /// 物理位置を算出して、rects / `clip_rects` と入力状態へマウント
    fn update_element_output_rect_and_clip(
        id: EntityId,
        window_size: LayoutSize,
        cont_input_contents: &mut InputContentsSparse,
        topo_active_entities: &mut ActiveEntitiesVec,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_taffy_tree: &TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_basic: &BasicLayoutsSecondary,
        out_rects: &mut RectsSecondary,
        out_clip_rects: &mut ClipRectsSecondary,
        sc_offsets: &ScrollOffsetsSecondary,
    ) {
        let (abs_rect, parent_clip) = OutputStore::calc_local_rect(
            id,
            window_size,
            topo_parents,
            lay_taffy_tree,
            lay_taffy_nodes,
            lay_basic,
            out_rects,
            out_clip_rects,
            sc_offsets,
        );

        out_rects.insert(id, abs_rect);

        let mask = topo_active_masks.at(id);

        if mask.has_input_content() {
            // マスクがあるなら Some のはず
            let contents = cont_input_contents.at_mut(id);
            contents.last_bounds = Some(abs_rect);
        }

        let current_clip = if mask.has(ComponentMask::STYLE_OVERFLOW) {
            parent_clip.intersect(&abs_rect)
        } else {
            parent_clip
        };

        out_clip_rects.insert(id, current_clip);
        topo_active_entities.push(id);
    }

    /// 1回目の出力領域決定（静的キャッシュバイパス判定含む）
    fn resolve_first_pass_rects(
        scrollbar_el_ids: &HashSet<EntityId>,
        window_size: LayoutSize,
        window_resized: bool,
        cont_input_contents: &mut InputContentsSparse,
        topo_active_entities: &mut ActiveEntitiesVec,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_flat_dfs_sequence: &FlatDfsSequenceVec,
        lay_taffy_tree: &TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_basic: &BasicLayoutsSecondary,
        out_rects: &mut RectsSecondary,
        out_clip_rects: &mut ClipRectsSecondary,
        out_prev_rects: &PrevRectsSecondary,
        out_prev_clip_rects: &PrevClipRectsSecondary,
        sc_offsets: &ScrollOffsetsSecondary,
    ) {
        topo_active_entities.clear();

        for &id in topo_flat_dfs_sequence {
            // スクロールバー専用子要素は手動で物理座標を強制更新するため、この走査ループから完全にスルー
            if scrollbar_el_ids.contains(&id) {
                continue;
            }

            let parent_changed = OutputStore::has_parent_changed(
                id,
                topo_active_masks,
                topo_parents,
                out_rects,
                out_clip_rects,
                out_prev_rects,
                out_prev_clip_rects,
            );

            let has_style_changed = topo_active_masks.at(id).has_queued_layout();

            // 静的キャッシュの判定と適用
            // 自分自身のスタイルが変わっておらず、親も動いていない、かつモニターリサイズもされていないならキャッシュ利用
            if !window_resized && !has_style_changed && !parent_changed {
                let cached_rect = *out_prev_rects.at(id);
                let cached_clip = *out_prev_clip_rects.at(id);

                out_rects.insert(id, cached_rect);
                out_clip_rects.insert(id, cached_clip);
                topo_active_entities.push(id);
                continue;
            }

            // キャッシュが無効な場合は共通ヘルパーで再計算
            Pipeline::update_element_output_rect_and_clip(
                id,
                window_size,
                cont_input_contents,
                topo_active_entities,
                topo_active_masks,
                topo_parents,
                lay_taffy_tree,
                lay_taffy_nodes,
                lay_basic,
                out_rects,
                out_clip_rects,
                sc_offsets,
            );
        }
    }

    /// 最終的な出力領域決定（スクロールバー要素を含む一括同期）
    fn resolve_final_pass_rects(
        window_size: LayoutSize,
        cont_input_contents: &mut InputContentsSparse,
        topo_active_entities: &mut ActiveEntitiesVec,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_flat_dfs_sequence: &FlatDfsSequenceVec,
        lay_taffy_tree: &TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_basic: &BasicLayoutsSecondary,
        out_rects: &mut RectsSecondary,
        out_clip_rects: &mut ClipRectsSecondary,
        sc_offsets: &ScrollOffsetsSecondary,
    ) {
        topo_active_entities.clear();

        for &id in topo_flat_dfs_sequence {
            Pipeline::update_element_output_rect_and_clip(
                id,
                window_size,
                cont_input_contents,
                topo_active_entities,
                topo_active_masks,
                topo_parents,
                lay_taffy_tree,
                lay_taffy_nodes,
                lay_basic,
                out_rects,
                out_clip_rects,
                sc_offsets,
            );
        }
    }

    // 溜まっているインスタンスを DrawBatch としてフラッシュ
    #[inline]
    fn flush_batch(
        batches: &mut Vec<DrawBatch>,
        instances_len: usize,
        last_flushed_offset: &mut usize,
        scissor_rect: LayoutRect,
        batch_type: BatchType,
    ) {
        let count = instances_len - *last_flushed_offset;
        if count == 0 {
            return;
        }

        batches.push(DrawBatch {
            scissor_rect,
            instance_offset: *last_flushed_offset,
            instance_count: count,
            batch_type,
        });

        // 次のバッチのために、現在の末尾位置を記録しておく
        *last_flushed_offset = instances_len;
    }

    /// UTF-16スライスからサロゲートペアを考慮して1文字を抽出、進めるべき長さを返す
    #[inline]
    fn get_char_and_u16_len(text_u16: &[u16], char_idx: usize) -> (char, usize) {
        if char_idx + 1 < text_u16.len() && (0xD800..=0xDBFF).contains(&text_u16[char_idx]) {
            let u16_chars = &text_u16[char_idx..char_idx + 2];
            let character = String::from_utf16(u16_chars)
                .ok()
                .and_then(|s| s.chars().next())
                .unwrap_or(' ');
            (character, 2)
        } else {
            let character = char::from_u32(text_u16[char_idx] as u32).unwrap_or(' ');
            (character, 1)
        }
    }

    fn text_size_to_align_offset(
        id: EntityId,
        params: &CommonParameters,
        buffer: &Buffer,
        border: EdgeInsets,
        padding: EdgeInsets,
        flex: &FlexLayout,
        sys_text_engine: &TextEngine,
        cont_input_contents: &InputContentsSparse,
    ) -> LayoutPoint {
        let (text_size, is_multiline) = if let Some(c) = cont_input_contents.get(id) {
            if let Some(l) = c.last_layout {
                // コンテンツが存在し前回のレイアウトもある場合
                (LayoutSize::new(l.width, l.height), c.is_multiline)
            } else {
                // コンテンツはあるがレイアウトがない場合
                (sys_text_engine.get_layout_size(buffer), c.is_multiline)
            }
        } else {
            // コンテンツ自体が存在しない場合
            (sys_text_engine.get_layout_size(buffer), false)
        };
        OutputStore::calc_align_offset(
            params.rect,
            border,
            padding,
            text_size,
            flex.text_align,
            flex.align_items,
            is_multiline,
        )
    }

    /// 指定された要素に含まれるすべての文字をアトラスにキャッシュ
    /// このフレームでアトラスの一括クリアが起きた場合は true
    #[inline]
    fn scan_and_register_element_glyphs(
        id: EntityId,
        view: &mut RendererView,
        buffer: &Buffer,
        default_visual: &VisualProperty,
        sys_text_engine: &mut TextEngine,
        win_scale_factor: f32,
        topo_active_masks: &ActiveMasksSecondary,
        cont_text_contents: &TextContentsSparse,
        cont_text_spans: &TextSpansSparse,
        cont_input_contents: &InputContentsSparse,
        rnd_visual: &VisualPropertiesSecondary,
    ) -> bool {
        let mut atlas_cleared = false;

        // レイアウト結果からグリフを直接取り出してキャッシュ
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                let offset = (glyph.x_offset, glyph.y_offset);
                let physical = glyph.physical(offset, win_scale_factor);
                let (_, cleared) = sys_text_engine.get_or_create_glyph_uv(
                    physical.cache_key,
                    view,
                    win_scale_factor,
                );
                if cleared {
                    atlas_cleared = true;
                }
            }
        }

        atlas_cleared
    }

    #[inline]
    fn resolve_text_color(
        id: EntityId,
        visual: &VisualProperty,
        cont_input_contents: &InputContentsSparse,
    ) -> Color {
        // 通常のテキスト色
        let default_color = visual.text_color.unwrap_or(Color::WHITE);

        // そもそも入力コンポーネントを持っていないなら通常色を返して終了
        let Some(input) = cont_input_contents.get(id) else {
            return default_color;
        };

        // IME変換中かどうか
        let is_ime_active = input
            .ime_state
            .as_ref()
            .is_some_and(|ime| !ime.composition_text.is_empty());

        // テキストが空 かつ IME非アクティブならプレースホルダー色を採用
        if input.text_empty() && !is_ime_active {
            input
                .placeholder_color
                .unwrap_or(Color::rgb_f32(0.5, 0.5, 0.5))
        } else {
            default_color
        }
    }

    /// パンチアウト用インスタンスを追加
    #[inline]
    fn push_punchout_instance(
        id: EntityId,
        render_data: &mut RenderData,
        params: &CommonParameters,
    ) {
        let punchout_instance = QuadInstance {
            rect: params.rect,
            transform: params.transform,
            transform_origin: params.transform_origin,
            color: Color::WHITE,
            corner_radius: params.corner_radius,
            opacity_mode_sizing: [params.opacity, 0.0, 0.0, 0.0],
            ..Default::default()
        };

        render_data.push(id, punchout_instance);
    }

    /// 前面装飾用インスタンスを追加
    #[inline]
    fn push_front_instance(id: EntityId, render_data: &mut RenderData, params: &CommonParameters) {
        let front_instance = QuadInstance {
            rect: params.rect,
            transform: params.transform,
            corner_radius: params.corner_radius,
            border_width: params.border_width,
            border_color: params.border_color,
            opacity_mode_sizing: [params.opacity, 0.0, 0.0, 0.0],
            transform_origin: params.transform_origin,
            border_lengths: params.border_lengths,
            outline_width: params.outline_width,
            outline_color: params.outline_color,
            outline_lengths: params.outline_lengths,
            outline_offset_and_flags: params.outline_offset_and_flags,
            ..Default::default()
        };

        render_data.push(id, front_instance);
    }

    /// webview2静止時用インスタンスを追加
    #[inline]
    fn push_static_instance(id: EntityId, render_data: &mut RenderData, params: &CommonParameters) {
        let static_instance = QuadInstance {
            rect: params.rect,
            transform: params.transform,
            corner_radius: params.corner_radius,
            border_width: params.border_width,
            border_color: params.border_color,
            opacity_mode_sizing: [params.opacity, 0.0, 0.0, 0.0],
            transform_origin: params.transform_origin,
            border_lengths: params.border_lengths,
            ..Default::default()
        };
        render_data.push(id, static_instance);
    }

    /// webview2静止時用前面インスタンスを追加
    #[inline]
    fn push_static_front_instance(
        id: EntityId,
        render_data: &mut RenderData,
        params: &CommonParameters,
    ) {
        let border_instance = QuadInstance {
            rect: params.rect,
            transform: params.transform,
            transform_origin: params.transform_origin,
            corner_radius: params.corner_radius,
            border_width: params.border_width,
            border_color: params.border_color,
            border_lengths: params.border_lengths,
            opacity_mode_sizing: [params.opacity, 0.0, 0.0, 0.0],
            shadow_color: Color::WHITE,
            outline_width: params.outline_width,
            outline_color: params.outline_color,
            outline_lengths: params.outline_lengths,
            outline_offset_and_flags: params.outline_offset_and_flags,
            ..Default::default()
        };
        render_data.push(id, border_instance);
    }

    /// 選択されているテキストの背景ハイライトを計算してインスタンスを追加
    #[inline]
    fn push_selection_highlight_instances(
        id: EntityId,
        render_data: &mut RenderData,
        params: &CommonParameters,
        align_offset: LayoutPoint,
        border: EdgeInsets,
        padding: EdgeInsets,
        scroll: LayoutPoint,
        sel_rects: &Vec<LayoutRect>,
        visual: &VisualProperty,
    ) {
        let sel_bg = visual
            .select_bg_color
            .unwrap_or(Color::rgba_f32(0.0, 0.47, 0.84, 0.35));

        for metric_rect in sel_rects {
            let sel_rect = LayoutRect::new(
                params.rect.x + border.left + padding.left + align_offset.x + metric_rect.x
                    - scroll.x,
                params.rect.y + border.top + padding.top + align_offset.y + metric_rect.y
                    - scroll.y,
                metric_rect.width,
                metric_rect.height,
            );

            let sel_instance = QuadInstance {
                rect: sel_rect,
                transform: params.transform,
                color: sel_bg,
                opacity_mode_sizing: [params.opacity, -1.0, 0.0, 0.0],
                ..Default::default()
            };
            render_data.push(id, sel_instance);
        }
    }

    /// 一般要素の背景のインスタンスを追加
    #[inline]
    fn push_background_instance(
        id: EntityId,
        render_data: &mut RenderData,
        params: &CommonParameters,
        visual: &VisualProperty,
    ) {
        let bg_color = visual.bg_color.unwrap_or_default();
        let (gradient_end_color, gradient_angle, bg_mode) = match visual.bg_gradient {
            Some(g) => (g.end_color, g.angle, 1.0f32),
            None => (bg_color, 0.0, 0.0f32),
        };

        let bg_instance = QuadInstance {
            rect: params.rect,
            transform: params.transform,
            transform_origin: params.transform_origin,
            color: bg_color,
            corner_radius: params.corner_radius,
            border_width: params.border_width,
            border_color: params.border_color,
            border_lengths: params.border_lengths,
            opacity_mode_sizing: [params.opacity, bg_mode, params.box_sizing_val, 0.0],
            gradient_end_color,
            gradient_angle,
            shadow_color: Color::WHITE,
            shadow_params: [0.0; 4],
            outline_width: params.outline_width,
            outline_color: params.outline_color,
            outline_lengths: params.outline_lengths,
            outline_offset_and_flags: params.outline_offset_and_flags,
            ..Default::default()
        };
        render_data.push(id, bg_instance);
    }

    /// テキスト用背面インスタンスを追加
    #[inline]
    fn push_text_background_instances(
        id: EntityId,
        render_data: &mut RenderData,
        params: &CommonParameters,
        buffer: &Buffer,
        spans: &[TextSpan],
        align_offset: LayoutPoint,
        border: EdgeInsets,
        padding: EdgeInsets,
        scroll: LayoutPoint,
    ) {
        for span in spans {
            if let Some(bg_color) = span.bg_color {
                let rects = TextEditStore::calc_selection_rects(id, buffer, span.range.clone());

                for metric_rect in rects {
                    let sel_rect = LayoutRect::new(
                        params.rect.x + border.left + padding.left + align_offset.x + metric_rect.x
                            - scroll.x,
                        params.rect.y + border.top + padding.top + align_offset.y + metric_rect.y
                            - scroll.y,
                        metric_rect.width,
                        metric_rect.height,
                    );

                    let sel_instance = QuadInstance {
                        rect: sel_rect,
                        transform: params.transform,
                        color: bg_color,
                        opacity_mode_sizing: [params.opacity, -1.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    render_data.push(id, sel_instance);
                }
            }
        }
    }

    /// 文字ごとのインスタンスを追加
    #[inline]
    fn push_text_metric_instances(
        id: EntityId,
        view: &mut RendererView,
        params: &CommonParameters,
        buffer: &Buffer,
        resolved_color: Color,
        border: EdgeInsets,
        padding: EdgeInsets,
        scroll: LayoutPoint,
        align_offset: LayoutPoint,
        win_scale_factor: f32,
        sys_text_engine: &mut TextEngine,
    ) {
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                //  グリフ原点 ＝ テキストブロック位置 ＋ 行・文字位置を丸めたもの
                let base_phys_x = (params.rect.x + border.left + padding.left + align_offset.x
                    - scroll.x)
                    * win_scale_factor;
                let base_phys_y =
                    (params.rect.y + border.top + padding.top + align_offset.y + run.line_y
                        - scroll.y)
                        * win_scale_factor;
                let physical = glyph.physical((base_phys_x, base_phys_y), win_scale_factor);

                // アトラスにパッキングされた文字がUIの画面上で縦横何ピクセルの大きさで描画されるべきかを逆算
                // アトラス上の UV 座標を取得
                let (value, _cleared) = sys_text_engine.get_or_create_glyph_uv(
                    physical.cache_key,
                    view,
                    win_scale_factor,
                );

                // 最終的なポリゴンの左上 ＝ グリフ原点 ＋ 画像オフセット
                let char_phys_x = physical.x + value.offset_x;
                let char_phys_y = physical.y - value.offset_y; // Swash の top は上向き正のため減算

                // 論理座標に戻す
                let char_x = char_phys_x as f32 / win_scale_factor;
                let char_y = char_phys_y as f32 / win_scale_factor;

                // 文字の矩形
                let char_rect = LayoutRect::new(char_x, char_y, value.width, value.height);

                // スパンごとに指定されたカラー、指定がなければベースの文字色を採用
                let char_color = glyph.color_opt.map_or(resolved_color, |c| Color {
                    r: c.r() as f32 / 255.0,
                    g: c.g() as f32 / 255.0,
                    b: c.b() as f32 / 255.0,
                    a: c.a() as f32 / 255.0,
                });

                let glyph_instance = QuadInstance {
                    rect: char_rect,
                    transform: params.transform,
                    transform_origin: params.transform_origin,
                    color: char_color,
                    opacity_mode_sizing: [params.opacity, 2.0, params.box_sizing_val, 0.0],
                    uv_min: value.uv_min,
                    uv_max: value.uv_max,
                    ..Default::default()
                };

                view.render_data.push(id, glyph_instance);
            }
        }
    }

    /// テキスト用前面インスタンスを追加
    fn push_text_front_instances(
        id: EntityId,
        render_data: &mut RenderData,
        params: &CommonParameters,
        spans: &[TextSpan],
        buffer: &Buffer,
        border: EdgeInsets,
        padding: EdgeInsets,
        scroll: LayoutPoint,
        align_offset: LayoutPoint,
        resolved_color: Color,
    ) {
        for span in spans {
            let has_ul = span.underline.is_some();
            let has_st = span.strikethrough.is_some();
            if !has_ul && !has_st {
                continue;
            }

            let rects = TextEditStore::calc_selection_rects(id, buffer, span.range.clone());

            for metric_rect in rects {
                let start_x =
                    params.rect.x + border.left + padding.left + align_offset.x + metric_rect.x
                        - scroll.x;
                let end_x = start_x + metric_rect.width;
                let base_y =
                    params.rect.y + border.top + padding.top + align_offset.y + metric_rect.y
                        - scroll.y;

                // 打消し線（中線）
                if let Some(st_style) = span.strikethrough {
                    let st_color = span
                        .strikethrough_color
                        .or(span.color)
                        .unwrap_or(resolved_color);
                    let thickness = match st_style {
                        StrikethroughStyle::Solid => 1.0,
                        StrikethroughStyle::Thick => 2.5,
                    };
                    let st_rect = LayoutRect::new(
                        start_x,
                        (base_y + metric_rect.height * 0.5 - thickness * 0.5).round(),
                        metric_rect.width,
                        thickness,
                    );

                    let st_instance = QuadInstance {
                        rect: st_rect,
                        transform: params.transform,
                        color: st_color,
                        opacity_mode_sizing: [params.opacity, -1.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    render_data.push(id, st_instance);
                }

                // 下線
                if let Some(ul_style) = span.underline {
                    let ul_color = span
                        .underline_color
                        .or(span.color)
                        .unwrap_or(resolved_color);
                    let thickness = match ul_style {
                        UnderlineStyle::Thick => 2.5,
                        UnderlineStyle::Solid | UnderlineStyle::Wave | UnderlineStyle::Double => {
                            1.0
                        }
                    };
                    let ul_y = (base_y + metric_rect.height - thickness - 1.0).round();

                    if ul_style == UnderlineStyle::Wave {
                        let wave_amplitude = 0.5;
                        let wave_step: f32 = 2.0;
                        let mut temp_x = start_x;
                        let mut y_up = false;

                        while temp_x < end_x {
                            let seg_w = wave_step.min(end_x - temp_x);
                            let seg_y = if y_up {
                                ul_y - wave_amplitude
                            } else {
                                ul_y + wave_amplitude
                            };

                            let wave_instance = QuadInstance {
                                rect: LayoutRect::new(temp_x, seg_y, seg_w, 1.0),
                                transform: params.transform,
                                color: ul_color,
                                opacity_mode_sizing: [params.opacity, -1.0, 0.0, 0.0],
                                ..Default::default()
                            };
                            render_data.push(id, wave_instance);

                            temp_x += wave_step;
                            y_up = !y_up;
                        }
                    } else {
                        let ul_rect = LayoutRect::new(start_x, ul_y, metric_rect.width, thickness);

                        let ul_instance = QuadInstance {
                            rect: ul_rect,
                            transform: params.transform,
                            color: ul_color,
                            opacity_mode_sizing: [params.opacity, -1.0, 0.0, 0.0],
                            ..Default::default()
                        };
                        render_data.push(id, ul_instance);
                    }
                }
            }
        }
    }

    /// キャレット用インスタンスを追加
    #[inline]
    fn push_caret_instance(
        id: EntityId,
        render_data: &mut RenderData,
        params: &CommonParameters,
        border: EdgeInsets,
        padding: EdgeInsets,
        scroll: LayoutPoint,
        flex: &FlexLayout,
        visual: &VisualProperty,
        win_scale_factor: f32,
        cont_input_contents: &InputContentsSparse,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
    ) {
        // キャレットがあるなら入力コンポーネントもあるはず
        let contents = cont_input_contents.at(id);

        if !ContentStore::should_show_caret(contents) {
            return;
        }

        let text_size = if let Some(layout_rect) = contents.last_layout {
            LayoutSize::new(layout_rect.width, layout_rect.height)
        } else {
            LayoutSize::ZERO
        };

        let align_offset = OutputStore::calc_align_offset(
            params.rect,
            border,
            padding,
            text_size,
            flex.text_align,
            flex.align_items,
            contents.is_multiline,
        );

        let caret_rect = TextEditStore::calculate_caret_rect(
            params.rect,
            border,
            padding,
            contents,
            win_scale_factor,
            scroll,
            align_offset,
        );

        let c_color = contents
            .caret_color
            .or(rnd_base_visual.get(id).and_then(|v| v.text_color))
            .or(visual.text_color)
            .unwrap_or(Color::WHITE);

        let caret_instance = QuadInstance {
            rect: caret_rect,
            transform: params.transform,
            color: c_color,
            opacity_mode_sizing: [params.opacity, -1.0, 0.0, 0.0],
            ..Default::default()
        };

        render_data.push(id, caret_instance);
    }

    /// ボーダーのみ描画が必要な要素のためのフォールバック用インスタンスを追加
    #[inline]
    fn push_fallback_border_instance(
        id: EntityId,
        render_data: &mut RenderData,
        params: &CommonParameters,
    ) {
        let instance = QuadInstance {
            rect: params.rect,
            transform: params.transform,
            transform_origin: params.transform_origin,
            color: Color::TRANSPARENT,
            corner_radius: params.corner_radius,
            border_width: params.border_width,
            border_color: params.border_color,
            border_lengths: params.border_lengths,
            opacity_mode_sizing: [params.opacity, 0.0, params.box_sizing_val, 0.0],
            outline_width: params.outline_width,
            outline_color: params.outline_color,
            outline_lengths: params.outline_lengths,
            outline_offset_and_flags: params.outline_offset_and_flags,
            ..Default::default()
        };

        render_data.push(id, instance);
    }

    /// 外部テクスチャ用インスタンスを追加
    #[inline]
    fn push_external_texture_instance(
        id: EntityId,
        render_data: &mut RenderData,
        params: &CommonParameters,
        cont_external_textures: &ExternalTextureSparse,
    ) {
        let provider = cont_external_textures.at(id);
        let meta = provider.metadata();

        let alpha_val = match meta.alpha_mode {
            ExternalTextureAlphaMode::Straight => 0.0f32,
            ExternalTextureAlphaMode::Premultiplied => 1.0f32,
        };
        let y_flip_val = if meta.y_flip { -1.0f32 } else { 1.0f32 };
        let srgb_val = if meta.is_srgb { 1.0f32 } else { 0.0f32 };

        let ex_instance = QuadInstance {
            rect: params.rect,
            transform: params.transform,
            corner_radius: params.corner_radius,
            opacity_mode_sizing: [params.opacity, 4.0, 0.0, 0.0], // 外部テクスチャ
            transform_origin: params.transform_origin,
            uv_min: [0.0, 0.0],
            uv_max: [1.0, 1.0],
            alpha_mode_y_flip_srgb: [alpha_val, y_flip_val, srgb_val, 0.0],
            ..Default::default()
        };

        render_data.push(id, ex_instance);
    }
}

#[cfg(test)]
mod tests;
