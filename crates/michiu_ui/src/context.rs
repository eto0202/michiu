//! # `Context`
//!
//! A central database maintaining the entire state of the UI application, serving as the core struct that aggregates pipeline operations.
//!
//! ---
//!
//! ## Overview
//!
//! `Context` is not a bloated “god object” that encapsulates logic; rather, it is designed as a centralized database bundling multiple role-specific stores.
//! Each store contains a Structure of Arrays (SoA) categorized by its responsibility along with dedicated associated functions. By explicitly passing only the required SoA slices to each processing phase via split borrowing, it achieves a zero-cost, memory-safe internal pipeline.
//!
//! For application developers, these functions are aggregated and exposed as convenient methods directly on `Context`.
//!
//! ---
//!
//! ## Important Constraint: Threading Model
//!
//! > **Warning: UI Thread Affinity**
//! > `Context` is bound to a thread-local context and **can only be accessed and manipulated on the main thread (UI thread)**.
//! > Direct manipulation of `Context` from background (worker) threads is not supported.
//!
//! To update state from a worker thread, dispatch messages back to the UI thread using the crate's asynchronous task dispatch system or standard channels (such as `std::sync::mpsc`).
//!
//! ---
//!
//! ## Two Primary Usage Scenarios
//!
//! In application development, you will primarily interact with `Context` (or `&mut Context`) in the following two contexts:
//!
//! ### 1. Lifecycle Control Within the Main Loop
//! When driving your own window procedure or event loop, you explicitly invoke each phase of the pipeline:
//!
//! * [`inject_action`](Context::inject_action): Enqueues OS input events
//! * [`begin_frame`](Context::begin_frame): Flushes the event queue and updates interaction states
//! * [`tick_system_frame`](Context::tick_system_frame): Advances animations and internal timers
//! * [`sync_layout`](Context::sync_layout): Executes two-pass Flexbox layout resolution
//! * [`update_accessibility`](Context::update_accessibility): Synchronizes the AccessKit tree
//!
//! ### 2. Element Manipulation Within Event Handlers
//! In `_with`-style handlers (such as `on_click_with`), the context is provided via the `cx: &mut Context` parameter:
//!
//! * `cx.current()`: Retrieves the element that triggered the event
//! * `cx.query_first::<T>()` / `cx.try_query_first::<T>()`: Searches for other elements using a marker tag
//! * `cx.find_root_entity()`: Retrieves the root element
//! * `cx.pointer_position()` / `cx.window_last_size()`: Retrieves the current pointer position or window dimensions
//!
//! For additional methods, please refer to the [`Context`] method documentation.
//!
//! ---
//!
//! ## Internal Structure: Independent Stores
//!
//! Internally, `Context` is divided into distinct stores aligned with the execution phases of the pipeline:
//!
//! | Category | Store | Responsibilities & Stored Data |
//! | :--- | :--- | :--- |
//! | **Environment & System** | `window` | Window properties |
//! | | `system` | Text layout engine and task messaging subsystem |
//! | **Structure & State** | `events` | Event listeners and active interaction states (hover, focus, etc.) |
//! | | `reactive` | Signals, reactive dependencies, and effect runtimes |
//! | | `contents` | Text strings and dynamic input content |
//! | | `topology` | Primary entity lifecycle map and structural tree hierarchies |
//! | | `states` | Drag-and-drop, resize, scrolling, and text selection state |
//! | **Computation & Evaluation** | `layouts` | Base, current, and resolved layout measurements |
//! | **Rendering & Output** | `renders` | Non-layout visual properties and animation states |
//! | | `outputs` | Absolute coordinate caches across current and previous frames |
//! | | `acce` | Node tree synchronized with AccessKit |
//! | **Diagnostics** | `debug` | Logs, profiling traces, and snapshot data for MichiuInspector |
//!
//! ---
//!
//! ## Internal Architecture
//!
//! In Rust, inter-method state mutation on a monolithic struct (`Context`) typically triggers `&mut self` borrow conflicts.
//! `michiu_ui` circumvents this without runtime overhead using a clean three-layer model:
//!
//! 1. **Store Layer**:
//!    Domain logic is encapsulated within associated functions on each store. This keeps namespaces clean, and each function takes only the exact SoA slices it needs.
//!    All parameters use store-specific prefixes, and argument ordering is strictly defined *(not that we always stick to it ourselves, though)*.
//! 2. **[`Pipeline`] Layer**:
//!    Executes **split borrowing** across the stores in `Context`, treating them as distinct fields, and dispatches them into the relevant store functions in sequential order. The entire frame lifecycle is centralized here.
//! 3. **Context Layer**:
//!    Wraps the store and pipeline invocations into a coherent, developer-friendly facade API.
//!
//! ---
//!
//! ## Low-Level Operations and Escape Hatches (`RawContext`)
//!
//! While `Context` fields are encapsulated via `pub(crate)`, advanced developers implementing custom renderers or tailoring the pipeline can use [`raw_context_mut`](Context::raw_context_mut) as an escape hatch to directly access internal stores and raw SoA buffers.
//!
//! ### Why Use `RawContext`? (Enabling Split Borrows)
//!
//! Calling methods on `&mut Context` causes Rust's borrow checker to mutably borrow the entire structure exclusively.
//! Because [`RawContext<'a>`] exposes all stores as individual `pub &'a mut Store` fields, it enables advanced split borrows—such as simultaneously taking mutable borrows of the layout and render stores and passing them to distinct functions.
//!
//! ```rust,ignore
//! use michiu_ui::prelude::*;
//!
//! let mut context = Context::new();
//!
//! // 1. Obtain a RawContext where each store is exposed as an independent mutable reference
//! let mut raw = context.raw_context_mut();
//!
//! // 2. Perform split borrows across multiple stores simultaneously
//! // Example: Access the raw SoA buffers for topology and layout concurrently
//! let entities: &mut EntitiesSlot = raw.topology.entities_mut();
//! let basic: &mut BasicLayoutsSecondary = raw.layouts.basic_mut();
//!
//! custom_low_level_pass(entities, basic);
//! ```
//!
//! ### Primary Use Cases
//!
//! * **Custom Pipeline Assembly**:
//!   When you need to reconstruct frame execution steps with custom timing or order, bypassing the default [`Pipeline`].
//! * **Direct SoA Mutation**:
//!   When bypassing library processing to directly modify or add to the SoA.
//! * **External Diagnostics & Tooling**:
//!   When extracting live internal metrics or entity structures for custom visualization. (Note: for general logging and snapshots, [`MichiuInspector`] is recommended).
//!
//! > **Warning: Preserving Internal Invariants**
//! > Methods with the `_mut` suffix (which return raw mutable references to SoA buffers) completely bypass automatic dependency propagation and dirty-flag tracking.
//! > Use these lower-level APIs only when fully aware of the engine's internal invariants to prevent entity ID desynchronization or broken lifecycle states.

