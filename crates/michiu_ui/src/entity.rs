//! # `entity`
//!
//! This module is responsible for generating the [`Entity`] that make up the UI tree, assembling the tree hierarchy, handling events,
//! and providing declarative styling via [`Modifier`].
//!
//! ---
//!
//! ## Architecture
//!
//! `michiu_ui` does not have fixed widget structures such as `Button` or `TextInput`.
//! All nodes are simply [`Entity`]s (whose actual values are lightweight, copyable [`Entity`]s),
//! and the capabilities and behaviors of each entity are determined entirely by which internal SoA holds its slot.
//!
//! ```rust
//! #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
//! pub struct Entity {
//!     pub(crate) id: Entity,
//! }
//! ```
//!
//! ### The Concept of Attaching Capabilities (Attachment, Membership, and Registration)
//! Method chaining is not merely configuring properties; it is an operation that **registers the entity (ID) into a specific internal SoA table**.
//!
//! * **`.text(...)`**: Registers the ID into the Text Content SoA, causing the entity itself to behave as shaped text.
//! * **`.input(...)`**: Registers the ID into the Input Content SoA, transforming the entity itself into a directly editable text field.
//! * **`.on_click(...)`**: Registers a listener into the Event SoA, granting the entity clickable interactivity.
//! * **`.draggable(...)`**: Registers the ID into the DnD SoA, granting the entity drag-and-drop capability.
//!
//! ### Why is Style (`Modifier`) Separated?
//! Rather than exposing styling methods like `.bg_color(...)` directly on `Entity`, passing an independent [`Modifier`] ([`ii()`](crate::ts)) as an argument serves two primary purposes:
//!
//! 1. **Architectural Consistency via DOD**:
//!    `Entity` does not own color or layout fields; it merely holds a key into the internal style SoA tables. Passing styles externally aligns with the unified mental model of injecting definitions from the outside.
//! 2. **Style Reusability and Readability (Zero-Cost Sharing via CoW)**:
//!    Styles can be centralized into reusable variables similar to CSS classes and applied across multiple entities.
//!    Because `Modifier` is internally wrapped in an [`Arc`], cloning incurs no heap reallocations. Branching occurs only when properties are mutated or overridden, handled safely and at minimal cost via **CoW (`Arc::make_mut`)**.
//!
//! ---
//!
//! ## Standardized Suffixes (Naming Conventions)
//!
//! * **`_d` suffix**:
//!   Accepts a dynamic closure that implicitly retrieves types (e.g., `Theme`) provided by ancestor entities via `.provide()` (automatically resolves `use_provided` internally).
//! * **`_with` suffix**:
//!   Supplies the context (`cx: &mut Context`) as the first argument to the event handler callback.
//!
//! ---
//!
//! ## Basic Steps for Building a UI
//!
//! 1. Initialize `Context`.
//! 2. Declare the UI tree using [`build_ui`] (must return a root [`Entity`]).
//! 3. Initialize base entities using layout primitives (`div`, `h_flex`, etc.).
//! 4. Apply styles using `.modify()` or `.style_d()` (or via factory arguments).
//! 5. Attach child entities, text, inputs, and event handlers.
//!
//! ```rust,ignore
//! use michiu_ui::prelude::*;
//!
//! let root = build_ui(&mut context, || {
//!     h_flex(ii().p(12.0).size(500.0).bg_color(Color::WHITE))
//!         .child(
//!             div(ii().size((40.0, 40.0)).bg_color(Color::BLUE))
//!                 .hovered(ii().bg_color(Color::RED))
//!                 .on_click(|| println!("Clicked!"))
//!         )
//!         .label("Button Text", ii().text_color(Color::BLACK))
//! });
//! ```
//!
//! ---
//!
//! ## Entity Factory Primitives (Built-in Functions)
//!
//! | Function | Initial `Display` / Layout Attribute | Primary Use Case |
//! | :--- | :--- | :--- |
//! | **`div(style)` / `div_d`** | `Display::Block` | Standard block entity |
//! | **`div_n()`** | `NO_STYLE: Option<Modifier> = None` | lightweight container completely bypassing style processing |
//! | **`flex(style)` / `flex_d`** | `Display::Flex` | General-purpose Flexbox container |
//! | **`h_flex(style)` / `h_flex_d`** | `Display::Flex` + `FlexDirection::Row` | **Horizontal (row)** layout container |
//! | **`v_flex(style)` / `v_flex_d`** | `Display::Flex` + `FlexDirection::Column` | **Vertical (column)** layout container |
//! | **`hidden_box(style)`** | `Display::None` | Conditional rendering or invisible layout boundary |
//!
//! ---
//!
//! ## Styling Specifications (`Modifier` / `ii()`)
//!
//! Styles are applied via `.modify()` or `.style_d()`. Both static `Modifier` instances and dynamic closures are supported and can be mixed.
//! If the same property is chained multiple times, **subsequent declarations override earlier ones (last write wins)**.
//!
//! ### 1. Pseudo-Classes (Interaction Styles)
//! Styles that adapt to user interaction can be defined by nesting `Modifier` one level deep (pseudo-classes nested deeper than one level are ignored).
//!
//! * **Core Pseudo-Classes**:
//!   `.hovered(...)`, `.focused(...)`, `.focused_visible(...)` (active only during keyboard navigation), `.pressed(...)`, `.disabled(...)`, `.actived(...)`, `.selected(...)`, `.dragged(...)`
//! * **`_within` (Child-Triggered)**:
//!   Applies styles to the entity (parent) when an interaction occurs on any of its descendants (e.g., `.hovered_within(...)`).
//! * **`_parent` (Parent-Triggered)**:
//!   Applies styles to the entity (child) when an interaction occurs on its parent (e.g., `.pressed_parent(...)` to alter text color when the enclosing button is pressed).
//!
//! ### 2. Focus Control Attributes (`focusable`)
//! By default, only input entities accept focus. Regular entities must explicitly declare focus attributes.
//!
//! * **Device Suffixes**:
//!   `_mouse` (mouse focus only), `_keyboard` (keyboard navigation only), `_both` (both inputs)
//! * **Application Target Suffixes**:
//!   * `_self`: Applies `.focused(...)` or `.focused_visible(...)` configured directly on the entity.
//!   * `_inherit`: **Inherits and applies the parent entities focus styles**. Ideal for unifying focus styling across composite child entities.
//!   * `_none`: Explicitly unfocusable.
//! * **Focus Steal Prevention**:
//!   * `.prevent_focus_steal(true)`: Prevents stealing focus from currently active entities.
//!   * `.prevent_focus_steal_within(true)`: Prevents child entities from taking focus away.
//!
//! ### 3. Drag-and-Drop Attributes (`draggable` / `droppable`)
//! * **Behavior by `DndDragPayload`**:
//!   * `Entity`: On drop confirmation, the library **automatically updates tree topology (SoA)** to reparent and move the entity.
//!   * `Entity`: Tree topology is unaltered; only dispatch events (ID telemetry) are fired.
//! * **Placeholder Positioning**:
//!   * `.dnd_draggable_root(...)`: Spawns the drag placeholder directly beneath the root, tracking the cursor via absolute coordinates.
//!   * `.dnd_draggable_parent(...)`: Spawns the placeholder under an arbitrary ancestor, tracking via absolute positioning.
//! * **DnD Dedicated Pseudo-Classes**:
//!   * `.dnd_draggable_original(...)`: Style of the source entity being dragged.
//!   * `.dnd_draggable_placeholder(...)`: Style of the floating placeholder tracking the pointer.
//!   * `.dnd_drag_over(...)`: Style of the receiving drop target when hovered over by a payload.
//!
//! ### 4. Additional Style Attributes
//! * **Resizing (`resizable`)**: Enables edge dragging along specific borders. `.resizable_cursor(...)` overrides the mouse pointer (highest precedence).
//! * **Cursor Precedence**: `resizable_cursor` (highest) > `cursor(...)` (entity-specific) > `cursor_global(...)` (ancestor baseline)
//! * **Backdrop Materials (`backdrop`)**: Applies OS materials like Mica or Acrylic (the window background must remain transparent).
//! * **Diagnostics (`debug...`)**: Shorthands rendering predefined colored borders or outlines to visualize layout bounds.
//! * **Style Merging**: Combine external styles via `.merge(other_style)`.
//!
//! ---
//!
//! ## Attaching Features and Events
//!
//! ### 1. Precise Distinction: Text vs. Labels
//! While visually comparable, they differ fundamentally in SoA placement and hierarchy:
//!
//! * **`entity.text(content)` / `text_d` (Entity IS the text)**:
//!   **The `entity` itself** is registered directly into the Text Content SoA. The entity becomes the direct subject of text measurement (`measure_content`) and consumes pointer events directly. Dynamic changes mutate the text property on the entity itself.
//!   *The shorthand `text(content)` is equivalent to `div_n().text(content)`.*
//! * **`entity.label(content, style)` / `label_d` (Spawns a child text entity)**:
//!   **A newly generated child** is registered into the Text Content SoA and appended as a child of `entity`.
//!   Because this child automatically receives **`PointerEvents::None`**, it never intercepts pointer hits intended for the parent container. This is optimal for centering text inside button frames. Dynamic updates replace the child entity.
//!
//! ### 2. Adding and Replacing Children
//! * **Appending**: `.child(entity)`, `.children([el1, el2])` (Dynamic variants: `.child_d`, `.children_d`)
//! * **Dynamic Content Swapping (`set_contents`)**:
//!   Atomically replaces child hierarchies without rebuilding the parent entity. Any reactive effects associated with discarded children are safely disposed of.
//!
//! ### 3. Attaching Input and External Surfaces
//! * **Input Editing**: `.input(contents)` / `input_d` (Shorthand `input(...)` is equivalent to `div_n().input(...)`)
//! * **External Textures**: `.external_texture(...)`
//! * **External Visuals**: `.external_visual(...)` (Hardware-composited `IDCompositionVisual2`)
//!
//! ### 4. Attaching Accessibility Semantics
//! * `.a11y(...)`: Attaches an accessibility node.
//! * `.a11y_with(|node| ...)`: Attaches a node and yields a **mutable reference (`&mut`)** for detailed configuration.
//! * `.a11y_role(...)`: Sets the semantic role only.
//! * `.a11y_label(...)`: Sets both the semantic role and accessible name.
//!
//! ### 5. Tag Queries
//! * **Registration**: `.tag::<MyTag>()`, `.tag_a11y::<CustomInferenceTag>()` (for custom `A11yInferenceTag` types).
//! * **Descendant Traversal**:
//!   * `entity.query_descendant::<T>()` / `entity.try_query_descendant::<T>()`
//!   * `entity.query_descendants::<T>()` (collects all matches into a `Vec`)
//!   * `entity.for_each_descendants::<T>(|child| ...)` (lazy iteration over matches)
//!   * To query starting globally or from arbitrary roots, use `Context` methods such as `cx.query_first::<T>()`.
//!
//! ### 6. Event Handler Registry
//! All handlers provide standard and `_with` variants (receiving `|cx|`). If multiple callbacks of the same event type are registered, they execute sequentially in registration order.
//!
//! | Category | Method | Dispatch Condition / Notes |
//! | :--- | :--- | :--- |
//! | **Pointer & Mouse** | `on_click`, `on_click_with` | Released over the same entity where pressed |
//! | | `on_right_click`, `on_right_click_with` | Secondary click confirmed |
//! | | `on_mouse_input`, `on_mouse_input_with` | Raw button press/release transitions |
//! | | `on_mouse_enter`, `on_mouse_enter_with` | Pointer boundary entry |
//! | | `on_mouse_leave`, `on_mouse_leave_with` | Pointer boundary exit |
//! | | `on_cursor_moved`, `on_cursor_moved_with` | Pointer motion within entity bounds |
//! | | `on_mouse_wheel`, `on_mouse_wheel_with` | Scroll wheel rotation (halts bubbling if consumed) |
//! | | `on_drag`, `on_drag_with` | Pointer motion while holding down button |
//! | | `on_hover`, `on_hover_with` | Sustained hover state |
//! | **Keyboard & Text** | `on_char_input`, `on_char_input_with` | Resolved unicode character input |
//! | | `on_keyboard_input`, `on_keyboard_input_with` | Physical hardware key state changes |
//! | | `on_ime`, `on_ime_with` | IME composition events |
//! | **File Drag & Drop** | `on_file_dropped`, `on_file_dropped_with` | OS native file drop completed |
//! | | `on_file_drag_enter`, `on_file_drag_enter_with` | OS file drag boundary entry |
//! | | `on_file_drag_leave`, `on_file_drag_leave_with` | OS file drag boundary exit |
//! | **State Transitions** | `on_focus`, `on_focus_with` | Focus gained (coupled with `focus(true)`) |
//! | | `on_blur`, `on_blur_with` | Focus lost |
//! | | `on_disable`, `on_disable_with` | Disabled state applied (coupled with `disable(true)`) |
//! | | `on_active`, `on_active_with` | Active state applied (coupled with `active(true)`) |
//! | | `on_select`, `on_select_with` | Selection state altered (coupled with `select(true)`) |
//! | **In-App DnD** | `on_dnd_drag_start`, `on_dnd_drag_start_with` | Movement exceeded drag threshold |
//! | | `on_dnd_drag_interactive(_with)`, `on_dnd_drag_passive(_with)` | Active dragging (branches based on payload) |
//! | | `on_dnd_drop_interactive(_with)`, `on_dnd_drop_passive(_with)` | Drop confirmed |
//!
//! ---
//!
//! ## Thread Safety and Execution Context
//!
//! * **[`Entity`] Methods**: Must execute exclusively on the **main thread (UI thread)** to interact with underlying SoA storage directly.
//! * **[`Modifier`]**: Wrapped in a thread-safe [`Arc`]. Being a pure data representation, it can be transferred safely across threads. Mutations execute strictly through `Arc::make_mut` only when an exclusive borrow can be formed, eliminating data races entirely.
//!    ```rust
//!    #[derive(Debug, Clone, Default)]
//!    pub struct Modifier {
//!        pub(crate) inner: Arc<ModInner>,
//!    }
//!    ```

