use crate::{
    ActiveEntitiesVec, ActiveFocusTrigger, ActiveMasksSecondary, BaseVisualPropertiesSecondary,
    BasicLayout, BasicLayoutsSecondary, BatchType, BoxSizing, ClipRectsSecondary, Color,
    ComponentMask, Context, CornerRadius, DEFAULT_BASIC, DEFAULT_FLEX, DebugStore,
    DirtyLayoutEntitiesVec, DrawBatch, EdgeInsets, ElementState, EntityId, EventStore,
    ExternalTextureAlphaMode, ExternalTextureSparse, FlatDfsSequenceVec, FlexLayout, FocusStore,
    IDENTITY_MATRIX, ImeState, InputContentsSparse, LayoutPoint, LayoutRect, LayoutSize,
    LayoutStore, MichiuSoA, Modifiers, MouseButton, OutputStore, ParentsSecondary,
    PrevClipRectsSecondary, PrevRectsSecondary, QuadInstance, ReactiveStore, RectsSecondary,
    RenderData, RenderStore, RendererView, ResolvedBasicSecondary, ResolvedFlexSecondary,
    ResolvedGeometry, ResolvedGridSparse, ScrollBarState, ScrollOffsetsSecondary, ScrollStore,
    ScrollbarStore, ScrollbarStylesSparse, StrikethroughStyle, SystemStore, TaffyNodesSecondary,
    TaffyResultTraceExt, TaffyTreeEntityId, TextEditStore, TextEngine, TextLayoutSize, TextSpan,
    TopologyStore, TraceEventList, UnderlineStyle, VirtualKey, VisualProperty, bind_context,
    handle_on_active, handle_on_char_input, handle_on_disable, handle_on_file_dropped,
    handle_on_ime, handle_on_select,
};
#[cfg(feature = "trace-lifecycle")]
use crate::{
    DirtyReason, FlatBufferTrace, FrameKinds, InstanceKinds, LayoutStage, MichiuTrace, RenderStage,
    RendererViewTrace, trace_lifecycle,
};
use cosmic_text::Buffer;
use slotmap::SparseSecondaryMap;
#[cfg(feature = "trace-lifecycle")]
use std::sync::Arc;
use std::{borrow::Cow, collections::HashSet, path::PathBuf};

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
    pub(crate) fn inject_user_action(cx: &mut Context, action: UserAction) {
        let _context_guard = bind_context(cx);

        match action {
            // キューの末尾が PointerMove なら最新座標で上書き
            UserAction::PointerMove(pos) => {
                if let Some(UserAction::PointerMove(last_pos)) =
                    cx.events.evt_pending_actions.last_mut()
                {
                    *last_pos = pos;
                    return;
                }
                cx.events.evt_pending_actions.push(action);
            }

            // スクロールの場合は移動量の加算
            UserAction::MouseWheel { scroll_x, scroll_y } => {
                if let Some(UserAction::MouseWheel {
                    scroll_x: last_x,
                    scroll_y: last_y,
                }) = cx.events.evt_pending_actions.last_mut()
                {
                    *last_x += scroll_x;
                    *last_y += scroll_y;
                    return;
                }
                cx.events.evt_pending_actions.push(action);
            }

            // それ以外は間引かない
            _ => {
                cx.events.evt_pending_actions.push(action);
            }
        }
    }

    #[allow(unused)]
    pub(crate) fn begin_frame(cx: &mut Context) {
        let _context_guard = bind_context(cx);

        let mut actions = std::mem::take(&mut cx.events.evt_pending_actions.0);

        // 古い順でイテレート
        for action in actions.drain(..) {
            let mut event_trace = TraceEventList::None;

            match action {
                UserAction::PointerMove(layout_point) => {
                    #[cfg(feature = "trace-lifecycle")]
                    {
                        event_trace = TraceEventList::PointerMove {
                            x: layout_point.x,
                            y: layout_point.y,
                        };
                    }

                    EventStore::inject_pointer_move(cx, layout_point);
                }
                UserAction::PointerButton {
                    button,
                    state,
                    modifiers,
                } => {
                    #[cfg(feature = "trace-lifecycle")]
                    {
                        event_trace = TraceEventList::PointerButton {
                            button,
                            state,
                            modifiers,
                        };
                    }

                    EventStore::inject_pointer_button(cx, button, state, modifiers);
                }
                UserAction::PointerDoubleClick { modifiers } => {
                    #[cfg(feature = "trace-lifecycle")]
                    {
                        event_trace = TraceEventList::PointerDoubleClick { modifiers };
                    }

                    EventStore::inject_pointer_double_click(cx);
                }
                UserAction::MouseWheel { scroll_x, scroll_y } => {
                    #[cfg(feature = "trace-lifecycle")]
                    {
                        event_trace = TraceEventList::MouseWheel {
                            x: scroll_x,
                            y: scroll_y,
                        };
                    }

                    EventStore::inject_mouse_wheel(cx, scroll_x, scroll_y);
                }
                UserAction::KeyboardKey {
                    key,
                    state,
                    modifiers,
                } => {
                    #[cfg(feature = "trace-lifecycle")]
                    {
                        event_trace = TraceEventList::Keyboard {
                            key,
                            state,
                            modifiers,
                        };
                    }

                    EventStore::inject_keyboard_key(cx, key, state, modifiers);
                }
                UserAction::Character(c) => {
                    let Some(focused_id) = cx.events.evt_interaction_states.focused else {
                        return;
                    };

                    #[cfg(feature = "trace-lifecycle")]
                    {
                        event_trace = TraceEventList::Character { char: c };
                    }

                    handle_on_char_input(cx, focused_id, c);
                }
                UserAction::Ime(ime_state) => {
                    let Some(focused_id) = cx.events.evt_interaction_states.focused else {
                        return;
                    };

                    #[cfg(feature = "trace-lifecycle")]
                    {
                        event_trace = TraceEventList::Ime {
                            is_open: ime_state.is_open,
                            conversion_mode: ime_state.conversion_mode,
                            sentence_mode: ime_state.sentence_mode,
                            keyboard_layout_id: ime_state.keyboard_layout_id,
                            composition_text: ime_state.composition_text.0.clone(),
                            result_text: ime_state.result_text.0.clone(),
                            caret_position: ime_state.caret_position,
                            composition_cursor: ime_state.composition_cursor.0,
                            composition_attrs: ime_state.composition_attrs.clone(),
                        };
                    }

                    handle_on_ime(cx, focused_id, ime_state);
                }
                UserAction::FileDropped(path_bufs) => {
                    let Some(target_id) = cx.events.evt_interaction_states.hovered else {
                        return;
                    };

                    #[cfg(feature = "trace-lifecycle")]
                    {
                        event_trace = TraceEventList::FileDropped {
                            path: Arc::from(path_bufs.clone()),
                        };
                    }

                    handle_on_file_dropped(cx, target_id, path_bufs);
                }
                UserAction::Paste(text) => {
                    #[cfg(feature = "trace-lifecycle")]
                    {
                        event_trace = TraceEventList::Paste { text: text.clone() };
                    }

                    EventStore::inject_paste(cx, &text.into());
                }
                UserAction::Cut => {
                    let cut = EventStore::inject_cut(cx);

                    #[cfg(feature = "trace-lifecycle")]
                    if let Some(text) = cut.clone() {
                        event_trace = TraceEventList::Cut { text: text.0 };
                    }

                    cx.contents.cont_cut_text = cut;
                }
                UserAction::Undo => {
                    EventStore::inject_undo(cx);

                    #[cfg(feature = "trace-lifecycle")]
                    {
                        event_trace = TraceEventList::Undo;
                    }
                }
                UserAction::Redo => {
                    EventStore::inject_redo(cx);

                    #[cfg(feature = "trace-lifecycle")]
                    {
                        event_trace = TraceEventList::Undo;
                    }
                }
            }

            #[cfg(feature = "trace-lifecycle")]
            trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Event {
                kinds: Arc::new(event_trace.clone()),
                add: None,
            });
        }

        // 空になったバッファを戻して次のフレームで再アロケーションが発生するのを防ぐ
        if cx.events.evt_pending_actions.0.is_empty() {
            cx.events.evt_pending_actions.0 = actions;
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

        let resolve_element = |cx: &mut Context, id: EntityId| {
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
                &mut cx.debug,
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
                &mut cx.debug,
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
                    &mut cx.debug,
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

    #[track_caller]
    #[inline]
    pub(crate) fn sync_layout_and_render(
        cx: &mut Context,
        root: EntityId,
        window_size: LayoutSize,
    ) {
        let _context_guard = bind_context(cx);

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Layout {
            stage: LayoutStage::Start,
            add: None,
        });

        // トポロジーが完全に完成したビルド完了後、または同期直前に、溜めてある初回評価を一挙に実行
        ReactiveStore::evaluate_pending_element_effects(
            &mut cx.reactive.react_pending_element_effects,
            &cx.reactive.react_effects,
        );

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Layout {
            stage: LayoutStage::FirstEffects,
            add: None,
        });

        // ウィンドウサイズの変更検知
        let window_resized = !(cx.window.win_last_size.replace(window_size) != Some(window_size));
        let no_dirty_entities = cx.layouts.lay_dirty_entities.is_empty();
        let no_structure_dirty = !cx.topology.topo_is_structure_dirty;
        let not_empty_output_rect = !cx.outputs.out_rects.is_empty();

        // 構造変更がなく、スタイル変更（レイアウト変更要求）もなく、ウィンドウサイズも変わっていないなら、
        // すべてスキップして早期リターン。
        if no_dirty_entities && no_structure_dirty && window_resized && not_empty_output_rect {
            #[cfg(feature = "trace-lifecycle")]
            trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Layout {
                stage: LayoutStage::EarlyReturn(DirtyReason {
                    window_resized,
                    has_dirty_entities: no_dirty_entities,
                    is_structure_dirty: no_structure_dirty,
                    is_empty_output_rect: not_empty_output_rect
                }),
                add: None,
            });

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
                &mut cx.debug,
            );

            #[cfg(feature = "trace-lifecycle")]
            trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Layout {
                stage: LayoutStage::RebuildDfs,
                add: Some(
                    "The fact that this is recorded means that there has been a change in the tree structure\n\
                     (is_structure_dirty = true)."
                ),
            });
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
                &mut cx.debug,
            );
        }

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Layout {
            stage: LayoutStage::ResolveLayout,
            add: None,
        });

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
            &mut cx.debug,
        );

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Layout {
            stage: LayoutStage::SyncTaffy,
            add: None,
        });

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
                cx.contents.cont_input_contents.measure_content(
                    id,
                    known_dims,
                    available_space,
                    &cx.topology.topo_active_masks,
                    &cx.renders.rnd_visual,
                    |auto_wrap, max_width| {
                        let text = cx.contents.cont_text_contents.at(id);
                        let font = cx.renders.rnd_visual.font(id);
                        let text_align = cx.layouts.lay_resolved_flex.text_algin(id);
                        let spans = cx.contents.cont_text_spans.span(id);
                        cx.system
                            .sys_text_engine
                            .measure_text(text, &font, text_align, max_width, auto_wrap, spans)
                    },
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
            .unwrap_or_trace(Some(root), &mut cx.debug);

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Layout {
            stage: LayoutStage::FirstMeasure,
            add: Some(
                "The first layout calculation takes time\n\
                 because there is no cache and text layout calculations are also performed."
            ),
        });

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
            &mut cx.debug,
        );

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Layout {
            stage: LayoutStage::FirstOutputRect,
            add: Some("The scrollbar layout is not included in the calculations here."),
        });

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
                    &cx.topology.topo_children,
                    &cx.layouts.lay_resolved_basic,
                    &cx.layouts.lay_resolved_flex,
                    &cx.layouts.scrollbar.bar_styles,
                    &cx.renders.rnd_visual,
                    &cx.outputs.out_rects,
                    &cx.states.scroll.sc_offsets,
                    &mut cx.debug,
                );
                cx.states.scroll.sc_sizes.insert(id, size);
            }
        }

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Layout {
            stage: LayoutStage::ScrollSize,
            add: None,
        });

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
            &mut cx.debug,
        );

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Layout {
            stage: LayoutStage::SyncScrollBar,
            add: None,
        });

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
                            .find(id)
                            .map_or(taffy::Size::ZERO, |rect| taffy::Size {
                                width: known_dims.width.unwrap_or(rect.width),
                                height: known_dims.height.unwrap_or(rect.height),
                            })
                    })
                },
            )
            .unwrap_or_trace(Some(root), &mut cx.debug);

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Layout {
            stage: LayoutStage::FinalMeasure,
            add: None,
        });

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
            &mut cx.debug,
        );

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Layout {
            stage: LayoutStage::FinalOutputRect,
            add: Some("Final calculation including scrollbar"),
        });

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
                    &mut cx.states.scroll.sc_offsets,
                    &mut cx.states.edit.edit_selections,
                    &cx.outputs.out_rects,
                    &cx.states.scroll.sc_sizes,
                    &mut cx.debug,
                );
            }
        }

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Layout {
            stage: LayoutStage::UpdateInputContents,
            add: None,
        });

        // 全アクティブコンテナのスクロールオフセット自動クランプ同期
        for &id in &cx.topology.topo_flat_dfs_sequence {
            let Some(current) = cx.states.scroll.sc_offsets.find(id).copied() else {
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
                &mut cx.states.scroll.sc_offsets,
                &cx.outputs.out_rects,
                &cx.states.scroll.sc_sizes,
                &mut cx.debug,
            );
        }

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Layout {
            stage: LayoutStage::SyncScrollOffsets,
            add: None,
        });
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
                &mut cx.debug,
            );

            #[cfg(feature = "trace-lifecycle")]
            trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Animation {
                kinds: FrameKinds::Transition,
                add: None,
            });
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
                &mut cx.debug,
            );

            #[cfg(feature = "trace-lifecycle")]
            trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Animation {
                kinds: FrameKinds::Animation,
                add: None,
            });
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
                &mut cx.states.scroll.sc_offsets,
                &cx.outputs.out_rects,
                &cx.outputs.out_clip_rects,
                &cx.states.scroll.sc_sizes,
                &mut cx.debug,
            );

            #[cfg(feature = "trace-lifecycle")]
            trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Animation {
                kinds: FrameKinds::AutoScroll,
                add: None,
            });

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
        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::PrepareRender {
            stage: RenderStage::Start,
            data: Some(Arc::new(RendererViewTrace {
                render_data: view.render_data.clone(),
                atlas: view.atlas.clone(),
                text_cache: view.text_cache.clone(),
                queue: view.queue.clone()
            })),
            add: None,
        });

        let default_visual = VisualProperty::default();
        TopologyStore::prepare_sorted_entities(
            cx.window.win_last_size,
            &mut cx.topology.topo_active_masks,
            &mut cx.topology.topo_effective_z_indices,
            &mut cx.topology.topo_sorted_entities,
            &mut cx.topology.topo_sort_cache,
            &mut cx.topology.topo_is_sort_dirty,
            &cx.topology.topo_parents,
            &cx.topology.topo_flat_dfs_sequence,
            &cx.renders.rnd_visual,
            &mut cx.outputs.out_clip_rects,
            &cx.outputs.out_rects,
            &mut cx.debug,
        );

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::PrepareRender {
            stage: RenderStage::SortedEntities,
            data: None,
            add: Some("The RendererView hasn't changed, so we won't record it here."),
        });

        let mut force_full_scan = false;
        for &id in &*cx.topology.topo_sorted_entities {
            let buffers = cx.system.sys_text_buffers.borrow();
            let Some(buffer) = buffers.find(id) else {
                continue;
            };

            let mask = cx.topology.topo_active_masks.at(id);
            let is_dirty_text = mask.has_text_content() && mask.has_queued_layout_or_render();

            if is_dirty_text {
                let cleared = Pipeline::scan_and_register_element_glyphs(
                    view,
                    buffer,
                    &mut cx.system.sys_text_engine,
                    cx.window.win_scale_factor,
                    &mut cx.debug,
                );

                if cleared {
                    // このスキャン中にアトラスのクリアが起きたため再構築が必要
                    force_full_scan = true;
                }
            }
        }

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::PrepareRender {
            stage: RenderStage::FirstGlyphsCache,
            data: Some(Arc::new(RendererViewTrace {
                render_data: view.render_data.clone(),
                atlas: view.atlas.clone(),
                text_cache: view.text_cache.clone(),
                queue: view.queue.clone()
            })),
            add: None,
        });

        if force_full_scan {
            for &id in &*cx.topology.topo_sorted_entities {
                let buffers = cx.system.sys_text_buffers.borrow();
                let Some(buffer) = buffers.find(id) else {
                    continue;
                };

                let has_text_content = cx.topology.topo_active_masks.at(id).has_text_content();

                if has_text_content {
                    let _ = Pipeline::scan_and_register_element_glyphs(
                        view,
                        buffer,
                        &mut cx.system.sys_text_engine,
                        cx.window.win_scale_factor,
                        &mut cx.debug,
                    );
                }
            }

            #[cfg(feature = "trace-lifecycle")]
            trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::PrepareRender {
                stage: RenderStage::FullGlyphsCache,
                data: Some(Arc::new(RendererViewTrace {
                    render_data: view.render_data.clone(),
                    atlas: view.atlas.clone(),
                    text_cache: view.text_cache.clone(),
                    queue: view.queue.clone()
                })),
                add: Some("The fact that this is recorded means that Atlas has been cleared."),
            });
        }

        view.render_data.clear();
        let mut last_flushed_offset = 0;
        let mut current_batch_type = BatchType::Normal;
        let mut last_clip = None;

        let mut has_webview_ready = false;
        let mut has_webview_static = false;
        let mut has_external_texture = false;
        let mut has_normal_element = false;
        let mut has_selection_highlight = false;
        let mut has_background = false;
        let mut has_text = false;
        let mut has_fallback_border = false;
        let mut has_caret = false;

        for &id in &*cx.topology.topo_sorted_entities {
            let rect = *cx.outputs.out_rects.at(id);
            if rect.width <= 0.0 || rect.height <= 0.0 {
                continue;
            }
            let clip = *cx.outputs.out_clip_rects.at(id);

            let basic = cx
                .layouts
                .lay_resolved_basic
                .find_or(id, &DEFAULT_BASIC, &mut cx.debug);
            let flex = cx
                .layouts
                .lay_resolved_flex
                .find_or(id, &DEFAULT_FLEX, &mut cx.debug);
            let _grid = cx
                .layouts
                .lay_resolved_grid
                .find(id)
                .cloned()
                .unwrap_or_default();
            let visual = cx
                .renders
                .rnd_visual
                .find_or(id, &default_visual, &mut cx.debug);

            let (border, padding) =
                LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);
            let scroll = cx
                .states
                .scroll
                .sc_offsets
                .find_or_default(id, &mut cx.debug);

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

            let geom = ResolvedGeometry {
                rect,
                border,
                padding,
                scroll,
            };

            let params = CommonParameters::new(geom, basic, visual, eff_transform);

            let is_webview = cx.topology.topo_active_masks.at(id).has_webveiw2_content();
            // コントローラーがまだ初期化されていない場合は通常通り背景を描画し透過を防止
            let is_webview_ready = is_webview && cx.renders.rnd_active_webviews.contains(&id);

            #[cfg(feature = "trace-lifecycle")]
            {
                has_webview_ready = is_webview_ready;
            }

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

            #[cfg(feature = "trace-lifecycle")]
            {
                has_webview_static = is_webview_static;
            }

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

            #[cfg(feature = "trace-lifecycle")]
            {
                has_external_texture = is_external_texture;
            }

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

                    #[cfg(feature = "trace-lifecycle")]
                    {
                        has_normal_element = true;
                    }
                }
            } else {
                last_clip = Some(clip);
            }

            // 選択ハイライト背景
            if let Some(sel_rects) = cx.states.edit.edit_selected_rects.find(id) {
                #[cfg(feature = "trace-lifecycle")]
                {
                    has_selection_highlight = true;
                }

                let auto_wrap = cx.renders.rnd_visual.auto_wrap(id);
                let content_width =
                    rect.width - border.right - border.left - padding.right - padding.left;
                let max_width_opt = (auto_wrap && content_width > 0.0).then_some(content_width);

                let buffer = SystemStore::get_or_create_text_buffer(
                    id,
                    max_width_opt,
                    &cx.system.sys_text_buffers,
                    || {
                        let text = cx.contents.cont_text_contents.at(id);
                        let font = cx.renders.rnd_visual.font(id);
                        let spans = cx.contents.cont_text_spans.span(id);
                        cx.system.sys_text_engine.create_buffer(
                            text,
                            &font,
                            flex.text_align,
                            max_width_opt,
                            auto_wrap,
                            spans,
                        )
                    },
                );

                let align_offset = Pipeline::text_size_to_align_offset(
                    id,
                    &params,
                    &buffer,
                    flex,
                    &cx.contents.cont_input_contents,
                );

                Pipeline::push_selection_highlight_instances(
                    id,
                    view.render_data,
                    &params,
                    align_offset,
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

            #[cfg(feature = "trace-lifecycle")]
            {
                has_background = has_bg;
                has_text = is_text;
                has_fallback_border = !is_text && !has_bg;
            }

            if has_bg {
                Pipeline::push_background_instance(id, view.render_data, &params, visual);
            }

            if is_text {
                let auto_wrap = cx.renders.rnd_visual.auto_wrap(id);
                let content_width =
                    rect.width - border.right - border.left - padding.right - padding.left;
                let max_width_opt = (auto_wrap && content_width > 0.0).then_some(content_width);

                let spans = cx.contents.cont_text_spans.span(id);

                let buffer = SystemStore::get_or_create_text_buffer(
                    id,
                    max_width_opt,
                    &cx.system.sys_text_buffers,
                    || {
                        let text = cx.contents.cont_text_contents.at(id);
                        let font = cx.renders.rnd_visual.font(id);
                        cx.system.sys_text_engine.create_buffer(
                            text,
                            &font,
                            flex.text_align,
                            max_width_opt,
                            auto_wrap,
                            spans,
                        )
                    },
                );

                let align_offset = Pipeline::text_size_to_align_offset(
                    id,
                    &params,
                    &buffer,
                    flex,
                    &cx.contents.cont_input_contents,
                );

                let resolved_color =
                    Pipeline::resolve_text_color(id, visual, &cx.contents.cont_input_contents);

                Pipeline::push_text_background_instances(
                    id,
                    view.render_data,
                    &params,
                    &buffer,
                    spans,
                    align_offset,
                );

                Pipeline::push_text_metric_instances(
                    id,
                    view,
                    &params,
                    &buffer,
                    resolved_color,
                    align_offset,
                    cx.window.win_scale_factor,
                    &mut cx.system.sys_text_engine,
                    &mut cx.debug,
                );

                Pipeline::push_text_front_instances(
                    id,
                    view.render_data,
                    &params,
                    spans,
                    &buffer,
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

            #[cfg(feature = "trace-lifecycle")]
            {
                has_caret = is_input && is_focused;
            }

            if is_input && is_focused {
                Pipeline::push_caret_instance(
                    id,
                    view.render_data,
                    &params,
                    flex,
                    visual,
                    cx.window.win_scale_factor,
                    &cx.contents.cont_input_contents,
                    &cx.renders.rnd_base_visual,
                );
            }
        }

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::PrepareRender {
            stage: RenderStage::CollectDate(InstanceKinds {
                has_webview_ready,
                has_webview_static,
                has_external_texture,
                has_normal_element,
                has_selection_highlight,
                has_background,
                has_text,
                has_fallback_border,
                has_caret
            }),
            data: Some(Arc::new(RendererViewTrace {
                render_data: view.render_data.clone(),
                atlas: view.atlas.clone(),
                text_cache: view.text_cache.clone(),
                queue: view.queue.clone()
            })),
            add: Some("The fact that this is recorded means that Atlas has been cleared."),
        });

        let instances_len = view.render_data.instances.len();
        let scissor_rect = last_clip.unwrap_or_default();

        Pipeline::flush_batch(
            &mut view.render_data.batches,
            instances_len,
            &mut last_flushed_offset,
            scissor_rect,
            current_batch_type,
        );

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::PrepareRender {
            stage: RenderStage::FlushBatch(FlatBufferTrace {
                instances_len,
                last_flushed_offset,
                scissor_rect,
                batch_type: current_batch_type
            }),
            data: None,
            add: Some("RendererViewTrace refers to RenderStage::CollectDate.")
        });
    }
}