pub mod acce_store;
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

pub use acce_store::*;
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
    a11y::A11yInferenceTag, bind_context, handle_on_click,
};
use slotmap::new_key_type;
use std::{borrow::Cow, sync::Arc};
use windows::Win32::Foundation::HWND;

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

impl From<EntityId> for accesskit::NodeId {
    fn from(value: EntityId) -> Self {
        Self(value.0.as_ffi())
    }
}

impl From<accesskit::NodeId> for EntityId {
    #[inline]
    fn from(value: accesskit::NodeId) -> Self {
        Self(slotmap::KeyData::from_ffi(value.0))
    }
}

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
    pub(crate) acce: AccessibilityStore,
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
                MichiuTaskSender {
                    inner: tx,
                    waker: None,
                },
                rx,
            ),
            debug: DebugStore::new(),
            acce: AccessibilityStore::new(),
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
                MichiuTaskSender {
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
            acce: AccessibilityStore::with_capacity(capacity),
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

    /// Initialize the accessibility features.
    #[inline]
    #[must_use]
    pub fn with_accessibility(mut self, hwnd: HWND) -> Self {
        self.acce = AccessibilityStore::init(hwnd);
        self
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
        self.acce.clear();
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
            &mut self.acce,
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
    ///
    /// You can also use `A11yInferenceTag` for accessibility.
    #[inline]
    pub fn tag<T: 'static>(&mut self, el: Element) {
        TopologyStore::tag::<T>(el, &mut self.topology.topo_tag_registry);
    }

    /// Configure the user-defined inference tags used when building the accessibility tree.
    ///
    /// `A11yInferenceTag` overrides only the default inferences.
    /// It does not override the [`Element::a11y`] roles.
    #[inline]
    pub fn tag_a11y<T: A11yInferenceTag + 'static>(&mut self, el: Element) {
        TopologyStore::tag_a11y::<T>(el, &mut self.topology.topo_tag_registry);
    }

    /// Get the first element with the tag `T` found.
    #[inline]
    #[must_use]
    pub fn try_query_first<T: 'static>(&self) -> Option<Element> {
        TopologyStore::try_query_first::<T>(&self.topology.topo_tag_registry)
    }

    /// Get the first element with the tag `T` found.
    #[track_caller]
    #[inline]
    #[must_use]
    pub fn query_first<T: 'static>(&mut self) -> Element {
        TopologyStore::query_first::<T>(&mut self.topology.topo_tag_registry, &mut self.debug)
    }

    /// Get all elements with the tag `T`.
    #[inline]
    pub fn query_all<T: 'static>(&self) -> impl Iterator<Item = Element> + '_ {
        TopologyStore::query_all::<T>(&self.topology.topo_tag_registry)
    }

    /// Retrieve the first element of type `T` found among the descendants.
    #[inline]
    #[must_use]
    pub fn try_query_descendant<T: 'static>(&self, parent: EntityId) -> Option<Element> {
        TopologyStore::try_query_descendant::<T>(
            parent,
            &self.topology.topo_tag_registry,
            &self.topology.topo_parents,
            &self.topology.topo_flat_dfs_sequence,
        )
    }

    /// Retrieve the first element of type `T` found among the descendants.
    #[track_caller]
    #[inline]
    #[must_use]
    pub fn query_descendant<T: 'static>(&mut self, parent: EntityId) -> Element {
        TopologyStore::query_descendant::<T>(
            parent,
            &self.topology.topo_tag_registry,
            &self.topology.topo_parents,
            &self.topology.topo_flat_dfs_sequence,
            &mut self.debug,
        )
    }

    /// Search for an element of type `T` among its descendants.
    #[inline]
    pub fn query_descendants<T: 'static>(
        &self,
        parent: EntityId,
    ) -> impl Iterator<Item = Element> + '_ {
        TopologyStore::query_descendants::<T>(
            parent,
            &self.topology.topo_tag_registry,
            &self.topology.topo_parents,
            &self.topology.topo_flat_dfs_sequence,
        )
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
    pub fn has_active_frame(&mut self) -> bool {
        RenderStore::has_active_frame(
            &self.events.evt_interaction_states,
            self.events.evt_current_pointer_position.as_ref(),
            &self.contents.cont_input_contents,
            &self.layouts.scrollbar.bar_styles,
            &self.renders.rnd_visual,
            &self.renders.rnd_active_transitions,
            &self.renders.rnd_active_animations,
            &self.outputs.out_clip_rects,
            &mut self.debug,
        )
    }

    /// Called before each frame is drawn, it advances all active transitions and animations by 1 tick.
    pub fn tick_system_frame(&mut self, tick: &TickType) {
        Pipeline::tick_system_frame(self, tick);
    }

    /// Get a thread-safe sender that can clone and send tasks.
    #[inline]
    pub fn task_sender(&self) -> MichiuTaskSender {
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

    /// Call it in the corresponding event and add the user action to the queue ([`EventAction`]).
    #[inline]
    pub fn inject_action(&mut self, action: EventAction) {
        Pipeline::inject_action(self, action);
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

    /// Whether the window is currently being resized
    #[inline]
    pub fn window_is_resized(&self) -> bool {
        self.window.win_is_resized
    }

    /// Retrieves the final window size.
    #[inline]
    pub fn window_last_size(&self) -> Option<LayoutSize> {
        self.window.win_last_size
    }

    /// Resolves and returns the `EntityId` of the top-level root element within the window.
    #[inline]
    pub fn find_root_entity(&self) -> Option<EntityId> {
        TopologyStore::find_root_entity(
            &self.topology.topo_entities,
            &self.topology.topo_parents,
            &self.topology.topo_flat_dfs_sequence,
        )
    }

    /// Retrieves the coordinates of the current pointer.
    #[inline]
    pub fn pointer_position(&self) -> Option<LayoutPoint> {
        self.events.evt_current_pointer_position
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

    /// Update accessibility. This must be called after the layout is calculated.
    #[inline]
    pub fn update_accessibility(&mut self) {
        Pipeline::handle_accessibility(self);
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