pub mod handler;
pub mod input_func;

use crate::{
    BasicLayout, ComponentMask, Context, DebugStore, EffectCategory, ExternalTexture,
    ExternalVisual, IntoOptionProp, MichiuError, MichiuSoA, MichiuTrace, ModTarget, Modifier,
    ReadSignal, ScrollBarState, ScrollbarDisplay, ScrollbarStyle, SystemStore, Val,
    a11y::A11yInferenceTag, div_n, trace_error,
};
#[cfg(feature = "trace-lifecycle")]
use crate::{ContextState, trace_lifecycle};
use accesskit::{Node, Role};
use slotmap::new_key_type;
use smallvec::SmallVec;
use std::{borrow::Cow, cell::Cell, rc::Rc, sync::Arc};

thread_local! {
    // 現在構築中のUIコンテキストへの生ポインタを一時的にバインドするグローバルスレッド領域
    static ACTIVE_CONTEXT: Cell<Option<*mut Context>> = const { Cell::new(None) };
}

/// Builds the UI and returns the root entity.
///
/// # Examples
///
/// ```rust
/// use michiu_ui::{build_ui, div, div_n, ts, Color, Context};
///
/// let mut cx = Context::new();
/// let root = build_ui(&mut cx, || {
///     div(ii().bg_color(Color::GREEN))
///         .child(div_n()) // empty container
/// });
///
/// ```
pub fn build_ui(cx: &mut Context, f: impl FnOnce() -> Entity) -> Entity {
    let old = ACTIVE_CONTEXT.get();
    let current = std::ptr::from_mut::<Context>(cx);

    ACTIVE_CONTEXT.set(Some(current));
    let _guard = ContextGuard { current, old };

    // // 未定義動作を避けるためこれ以降は `cx` を直接触らない
    let marker = unsafe { (*current).start_session() };
    let result = f();
    unsafe {
        // 戻り値に含まれるハンドルをルート要素として登録
        (*current).register_root(result);
        // 親子関係に組み込まれなかった無駄な孤児を自動一掃
        (*current).end_session(marker);

        // ツリーのすべてのトポロジーおよび provide 関係が組み上がったこの瞬間に、
        // キューされて保留されていた全子孫要素のエフェクトを一括して初回評価
        (*current).evaluate_pending_entity_effects();
    }

    #[cfg(feature = "trace-lifecycle")]
    trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::BuildEntity {
        old: old.map(<*mut Context>::addr),
        current: <*mut Context>::addr(current),
        root: result.id,
        add: None,
    });

    result
}

