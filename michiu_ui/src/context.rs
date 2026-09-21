pub mod config;
pub mod content_store;
pub mod debug_store;
pub mod event_store;
pub mod layout_store;
pub mod output_store;
pub mod pipeline;
pub mod reactive_store;
pub mod render_store;
pub mod soa;
pub mod state_store;
pub mod system_store;
pub mod topology_store;
pub mod window_store;

pub use config::*;
pub use content_store::*;
pub use debug_store::*;
pub use event_store::*;
pub use layout_store::*;
pub use output_store::*;
pub use pipeline::*;
pub use reactive_store::*;
pub use render_store::*;
pub use state_store::*;
pub use system_store::*;
pub use topology_store::*;
pub use window_store::*;

#[cfg(feature = "trace-lifecycle")]
use crate::trace_lifecycle;
use crate::{
    BasicLayout, ComponentMask, CursorIcon, Element, FlexLayout, GridLayout, InteractionState,
    LayoutPoint, LayoutRect, LayoutSize, MichiuSoA, ReadSignal, VisualProperty, WriteSignal,
    bind_context, handle_on_click,
};
use slotmap::new_key_type;
use std::{borrow::Cow, sync::Arc};

new_key_type! {
    /// A unique generation management ID that identifies each element ([`Element`]) within the UI.
    pub struct EntityId;
}

impl EntityId {
    #[must_use]
    #[inline]
    pub fn into_el(self) -> Element {
        Element::from(self)
    }
}

// TODO:
// 利用者用 Context を用意して安定APIはそちらで公開
// pub struct EventContext<'a> {
//    cx: &'a mut Context,
// }

