use windows::Win32::Graphics::DirectWrite::{DWRITE_HIT_TEST_METRICS, IDWriteTextLayout};

use crate::{
    ActiveFocusTrigger, ActiveMasksSecondary, BaseVisualPropertiesSecondary, BasicLayout,
    BatchType, BoxSizing, Color, ComponentMask, ContentStore, Context, CornerRadius, DrawBatch,
    EdgeInsets, ElementState, EntityId, EventStore, ExternalTextureAlphaMode,
    ExternalTextureSparseSecondary, FlexLayout, IDENTITY_MATRIX, ImeState,
    InputContentsSparseSecondary, LayoutPoint, LayoutRect, LayoutSize, LayoutStore, Modifiers,
    MouseButton, OutputStore, PointerEvents, QuadInstance, ReactiveStore, RenderData, RenderStore,
    RendererView, StrikethroughStyle, SystemStore, TextCacheKey, TextContentsSparseSecondary,
    TextEngine, TextSpan, TextSpansSparseSecondary, TopologyStore, UnderlineStyle, VirtualKey,
    VisualPropertiesSecondary, VisualProperty, WindowStore, bind_context, handle_on_char_input,
    handle_on_file_dropped, handle_on_ime, with_context,
};
use std::{borrow::Cow, path::PathBuf};

#[derive(Debug, Clone, PartialEq)]
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
pub enum TickType {
    All,
    Transition,
    Animation,
    TransitionAndAnimation,
    AutoScroll,
}

