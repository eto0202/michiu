pub mod handler;
pub mod input_func;

use crate::{
    BasicLayout, ComponentMask, Context, DebugStore, EffectCategory, EntityId, ExternalTexture,
    MichiuError, MichiuSoA, MichiuTrace, ReadSignal, ScrollBarState, ScrollbarDisplay,
    ScrollbarStyle, StyleTarget, SystemStore, ThisStyle, UiaValue, Val, WebView2Contents,
    create_effect, div_n, trace_error,
};
#[cfg(feature = "trace-lifecycle")]
use crate::{ContextState, trace_lifecycle};
use smallvec::SmallVec;
use std::{borrow::Cow, cell::Cell, rc::Rc, sync::Arc};

thread_local! {
    // 現在構築中のUIコンテキストへの生ポインタを一時的にバインドするグローバルスレッド領域
    static ACTIVE_CONTEXT: Cell<Option<*mut Context>> = const { Cell::new(None) };
}

/// Builds the UI and returns the root element.
///
/// # Examples
///
/// ```rust
/// use michiu_ui::{build_ui, div, div_n, ts, Color, Context};
///
/// let mut cx = Context::new();
/// let root = build_ui(cx, || {
///     div(ts().bg_color(Color::GREEN))
///         .child(div_n()) // empty container
/// });
///
/// ```
pub fn build_ui(cx: &mut Context, f: impl FnOnce() -> Element) -> Element {
    let old = ACTIVE_CONTEXT.get();
    let current = std::ptr::from_mut::<Context>(cx);

    ACTIVE_CONTEXT.set(Some(current));
    let _guard = ContextGuard { current, old };

    // // 未定義動作を避けるためこれ以降は `cx` を直接触らない
    let marker = unsafe { (*current).start_session() };
    let result = f();
    unsafe {
        // 戻り値に含まれるハンドルをルート要素として登録
        (*current).register_root(result.id);
        // 親子関係に組み込まれなかった無駄な孤児を自動一掃
        (*current).end_session(marker);

        // ツリーのすべてのトポロジーおよび provide 関係が組み上がったこの瞬間に、
        // キューされて保留されていた全子孫要素のエフェクトを一括して初回評価
        (*current).evaluate_pending_element_effects();
    }

    #[cfg(feature = "trace-lifecycle")]
    trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::BuildElement {
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
#[allow(clippy::panic)]
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

/// A lightweight handle representing a completed UI element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Element {
    pub(crate) id: EntityId,
}

impl Default for Element {
    fn default() -> Self {
        Self::new()
    }
}

impl From<EntityId> for Element {
    fn from(id: EntityId) -> Self {
        Self { id }
    }
}

impl Element {
    /// Start building the new elements.
    ///
    /// Prefer using [`crate::div`], [`crate::div_n`], [`crate::v_flex`], [`crate::h_flex`] etc.
    /// functions for a cleaner syntax.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        // スレッドローカルの Context から安全に要素を spawn
        let id = with_context(|cx| cx.spawn(None));
        Self { id }
    }

    /// Get the internal [`EntityId`].
    #[inline]
    #[must_use]
    pub fn id(&self) -> EntityId {
        self.id
    }

    /// Get the handle of the parent element.
    #[inline]
    #[must_use]
    pub fn parent_element(self) -> Option<Element> {
        with_context(|cx| cx.parent_element(self))
    }

    /// Get the list of handles for the child elements.
    #[inline]
    #[must_use]
    pub fn children_list(self) -> Vec<Element> {
        with_context(|cx| cx.children_list(self).collect())
    }

    /// Provides a context (signal) of type `T`.
    ///
    /// Obtainable in this element and all its descendant elements via `use_provided::<T>()`.
    #[inline]
    #[must_use]
    pub fn provide<T: Send + 'static>(self, read_signal: ReadSignal<T>) -> Self {
        with_context(|cx| {
            cx.provide::<T>(Some(self.id), read_signal);
        });
        self
    }

    /// This element will be tagged `T`.
    ///
    /// Multiple tags can be attached to the same element.
    ///
    /// # Examples
    ///
    /// ```rust
    /// struct Tag;
    ///
    /// div_n().tag::<Tag>()
    ///
    /// ```
    #[inline]
    #[must_use]
    pub fn tag<T: 'static>(self) -> Self {
        with_context(|cx| cx.tag::<T>(self));
        self
    }

    /// Retrieve the first element of type `T` found among the descendants.
    #[inline]
    #[must_use]
    pub fn try_query_descendant<T: 'static>(&self) -> Option<Element> {
        with_context(|cx| cx.try_query_descendant::<T>(self.id))
    }

    /// Retrieve the first element of type `T` found among the descendants.
    #[track_caller]
    #[inline]
    #[must_use]
    pub fn query_descendant<T: 'static>(&self) -> Element {
        with_context(|cx| cx.query_descendant::<T>(self.id))
    }

    /// Search for an element of type `T` among its descendants.
    #[inline]
    #[must_use]
    pub fn query_descendants<T: 'static>(&self) -> Vec<Element> {
        with_context(|cx| cx.query_descendants::<T>(self.id).collect())
    }

    /// Search for an element of type `T` among its descendants.
    #[inline]
    pub fn for_each_descendants<T: 'static>(&self, mut f: impl FnMut(Element)) {
        with_context(|cx| {
            for id in cx.query_descendants::<T>(self.id) {
                f(id);
            }
        });
    }

    /// 静的な値、または動的に変化する Prop を該当する `EffectCategory` を通じて自動バインド
    fn bind_prop<T: 'static>(
        self,
        prop: impl Into<Prop<T>>,
        category: EffectCategory,
        mut apply_fn: impl FnMut(&mut Context, EntityId, T) + 'static,
    ) -> Self {
        let id = self.id;
        match prop.into() {
            Prop::None => {}
            Prop::Static(val) => {
                with_context(|cx| apply_fn(cx, id, val));
            }
            Prop::Dynamic(f) => {
                with_context(|cx| {
                    cx.create_element_effect(id, category, move |cx| {
                        let val = f();
                        apply_fn(cx, id, val);
                    });
                });
            }
        }
        self
    }

    /// Applies styles.
    #[must_use]
    pub fn style(self, style: impl Into<Prop<ThisStyle>>) -> Self {
        match style.into() {
            Prop::None => {}
            Prop::Static(s) => {
                let id = self.id;
                with_context(|cx| {
                    // 静的なスタイルプロパティを通常通りマウント
                    // 静的チェーンはマージ（merge = true）
                    with_context(|cx| Element::style_internal(cx, id, &s, true));

                    // 動的なセッターが存在する場合、それらを単一のエフェクトとして登録
                    if !s.inner.dynamic_setters.is_empty() {
                        let setters = s.inner.dynamic_setters.clone();
                        cx.create_element_effect(id, EffectCategory::Style, move |cx| {
                            // すべての動的セッターを実行
                            for setter in &setters {
                                setter(cx, id, StyleTarget::Base);
                            }
                            // 状態が変化したため、最後に必ずスタイル解決
                            cx.resolve_element_style_state(id, false);
                        });
                    }
                });
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                with_context(|cx| {
                    cx.create_element_effect(id, EffectCategory::Style, move |cx| {
                        let s = f();

                        // 動的評価された最新スタイルは蓄積を避けるため置換（merge = false）
                        // 修正: 動的評価されたスタイルもマージ（true）としてマウント
                        Element::style_internal(cx, id, &s, true);

                        // 動的スタイルが自身の中で動的なプロバイダーを含む場合も評価
                        for setter in &s.inner.dynamic_setters {
                            setter(cx, id, StyleTarget::Base);
                        }
                        cx.resolve_element_style_state(id, false);
                    });
                });
            }
        }
        self
    }

    /// Dynamically resolve [`ThisStyle`] from provider `P` and apply the style.
    ///
    /// When `P` changes, the closure is re-evaluated.
    #[must_use]
    #[inline]
    pub fn style_d<P, F>(self, f: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> ThisStyle + Send + Sync + 'static,
    {
        // 1引数のクロージャを、プロバイダー探索とシグナル購読（.get()）を内包した
        // 引数なしの Prop::Dynamic クロージャへラップして既存の style メソッドへ委譲
        let dynamic_prop = Prop::Dynamic(Box::new(move || {
            let signal = with_context(|cx| cx.use_provided::<P>());
            let val = signal.get();
            f(&val)
        }));

        self.style(dynamic_prop)
    }

    /// スタイルの適用
    pub(crate) fn style_internal(cx: &mut Context, id: EntityId, style: &ThisStyle, merge: bool) {
        let inner = &style.inner;
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
            Element::ensure_scrollbar_elements(cx, id, sb, merge);
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

        cx.resolve_element_style_state(id, false);
    }

    /// Add a child element.
    #[must_use]
    #[inline]
    pub fn child(self, element: impl Into<Prop<Element>>) -> Self {
        match element.into() {
            Prop::None => {}
            Prop::Static(child) => {
                with_context(|cx| cx.add_child(self.id, child.id));
            }
            Prop::Dynamic(f) => {
                let parent_id = self.id;

                with_context(|cx| {
                    // 前回配置された子要素の ID を安全に記録・保持するセル
                    // エフェクトのクロージャ内に所有権を持たせて生存期間を保証
                    let current_child: Rc<Cell<Option<EntityId>>> = Rc::new(Cell::new(None));
                    let current_child_clone = current_child.clone();

                    cx.create_element_effect(parent_id, EffectCategory::Contents, move |cx| {
                        // 新しい子要素をクロージャから生成
                        let new_child = f();

                        if let Some(old_child) = current_child_clone.get() {
                            // 2回目以降の評価: 旧要素を直接新しい要素へ置換
                            cx.replace_child(parent_id, old_child, new_child.id);
                        } else {
                            // 初回の評価: ダイレクトに親の子要素リストへ追加
                            cx.add_child(parent_id, new_child.id);
                        }

                        // 最新の生成要素 ID を退避・更新
                        current_child_clone.set(Some(new_child.id));
                    });
                });
            }
        }
        self
    }

    /// Dynamically resolve and add [`Element`] from provider `P`.
    #[must_use]
    #[inline]
    pub fn child_d<P, F>(self, f: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> Element + Send + Sync + 'static,
    {
        let dynamic_prop = Prop::Dynamic(Box::new(move || {
            let signal = with_context(|cx| cx.use_provided::<P>());
            let val = signal.get();
            f(&val)
        }));
        self.child(dynamic_prop)
    }

    /// Adds multiple child elements at once.
    #[must_use]
    #[inline]
    pub fn children<I, E>(mut self, elements: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<Prop<Element>>,
    {
        for el in elements {
            self = self.child(el);
        }
        self
    }

    /// It dynamically resolves and adds multiple child elements in bulk from provider `P`.
    ///
    /// # Panics
    /// Not supported dynamic nested elements inside `children_d`.
    #[allow(clippy::panic)]
    #[track_caller]
    #[must_use]
    pub fn children_d<P, F, I, E>(self, f: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> I + Send + Sync + 'static,
        I: IntoIterator<Item = E> + 'static,
        E: Into<Prop<Element>>,
    {
        let parent_id = self.id;

        with_context(|cx| {
            // 前回の評価でこのスロットによって生成・追加された子要素群のIDを保持するセル
            let current_children: Rc<std::cell::RefCell<Vec<EntityId>>> =
                Rc::new(std::cell::RefCell::new(Vec::new()));
            let current_children_clone = current_children.clone();

            cx.create_element_effect(parent_id, EffectCategory::Contents, move |cx| {
                // プロバイダーの値を動的解決
                let signal = with_context(|cx| cx.use_provided::<P>());
                let val = signal.get();

                // 新しい子要素群の生成
                let mut new_elements: SmallVec<[Element; 8]> = SmallVec::new();

                for e in f(&val) {
                    if let Prop::Static(el) = e.into() {
                        new_elements.push(el);
                    } else {
                        #[cfg(feature = "trace-error")]
                        trace_error!(Some(parent_id), &mut cx.debug, || MichiuTrace::Error {
                            detail: MichiuError::UnsupportedDynamicNesting {
                                caller: "children_d"
                            },
                            add: None,
                        });

                        #[cfg(debug_assertions)]
                        panic!(
                            "children_d: Not supported dynamic nested elements inside children_d"
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
                for new_el in &new_elements {
                    cx.add_child(parent_id, new_el.id);
                    old_children.push(new_el.id);
                }

                // 親コンテナのレイアウト再計算と描画をダーティマーク
                cx.mark_dirty(parent_id);
            });
        });

        self
    }

    /// Add a text label as a child element that allows interactions to pass through.
    #[must_use]
    #[inline]
    pub fn label(
        self,
        content: impl Into<Prop<Cow<'static, str>>>,
        style: impl Into<Prop<ThisStyle>>,
    ) -> Self {
        let style = match style.into() {
            Prop::None => Prop::Static(ThisStyle::new().pointer_events_none()),
            Prop::Static(s) => Prop::Static(s.pointer_events_none()),
            Prop::Dynamic(f) => Prop::Dynamic(Box::new(move || f().pointer_events_none())),
        };
        let label_el = div_n().text(content).style(style);
        self.child(label_el)
    }

    /// Dynamically resolve styles from provider `P` and add text labels.
    #[must_use]
    #[inline]
    pub fn label_d<P, F>(self, content: impl Into<Prop<Cow<'static, str>>>, style: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> ThisStyle + Send + Sync + 'static,
    {
        // スタイル側のみ、1引数のクロージャをプロバイダー解決を伴う Prop::Dynamic へラップ
        let style_prop = Prop::Dynamic(Box::new(move || {
            let signal = with_context(|cx| cx.use_provided::<P>());
            let val = signal.get();
            style(&val)
        }));

        self.label(content, style_prop)
    }

    /// This will replace the contents of this container. All previous contents will be discarded.
    #[allow(clippy::return_self_not_must_use)]
    pub fn set_contents(self, contents: impl Into<Prop<Element>>) -> Self {
        match contents.into() {
            Prop::None => {}
            Prop::Static(new_child) => {
                let id = self.id;
                with_context(|cx| {
                    // 静的なコンテンツ上書き時のみ古い動的評価エフェクトを一括破棄
                    if let Some(effects) = cx.reactive.react_element_effects.find_mut(id)
                        && let Some(i) = effects
                            .iter()
                            .position(|(cat, _)| *cat == EffectCategory::Contents)
                    {
                        let (_, old_effect) = effects.remove(i);
                        cx.reactive.react_effects.remove(old_effect);
                        cx.reactive.react_effect_to_element.remove(old_effect);
                        cx.reactive
                            .react_pending_element_effects
                            .retain(|&x| x != old_effect);
                    }
                    self.set_contents_internal(cx, new_child);
                });
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                with_context(|cx| {
                    cx.create_element_effect(id, EffectCategory::Contents, move |cx| {
                        let new_child = f();
                        let container = Element { id };
                        // Dynamic 実行時は自殺させないためそのままマウントを実行
                        container.set_contents_internal(cx, new_child);
                    });
                });
            }
        }
        self
    }

    /// 既存のクロージャをクリアせずに、子要素の差し替え（マウント）のみを実行する
    fn set_contents_internal(self, cx: &mut Context, new_child: Element) {
        let id = self.id;

        // 親コンテナに紐づくスクロールバー専用要素のIDを安全に抽出
        let mut scrollbar_ids = std::collections::HashSet::new();
        if let Some(sb) = cx.layouts.scrollbar.bar_styles.find(id) {
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
        let children_list = cx.topology.topo_children.at(id);
        let old_children: SmallVec<[EntityId; 8]> = children_list.iter().copied().collect();
        for child_id in old_children {
            if !scrollbar_ids.contains(&child_id) {
                cx.despawn_internal(child_id);
            }
        }

        // 新しい子要素を追加（親子トポロジーおよび Taffy ツリーの同期）
        cx.add_child(id, new_child.id);

        // レイアウトと描画の再計算を要求
        cx.mark_dirty(id);
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
    ///   The target UI element is despawned and then regenerated as an element with a new source specified
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
        let id = self.id;

        let metadata = texture_arc.metadata();

        with_context(|cx| {
            cx.contents.cont_external_textures.insert(id, texture_arc);
            cx.topology
                .topo_active_masks
                .at_mut(id)
                .set(ComponentMask::COMP_EXTERNAL_TEXTURE_CONTENT);

            if !cx.layouts.lay_base_basic.contains_key(id) {
                cx.layouts.lay_base_basic.insert(id, BasicLayout::default());
            }
            let basic = cx.layouts.lay_base_basic.at_mut(id);
            basic.size.width = Val::Px(metadata.size.width);
            basic.size.height = Val::Px(metadata.size.height);

            cx.mark_dirty(id);
        });
        self
    }

    /// Place the webview2 component.
    #[inline]
    #[must_use]
    pub fn webview2(self, contents: impl Into<Prop<WebView2Contents>>) -> Self {
        self.bind_prop(contents, EffectCategory::Movie, |cx, id, src| {
            cx.contents.cont_webview_contents.insert(id, src);
            cx.topology.topo_webview_entities.push(id);
            cx.topology
                .topo_active_masks
                .at_mut(id)
                .set(ComponentMask::COMP_WEBVIEW_CONTENT);
            cx.mark_dirty(id);
        })
    }

    /// Dynamically resolves and attaches webview2 settings from provider `P`.
    #[must_use]
    #[inline]
    pub fn webview2_d<P, F>(self, f: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> WebView2Contents + Send + Sync + 'static,
    {
        let dynamic_prop = Prop::Dynamic(Box::new(move || {
            let signal = with_context(|cx| cx.use_provided::<P>());
            let val = signal.get();
            f(&val)
        }));
        self.webview2(dynamic_prop)
    }

    /// Not implemented
    #[must_use]
    #[inline]
    pub fn uia_property(self, property_id: i32, value: impl Into<UiaValue>) -> Self {
        with_context(|cx| self.uia_property_internal(cx, property_id, value.into()));
        self
    }

    /// Not implemented
    #[must_use]
    pub fn uia_name(self, name: impl Into<Prop<Cow<'static, str>>>) -> Self {
        match name.into() {
            Prop::None => self,
            Prop::Static(s) => self.uia_property(30005, UiaValue::String(s.into())),
            Prop::Dynamic(f) => {
                let id = self.id;
                let effect_id = create_effect(move |cx| {
                    let s = f();
                    let el = Element { id };
                    el.uia_property_internal(cx, 30005, UiaValue::String(s.into()));
                });
                with_context(|cx| {
                    cx.register_element_effect(id, EffectCategory::UiaName, effect_id);
                });
                self
            }
        }
    }

    fn uia_property_internal(self, cx: &mut Context, property_id: i32, value: UiaValue) {
        if !cx.system.sys_uia_properties.contains_key(self.id) {
            cx.system.sys_uia_properties.insert(self.id, Vec::new());
        }
        let list = cx.system.sys_uia_properties.at_mut(self.id);
        if let Some(pos) = list.iter().position(|(k, _)| *k == property_id) {
            list[pos].1 = value;
        } else {
            list.push((property_id, value));
        }
        cx.topology
            .topo_active_masks
            .at_mut(self.id)
            .set(ComponentMask::COMP_UIA_CONTENT);
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn uia_automation_id(self, id: impl Into<Prop<Cow<'static, str>>>) -> Self {
        match id.into() {
            Prop::None => self,
            Prop::Static(s) => self.uia_property(30011, UiaValue::String(s.into())),
            Prop::Dynamic(f) => {
                let id = self.id;
                let effect_id = create_effect(move |cx| {
                    let s = f();
                    let el = Element { id };
                    el.uia_property_internal(cx, 30011, UiaValue::String(s.into()));
                });
                with_context(|cx| {
                    cx.register_element_effect(id, EffectCategory::UiaAutomationId, effect_id);
                });
                self
            }
        }
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn uia_control_type(self, control_type_id: i32) -> Self {
        self.uia_property(30003, control_type_id)
    }

    /// スクロールコンテナのスタイル設定に連動し、
    /// トラック・サムに相当する要素を遅延生成して親子関係にアタッチする。
    #[inline]
    pub(crate) fn ensure_scrollbar_elements(
        cx: &mut Context,
        id: EntityId,
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

            Element::style_internal(cx, v_track, &track_style, merge);

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

            Element::style_internal(cx, v_thumb, &thumb_style, merge);

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

            Element::style_internal(cx, h_track, &track_style, merge);

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

            Element::style_internal(cx, h_thumb, &thumb_style, merge);
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