/// スレッドローカルから安全にContextへのアクセスを解決する内部ヘルパー
///
/// Context が無く、トレースが送信出来ないためパニックで落とす。
#[track_caller]
#[expect(clippy::panic)]
#[inline]
pub(crate) fn with_context<R>(f: impl FnOnce(&mut Context) -> R) -> R {
    let ptr = ACTIVE_CONTEXT.get().unwrap_or_else(|| {
        let caller = std::panic::Location::caller();
        panic!(
            "\n=======================================================\n\
            [Michiu UI Fatal Error: No Active Context]\n\
            Called at: {caller}\n\
            \n\
            Possible causes:\n\
             - Calling UI / Reactive APIs from a background thread (must be called on the UI main thread).\n\
             - Accessing Context before it was initialized or after it was dropped.\n\
             - Calling UI methods outside of the application event loop.\n\
            ======================================================="
            );
        });
    // UIスレッドは単一かつ非同期にまたがらないため、ポインタの生存期間は保証される。
    unsafe { f(&mut *ptr) }
}

// コンテキストを復元するための一時的なガード構造体
#[allow(unused)]
pub(crate) struct ContextGuard {
    // drop 時にログを出すために自身がバインドしたポインタを保持
    current: *mut Context,
    // drop 時に復元するために過去のポインタを保持
    old: Option<*mut Context>,
}

/// 現在のスレッドローカルに `Context` を一時的にバインド。
/// 戻り値のガードオブジェクトがスコープを抜ける際、自動的に元のコンテキストに復元。
#[track_caller]
#[inline]
pub(crate) fn bind_context(cx: &mut Context) -> ContextGuard {
    let old = ACTIVE_CONTEXT.get();
    let current = std::ptr::from_mut::<Context>(cx);

    ACTIVE_CONTEXT.set(Some(current));

    // 生ポインタから直接フィールドの可変参照を取り、cx の他の部分との衝突やエイリアス規則違反を防ぐ
    #[cfg(feature = "trace-lifecycle")]
    trace_lifecycle!(None, unsafe { &mut (*current).debug }, || {
        MichiuTrace::Context {
            current: Some(<*mut Context>::addr(current)),
            state: ContextState::Bind,
            add: None,
        }
    });

    ContextGuard { current, old }
}