#[derive(Debug, Clone, PartialEq)]
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
    /// マウス座標などが、要素の描画領域かつ表示枠内に収まっているかを判定。
    /// 階層的な早期枝刈りヒットテスト
    #[inline]
    pub(crate) fn hit_test(cx: &mut Context, point: LayoutPoint) -> Option<EntityId> {
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
        for &id in cx.topology.topo_sorted_entities.iter().rev() {
            let is_drag_over = cx
                .topology
                .topo_active_masks
                .get(id)
                .is_some_and(|mask| mask.has(ComponentMask::STATE_DND_DRAG_OVER));

            // ドラッグ中かつゴースト化した元の実体要素、およびプレースホルダー要素はヒットテストを強制スルーさせる
            if Some(id) == cx.events.evt_interaction_states.dragged || is_drag_over {
                continue;
            }

            // 物理範囲に含まれているか
            let Some(rect) = cx.outputs.out_rects.get(id).copied() else {
                continue;
            };
            if !rect.contains(point) {
                continue;
            }

            // 親などの overflow 等でクリップされている表示範囲外ならスキップ
            if let Some(clip) = cx.outputs.out_clip_rects.get(id)
                && !clip.contains(point)
            {
                continue;
            }

            // pointer-events 設定の解決
            let pointer_events = cx
                .renders
                .rnd_visual
                .get(id)
                .and_then(|v| v.pointer_events)
                .or_else(|| {
                    cx.renders
                        .rnd_base_visual
                        .get(id)
                        .and_then(|v| v.pointer_events)
                })
                .unwrap_or_default();

            if pointer_events == PointerEvents::None {
                continue; // 透過設定
            }

            return Some(id);
        }
        None
    }

    #[inline]
    pub(crate) fn inject_user_action(cx: &mut Context, action: UserAction) {
        let _context_guard = bind_context(cx);

        match action {
            UserAction::PointerMove(layout_point) => {
                EventStore::inject_pointer_move_internal(cx, layout_point);
            }
            UserAction::PointerButton {
                button,
                state,
                modifiers,
            } => EventStore::inject_pointer_button_internal(cx, button, state, modifiers),
            UserAction::PointerDoubleClick { modifiers } => {
                EventStore::inject_pointer_double_click_internal(cx, modifiers);
            }
            UserAction::MouseWheel { scroll_x, scroll_y } => {
                EventStore::inject_mouse_wheel_internal(cx, scroll_x, scroll_y);
            }
            UserAction::KeyboardKey {
                key,
                state,
                modifiers,
            } => EventStore::inject_keyboard_key_internal(cx, key, state, modifiers),
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
            UserAction::Paste(text) => EventStore::inject_paste_internal(cx, &text),
            UserAction::Cut => {
                cx.contents.cont_cut_text = EventStore::inject_cut_internal(cx);
            }
            UserAction::Undo => EventStore::inject_undo_internal(cx),
            UserAction::Redo => EventStore::inject_redo_internal(cx),
        }
    }

    #[inline]
    pub(crate) fn update_states(cx: &mut Context, id: EntityId, flag: &StateFlag, actived: bool) {
        match flag {
            StateFlag::Hovered => {
                EventStore::update_state(cx, id, ComponentMask::STATE_HOVERED, actived);
            }
            StateFlag::Focused => {
                EventStore::set_focused_by_trigger(cx, id, actived, ActiveFocusTrigger::Mouse);
            }
            StateFlag::FocusedVisible => {
                EventStore::set_focused_by_trigger(cx, id, actived, ActiveFocusTrigger::Keyboard);
            }
            StateFlag::Pressed => {
                EventStore::update_state(cx, id, ComponentMask::STATE_PRESSED, actived);
            }
            StateFlag::Disabled => {
                EventStore::update_state(cx, id, ComponentMask::STATE_DISABLED, actived);
            }
            StateFlag::Actived => {
                EventStore::update_state(cx, id, ComponentMask::STATE_ACTIVED, actived);
            }
            StateFlag::Selected => {
                EventStore::update_state(cx, id, ComponentMask::STATE_SELECTED, actived);
            }
            StateFlag::Dragged => {
                EventStore::update_state(cx, id, ComponentMask::STATE_DRAGGED, actived);
            }
            StateFlag::DndDragging => {
                EventStore::update_state(cx, id, ComponentMask::STATE_DND_DRAGGING, actived);
            }
            StateFlag::DndDragIn => {
                EventStore::update_state(cx, id, ComponentMask::STATE_DND_DRAG_IN, actived);
            }
            StateFlag::DndDragOver => {
                EventStore::update_state(cx, id, ComponentMask::STATE_DND_DRAG_OVER, actived);
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

        // レイアウトが再計算される前に、溜まっているすべてのエフェクトを評価完了させる
        ReactiveStore::evaluate_pending_element_effects(
            &mut cx.reactive.react_effects,
            &mut cx.reactive.react_pending_element_effects,
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
        let scrollbar_el_ids = LayoutStore::scrollbar_el_ids(&cx.layouts.lay_scrollbar_styles);

        // Taffy永続ツリーへの差分同期
        OutputStore::sync_dirty_styles_to_taffy(
            &scrollbar_el_ids,
            &mut cx.layouts.lay_taffy_tree,
            &cx.layouts.lay_dirty_entities,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_resolved_basic,
            &cx.layouts.lay_resolved_flex,
            &cx.layouts.lay_resolved_grid,
            &cx.layouts.lay_scrollbar_styles,
        );

        // Taffy 1回目レイアウト計算
        if let Some(&root_node) = cx.layouts.lay_taffy_nodes.get(root) {
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
                    with_context(|cx| {
                        ContentStore::measure_content(
                            id,
                            known_dims,
                            available_space,
                            &cx.system.sys_text_engine,
                            &mut cx.contents.cont_input_contents,
                            &cx.contents.cont_text_contents,
                            &cx.contents.cont_text_spans,
                            &cx.topology.topo_active_masks,
                            &cx.renders.rnd_visual,
                        )
                    })
                })
            };

            let _ = cx.layouts.lay_taffy_tree.compute_layout_with_measure(
                root_node,
                taffy::Size {
                    width: taffy::AvailableSpace::Definite(window_size.width),
                    height: taffy::AvailableSpace::Definite(window_size.height),
                },
                measure_func,
            );
        }

        // ダブルバッファをスワップし、1回目の出力座標を決定
        // scroll_size を正しく算出するため、スワップおよび一旦コンテンツの out_rects のみを確定
        OutputStore::swap_output_rect(
            &mut cx.outputs.out_rects,
            &mut cx.outputs.out_clip_rects,
            &mut cx.outputs.out_prev_rects,
            &mut cx.outputs.out_prev_clip_rects,
        );
        OutputStore::resolve_first_pass_rects(
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
            &cx.outputs.out_scroll_offsets,
        );

        // 全スクロールコンテナの scroll_size を事前計算
        cx.outputs.out_scroll_sizes.clear();
        for &id in &cx.topology.topo_flat_dfs_sequence {
            if cx
                .topology
                .topo_active_masks
                .get(id)
                .is_some_and(|m| m.has(ComponentMask::STYLE_OVERFLOW))
            {
                let size = OutputStore::get_scroll_size(
                    id,
                    &cx.system.sys_text_engine,
                    &cx.system.sys_dwrite_layouts,
                    &cx.contents.cont_text_contents,
                    &cx.contents.cont_text_spans,
                    &cx.contents.cont_input_contents,
                    &cx.topology.topo_active_masks,
                    &cx.topology.topo_parents,
                    &cx.topology.topo_children,
                    &cx.layouts.lay_resolved_basic,
                    &cx.layouts.lay_scrollbar_styles,
                    &cx.renders.rnd_visual,
                    &cx.renders.rnd_interaction,
                    &cx.renders.rnd_active_transitions,
                    &cx.outputs.out_rects,
                    &cx.outputs.out_scroll_offsets,
                );
                cx.outputs.out_scroll_sizes.insert(id, size);
            }
        }

        // スクロールバー要素（Track & Thumb）のサイズ・配置・不透明度を一括同期更新
        LayoutStore::sync_scrollbar_styles(
            cx.window.win_last_size,
            &cx.system.sys_text_engine,
            &cx.system.sys_dwrite_layouts,
            &cx.contents.cont_text_contents,
            &cx.contents.cont_text_spans,
            &cx.contents.cont_input_contents,
            &cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &cx.topology.topo_children,
            &mut cx.layouts.lay_taffy_tree,
            &mut cx.layouts.lay_basic,
            &mut cx.layouts.lay_base_basic,
            &mut cx.layouts.lay_resolved_basic,
            &mut cx.layouts.lay_resolved_flex,
            &mut cx.layouts.lay_resolved_grid,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_flex,
            &cx.layouts.lay_grid,
            &cx.layouts.lay_scrollbar_styles,
            &mut cx.renders.rnd_visual,
            &mut cx.renders.rnd_base_visual,
            &cx.renders.rnd_interaction,
            &cx.renders.rnd_active_transitions,
            &cx.outputs.out_rects,
            &cx.outputs.out_scroll_offsets,
            &cx.outputs.out_scroll_sizes,
        );

        // Taffy の 2回目レイアウト計算（スクロールバー配置確定後）
        if let Some(&root_node) = cx.layouts.lay_taffy_nodes.get(root) {
            let _ = cx.layouts.lay_taffy_tree.compute_layout_with_measure(
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
                        with_context(|cx| {
                            let is_input = cx
                                .topology
                                .topo_active_masks
                                .get(id)
                                .is_some_and(ComponentMask::has_input_content);

                            if is_input
                                && let Some(contents) = cx.contents.cont_input_contents.get(id)
                                && let Some(layout_rect) = contents.last_layout
                            {
                                return taffy::Size {
                                    width: known_dims.width.unwrap_or(layout_rect.width),
                                    height: known_dims.height.unwrap_or(layout_rect.height),
                                };
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
                    })
                },
            );
        }

        // スクロールバーも加えた、最終的な出力座標の決定
        OutputStore::resolve_final_pass_rects(
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
            &cx.outputs.out_scroll_offsets,
        );

        // リサイズ追従に伴い、インプットのキャレット・選択ハイライトを同期
        for &id in &cx.topology.topo_flat_dfs_sequence {
            let has_input = cx
                .topology
                .topo_active_masks
                .get(id)
                .is_some_and(ComponentMask::has_input_content);
            let is_focused = cx.events.evt_interaction_states.focused == Some(id);
            // フォーカスを得ている入力要素のみ、レイアウト確定後にキャレット・スクロールを同期
            if has_input && is_focused {
                OutputStore::update_input_caret_position(
                    id,
                    cx.window.win_scale_factor,
                    cx.window.win_last_size,
                    &cx.system.sys_text_engine,
                    &cx.system.sys_dwrite_layouts,
                    &mut cx.contents.cont_text_contents,
                    &mut cx.contents.cont_input_contents,
                    &cx.contents.cont_text_spans,
                    &mut cx.topology.topo_active_masks,
                    &cx.topology.topo_parents,
                    &mut cx.layouts.lay_dirty_entities,
                    &mut cx.layouts.lay_taffy_tree,
                    &mut cx.layouts.lay_scrollbar_styles,
                    &cx.layouts.lay_taffy_nodes,
                    &cx.layouts.lay_resolved_basic,
                    &cx.layouts.lay_resolved_flex,
                    &cx.layouts.lay_resolved_grid,
                    &mut cx.renders.rnd_visual,
                    &cx.renders.rnd_base_visual,
                    &cx.renders.rnd_interaction,
                    &cx.renders.rnd_active_transitions,
                    &mut cx.outputs.out_scroll_offsets,
                    &mut cx.outputs.out_text_selections,
                    &cx.outputs.out_rects,
                    &cx.outputs.out_scroll_sizes,
                );
            }
        }

        // 全アクティブコンテナのスクロールオフセット自動クランプ同期
        OutputStore::auto_clamp_scroll_offsets(
            cx.window.win_last_size,
            &mut cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &cx.topology.topo_flat_dfs_sequence,
            &mut cx.layouts.lay_dirty_entities,
            &mut cx.layouts.lay_taffy_tree,
            &mut cx.layouts.lay_scrollbar_styles,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_resolved_basic,
            &cx.renders.rnd_visual,
            &cx.renders.rnd_interaction,
            &cx.renders.rnd_active_transitions,
            &mut cx.outputs.out_scroll_offsets,
            &cx.outputs.out_rects,
            &cx.outputs.out_scroll_sizes,
        );

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
            let (autoscroll_occurred, active_pos) = OutputStore::autoscroll_occurred(
                id,
                cx.window.win_last_size,
                &cx.system.sys_text_engine,
                &cx.system.sys_dwrite_layouts,
                cx.events.evt_current_pointer_position,
                &cx.contents.cont_text_contents,
                &cx.contents.cont_text_spans,
                &cx.contents.cont_input_contents,
                &mut cx.topology.topo_active_masks,
                &cx.topology.topo_parents,
                &cx.topology.topo_children,
                &mut cx.layouts.lay_dirty_entities,
                &mut cx.layouts.lay_taffy_tree,
                &mut cx.layouts.lay_scrollbar_styles,
                &cx.layouts.lay_taffy_nodes,
                &cx.layouts.lay_resolved_basic,
                &cx.renders.rnd_visual,
                &cx.renders.rnd_interaction,
                &cx.renders.rnd_active_transitions,
                &mut cx.outputs.out_scroll_offsets,
                &cx.outputs.out_rects,
                &cx.outputs.out_clip_rects,
                &cx.outputs.out_scroll_sizes,
            );

            if autoscroll_occurred && let Some(pos) = active_pos {
                // スクロールによりテキストが流れたため、
                // 現在のポインタ座標で仮想的にポインタ移動を再トリガーし、
                // 選択文字インデックスおよびキャレット位置を同期
                EventStore::inject_pointer_move_internal(cx, pos);
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
            let is_dirty_text = cx
                .topology
                .topo_active_masks
                .get(id)
                .is_some_and(|m| m.has_text_content() && m.has_queued_layout_or_render());

            if is_dirty_text {
                let cleared = Pipeline::scan_and_register_element_glyphs(
                    id,
                    view,
                    &default_visual,
                    &cx.system.sys_text_engine,
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
                let has_text_content = cx
                    .topology
                    .topo_active_masks
                    .get(id)
                    .is_some_and(ComponentMask::has_text_content);
                if has_text_content {
                    let _ = Pipeline::scan_and_register_element_glyphs(
                        id,
                        view,
                        &default_visual,
                        &cx.system.sys_text_engine,
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
            let rect = cx.outputs.out_rects.get(id).copied().unwrap_or_default();
            if rect.width <= 0.0 || rect.height <= 0.0 {
                continue;
            }
            let clip = cx
                .outputs
                .out_clip_rects
                .get(id)
                .copied()
                .unwrap_or_default();

            let basic = cx
                .layouts
                .lay_resolved_basic
                .get(id)
                .copied()
                .unwrap_or_default();
            let flex = cx
                .layouts
                .lay_resolved_flex
                .get(id)
                .copied()
                .unwrap_or_default();
            let grid = cx
                .layouts
                .lay_resolved_grid
                .get(id)
                .cloned()
                .unwrap_or_default();
            let visual = cx.renders.rnd_visual.get(id).unwrap_or(&default_visual);

            let (border, padding) =
                LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);
            let scroll = cx
                .outputs
                .out_scroll_offsets
                .get(id)
                .copied()
                .unwrap_or_default();

            // トランスフォームブランチの場合のみその場で累積を解決
            // それ以外は IDENTITY_MATRIX
            let eff_transform =
                if cx.topology.topo_active_masks[id].has(ComponentMask::STATE_TRANSFORM_ACTIVE) {
                    RenderStore::resolve_effective_transform(
                        id,
                        &cx.topology.topo_parents,
                        &cx.renders.rnd_visual,
                    )
                } else {
                    IDENTITY_MATRIX
                };

            let params = CommonParameters::new(id, rect, &basic, visual, eff_transform);

            let is_webview = cx
                .topology
                .topo_active_masks
                .get(id)
                .is_some_and(ComponentMask::has_webveiw2_content);
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
                .get(id)
                .is_some_and(|m| m.has(ComponentMask::COMP_EXTERNAL_TEXTURE_CONTENT));
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
            if let Some(sel_rects) = cx.outputs.out_selected_rects.get(id)
                && let Some(dw_layout) = SystemStore::get_or_create_layout(
                    id,
                    &cx.system.sys_text_engine,
                    &cx.system.sys_dwrite_layouts,
                    &cx.contents.cont_text_contents,
                    &cx.contents.cont_text_spans,
                    &cx.layouts.lay_resolved_basic,
                    &cx.renders.rnd_visual,
                    &cx.outputs.out_rects,
                )
            {
                let align_offset = Pipeline::text_size_to_align_offset(
                    id,
                    &params,
                    &dw_layout,
                    border,
                    padding,
                    &flex,
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
            let is_text = cx
                .topology
                .topo_active_masks
                .get(id)
                .is_some_and(ComponentMask::has_text_content);
            let has_bg = visual.bg_color.is_some()
                || visual.bg_gradient.is_some()
                || visual.border_color.is_some()
                || visual.shadow_params.is_some();

            if has_bg {
                Pipeline::push_background_instance(id, view.render_data, &params, visual);
            }

            if is_text
                && let Some(dw_layout) = SystemStore::get_or_create_layout(
                    id,
                    &cx.system.sys_text_engine,
                    &cx.system.sys_dwrite_layouts,
                    &cx.contents.cont_text_contents,
                    &cx.contents.cont_text_spans,
                    &cx.layouts.lay_resolved_basic,
                    &cx.renders.rnd_visual,
                    &cx.outputs.out_rects,
                )
            {
                let align_offset = Pipeline::text_size_to_align_offset(
                    id,
                    &params,
                    &dw_layout,
                    border,
                    padding,
                    &flex,
                    &cx.system.sys_text_engine,
                    &cx.contents.cont_input_contents,
                );

                let (metrics, text_u16_vec) = Pipeline::get_metrics_and_u16_vec(
                    id,
                    &dw_layout,
                    &cx.system.sys_text_engine,
                    &cx.contents.cont_text_contents,
                );

                let spans = cx
                    .contents
                    .cont_text_spans
                    .get(id)
                    .map_or(&[][..], Vec::as_slice);
                let resolved_color =
                    Pipeline::resolv_text_color(id, visual, &cx.contents.cont_input_contents);

                Pipeline::push_text_background_instances(
                    id,
                    view.render_data,
                    &params,
                    &dw_layout,
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
                    spans,
                    &metrics,
                    &text_u16_vec,
                    visual,
                    resolved_color,
                    border,
                    padding,
                    scroll,
                    align_offset,
                    cx.window.win_scale_factor,
                    &cx.system.sys_text_engine,
                    &cx.contents.cont_text_contents,
                );

                Pipeline::push_text_front_instances(
                    id,
                    view.render_data,
                    &params,
                    spans,
                    &dw_layout,
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
            let is_input = cx
                .topology
                .topo_active_masks
                .get(id)
                .is_some_and(ComponentMask::has_input_content);
            let is_focused = cx.events.evt_interaction_states.focused == Some(id);

            if is_input && is_focused {
                Pipeline::push_caret_instance(
                    id,
                    view.render_data,
                    &params,
                    border,
                    padding,
                    scroll,
                    &flex,
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
        dw_layout: &IDWriteTextLayout,
        border: EdgeInsets,
        padding: EdgeInsets,
        flex: &FlexLayout,
        sys_text_engine: &TextEngine,
        cont_input_contents: &InputContentsSparseSecondary,
    ) -> LayoutPoint {
        let (text_size, is_multiline) = if let Some(c) = cont_input_contents.get(id) {
            if let Some(l) = c.last_layout {
                // コンテンツが存在し前回のレイアウトもある場合
                (LayoutSize::new(l.width, l.height), c.is_multiline)
            } else {
                // コンテンツはあるがレイアウトがない場合
                (sys_text_engine.get_layout_size(dw_layout), c.is_multiline)
            }
        } else {
            // コンテンツ自体が存在しない場合
            (sys_text_engine.get_layout_size(dw_layout), false)
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
        default_visual: &VisualProperty,
        sys_text_engine: &TextEngine,
        win_scale_factor: f32,
        topo_active_masks: &ActiveMasksSecondary,
        cont_text_contents: &TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        cont_input_contents: &InputContentsSparseSecondary,
        rnd_visual: &VisualPropertiesSecondary,
    ) -> bool {
        let text = cont_text_contents
            .get(id)
            .cloned()
            .unwrap_or_else(|| "".into());
        let spans = cont_text_spans.get(id).map_or(&[][..], Vec::as_slice);

        let visual = rnd_visual.get(id).unwrap_or(default_visual);

        let text_u16_vec: Vec<u16> = text.encode_utf16().collect();

        let mut char_idx = 0;
        let mut atlas_cleared = false;

        while char_idx < text_u16_vec.len() {
            // サロゲートペア対応文字の抽出
            let (character, u16_len) = Pipeline::get_char_and_u16_len(&text_u16_vec, char_idx);
            let span = spans.iter().find(|s| s.range.contains(&char_idx));
            let key = TextCacheKey::new(span, visual, character, win_scale_factor);

            let (_, _, cleared) = sys_text_engine.get_or_create_glyph_uv(&key, view);

            if cleared {
                atlas_cleared = true;
            }

            char_idx += u16_len;
        }

        atlas_cleared
    }

    #[inline]
    fn resolv_text_color(
        id: EntityId,
        visual: &VisualProperty,
        cont_input_contents: &InputContentsSparseSecondary,
    ) -> Color {
        let is_ime_active = cont_input_contents
            .get(id)
            .and_then(|c| c.ime_state.as_ref())
            .is_some_and(|ime| !ime.composition_text.is_empty());

        let base_text_empty = cont_input_contents
            .get(id)
            .is_some_and(|c| c.text.0.get().is_empty());

        let placeholder_color = cont_input_contents
            .get(id)
            .and_then(|p| p.placeholder_color)
            .unwrap_or(Color::rgb_f32(0.5, 0.5, 0.5));

        if base_text_empty && !is_ime_active {
            placeholder_color
        } else {
            visual.text_color.unwrap_or(Color::WHITE)
        }
    }

    #[inline]
    fn get_metrics_and_u16_vec(
        id: EntityId,
        dw_layout: &IDWriteTextLayout,
        sys_text_engine: &TextEngine,
        cont_text_contents: &TextContentsSparseSecondary,
    ) -> (Vec<DWRITE_HIT_TEST_METRICS>, Vec<u16>) {
        let text = cont_text_contents
            .get(id)
            .cloned()
            .unwrap_or_else(|| "".into());

        let text_u16_len = text.encode_utf16().count();
        let metrics = sys_text_engine.get_all_char_metrics(dw_layout, text_u16_len);

        let text_u16_vec: Vec<u16> = text.encode_utf16().collect();

        (metrics, text_u16_vec)
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
        dw_layout: &IDWriteTextLayout,
        spans: &[TextSpan],
        align_offset: LayoutPoint,
        border: EdgeInsets,
        padding: EdgeInsets,
        scroll: LayoutPoint,
    ) {
        for span in spans {
            if let Some(bg_color) = span.bg_color {
                let rects = OutputStore::calc_selection_rects(id, dw_layout, span.range.clone());

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
        spans: &[TextSpan],
        metrics: &Vec<DWRITE_HIT_TEST_METRICS>,
        text_u16_vec: &[u16],
        visual: &VisualProperty,
        resolved_color: Color,
        border: EdgeInsets,
        padding: EdgeInsets,
        scroll: LayoutPoint,
        align_offset: LayoutPoint,
        win_scale_factor: f32,
        sys_text_engine: &TextEngine,
        cont_text_contents: &TextContentsSparseSecondary,
    ) {
        for metric in metrics {
            let char_idx = metric.textPosition as usize;
            if char_idx >= text_u16_vec.len() {
                continue;
            }

            let (character, _) = Pipeline::get_char_and_u16_len(text_u16_vec, char_idx);
            // 該当文字に当たるテキストスパンのフォントオーバーライド
            let span = spans.iter().find(|s| s.range.contains(&char_idx));
            let key = TextCacheKey::new(span, visual, character, win_scale_factor);
            let char_color = span.and_then(|s| s.color).unwrap_or(resolved_color);

            // DWrite リソース解決APIを呼び出して UV を取得
            let (uv_min, uv_max, _cleared) = sys_text_engine.get_or_create_glyph_uv(&key, view);

            // アトラスに登録された実際の物理テクスチャ解像度を逆算
            let tex_phys_w = (uv_max[0] - uv_min[0]) * view.atlas.size as f32;
            let tex_phys_h = (uv_max[1] - uv_min[1]) * view.atlas.size as f32;

            // 論理サイズに逆算
            let tex_log_w = tex_phys_w / win_scale_factor;
            let tex_log_h = tex_phys_h / win_scale_factor;

            // 文字の配置
            let char_rect = LayoutRect::new(
                params.rect.x + border.left + padding.left + align_offset.x + metric.left
                    - scroll.x,
                params.rect.y + border.top + padding.top + align_offset.y + metric.top - scroll.y,
                tex_log_w,
                tex_log_h,
            );

            let glyph_instance = QuadInstance {
                rect: char_rect,
                transform: params.transform,
                transform_origin: params.transform_origin,
                color: char_color,
                opacity_mode_sizing: [params.opacity, 2.0, params.box_sizing_val, 0.0],
                uv_min,
                uv_max,
                ..Default::default()
            };

            view.render_data.push(id, glyph_instance);
        }
    }

    /// テキスト用前面インスタンスを追加
    fn push_text_front_instances(
        id: EntityId,
        render_data: &mut RenderData,
        params: &CommonParameters,
        spans: &[TextSpan],
        dw_layout: &IDWriteTextLayout,
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

            let rects = OutputStore::calc_selection_rects(id, dw_layout, span.range.clone());

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
        cont_input_contents: &InputContentsSparseSecondary,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
    ) {
        let Some(contents) = cont_input_contents.get(id) else {
            return;
        };

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

        let caret_rect = OutputStore::calculate_caret_rect(
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
        cont_external_textures: &ExternalTextureSparseSecondary,
    ) {
        let provider = cont_external_textures.get(id).unwrap();
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