struct CommonParameters {
    geom: ResolvedGeometry,
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
        geom: ResolvedGeometry,
        basic: &BasicLayout,
        visual: &VisualProperty,
        eff_transform: [[f32; 4]; 4],
    ) -> Self {
        let (transform, transform_origin) =
            RenderStore::get_transform_and_origin(visual, eff_transform);
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
            geom,
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
    #[track_caller]
    fn sync_dirty_styles_to_taffy(
        scrollbar_el_ids: &HashSet<EntityId>,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_dirty_entities: &DirtyLayoutEntitiesVec,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparse,
        bar_styles: &ScrollbarStylesSparse,
        debug: &mut DebugStore,
    ) {
        for &id in lay_dirty_entities {
            if scrollbar_el_ids.contains(&id) {
                continue;
            }

            let basic = lay_resolved_basic.find_or(id, &DEFAULT_BASIC, debug);
            let flex = lay_resolved_flex.find_or(id, &DEFAULT_FLEX, debug);
            let grid = lay_resolved_grid.find(id).cloned();

            // トランジション（アニメーション）中プロパティの現在値による上書き
            // 削除：resolve_active_layouts の段階でアニメーション中のサイズが正しく反映されたレイアウト）が返ってくるため
            // if let Some(active_list) = rnd_active_transitions.get(id) {}

            let taffy_style =
                LayoutStore::resolve_taffy_style(id, basic, flex, grid.as_ref(), bar_styles);

            let taffy_node = *lay_taffy_nodes.at(id);
            lay_taffy_tree
                .set_style(taffy_node, taffy_style)
                .unwrap_or_trace(Some(id), debug);
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
        debug: &mut DebugStore,
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
            debug,
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
        debug: &mut DebugStore,
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
                debug,
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
        debug: &mut DebugStore,
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
                debug,
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

    fn text_size_to_align_offset(
        id: EntityId,
        params: &CommonParameters,
        buffer: &Buffer,
        flex: &FlexLayout,
        cont_input_contents: &InputContentsSparse,
    ) -> LayoutPoint {
        let text_size = if let Some(c) = cont_input_contents.find(id) {
            if let Some(l) = c.last_layout {
                // コンテンツが存在し前回のレイアウトもある場合
                TextLayoutSize::new(l.width, l.height, Some(c.is_multiline))
            } else {
                // コンテンツはあるがレイアウトがない場合
                TextEngine::get_layout_size(buffer).set_multiline(Some(c.is_multiline))
            }
        } else {
            // コンテンツ自体が存在しない場合
            TextEngine::get_layout_size(buffer).set_multiline(Some(false))
        };

        params
            .geom
            .calc_align_offset(text_size, flex.text_align, flex.align_items)
    }

    /// 指定された要素に含まれるすべての文字をアトラスにキャッシュ
    /// このフレームでアトラスの一括クリアが起きた場合は true
    #[inline]
    fn scan_and_register_element_glyphs(
        view: &mut RendererView,
        buffer: &Buffer,
        sys_text_engine: &mut TextEngine,
        win_scale_factor: f32,
        debug: &mut DebugStore,
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
                    debug,
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
        let Some(input) = cont_input_contents.find(id) else {
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
            rect: params.geom.rect,
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
            rect: params.geom.rect,
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
            rect: params.geom.rect,
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
            rect: params.geom.rect,
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
        sel_rects: &Vec<LayoutRect>,
        visual: &VisualProperty,
    ) {
        let sel_bg = visual
            .select_bg_color
            .unwrap_or(Color::rgba_f32(0.0, 0.47, 0.84, 0.35));

        let rect = params.geom.rect;
        let border = params.geom.border;
        let padding = params.geom.padding;
        let scroll = params.geom.scroll;

        for metric_rect in sel_rects {
            let sel_rect = LayoutRect::new(
                rect.x + border.left + padding.left + align_offset.x + metric_rect.x - scroll.x,
                rect.y + border.top + padding.top + align_offset.y + metric_rect.y - scroll.y,
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
            rect: params.geom.rect,
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
    ) {
        let rect = params.geom.rect;
        let border = params.geom.border;
        let padding = params.geom.padding;
        let scroll = params.geom.scroll;

        for span in spans {
            if let Some(bg_color) = span.bg_color {
                let rects = TextEditStore::calc_selection_rects(buffer, span.range.clone());

                for metric_rect in rects {
                    let sel_rect = LayoutRect::new(
                        rect.x + border.left + padding.left + align_offset.x + metric_rect.x
                            - scroll.x,
                        params.geom.rect.y
                            + border.top
                            + padding.top
                            + align_offset.y
                            + metric_rect.y
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
        align_offset: LayoutPoint,
        win_scale_factor: f32,
        sys_text_engine: &mut TextEngine,
        debug: &mut DebugStore,
    ) {
        let rect = params.geom.rect;
        let border = params.geom.border;
        let padding = params.geom.padding;
        let scroll = params.geom.scroll;

        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                //  グリフ原点 ＝ テキストブロック位置 ＋ 行・文字位置を丸めたもの
                let base_phys_x = (rect.x + border.left + padding.left + align_offset.x - scroll.x)
                    * win_scale_factor;
                let base_phys_y = (rect.y + border.top + padding.top + align_offset.y + run.line_y
                    - scroll.y)
                    * win_scale_factor;
                let physical = glyph.physical((base_phys_x, base_phys_y), win_scale_factor);

                // アトラスにパッキングされた文字がUIの画面上で縦横何ピクセルの大きさで描画されるべきかを逆算
                // アトラス上の UV 座標を取得
                let (value, _cleared) = sys_text_engine.get_or_create_glyph_uv(
                    physical.cache_key,
                    view,
                    win_scale_factor,
                    debug,
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
        align_offset: LayoutPoint,
        resolved_color: Color,
    ) {
        let rect = params.geom.rect;
        let border = params.geom.border;
        let padding = params.geom.padding;
        let scroll = params.geom.scroll;

        for span in spans {
            let has_ul = span.underline.is_some();
            let has_st = span.strikethrough.is_some();
            if !has_ul && !has_st {
                continue;
            }

            let rects = TextEditStore::calc_selection_rects(buffer, span.range.clone());

            for metric_rect in rects {
                let start_x =
                    rect.x + border.left + padding.left + align_offset.x + metric_rect.x - scroll.x;
                let end_x = start_x + metric_rect.width;
                let base_y =
                    rect.y + border.top + padding.top + align_offset.y + metric_rect.y - scroll.y;

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
        flex: &FlexLayout,
        visual: &VisualProperty,
        win_scale_factor: f32,
        cont_input_contents: &InputContentsSparse,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
    ) {
        // キャレットがあるなら入力コンポーネントもあるはず
        let contents = cont_input_contents.at(id);

        if !contents.should_show_caret() {
            return;
        }

        let text_size = if let Some(layout_rect) = contents.last_layout {
            TextLayoutSize::new(
                layout_rect.width,
                layout_rect.height,
                Some(contents.is_multiline),
            )
        } else {
            TextLayoutSize::DEFAULT
        };

        let align_offset =
            params
                .geom
                .calc_align_offset(text_size, flex.text_align, flex.align_items);

        let caret_rect = TextEditStore::calculate_caret_rect(
            &params.geom,
            contents,
            win_scale_factor,
            align_offset,
        );

        let c_color = contents
            .caret_color
            .or(rnd_base_visual.find(id).and_then(|v| v.text_color))
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
            rect: params.geom.rect,
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
            rect: params.geom.rect,
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