/// The central context owning all retained state stores and reactive pipelines necessary to drive the GUI.
pub struct Context {
    pub(crate) window: WindowStore,
    pub(crate) system: SystemStore,
    pub(crate) reactive: ReactiveStore,
    pub(crate) events: EventStore,
    pub(crate) contents: ContentStore,
    pub(crate) topology: TopologyStore,
    pub(crate) states: StateStore,
    pub(crate) layouts: LayoutStore,
    pub(crate) renders: RenderStore,
    pub(crate) outputs: OutputStore,
    pub(crate) debug: DebugStore,
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    /// Initializes the `Context`.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            topology: TopologyStore::new(),
            states: StateStore::new(),
            layouts: LayoutStore::new(),
            renders: RenderStore::new(),
            outputs: OutputStore::new(),
            contents: ContentStore::new(),
            events: EventStore::new(),
            reactive: ReactiveStore::new(),
            window: WindowStore::new(),
            system: SystemStore::new(
                TaskSender {
                    inner: tx,
                    waker: None,
                },
                rx,
            ),
            debug: DebugStore::new(),
        }
    }

    /// Initialize it by passing [`CapacityConfig`].
    #[inline]
    #[must_use]
    pub fn with_capacity(capacity: &CapacityConfig) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            window: WindowStore::new(),
            system: SystemStore::with_capacity(
                TaskSender {
                    inner: tx,
                    waker: None,
                },
                rx,
                capacity,
            ),
            reactive: ReactiveStore::with_capacity(capacity),
            events: EventStore::with_capacity(capacity),
            contents: ContentStore::with_capacity(capacity),
            topology: TopologyStore::with_capacity(capacity),
            states: StateStore::with_capacity(capacity),
            layouts: LayoutStore::with_capacity(capacity),
            renders: RenderStore::with_capacity(capacity),
            outputs: OutputStore::with_capacity(capacity),
            debug: DebugStore::new(),
        }
    }

    /// Initialize it by passing [`MichiuInspector`].
    #[cfg(feature = "trace-error")]
    #[inline]
    #[must_use]
    pub fn with_inspector(inspector: &MichiuInspector) -> Self {
        let mut cx = Self::new();
        cx.set_inspector(inspector);

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Init {
            capacity: None,
            add: None,
        });

        cx
    }

    /// Initialize it by passing [`CapacityConfig`] and [`MichiuInspector`].
    #[cfg(feature = "trace-error")]
    #[inline]
    #[must_use]
    pub fn with_capacity_and_inspector(cap: &CapacityConfig, inspector: &MichiuInspector) -> Self {
        let mut cx = Self::with_capacity(cap);
        cx.set_inspector(inspector);

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Init {
            capacity: Some(Arc::new(*cap)),
            add: None,
        });

        cx
    }

    /// Clear the status.
    #[inline]
    pub fn clear(&mut self) {
        self.topology.clear();
        self.states.clear();
        self.layouts.clear();
        self.renders.clear();
        self.outputs.clear();
        self.contents.clear();
        self.events.clear();
        self.reactive.clear();
        self.window.clear();
        self.system.clear();
    }

    /// 親を持たないルート要素の破棄に使用。
    #[allow(unused)]
    #[inline]
    pub(crate) fn despawn(&mut self, id: EntityId) {
        TopologyStore::despawn_internal(
            id,
            &mut self.window,
            &mut self.system,
            &mut self.reactive,
            &mut self.events,
            &mut self.contents,
            &mut self.topology,
            &mut self.states,
            &mut self.layouts,
            &mut self.renders,
            &mut self.outputs,
            &mut self.debug,
        );
    }

    /// Retrieves the parent element of the specified element.
    #[track_caller]
    #[inline]
    pub fn parent_element(&self, el: Element) -> Option<Element> {
        self.topology
            .topo_parents
            .find(el.id)
            .and_then(|f| f.map(Element::from))
    }

    /// Get the list of handles for the child elements.
    #[inline]
    pub fn children_list(&self, el: Element) -> impl Iterator<Item = Element> + '_ {
        let list = self.topology.topo_children.find(el.id);
        list.into_iter().flatten().copied().map(Element::from)
    }

    /// Retrieves the total number of active elements on the screen.
    #[inline]
    pub fn active_entities(&self) -> usize {
        self.topology.topo_active_entities.len()
    }

    /// Get the [`BasicLayout`] currently set for the specified element.
    #[inline]
    #[must_use]
    pub fn try_basic_layout(&self, el: Element) -> Option<BasicLayout> {
        self.layouts.lay_basic.find(el.id).copied()
    }

    /// Get the [`FlexLayout`] currently set for the specified element.
    #[inline]
    #[must_use]
    pub fn get_flex_layout(&self, el: Element) -> Option<FlexLayout> {
        self.layouts.lay_flex.find(el.id).copied()
    }

    /// Get the [`GridLayout`] currently set for the specified element.
    #[inline]
    #[must_use]
    pub fn get_grid_layout(&self, el: Element) -> Option<GridLayout> {
        self.layouts.lay_grid.find(el.id).cloned()
    }

    /// Get the [`VisualProperty`] currently set for the specified element.
    #[inline]
    #[must_use]
    pub fn get_visual_property(&self, el: Element) -> Option<VisualProperty> {
        self.renders.rnd_visual.find(el.id).cloned()
    }

    /// Sets whether the window has been resized.
    #[inline]
    pub fn set_window_resized(&mut self, resized: bool) {
        self.window.win_is_resized = resized;
    }

    /// Determines whether the specified element is currently being hovered over.
    #[track_caller]
    #[inline]
    pub fn is_hovered(&self, el: Element) -> bool {
        self.topology
            .topo_active_masks
            .at(el.id)
            .has(ComponentMask::STATE_HOVERED)
    }

    /// Determine if the specified element currently has focus.
    #[track_caller]
    #[inline]
    pub fn is_focused(&self, el: Element) -> bool {
        self.topology
            .topo_active_masks
            .at(el.id)
            .has(ComponentMask::STATE_FOCUSED)
    }

    /// Determine if the specified element is currently pressed.
    #[track_caller]
    #[inline]
    pub fn is_pressed(&self, el: Element) -> bool {
        self.topology
            .topo_active_masks
            .at(el.id)
            .has(ComponentMask::STATE_PRESSED)
    }

    /// Determine if the specified element is currently disabled.
    #[track_caller]
    #[inline]
    pub fn is_disabled(&self, el: Element) -> bool {
        self.topology
            .topo_active_masks
            .at(el.id)
            .has(ComponentMask::STATE_DISABLED)
    }

    /// Determine if the specified element is currently actived.
    #[track_caller]
    #[inline]
    pub fn is_actived(&self, el: Element) -> bool {
        self.topology
            .topo_active_masks
            .at(el.id)
            .has(ComponentMask::STATE_ACTIVED)
    }

    /// Determine if the specified element is currently selected.
    #[track_caller]
    #[inline]
    pub fn is_selected(&self, el: Element) -> bool {
        self.topology
            .topo_active_masks
            .at(el.id)
            .has(ComponentMask::STATE_SELECTED)
    }

    /// Determine if the specified element is currently dragged.
    #[track_caller]
    #[inline]
    pub fn is_dragged(&self, el: Element) -> bool {
        self.topology
            .topo_active_masks
            .at(el.id)
            .has(ComponentMask::STATE_DRAGGED)
    }

    /// Determine if there is a redraw request (for elements marked as dirty).
    #[inline]
    pub fn has_dirty(&self) -> bool {
        !self.renders.rnd_dirty_entities.is_empty()
            || !self.layouts.lay_dirty_entities.is_empty()
            || self.topology.topo_is_structure_dirty
    }

    /// Gets the element currently executing the event handler.
    #[inline]
    pub fn try_current(&self) -> Option<Element> {
        crate::signal::ACTIVE_ELEMENT
            .with(std::cell::Cell::get)
            .map(Element::from)
    }

    /// Gets the element currently executing the event handler.
    #[track_caller]
    #[inline]
    pub fn current(&mut self) -> Element {
        let el = self
            .try_current()
            .unwrap_or_trace(None, &mut self.debug, || MichiuError::NoActiveElement);
        Element::from(el.id)
    }

    /// Mark the specified element as drity, causing it to redraw and re-render.
    #[inline]
    pub fn mark_dirty(&mut self, id: EntityId) {
        TopologyStore::mark_dirty(
            id,
            &mut self.topology.topo_active_masks,
            &self.topology.topo_parents,
            &mut self.layouts.lay_dirty_entities,
            &mut self.layouts.lay_taffy_tree,
            &self.layouts.lay_taffy_nodes,
            &mut self.renders.rnd_dirty_entities,
            &mut self.debug,
        );
    }

    /// Mark the specified element as drity, causing it to redraw.
    #[inline]
    pub fn mark_layout_dirty(&mut self, id: EntityId) {
        LayoutStore::mark_layout_dirty(
            id,
            &mut self.topology.topo_active_masks,
            &self.topology.topo_parents,
            &mut self.layouts.lay_dirty_entities,
            &mut self.layouts.lay_taffy_tree,
            &self.layouts.lay_taffy_nodes,
            &mut self.debug,
        );
    }

    /// Mark the specified element as drity, causing it to re-render.
    #[inline]
    pub fn mark_render_dirty(&mut self, id: EntityId) {
        RenderStore::mark_render_dirty(
            id,
            &mut self.topology.topo_active_masks,
            &mut self.renders.rnd_dirty_entities,
        );
    }

    /// Clear all the dirty marks.
    #[inline]
    pub fn clear_dirty(&mut self) {
        LayoutStore::clear_layout_dirty(
            &mut self.topology.topo_active_masks,
            &mut self.layouts.lay_dirty_entities,
        );
        RenderStore::clear_render_dirty(
            &mut self.topology.topo_active_masks,
            &mut self.renders.rnd_dirty_entities,
        );
    }

    /// Clear all the layout dirty marks.
    #[inline]
    pub fn clear_layout_dirty(&mut self) {
        LayoutStore::clear_layout_dirty(
            &mut self.topology.topo_active_masks,
            &mut self.layouts.lay_dirty_entities,
        );
    }

    /// Clear all the render dirty marks.
    #[inline]
    pub fn clear_render_dirty(&mut self) {
        RenderStore::clear_render_dirty(
            &mut self.topology.topo_active_masks,
            &mut self.renders.rnd_dirty_entities,
        );
    }

    /// Get the absolute coordinates of the specified element on the screen.
    #[inline]
    pub fn rect(&self, id: EntityId) -> Option<LayoutRect> {
        self.outputs.out_rects.find(id).copied()
    }

    /// Get the on-screen clipping boundary of the specified element.
    #[inline]
    pub fn clip_rect(&self, id: EntityId) -> Option<LayoutRect> {
        self.outputs.out_clip_rects.find(id).copied()
    }

    /// Gets the selected text within the currently focused element.
    #[inline]
    pub fn get_selected_text(&self) -> Option<String> {
        TextEditStore::get_selected_text(
            &self.events.evt_interaction_states,
            &self.contents.cont_text_contents,
            &self.renders.rnd_visual,
            &self.states.edit.edit_selections,
        )
        .map(std::convert::Into::into)
    }

    /// Get current scroll position
    #[inline]
    #[must_use]
    pub fn scroll_offset(&self, id: EntityId) -> Option<LayoutPoint> {
        self.states.scroll.sc_offsets.find(id).copied()
    }

    /// Moves relative to the current scroll position.
    #[inline]
    pub fn scroll_by(&mut self, id: EntityId, dx: f32, dy: f32) -> bool {
        ScrollStore::scroll_by(
            id,
            dx,
            dy,
            self.window.win_last_size,
            &mut self.topology.topo_active_masks,
            &self.topology.topo_parents,
            &mut self.layouts.lay_dirty_entities,
            &mut self.layouts.lay_taffy_tree,
            &mut self.layouts.scrollbar.bar_styles,
            &self.layouts.lay_taffy_nodes,
            &self.layouts.lay_resolved_basic,
            &mut self.states.scroll.sc_offsets,
            &self.outputs.out_rects,
            &self.states.scroll.sc_sizes,
            &mut self.debug,
        )
    }

    /// Get the cut text
    #[inline]
    pub fn cut_text(&self) -> Option<Cow<'_, str>> {
        self.contents.cont_cut_text.clone().map(|f| f.0)
    }

    /// Generates signals directly from the `Context`.
    #[inline]
    pub fn create_signal<T: Send + 'static>(
        &mut self,
        initial_value: T,
    ) -> (ReadSignal<T>, WriteSignal<T>) {
        ReactiveStore::create_signal(
            initial_value,
            &mut self.reactive.react_signals,
            &mut self.reactive.react_subscribers,
        )
    }

    /// Provides a signal context to the specified element, or to the root element if None is specified.
    #[inline]
    pub fn provide<T: Send + 'static>(&mut self, id: Option<EntityId>, read_signal: ReadSignal<T>) {
        ReactiveStore::provide::<T>(
            id,
            read_signal,
            &mut self.reactive.react_providers,
            &self.topology.topo_entities,
            &self.topology.topo_parents,
            &self.topology.topo_flat_dfs_sequence,
        );
    }

    /// From the current thread-local `Context`,
    /// we obtain a [`WriteSignal`] for a signal of type `T`, automatically resolving it by traversing the parent tree.
    #[track_caller]
    #[inline]
    pub fn use_provided_setter<T: Send + 'static>(&mut self) -> WriteSignal<T> {
        let element_id =
            ReactiveStore::resolve_element_effect(&self.reactive.react_effect_to_element)
                .unwrap_or_trace(None, &mut self.debug, || MichiuError::ScopeViolation {
                    caller: "use_provided_setter",
                });

        ReactiveStore::use_provided_setter_from::<T>(
            element_id,
            &self.reactive.react_providers,
            &self.topology.topo_parents,
        )
        .unwrap_or_trace(Some(element_id), &mut self.debug, || {
            MichiuError::EntityNotFound { id: element_id }
        })
    }

    /// Identify the target element from the current thread-local `Context`
    /// and resolve the [`ReadSignal`] of type `T` by traversing the parent tree.
    #[track_caller]
    #[inline]
    pub fn use_provided<T: Clone + 'static>(&mut self) -> ReadSignal<T> {
        let element_id =
            ReactiveStore::resolve_element_effect(&self.reactive.react_effect_to_element)
                .unwrap_or_trace(None, &mut self.debug, || MichiuError::ScopeViolation {
                    caller: "use_provided_setter",
                });

        ReactiveStore::use_provided_from::<T>(
            element_id,
            &self.reactive.react_providers,
            &self.topology.topo_parents,
        )
        .unwrap_or_trace(Some(element_id), &mut self.debug, || {
            MichiuError::EntityNotFound { id: element_id }
        })
    }

    /// Identify the target element from the current thread-local `Context`
    /// and resolve the [`ReadSignal`] of type `T` by traversing the parent tree.
    #[inline]
    pub fn try_use_provided<T: Clone + 'static>(&self) -> Option<ReadSignal<T>> {
        let element_id =
            ReactiveStore::resolve_element_effect(&self.reactive.react_effect_to_element)?;
        ReactiveStore::use_provided_from::<T>(
            element_id,
            &self.reactive.react_providers,
            &self.topology.topo_parents,
        )
    }

    /// This element will be tagged `T`.
    ///
    /// Multiple tags can be attached to the same element.
    #[inline]
    pub fn tag<T: 'static>(&mut self, el: Element) {
        self.topology.topo_tag_registry.register_entity::<T>(el.id);
    }

    /// Get the first element with the tag `T` found.
    #[inline]
    #[must_use]
    pub fn try_query_first<T: 'static>(&self) -> Option<Element> {
        self.topology
            .topo_tag_registry
            .get_entities::<T>()
            .and_then(|t| t.first().copied())
            .map(EntityId::into_el)
    }

    /// Get the first element with the tag `T` found.
    #[track_caller]
    #[inline]
    #[must_use]
    pub fn quer_first<T: 'static>(&mut self) -> Element {
        self.topology
            .topo_tag_registry
            .get_entities::<T>()
            .and_then(|t| t.first().copied())
            .map(EntityId::into_el)
            .unwrap_or_trace(None, &mut self.debug, || MichiuError::TagNotFound {
                type_name: std::any::type_name::<T>(),
            })
    }

    /// Get all elements with the tag `T`.
    #[inline]
    pub fn query_all<T: 'static>(&self) -> impl Iterator<Item = Element> + '_ {
        self.topology
            .topo_tag_registry
            .get_entities::<T>()
            .map(|t| t.iter().copied())
            .into_iter()
            .flatten()
            .map(EntityId::into_el)
    }

    /// Retrieve the first element of type `T` found among the descendants.
    #[inline]
    #[must_use]
    pub fn try_query_descendant<T: 'static>(&self, parent: EntityId) -> Option<Element> {
        self.topology
            .topo_tag_registry
            .query_first_descendant_of_type::<T>(
                parent,
                &self.topology.topo_flat_dfs_sequence,
                &self.topology.topo_parents,
            )
            .map(EntityId::into_el)
    }

    /// Retrieve the first element of type `T` found among the descendants.
    #[track_caller]
    #[inline]
    #[must_use]
    pub fn query_descendant<T: 'static>(&mut self, parent: EntityId) -> Element {
        self.topology
            .topo_tag_registry
            .query_first_descendant_of_type::<T>(
                parent,
                &self.topology.topo_flat_dfs_sequence,
                &self.topology.topo_parents,
            )
            .map(EntityId::into_el)
            .unwrap_or_trace(None, &mut self.debug, || MichiuError::TagNotFound {
                type_name: std::any::type_name::<T>(),
            })
    }

    /// Search for an element of type `T` among its descendants.
    #[inline]
    pub fn query_descendants<T: 'static>(
        &self,
        parent: EntityId,
    ) -> impl Iterator<Item = Element> + '_ {
        self.topology
            .topo_tag_registry
            .query_descendants_of_type::<T>(
                parent,
                &self.topology.topo_flat_dfs_sequence,
                &self.topology.topo_parents,
            )
            .map(EntityId::into_el)
    }

    /// Resolve [`CursorIcon`] by traversing the parent tree from the currently hovered element.
    #[inline]
    pub fn resolve_cursor(&self, hovered_id: EntityId) -> CursorIcon {
        RenderStore::resolve_cursor(
            hovered_id,
            &self.events.evt_interaction_states,
            &self.topology.topo_parents,
            &self.renders.rnd_visual,
            &self.renders.rnd_base_visual,
        )
    }

    /// Determine if there are active transitions or animations.
    #[inline]
    pub fn has_active_frame(&self) -> bool {
        RenderStore::has_active_frame(
            &self.events.evt_interaction_states,
            self.events.evt_current_pointer_position.as_ref(),
            &self.contents.cont_input_contents,
            &self.layouts.scrollbar.bar_styles,
            &self.renders.rnd_visual,
            &self.renders.rnd_active_transitions,
            &self.renders.rnd_active_animations,
            &self.outputs.out_clip_rects,
        )
    }

    /// Called before each frame is drawn, it advances all active transitions and animations by 1 tick.
    pub fn tick_system_frame(&mut self, tick: &TickType) {
        Pipeline::tick_system_frame(self, tick);
    }

    /// Get a thread-safe sender that can clone and send tasks.
    #[inline]
    pub fn task_sender(&self) -> TaskSender {
        self.system.sys_task_sender.clone()
    }

    /// After the window is created, register a callback for waking up.
    #[inline]
    pub fn set_waker<F>(&mut self, f: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.system.sys_task_sender.waker = Some(Arc::new(f));
    }

    /// This function is called at the start of each frame in the main thread,
    /// or at the beginning of an event handler,
    /// and executes tasks and other processes received from other threads in a batch.
    #[inline]
    pub fn process_main_thread_tasks(&mut self) {
        let _context_guard = bind_context(self);
        // キューに溜まっているクロージャをすべてメインスレッドのコンテキスト上で実行
        while let Ok(task) = self.system.sys_task_receiver.try_recv() {
            task(self);
        }
    }

    /// Get the `EntityId` of the entity currently being interacted with.
    #[inline]
    pub fn interaction_id(&self, interaction: InteractionState) -> Option<EntityId> {
        match interaction {
            InteractionState::Hovered => self.events.evt_interaction_states.hovered,
            InteractionState::Focused => self.events.evt_interaction_states.focused,
            InteractionState::Pressed => self.events.evt_interaction_states.pressed,
            InteractionState::Dragged => self.events.evt_interaction_states.dragged,
        }
    }

    /// Programmatically click the specified element.
    #[inline]
    pub fn trigger_element_click(&mut self, el: Element) {
        if !self.topology.topo_entities.contains_key(el.id) || self.is_disabled(el) {
            return;
        }
        handle_on_click(self, el.id);
    }

    /// Set the focus trigger ([`ActiveFocusTrigger`]).
    #[inline]
    pub fn auto_focus_switch_by_trigger(&mut self, id: EntityId, trigger: ActiveFocusTrigger) {
        FocusStore::auto_focus_switch_by_trigger(self, id, trigger);
    }

    /// Sets the pseudo-class ([`StateFlag`]).
    #[inline]
    pub fn set_states(&mut self, id: EntityId, flag: &StateFlag, actived: bool) {
        Pipeline::set_states(self, id, flag, actived);
    }

    /// Sets the interaction state ([`InteractionState`]).
    #[inline]
    pub fn interaction_states(&mut self, id: Option<EntityId>, interaction: InteractionState) {
        match interaction {
            InteractionState::Hovered => self.events.evt_interaction_states.hovered = id,
            InteractionState::Focused => self.events.evt_interaction_states.focused = id,
            InteractionState::Pressed => self.events.evt_interaction_states.pressed = id,
            InteractionState::Dragged => self.events.evt_interaction_states.dragged = id,
        }
    }

    /// Call it in the corresponding event and add the user action to the queue ([`UserAction`]).
    #[inline]
    pub fn inject_user_action(&mut self, action: UserAction) {
        Pipeline::inject_user_action(self, action);
    }

    /// Called at the start of a frame to execute the user action registered in the queue.
    #[inline]
    pub fn begin_frame(&mut self) {
        Pipeline::begin_frame(self);
    }

    /// Moves the keyboard focus to the next element.
    #[inline]
    pub fn cycle_keyboard_focus(&mut self, reverse: bool) {
        EventStore::cycle_keyboard_focus_internal(self, reverse);
    }

    /// Retrieves the first valid element found at the specified coordinates.
    #[inline]
    pub fn hit_test(&mut self, point: LayoutPoint) -> Option<EntityId> {
        TopologyStore::hit_test(
            point,
            self.window.win_last_size,
            &self.events.evt_interaction_states,
            &mut self.topology.topo_active_masks,
            &mut self.topology.topo_effective_z_indices,
            &mut self.topology.topo_sorted_entities,
            &mut self.topology.topo_sort_cache,
            &mut self.topology.topo_is_sort_dirty,
            &self.topology.topo_parents,
            &self.topology.topo_flat_dfs_sequence,
            &self.renders.rnd_visual,
            &self.renders.rnd_base_visual,
            &mut self.outputs.out_clip_rects,
            &self.outputs.out_rects,
            &mut self.debug,
        )
    }

    /// Perform layout calculations and synchronization.
    #[inline]
    pub fn sync_layout(&mut self, root: EntityId, window_size: LayoutSize) {
        Pipeline::sync_layout(self, root, window_size);
    }

    /// Get the `RawContext`.
    /// 
    /// This is an escape hatch. While it allows you to directly manipulate the internals of `Context`,
    /// the integrity of its lifecycle and internal data is not guaranteed.
    #[inline]
    pub fn raw_context_mut(&mut self) -> RawContext<'_> {
        RawContext {
            window: &mut self.window,
            system: &mut self.system,
            reactive: &mut self.reactive,
            events: &mut self.events,
            contents: &mut self.contents,
            topology: &mut self.topology,
            states: &mut self.states,
            layouts: &mut self.layouts,
            renders: &mut self.renders,
            outputs: &mut self.outputs,
            debug: &mut self.debug,
        }
    }
}

pub struct RawContext<'a> {
    pub window: &'a mut WindowStore,
    pub system: &'a mut SystemStore,
    pub reactive: &'a mut ReactiveStore,
    pub events: &'a mut EventStore,
    pub contents: &'a mut ContentStore,
    pub topology: &'a mut TopologyStore,
    pub states: &'a mut StateStore,
    pub layouts: &'a mut LayoutStore,
    pub renders: &'a mut RenderStore,
    pub outputs: &'a mut OutputStore,
    pub debug: &'a mut DebugStore,
}

#[cfg(test)]
mod tests;