impl Drop for ContextGuard {
    #[inline]
    fn drop(&mut self) {
        #[cfg(feature = "trace-lifecycle")]
        unsafe {
            trace_lifecycle!(None, &mut (*self.current).debug, || MichiuTrace::Context {
                current: Some(<*mut Context>::addr(self.current)), // 解除される自身のポインタアドレス
                state: ContextState::Drop,
                add: None
            });
        }

        ACTIVE_CONTEXT.set(self.old);
    }
}

/// A type that abstracts static or dynamically changing values.
pub enum Prop<T> {
    None,
    Static(T),
    Dynamic(Box<dyn Fn() -> T + 'static>),
}

new_key_type! {
    /// A unique generation management ID that identifies each entity ([`Entity`]) within the UI.
    pub struct Entity;
}

impl From<Entity> for accesskit::NodeId {
    fn from(value: Entity) -> Self {
        Self(value.0.as_ffi())
    }
}

impl From<accesskit::NodeId> for Entity {
    #[inline]
    fn from(value: accesskit::NodeId) -> Self {
        Self(slotmap::KeyData::from_ffi(value.0))
    }
}

impl Entity {
    /// Start building the new entities.
    ///
    /// Prefer using [`crate::div`], [`crate::div_n`], [`crate::v_flex`], [`crate::h_flex`] etc.
    /// functions for a cleaner syntax.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        // スレッドローカルの Context から安全に要素を spawn
        with_context(|cx| cx.spawn(None))
    }

    /// Get the handle of the parent entity.
    #[inline]
    #[must_use]
    pub fn parent_entity(self) -> Option<Entity> {
        with_context(|cx| cx.parent_entity(self))
    }

    /// Get the list of handles for the child entities.
    #[inline]
    #[must_use]
    pub fn children_list(self) -> Vec<Entity> {
        with_context(|cx| cx.children_list(self).collect())
    }

    /// Provides a context (signal) of type `T`.
    ///
    /// Obtainable in this entity and all its descendant entities via `use_provided::<T>()`.
    #[inline]
    #[must_use]
    pub fn provide<T: Send + 'static>(self, read_signal: ReadSignal<T>) -> Self {
        with_context(|cx| {
            cx.provide::<T>(Some(self), read_signal);
        });
        self
    }

    /// This entity will be tagged `T`.
    ///
    /// Multiple tags can be attached to the same entity.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use michiu_ui::div_n;
    ///
    /// struct Tag;
    ///
    /// let _tag = div_n().tag::<Tag>();
    ///
    /// ```
    #[inline]
    #[must_use]
    pub fn tag<T: 'static>(self) -> Self {
        with_context(|cx| cx.tag::<T>(self));
        self
    }

    /// Configure the user-defined `A11yInferenceTag` used when building the accessibility tree.
    ///
    /// `A11yInferenceTag` overrides only the default inferences.
    /// It does not override the [`Entity::a11y`] roles.
    #[inline]
    #[must_use]
    pub fn tag_a11y<T: A11yInferenceTag + 'static>(self) -> Self {
        with_context(|cx| cx.tag_a11y::<T>(self));
        self
    }

    /// Retrieve the first entity of type `T` found among the descendants.
    #[inline]
    #[must_use]
    pub fn try_query_descendant<T: 'static>(&self) -> Option<Entity> {
        with_context(|cx| cx.try_query_descendant::<T>(*self))
    }

    /// Retrieve the first entity of type `T` found among the descendants.
    #[track_caller]
    #[inline]
    #[must_use]
    pub fn query_descendant<T: 'static>(&self) -> Entity {
        with_context(|cx| cx.query_descendant::<T>(*self))
    }

    /// Search for an entity of type `T` among its descendants.
    #[inline]
    #[must_use]
    pub fn query_descendants<T: 'static>(&self) -> Vec<Entity> {
        with_context(|cx| cx.query_descendants::<T>(*self).collect())
    }

    /// Search for an entity of type `T` among its descendants.
    #[inline]
    pub fn for_each_descendants<T: 'static>(&self, mut f: impl FnMut(Entity)) {
        with_context(|cx| {
            for id in cx.query_descendants::<T>(*self) {
                f(id);
            }
        });
    }

    /// 静的な値、または動的に変化する Prop を該当する `EffectCategory` を通じて自動バインド
    fn bind_prop<T: 'static>(
        self,
        prop: impl Into<Prop<T>>,
        category: EffectCategory,
        mut apply_fn: impl FnMut(&mut Context, Entity, T) + 'static,
    ) -> Self {
        match prop.into() {
            Prop::None => {}
            Prop::Static(val) => {
                with_context(|cx| apply_fn(cx, self, val));
            }
            Prop::Dynamic(f) => {
                with_context(|cx| {
                    cx.create_entity_effect(self, category, move |cx| {
                        let val = f();
                        apply_fn(cx, self, val);
                    });
                });
            }
        }
        self
    }

    /// Applies styles.
    #[must_use]
    pub fn modify(self, m: impl Into<Prop<Modifier>>) -> Self {
        match m.into() {
            Prop::None => {}
            Prop::Static(m) => {
                with_context(|cx| {
                    // 静的なスタイルプロパティを通常通りマウント
                    // 静的チェーンはマージ（merge = true）
                    with_context(|cx| Entity::modify_internal(cx, self, &m, true));

                    // 動的なセッターが存在する場合、それらを単一のエフェクトとして登録
                    if !m.inner.dynamic_setters.is_empty() {
                        let setters = m.inner.dynamic_setters.clone();
                        cx.create_entity_effect(self, EffectCategory::Style, move |cx| {
                            // すべての動的セッターを実行
                            for setter in &setters {
                                setter(cx, self, ModTarget::Base);
                            }
                            // 状態が変化したため、最後に必ずスタイル解決
                            cx.resolve_entity_style_state(self, false);
                        });
                    }
                });
            }
            Prop::Dynamic(f) => {
                with_context(|cx| {
                    cx.create_entity_effect(self, EffectCategory::Style, move |cx| {
                        let m = f();

                        // 動的評価された最新スタイルは蓄積を避けるため置換（merge = false）
                        // 修正: 動的評価されたスタイルもマージ（true）としてマウント
                        Entity::modify_internal(cx, self, &m, true);

                        // 動的スタイルが自身の中で動的なプロバイダーを含む場合も評価
                        for setter in &m.inner.dynamic_setters {
                            setter(cx, self, ModTarget::Base);
                        }
                        cx.resolve_entity_style_state(self, false);
                    });
                });
            }
        }
        self
    }

    /// Applies a reactive style block evaluated from a provided context.
    ///
    /// Wraps the closure in a dynamic property that automatically retrieves the provided context of type `P` and tracks signal updates.
    ///  Whenever the context updates, the style block is re-evaluated and applied.
    ///
    /// Use this when multiple style properties depend on the same context (e.g., theming),
    /// as an alternative to specifying [`dynamic`](crate::dynamic) for each property individually.
    ///
    /// # Examples
    /// ```no_run
    /// # use crate::prelude::*;
    /// #[derive(Debug, Clone)]
    /// struct Theme {
    ///     bg_color: Color,
    ///     text_color: Color,
    /// }
    ///
    /// fn root() -> Entity {
    ///     let (theme, _) = create_signal(Theme {
    ///         bg_color: Color::BLUE,
    ///         text_color: Color::WHITE,
    ///     });
    ///
    ///     div_n()
    ///         .provide(theme)
    ///         .child(
    ///             div_n().style_d(|t: &Theme| {
    ///                 ii().bg_color(t.bg_color)
    ///                     .text_color(t.text_color)
    ///             })
    ///         )
    /// }
    /// ```
    #[must_use]
    #[inline]
    pub fn modify_d<P, F>(self, f: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> Modifier + Send + Sync + 'static,
    {
        // 1引数のクロージャを、プロバイダー探索とシグナル購読（.get()）を内包した
        // 引数なしの Prop::Dynamic クロージャへラップして既存の style メソッドへ委譲
        let dynamic_prop = Prop::Dynamic(Box::new(move || {
            let signal = with_context(|cx| cx.use_provided::<P>());
            let val = signal.get();
            f(&val)
        }));

        self.modify(dynamic_prop)
    }

    /// スタイルの適用
    pub(crate) fn modify_internal(cx: &mut Context, id: Entity, m: &Modifier, merge: bool) {
        let inner = &m.inner;
        let mask = inner.mask;

        // topo_active_masks にスタイル側のマスクをマージするが、
        // 動的なインタラクション状態フラグ（STYLE_INTERACTION_PROPERTY）は
        // 実行時にのみ制御されるべきなのでここでは除外する
        let property_only_mask = mask.0 & !ComponentMask::STYLE_INTERACTION_PROPERTY;

        let mut need_push_dirty = false;

        cx.topology
            .topo_active_masks
            .at_mut(id)
            .set(property_only_mask);

        if mask.has_basic_layout()
            || mask.has(ComponentMask::STYLE_FONT_SIZE)
            || mask.has(ComponentMask::STYLE_AUTO_WRAP)
        {
            if merge && cx.layouts.lay_base_basic.contains_key(id) {
                let base = cx.layouts.lay_base_basic.at_mut(id);
                base.override_with(&inner.basic_layout, mask);
            } else {
                // 前回の設定蓄積をクリアして置換
                cx.layouts.lay_base_basic.insert(id, inner.basic_layout);
            }
            need_push_dirty = true;
        }

        let has_visual =
            mask.has_visual_property() || inner.visual_property.border_lengths.is_some();
        if has_visual {
            if merge && cx.renders.rnd_base_visual.contains_key(id) {
                // マスクがあるなら Some のはず
                let vis = cx.renders.rnd_base_visual.at_mut(id);
                vis.override_with(&inner.visual_property, mask);
            } else {
                cx.renders
                    .rnd_base_visual
                    .insert(id, inner.visual_property.clone());
            }
        }

        if mask.has_interaction_property()
            || mask.has(ComponentMask::STYLE_INTERACTION_WITHIN)
            || mask.has(ComponentMask::STYLE_INTERACTION_PARENT)
        {
            if merge && cx.renders.rnd_interaction.contains_key(id) {
                // マスクがあるなら Some のはず
                let interaction = cx.renders.rnd_interaction.at_mut(id);
                interaction.override_with(&inner.interaction_styles, mask);
            } else {
                cx.renders
                    .rnd_interaction
                    .insert(id, inner.interaction_styles.clone());
            }
        }

        if mask.has_flex_layout() {
            if merge && cx.layouts.lay_flex.contains_key(id) {
                let flex = cx.layouts.lay_flex.at_mut(id);
                flex.override_with(&inner.flex_layout, mask);
            } else {
                cx.layouts.lay_flex.insert(id, inner.flex_layout);
            }
            need_push_dirty = true;
        }

        if mask.has_grid_layout()
            && let Some(ref grid) = inner.grid_layout
        {
            cx.layouts.lay_grid.insert(id, grid.clone());
            need_push_dirty = true;
        }

        if mask.has(ComponentMask::STYLE_SCROLLBAR)
            && let Some(ref sb) = inner.scrollbar_style
        {
            Entity::ensure_scrollbar_entities(cx, id, sb, merge);
            need_push_dirty = true;
        }

        if mask.has(ComponentMask::STYLE_DND_DRAGGABLE)
            && let Some(dp) = inner.drag_property
        {
            cx.states.dnd.dnd_drag_properties.insert(id, dp);
        }
        if mask.has(ComponentMask::STYLE_DND_DROPPABLE)
            && let Some(dp) = inner.drop_property
        {
            cx.states.dnd.dnd_drop_properties.insert(id, dp);
        }

        if need_push_dirty {
            cx.mark_layout_dirty(id);
        }

        cx.resolve_entity_style_state(id, false);
    }

    /// Add a child entity.
    #[must_use]
    #[inline]
    pub fn child(self, entity: impl Into<Prop<Entity>>) -> Self {
        match entity.into() {
            Prop::None => {}
            Prop::Static(child) => {
                with_context(|cx| cx.add_child(self, child));
            }
            Prop::Dynamic(f) => {
                with_context(|cx| {
                    // 前回配置された子要素の ID を安全に記録・保持するセル
                    // エフェクトのクロージャ内に所有権を持たせて生存期間を保証
                    let current_child: Rc<Cell<Option<Entity>>> = Rc::new(Cell::new(None));
                    let current_child_clone = current_child.clone();

                    cx.create_entity_effect(self, EffectCategory::Contents, move |cx| {
                        // 新しい子要素をクロージャから生成
                        let new_child = f();

                        if let Some(old_child) = current_child_clone.get() {
                            // 2回目以降の評価: 旧要素を直接新しい要素へ置換
                            cx.replace_child(self, old_child, new_child);
                        } else {
                            // 初回の評価: ダイレクトに親の子要素リストへ追加
                            cx.add_child(self, new_child);
                        }

                        // 最新の生成要素 ID を退避・更新
                        current_child_clone.set(Some(new_child));
                    });
                });
            }
        }
        self
    }

    /// Dynamically resolve and add [`Entity`] from provider `P`.
    #[must_use]
    #[inline]
    pub fn child_d<P, F>(self, f: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> Entity + Send + Sync + 'static,
    {
        let dynamic_prop = Prop::Dynamic(Box::new(move || {
            let signal = with_context(|cx| cx.use_provided::<P>());
            let val = signal.get();
            f(&val)
        }));
        self.child(dynamic_prop)
    }

    /// Adds multiple child entities at once.
    #[must_use]
    #[inline]
    pub fn children<I, E>(mut self, entities: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<Prop<Entity>>,
    {
        for e in entities {
            self = self.child(e);
        }
        self
    }

    /// It dynamically resolves and adds multiple child entities in bulk from provider `P`.
    ///
    /// # Panics
    /// Not supported dynamic nested entities inside `children_d`.
    #[expect(clippy::panic)]
    #[track_caller]
    #[must_use]
    pub fn children_d<P, F, I, E>(self, f: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> I + Send + Sync + 'static,
        I: IntoIterator<Item = E> + 'static,
        E: Into<Prop<Entity>>,
    {
        with_context(|cx| {
            // 前回の評価でこのスロットによって生成・追加された子要素群のIDを保持するセル
            let current_children: Rc<std::cell::RefCell<Vec<Entity>>> =
                Rc::new(std::cell::RefCell::new(Vec::new()));
            let current_children_clone = current_children.clone();

            cx.create_entity_effect(self, EffectCategory::Contents, move |cx| {
                // プロバイダーの値を動的解決
                let signal = with_context(|cx| cx.use_provided::<P>());
                let val = signal.get();

                // 新しい子要素群の生成
                let mut new_entities: SmallVec<[Entity; 8]> = SmallVec::new();

                for e in f(&val) {
                    if let Prop::Static(entity) = e.into() {
                        new_entities.push(entity);
                    } else {
                        #[cfg(feature = "trace-error")]
                        trace_error!(Some(self), &mut cx.debug, || MichiuTrace::Error {
                            detail: MichiuError::UnsupportedDynamicNesting {
                                caller: "children_d"
                            },
                            add: None,
                        });

                        #[cfg(debug_assertions)]
                        panic!(
                            "children_d: Not supported dynamic nested entities inside children_d"
                        );

                        // リリースビルド時は None を返す代わりにこの要素を無視して次のループへ
                    }
                }

                // 前回マウントした古い子要素群を安全に一括破棄（Taffyツリーからのデタッチ含む）
                let mut old_children = current_children_clone.borrow_mut();
                for old_id in old_children.drain(..) {
                    cx.despawn_internal(old_id);
                }

                // 生成された新しい子要素群を親コンテナの直下にマウント
                for new_c in new_entities {
                    cx.add_child(self, new_c);
                    old_children.push(new_c);
                }

                // 親コンテナのレイアウト再計算と描画をダーティマーク
                cx.mark_dirty(self);
            });
        });

        self
    }

    /// Add a text label as a child entity that allows interactions to pass through.
    #[must_use]
    #[inline]
    pub fn label(
        self,
        content: impl Into<Prop<Cow<'static, str>>>,
        modify: impl Into<Prop<Modifier>>,
    ) -> Self {
        let style = match modify.into() {
            Prop::None => Prop::Static(Modifier::new().pointer_events_none()),
            Prop::Static(s) => Prop::Static(s.pointer_events_none()),
            Prop::Dynamic(f) => Prop::Dynamic(Box::new(move || f().pointer_events_none())),
        };
        let label_el = div_n().text(content).modify(style);
        self.child(label_el)
    }

    /// Dynamically resolve styles from provider `P` and add text labels.
    #[must_use]
    #[inline]
    pub fn label_d<P, F>(self, content: impl Into<Prop<Cow<'static, str>>>, modify: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> Modifier + Send + Sync + 'static,
    {
        // スタイル側のみ、1引数のクロージャをプロバイダー解決を伴う Prop::Dynamic へラップ
        let style_prop = Prop::Dynamic(Box::new(move || {
            let signal = with_context(|cx| cx.use_provided::<P>());
            let val = signal.get();
            modify(&val)
        }));

        self.label(content, style_prop)
    }

    /// This will replace the contents of this container. All previous contents will be discarded.
    #[expect(clippy::return_self_not_must_use)]
    pub fn set_contents(self, contents: impl Into<Prop<Entity>>) -> Self {
        match contents.into() {
            Prop::None => {}
            Prop::Static(new_child) => {
                with_context(|cx| {
                    // 静的なコンテンツ上書き時のみ古い動的評価エフェクトを一括破棄
                    if let Some(effects) = cx.reactive.react_entity_effects.find_mut(self)
                        && let Some(i) = effects
                            .iter()
                            .position(|(cat, _)| *cat == EffectCategory::Contents)
                    {
                        let (_, old_effect) = effects.remove(i);
                        cx.reactive.react_effects.remove(old_effect);
                        cx.reactive.react_effect_to_entity.remove(old_effect);
                        cx.reactive
                            .react_pending_entity_effects
                            .retain(|&x| x != old_effect);
                    }
                    self.set_contents_internal(cx, new_child);
                });
            }
            Prop::Dynamic(f) => {
                with_context(|cx| {
                    cx.create_entity_effect(self, EffectCategory::Contents, move |cx| {
                        let new_child = f();
                        // Dynamic 実行時は自殺させないためそのままマウントを実行
                        self.set_contents_internal(cx, new_child);
                    });
                });
            }
        }
        self
    }

    /// 既存のクロージャをクリアせずに、子要素の差し替え（マウント）のみを実行する
    #[track_caller]
    fn set_contents_internal(self, cx: &mut Context, new_child: Entity) {
        // 親コンテナに紐づくスクロールバー専用要素のIDを安全に抽出
        let mut scrollbar_ids = std::collections::HashSet::new();
        if let Some(sb) = cx.layouts.scrollbar.bar_styles.find(self) {
            if let Some(tid) = sb.v_track_id {
                scrollbar_ids.insert(tid);
            }
            if let Some(tid) = sb.v_thumb_id {
                scrollbar_ids.insert(tid);
            }
            if let Some(tid) = sb.h_track_id {
                scrollbar_ids.insert(tid);
            }
            if let Some(tid) = sb.h_thumb_id {
                scrollbar_ids.insert(tid);
            }
        }

        // 現在の子要素のうち、スクロールバー関係の要素以外のコンテンツのみを再帰破棄
        let children_list = cx.topology.topo_children.at(self);
        let old_children: SmallVec<[Entity; 8]> = children_list.iter().copied().collect();
        for child_id in old_children {
            if !scrollbar_ids.contains(&child_id) {
                cx.despawn_internal(child_id);
            }
        }

        // 新しい子要素を追加（親子トポロジーおよび Taffy ツリーの同期）
        cx.add_child(self, new_child);

        // レイアウトと描画の再計算を要求
        cx.mark_dirty(self);
    }

    /// Set text.
    #[must_use]
    #[inline]
    pub fn text(self, content: impl Into<Prop<Cow<'static, str>>>) -> Self {
        self.bind_prop(content, EffectCategory::Text, |cx, id, val| {
            cx.contents.cont_text_contents.insert(id, val.into());
            cx.topology
                .topo_active_masks
                .at_mut(id)
                .set(ComponentMask::COMP_TEXT_CONTENT);
            SystemStore::clear_text_buffer_cache(id, &cx.system.sys_text_buffers);
            cx.mark_dirty(id);
        })
    }

    /// The text is dynamically set from provider `P`.
    #[must_use]
    #[inline]
    pub fn text_d<P, F, S>(self, f: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> S + Send + Sync + 'static,
        S: Into<Cow<'static, str>>,
    {
        let dynamic_prop = Prop::Dynamic(Box::new(move || {
            let signal = with_context(|cx| cx.use_provided::<P>());
            let val = signal.get();
            f(&val).into()
        }));
        self.text(dynamic_prop)
    }

    /// Bind the texture provider ([`ExternalTexture`]).
    ///
    /// - Dynamic texture updates
    ///
    ///   Dynamic requirements, such as updating the texture content every frame,
    ///   are automatically handled within the provider's internal processing
    ///   because [`ExternalTexture::resolve_view()`] is called every time just before rendering.
    ///
    /// - If you want to dynamically change the texture source itself, please choose one of the following methods:
    ///
    ///   1. Replace topology
    ///
    ///   The target UI entity is despawned and then regenerated as an entity with a new source specified
    ///   (this safely releases unnecessary resources from VRAM).
    ///
    ///   2. Switch within the provider's network.
    ///
    ///   The object that implements `ExternalTexture` is maintained,
    ///   and commands are sent to the internal decoder, etc.,
    ///   to dynamically switch the texture ([`wgpu::TextureView`]) returned by `resolve_view()`.
    ///
    /// # Design considerations
    ///
    /// Because creating or recreating a [`wgpu::BindGroup`] is computationally expensive,
    /// this method does not support dynamically swapping textures via the lightweight `Prop<T>` (reactive property).
    /// Allowing swaps through `Prop<T>` would make it too easy to write code
    /// that drastically degrades performance—such as instantiating a new provider every frame within a reactive effect.
    #[inline]
    #[must_use]
    pub fn external_texture(self, texture: impl ExternalTexture + 'static) -> Self {
        let texture_arc = Arc::new(texture);
        let metadata = texture_arc.metadata();

        with_context(|cx| {
            cx.contents.cont_external_textures.insert(self, texture_arc);
            cx.topology
                .topo_active_masks
                .at_mut(self)
                .set(ComponentMask::COMP_EXTERNAL_TEXTURE_CONTENT);

            if !cx.layouts.lay_base_basic.contains_key(self) {
                cx.layouts
                    .lay_base_basic
                    .insert(self, BasicLayout::default());
            }
            let basic = cx.layouts.lay_base_basic.at_mut(self);
            basic.size.width = Val::Px(metadata.size.width);
            basic.size.height = Val::Px(metadata.size.height);

            cx.mark_dirty(self);
        });
        self
    }

    /// Integrates an arbitrary `IDCompositionVisual2` into the layout tree.
    #[inline]
    #[must_use]
    pub fn external_visual(self, visual: impl ExternalVisual + 'static) -> Self {
        let visual_arc = Arc::new(visual);
        let metadata = visual_arc.metadata();

        with_context(|cx| {
            cx.contents.cont_external_visual.insert(self, visual_arc);
            cx.topology
                .topo_active_masks
                .at_mut(self)
                .set(ComponentMask::COMP_EXTERNAL_VISUAL_CONTENT);

            if metadata.size.width > 0.0 || metadata.size.height > 0.0 {
                if !cx.layouts.lay_base_basic.contains_key(self) {
                    cx.layouts
                        .lay_base_basic
                        .insert(self, BasicLayout::default());
                }
                let basic = cx.layouts.lay_base_basic.at_mut(self);
                basic.size.width = Val::Px(metadata.size.width);
                basic.size.height = Val::Px(metadata.size.height);
            }

            cx.mark_dirty(self);
        });
        self
    }

    /// Sets accessibility roles and labels.
    ///
    /// The `a11y::Role` set here takes precedence over [`Entity::tag_a11y`] , [`Entity::tag`].
    #[must_use]
    pub fn a11y<M>(self, node: impl IntoOptionProp<Node, M>) -> Self {
        let prop = node.into_option_prop();
        self.bind_prop(prop, EffectCategory::Accessibility, move |cx, id, val| {
            cx.acce.acce_accessibility.insert(id, val);
            cx.topology
                .topo_active_masks
                .at_mut(id)
                .set(ComponentMask::COMP_A11Y);
            cx.mark_layout_dirty(id);
        })
    }

    /// Sets accessibility roles and labels.
    ///
    /// The `a11y::Role` set here takes precedence over [`Entity::tag_a11y`] , [`Entity::tag`].
    #[must_use]
    pub fn a11y_with<M, F>(self, node: impl IntoOptionProp<Node, M>, f: F) -> Self
    where
        F: Fn(&mut Node) + 'static,
    {
        let prop = match node.into_option_prop() {
            Prop::None => Prop::None,
            // node 自体は静的だが、クロージャ f 内部で Signal を読む可能性がある場合
            Prop::Static(base) => Prop::Dynamic(Box::new(move || {
                let mut n = base.clone();
                if let Some(ref mut node) = n {
                    f(node);
                }
                n
            })),
            // node 自体も動的で、さらにクロージャ f 内でも Signal を読む場合
            Prop::Dynamic(dyn_fn) => Prop::Dynamic(Box::new(move || {
                let mut n = dyn_fn();
                if let Some(ref mut node) = n {
                    f(node);
                }
                n
            })),
        };

        self.a11y(prop)
    }

    /// Sets accessibility roles.
    ///
    /// The `a11y::Role` set here takes precedence over [`Entity::tag_a11y`] , [`Entity::tag`].
    #[must_use]
    pub fn a11y_role<M>(self, role: impl IntoOptionProp<Role, M>) -> Self {
        let prop = role.into_option_prop();
        self.bind_prop(prop, EffectCategory::Accessibility, move |cx, id, val| {
            cx.acce.acce_accessibility.insert(id, val.map(Node::new));
            cx.topology
                .topo_active_masks
                .at_mut(id)
                .set(ComponentMask::COMP_A11Y);
            cx.mark_layout_dirty(id);
        })
    }

    /// Sets accessibility roles and label.
    ///
    /// The `a11y::Role` set here takes precedence over [`Entity::tag_a11y`] , [`Entity::tag`].
    #[must_use]
    pub fn a11y_label<M>(
        self,
        role: impl IntoOptionProp<Role, M>,
        label: impl Into<Cow<'static, str>>,
    ) -> Self {
        let label = label.into();
        let prop = role.into_option_prop();
        self.bind_prop(prop, EffectCategory::Accessibility, move |cx, id, val| {
            let mut node = val.map(Node::new);
            if let Some(n) = &mut node {
                n.set_label(label.clone());
            }
            cx.acce.acce_accessibility.insert(id, node);
            cx.topology
                .topo_active_masks
                .at_mut(id)
                .set(ComponentMask::COMP_A11Y);
            cx.mark_layout_dirty(id);
        })
    }

    /// スクロールコンテナのスタイル設定に連動し、
    /// トラック・サムに相当する要素を遅延生成して親子関係にアタッチする。
    #[track_caller]
    #[inline]
    pub(crate) fn ensure_scrollbar_entities(
        cx: &mut Context,
        id: Entity,
        sb: &ScrollbarStyle,
        merge: bool,
    ) {
        if !cx.layouts.scrollbar.bar_styles.contains_key(id) {
            cx.layouts.scrollbar.bar_styles.insert(
                id,
                ScrollBarState {
                    style: sb.clone(),
                    ..Default::default()
                },
            );
        }

        let mut state = cx.layouts.scrollbar.bar_styles.at(id).clone();
        state.style = sb.clone();
        let mut changed = false;

        if sb.display != ScrollbarDisplay::None {
            // 縦スクロールバー (V-Track)
            let v_track = if let Some(v_track) = state.v_track_id {
                v_track
            } else {
                let v_track = cx.spawn(Some(id));
                cx.add_child(id, v_track);
                state.v_track_id = Some(v_track);
                changed = true;
                v_track
            };

            // トラックは常に絶対配置（コンテナの右端に固定）
            let track_style = sb
                .v_track
                .clone()
                .unwrap_or_default()
                .absolute()
                .z(9999)
                .w(sb.width)
                .inset((0.0, 0.0, 0.0, crate::auto()))
                .pointer_events_auto(); // イベントを透過させない

            Entity::modify_internal(cx, v_track, &track_style, merge);

            // 縦つまみ (V-Thumb、V-Track の子要素としてアタッチ)
            let v_thumb = if let Some(v_thumb) = state.v_thumb_id {
                v_thumb
            } else {
                let v_thumb = cx.spawn(Some(v_track));
                cx.add_child(v_track, v_thumb);
                state.v_thumb_id = Some(v_thumb);
                changed = true;
                v_thumb
            };

            let mut thumb_width = sb.width;
            if let Some(ref thumb_style) = sb.v_thumb
                && let Val::Px(w) = thumb_style.inner.basic_layout.size.width
            {
                thumb_width = w.min(sb.width);
            }

            // サムは V-Track の絶対座標を原点とし、Y方向のみ absolute スライド
            let thumb_style = sb
                .v_thumb
                .clone()
                .unwrap_or_default()
                .absolute()
                .w(thumb_width)
                .inset((0.0, crate::auto(), crate::auto(), crate::auto()))
                .pointer_events_auto();

            Entity::modify_internal(cx, v_thumb, &thumb_style, merge);

            // 横スクロールバー (H-Track)
            let h_track = if let Some(h_track) = state.h_track_id {
                h_track
            } else {
                let h_track = cx.spawn(Some(id));
                cx.add_child(id, h_track);
                state.h_track_id = Some(h_track);
                changed = true;
                h_track
            };

            let track_style = sb
                .h_track
                .clone()
                .unwrap_or_default()
                .absolute()
                .z(9999)
                .h(sb.width)
                .inset((crate::auto(), 0.0, 0.0, 0.0))
                .pointer_events_auto();

            Entity::modify_internal(cx, h_track, &track_style, merge);

            // B-1. 横つまみ (H-Thumb、H-Track の子要素としてアタッチ)
            let h_thumb = if let Some(h_thumb) = state.h_thumb_id {
                h_thumb
            } else {
                let h_thumb = cx.spawn(Some(h_track));
                cx.add_child(h_track, h_thumb);
                state.h_thumb_id = Some(h_thumb);
                changed = true;
                h_thumb
            };

            let mut thumb_height = sb.width;
            if let Some(ref thumb_style) = sb.h_thumb
                && let Val::Px(h) = thumb_style.inner.basic_layout.size.height
            {
                thumb_height = h.min(sb.width);
            }

            let thumb_style = sb
                .h_thumb
                .clone()
                .unwrap_or_default()
                .absolute()
                .h(thumb_height)
                .inset((crate::auto(), crate::auto(), crate::auto(), 0.0))
                .pointer_events_auto();

            Entity::modify_internal(cx, h_thumb, &thumb_style, merge);
        }

        if changed {
            *cx.layouts.scrollbar.bar_styles.at_mut(id) = state;
            cx.topology.topo_is_structure_dirty = true;
            cx.topology.topo_is_sort_dirty = true;
        }
    }
}

#[cfg(test)]
mod tests;
