use crate::{
    BasicLayout, COMP_IMAGE_CONTENT, COMP_INPUT_CONTENT, COMP_MOVIE_CONTENT, COMP_TEXT_CONTENT,
    COMP_UIA_CONTENT, COMP_WEBVIEW_CONTENT, Context, EffectCategory, ElementState, EntityId,
    EventListeners, ImageMetadata, ImageSource, ImeState, InputContents, LayoutPoint, LayoutSize,
    LayoutStore, Modifiers, MouseButton, MovieMetadata, MovieProperty, ReadSignal, Rect,
    STYLE_DND_DRAGGABLE, STYLE_DND_DROPPABLE, STYLE_INTERACTION_PARENT, STYLE_INTERACTION_PROPERTY,
    STYLE_INTERACTION_WITHIN, STYLE_SCROLLBAR, STYLE_TEXT_SPANS, ScrollBarState, ScrollbarDisplay,
    ScrollbarStyle, Size, StyleTarget, TextAlign, TextSpan, ThisStyle, UiaValue, UnderlineStyle,
    Val, VirtualKey, VisualProperty, WebView2Contents, create_effect, div_n,
};
use std::{borrow::Cow, cell::Cell, path::PathBuf, rc::Rc};

thread_local! {
    // 現在構築中のUIコンテキストへの生ポインタを一時的にバインドするグローバルスレッド領域。
    // UI構築は常に単一のスレッド（メインスレッド）で行われるため、このアプローチは安全に機能。
    static ACTIVE_CONTEXT: Cell<Option<*mut Context>> = const { Cell::new(None) };
}

/// ユーザーがコンポーネントを評価する際に呼び出すグローバルラッパー
pub fn build_ui(cx: &mut Context, f: impl FnOnce() -> Element) -> Element {
    let old = ACTIVE_CONTEXT.get();
    ACTIVE_CONTEXT.set(Some(std::ptr::from_mut::<Context>(cx)));
    let _guard = ContextGuard { old };

    let marker = cx.start_session();
    let result = f();
    // 戻り値に含まれるハンドルをルート要素として登録
    cx.register_root(result.id);
    // 親子関係に組み込まれなかった無駄な孤児を自動一掃
    cx.end_session(marker);

    // ツリーのすべてのトポロジーおよび provide 関係が組み上がったこの瞬間に、
    // キューされて保留されていた全子孫要素のエフェクトを一括して初回評価
    cx.evaluate_pending_element_effects();

    result
}

/// スレッドローカルから安全にContextへのアクセスを解決する内部ヘルパー
#[inline]
pub(crate) fn with_context<R>(f: impl FnOnce(&mut Context) -> R) -> R {
    let ptr = ACTIVE_CONTEXT
        .get()
        .expect("No active UI Context found in this thread context");
    // UIスレッドは単一かつ非同期にまたがらないため、ポインタの生存期間は保証される。
    unsafe { f(&mut *ptr) }
}

// コンテキストを復元するための一時的なガード構造体
pub(crate) struct ContextGuard {
    old: Option<*mut Context>,
}

/// `現在のスレッドローカル（ACTIVE_CONTEXT）に` Context を一時的にバインドします。
/// 戻り値のガードオブジェクト（ContextGuard）がスコープを抜ける際、自動的に元のコンテキストに復元されます。
#[inline]
pub(crate) fn bind_context(cx: &Context) -> ContextGuard {
    let old = ACTIVE_CONTEXT.get();
    // 借用チェッカーと衝突しないよう生ポインタキャストを行ってスレッドローカルに格納
    ACTIVE_CONTEXT.set(Some(std::ptr::from_ref::<Context>(cx).cast_mut()));
    ContextGuard { old }
}

impl Drop for ContextGuard {
    #[inline]
    fn drop(&mut self) {
        ACTIVE_CONTEXT.set(self.old);
    }
}

/// 静的な値、または動的に変化する値（Signalやクロージャ）を抽象化する型
pub enum Prop<T> {
    None,
    Static(T),
    Dynamic(Box<dyn Fn() -> T + 'static>),
}

/// 構築が完了したUI要素を表す軽量なハンドル
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
    /// 新規要素の構築を開始します
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        // スレッドローカルの Context から安全に要素を spawn
        let id = with_context(|cx| cx.spawn(None));
        Self { id }
    }

    #[inline]
    #[must_use]
    pub fn id(&self) -> EntityId {
        self.id
    }

    /// 要素が現在保持している子要素のハンドルリストを安全に取得します。
    #[inline]
    #[must_use]
    pub fn get_children(self) -> Vec<Element> {
        with_context(|cx| cx.childrnd_list(self).unwrap_or_default())
    }

    /// 要素に現在設定されている最新の Inset（位置・オフセット）を安全に読み取ります。
    #[inline]
    #[must_use]
    pub fn get_inset(self) -> Rect<Val> {
        with_context(|cx| {
            cx.layouts
                .lay_basic
                .get(self.id)
                .map_or_else(|| BasicLayout::default().inset, |l| l.inset)
        })
    }

    /// 要素に現在設定されている最新の Size（幅・高さ）を安全に読み取ります。
    #[inline]
    #[must_use]
    pub fn get_size(self) -> Size<Val> {
        with_context(|cx| {
            cx.layouts
                .lay_basic
                .get(self.id)
                .map_or_else(|| BasicLayout::default().size, |l| l.size)
        })
    }

    /// `要素がドラッグ可能なスタイル設定（draggable_root` / `draggable_parent）を持っているか判定します`。
    #[inline]
    #[must_use]
    pub fn is_draggable(self) -> bool {
        with_context(|cx| {
            cx.topology
                .topo_active_masks
                .get(self.id)
                .is_some_and(|m| m.has(STYLE_DND_DRAGGABLE))
        })
    }

    /// この要素に対して、型 T のコンテキスト（シグナル）を提供（Provide）します。
    /// この要素、およびそのすべての子孫要素のエフェクトから `use_provided::<T>()` で取得可能になります。
    #[must_use]
    pub fn provide<T: Send + 'static>(self, read_signal: ReadSignal<T>) -> Self {
        with_context(|cx| {
            cx.provide_context::<T>(self.id, read_signal.id);
        });
        self
    }

    /// スタイルを適用します（静的な値、Signal、またはクロージャ）。
    #[must_use]
    pub fn style(self, style: impl Into<Prop<ThisStyle>>) -> Self {
        match style.into() {
            Prop::None => {}
            Prop::Static(s) => {
                let id = self.id;
                with_context(|cx| {
                    // 静的なスタイルプロパティを通常通りインラインマウント
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
                            // 状態が変化したため、最後に必ずスタイル解決を走り込ませる
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
                        // 動的評価された最新スタイルは、蓄積を避けるため置換（merge = false）
                        // 修正: 動的評価されたスタイルもマージ（true）としてマウントします。
                        // これにより、v_flex_c 等のColumn構造が破壊されるのを完全に防ぎます。
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

    /// プロバイダー `P` から動的に `ThisStyle` を解決してスタイルを適用します。
    #[must_use]
    #[inline]
    pub fn style_d<P, F>(self, f: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> ThisStyle + Send + Sync + 'static,
    {
        // 1引数のクロージャを、プロバイダー探索とシグナル購読（.get()）を内包した
        // 引数なしの Prop::Dynamic クロージャへラップして既存の style メソッドへ委譲します。
        let dynamic_prop = Prop::Dynamic(Box::new(move || {
            let signal = crate::use_provided::<P>();
            let val = signal.get();
            f(&val)
        }));

        self.style(dynamic_prop)
    }

    /// スタイルの適用（一括インライン展開）
    pub(crate) fn style_internal(cx: &mut Context, id: EntityId, style: &ThisStyle, merge: bool) {
        let inner = &style.inner;
        let mask = inner.mask;

        // topo_active_masks にスタイル側のマスクをマージするが、
        // 動的なインタラクション状態フラグ（STYLE_INTERACTION_PROPERTY）は
        // 実行時にのみ制御されるべきなので、ここでは除外（マスクアウト）する
        let property_only_mask = mask.0 & !STYLE_INTERACTION_PROPERTY;
        cx.topology.topo_active_masks[id].0 |= property_only_mask;

        // ベースの基本レイアウトをマージ
        if mask.has_basic_layout() {
            if merge && let Some(base) = cx.layouts.lay_base_basic.get_mut(id) {
                base.override_with(&inner.basic_layout, mask);
            } else {
                // 置換モード：前回の設定蓄積をクリアして完全置換
                cx.layouts.lay_base_basic.insert(id, inner.basic_layout);
            }
            cx.mark_layout_dirty(id);
        }

        // ベースのビジュアルプロパティをマージ
        let has_visual =
            mask.has_visual_property() || inner.visual_property.border_lengths.is_some();
        if has_visual {
            if merge && let Some(vis) = cx.renders.rnd_base_visual.get_mut(id) {
                vis.override_with(&inner.visual_property, mask);
            } else {
                cx.renders
                    .rnd_base_visual
                    .insert(id, inner.visual_property.clone());
            }
        }

        // 疑似クラス（インタラクションスタイル）をマージ
        if mask.has_interaction_property()
            || mask.has(STYLE_INTERACTION_WITHIN)
            || mask.has(STYLE_INTERACTION_PARENT)
        {
            if merge && let Some(interaction) = cx.renders.rnd_interaction.get_mut(id) {
                interaction.override_with(&inner.interaction_styles, mask);
            } else {
                cx.renders
                    .rnd_interaction
                    .insert(id, inner.interaction_styles.clone());
            }
        }

        // 4. Flexレイアウト
        if mask.has_flex_layout() {
            if merge && let Some(flex) = cx.layouts.lay_flex.get_mut(id) {
                flex.override_with(&inner.flex_layout, mask);
            } else {
                cx.layouts.lay_flex.insert(id, inner.flex_layout);
            }
            cx.mark_layout_dirty(id);
        }

        // 5. Gridレイアウト
        if mask.has_grid_layout()
            && let Some(ref grid) = inner.grid_layout
        {
            cx.layouts.lay_grid.insert(id, grid.clone());
            cx.mark_layout_dirty(id);
        }

        // 6. スクロールバー
        if mask.has(STYLE_SCROLLBAR)
            && let Some(ref sb) = inner.scrollbar_style
        {
            Element::ensure_scrollbar_elements(cx, id, sb, merge);
            cx.mark_layout_dirty(id);
        }

        // D&D のコールド SoA スロットへのマウント同期
        if mask.has(STYLE_DND_DRAGGABLE)
            && let Some(dp) = inner.drag_property
        {
            cx.events.evt_dnd_drag_properties.insert(id, dp);
        }
        if mask.has(STYLE_DND_DROPPABLE)
            && let Some(dp) = inner.drop_property
        {
            cx.events.evt_dnd_drop_properties.insert(id, dp);
        }

        cx.resolve_element_style_state(id, false);
    }

    /// 子要素を追加します（Element単体、Signal、またはクロージャ）。
    /// 動的な値が渡された場合、自動的にスロット要素が作成され、その中身がリアクティブに切り替わります。
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

    /// プロバイダー `P` から動的に単一の子要素（Element）を解決して追加します。
    #[must_use]
    #[inline]
    pub fn child_d<P, F>(self, f: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> Element + Send + Sync + 'static,
    {
        let dynamic_prop = Prop::Dynamic(Box::new(move || {
            let signal = crate::use_provided::<P>();
            let val = signal.get();
            f(&val)
        }));
        self.child(dynamic_prop)
    }

    /// 複数の子要素を一括して追加します。
    /// 静的な要素、シグナル、またはクロージャ（Prop<Element> に変換可能なオブジェクト）のコレクションを受け入れます。
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

    /// プロバイダー `P` から動的に複数の子要素（コレクション）を解決して、
    /// `中間コンテナ（div_n）を挟むことなく、親要素の直下へフラットに一括追加・置換します`。
    // childrnd_c を持つコンテナには他の静的子要素を混在させない
    #[must_use]
    #[inline]
    pub fn childrnd_d<P, F, I, E>(self, f: F) -> Self
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
            let current_childrnd_clone = current_children.clone();

            cx.create_element_effect(parent_id, EffectCategory::Contents, move |cx| {
                // プロバイダーの値を動的解決
                let signal = crate::use_provided::<P>();
                let val = signal.get();

                // 新しい子要素群の生成
                let new_elements: Vec<Element> = f(&val)
                    .into_iter()
                    .map(|e| match e.into() {
                        Prop::Static(el) => el,
                        _ => panic!("Dynamic nested elements inside topo_childrnd_c are not supported"),
                    })
                    .collect();

                // 前回マウントした古い子要素群を安全に一括破棄（Taffyツリーからのデタッチ含む）
                let mut old_children = current_childrnd_clone.borrow_mut();
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

    /// 子要素として、インタラクション（クリック等）を自動的に透過するテキストラベルを挿入します。
    /// 親子分離
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

    /// プロバイダー `P` から動的にスタイルを解決しつつ、ラベルテキストを設定して追加します。
    /// 第1引数のテキストには、静的な文字列や動的なプロパティ、シグナルを柔軟に渡すことができます。
    #[must_use]
    #[inline]
    pub fn label_d<P, FS>(self, content: impl Into<Prop<Cow<'static, str>>>, style: FS) -> Self
    where
        P: Clone + 'static,
        FS: Fn(&P) -> ThisStyle + Send + Sync + 'static,
    {
        // スタイル側のみ、1引数のクロージャをプロバイダー解決を伴う Prop::Dynamic へラップ
        let style_prop = Prop::Dynamic(Box::new(move || {
            let signal = crate::use_provided::<P>();
            let val = signal.get();
            style(&val)
        }));

        self.label(content, style_prop)
    }

    /// このコンテナの内容を差し替えます。以前の内容はすべて破棄されます。
    #[allow(clippy::return_self_not_must_use)]
    pub fn set_contents(self, contents: impl Into<Prop<Element>>) -> Self {
        match contents.into() {
            Prop::None => {}
            Prop::Static(new_child) => {
                let id = self.id;
                with_context(|cx| {
                    // 静的なコンテンツ上書き時のみ古い動的評価エフェクト（Contentsカテゴリ）を一括破棄
                    if let Some(react_effects) = cx.reactive.react_element_effects.get_mut(id)
                        && let Some(pos) = react_effects
                            .iter()
                            .position(|(cat, _)| *cat == EffectCategory::Contents)
                    {
                        let (_, old_effect_id) = react_effects.remove(pos);
                        cx.reactive.react_effects.remove(old_effect_id);
                        cx.reactive.react_effect_to_element.remove(old_effect_id);
                        cx.reactive
                            .react_pending_element_effects
                            .retain(|&x| x != old_effect_id);
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
                        // Dynamic 実行時は自身を自殺させないためそのままマウントを実行
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

        // 1. 親コンテナに紐づくスクロールバー専用要素のIDを安全に抽出
        let mut scrollbar_ids = std::collections::HashSet::new();
        if let Some(sb) = cx.layouts.lay_scrollbar_styles.get(id) {
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

        // 2. 現在の子要素のうち、スクロールバー関係の要素以外のコンテンツのみを再帰破棄
        if let Some(childrnd_list) = cx.topology.topo_children.get(id) {
            let old_children: Vec<EntityId> = childrnd_list.iter().copied().collect();
            for child_id in old_children {
                if !scrollbar_ids.contains(&child_id) {
                    cx.despawn_internal(child_id);
                }
            }
        }

        // 新しい子要素を追加（親子トポロジーおよび Taffy ツリーの同期）
        cx.add_child(id, new_child.id);

        // レイアウトと描画の再計算を要求
        cx.mark_dirty(id);
    }

    /// テキストを設定します。
    /// 引数には &str, String, `ReadSignal`<T>, またはクロージャを渡せます。
    /// 1ノードパターン
    #[must_use]
    #[inline]
    pub fn text(self, content: impl Into<Prop<Cow<'static, str>>>) -> Self {
        match content.into() {
            Prop::None => {}
            Prop::Static(val) => {
                let id = self.id;
                with_context(|cx| {
                    cx.contents.cont_text_contents.insert(id, val);
                    cx.topology.topo_active_masks[id].set(COMP_TEXT_CONTENT);
                    cx.clear_layout_cache(id);
                    cx.mark_dirty(id);
                });
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                with_context(|cx| {
                    cx.create_element_effect(id, EffectCategory::Text, move |cx| {
                        let new_text = f();
                        cx.contents.cont_text_contents.insert(id, new_text);
                        cx.topology.topo_active_masks[id].set(COMP_TEXT_CONTENT);
                        cx.clear_layout_cache(id);
                        cx.mark_dirty(id);
                    });
                });
            }
        }
        self
    }

    /// プロバイダー `P` から動的にテキストを設定します。
    #[must_use]
    #[inline]
    pub fn text_d<P, F, S>(self, f: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> S + Send + Sync + 'static,
        S: Into<Cow<'static, str>>,
    {
        let dynamic_prop = Prop::Dynamic(Box::new(move || {
            let signal = crate::use_provided::<P>();
            let val = signal.get();
            f(&val).into()
        }));
        self.text(dynamic_prop)
    }

    /// 画像を設定します。
    #[must_use]
    pub fn image(self, content: impl Into<Prop<ImageSource>>) -> Self {
        match content.into() {
            Prop::None => {}
            Prop::Static(src) => {
                with_context(|cx| {
                    cx.contents.cont_image_sources.insert(self.id, src);
                    cx.topology.topo_active_masks[self.id].set(COMP_IMAGE_CONTENT);
                    cx.mark_layout_dirty(self.id);
                    cx.mark_render_dirty(self.id);
                });
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                with_context(|cx| {
                    cx.create_element_effect(id, EffectCategory::Image, move |cx| {
                        let src = f();
                        cx.contents.cont_image_sources.insert(id, src);
                        cx.topology.topo_active_masks[id].set(COMP_IMAGE_CONTENT);
                        cx.mark_dirty(id);
                    });
                });
            }
        }
        self
    }

    /// プロバイダー `P` から動的に画像ソースを解決して設定します。
    #[must_use]
    #[inline]
    pub fn image_d<P, F>(self, f: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> ImageSource + Send + Sync + 'static,
    {
        let dynamic_prop = Prop::Dynamic(Box::new(move || {
            let signal = crate::use_provided::<P>();
            let val = signal.get();
            f(&val)
        }));
        self.image(dynamic_prop)
    }

    /// 動画を設定します。
    #[must_use]
    pub fn movie(self, content: impl Into<Prop<MovieProperty>>) -> Self {
        match content.into() {
            Prop::None => {}
            Prop::Static(p) => {
                with_context(|cx| {
                    cx.contents.cont_movie_properties.insert(self.id, p);
                    cx.topology.topo_active_masks[self.id].set(COMP_MOVIE_CONTENT);
                    cx.mark_dirty(self.id);
                });
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                with_context(|cx| {
                    cx.create_element_effect(id, EffectCategory::Movie, move |cx| {
                        let p = f();
                        cx.contents.cont_movie_properties.insert(id, p);
                        cx.topology.topo_active_masks[id].set(COMP_MOVIE_CONTENT);
                        cx.mark_dirty(id);
                    });
                });
            }
        }
        self
    }

    /// プロバイダー `P` から動的に動画ソースを解決して設定します。
    #[must_use]
    #[inline]
    pub fn movie_d<P, F>(self, f: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> MovieProperty + Send + Sync + 'static,
    {
        let dynamic_prop = Prop::Dynamic(Box::new(move || {
            let signal = crate::use_provided::<P>();
            let val = signal.get();
            f(&val)
        }));
        self.movie(dynamic_prop)
    }

    /// `WebView2` コンポーネントを配置します（静的設定、またはSignal / クロージャに対応）。
    #[must_use]
    pub fn webview2(self, contents: impl Into<Prop<WebView2Contents>>) -> Self {
        match contents.into() {
            Prop::None => {}
            Prop::Static(contents) => {
                with_context(|cx| {
                    cx.contents.cont_webview_contents.insert(self.id, contents);
                    cx.topology.topo_active_masks[self.id].set(COMP_WEBVIEW_CONTENT);
                    cx.mark_dirty(self.id);
                });
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                with_context(|cx| {
                    cx.create_element_effect(id, EffectCategory::WebView2, move |cx| {
                        let contents = f();
                        cx.contents.cont_webview_contents.insert(id, contents);
                        cx.topology.topo_active_masks[id].set(COMP_WEBVIEW_CONTENT);
                        cx.mark_dirty(id);
                    });
                });
            }
        }
        self
    }

    /// プロバイダー `P` `から動的にWebView2設定を解決してアタッチします`。
    #[must_use]
    #[inline]
    pub fn webview2_d<P, F>(self, f: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> WebView2Contents + Send + Sync + 'static,
    {
        let dynamic_prop = Prop::Dynamic(Box::new(move || {
            let signal = crate::use_provided::<P>();
            let val = signal.get();
            f(&val)
        }));
        self.webview2(dynamic_prop)
    }

    /// このコンテナを入力フィールド（テキストボックス）化し、IME制御や入力ロジックをバインドします。
    #[must_use]
    pub fn input(self, contents: impl Into<Prop<InputContents>>) -> Self {
        match contents.into() {
            Prop::None => {}
            Prop::Static(c) => {
                with_context(|cx| self.input_internal(cx, c));
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                with_context(|cx| {
                    cx.create_element_effect(id, EffectCategory::Input, move |cx| {
                        let c = f();
                        let el = Element { id };
                        el.input_internal(cx, c);
                    });
                });
            }
        }
        self
    }

    /// プロバイダー `P` から動的に設定を読み込んで入力フィールド化します。
    #[must_use]
    #[inline]
    pub fn input_d<P, F>(self, f: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> InputContents + Send + Sync + 'static,
    {
        let dynamic_prop = Prop::Dynamic(Box::new(move || {
            let signal = crate::use_provided::<P>();
            let val = signal.get();
            f(&val)
        }));
        self.input(dynamic_prop)
    }

    /// 複数行入力（テキストエリア）をバインドします。
    #[must_use]
    pub fn input_area(self, contents: impl Into<Prop<InputContents>>) -> Self {
        match contents.into() {
            Prop::None => {}
            Prop::Static(mut c) => {
                c.is_multiline = true; // マルチライン化を強制
                with_context(|cx| self.input_internal(cx, c));
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                with_context(|cx| {
                    cx.create_element_effect(id, EffectCategory::Input, move |cx| {
                        let mut c = f();
                        c.is_multiline = true;
                        let el = Element { id };
                        el.input_internal(cx, c);
                    });
                });
            }
        }
        self
    }

    /// プロバイダー `P` から動的に設定を読み込んで複数行入力フィールド化します。
    #[must_use]
    #[inline]
    pub fn input_area_d<P, F>(self, f: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> InputContents + Send + Sync + 'static,
    {
        let dynamic_prop = Prop::Dynamic(Box::new(move || {
            let signal = crate::use_provided::<P>();
            let val = signal.get();
            f(&val)
        }));
        self.input_area(dynamic_prop)
    }

    /// 入力イベント（キー、IME、文字入力、フォーカス）を自動的にマッピングして代行するロジック
    #[allow(clippy::too_many_lines)]
    fn input_internal(self, cx: &mut Context, mut c: InputContents) {
        let id = self.id;

        // シグナル更新やテーマ変更、親コンポーネントの再レンダリングによる
        // キャレット位置（selected_range）や Undo/Redo 履歴の末尾への強制初期化を防止
        // 既存の状態を検知した場合はデザイン設定のみを上書き
        if let Some(existing) = cx.contents.cont_input_contents.get_mut(id) {
            existing.placeholder = c.placeholder;
            existing.placeholder_color = c.placeholder_color;
            existing.caret_color = c.caret_color;
            existing.caret_width = c.caret_width;
            existing.caret_height = c.caret_height;
            existing.caret_offset = c.caret_offset;
            existing.is_blink = c.is_blink;
            existing.blink_frequency = c.blink_frequency;
            existing.has_caret = c.has_caret;
            existing.placeholder_select = c.placeholder_select;
            existing.is_multiline = c.is_multiline;
            existing.is_password = c.is_password;
            existing.mask_text = c.mask_text;

            // 動的なテキスト長の変更に伴い、既存の選択範囲が枠外へ飛び出さないようクランプ
            let current_text = existing.text.0.get();
            let u16_len = current_text.encode_utf16().count();
            existing.selected_range.start = existing.selected_range.start.min(u16_len);
            existing.selected_range.end = existing.selected_range.end.min(u16_len);

            // 早期リターンを抜ける前に、最新の文字列状態を SoA / DWrite 側へ即座に同期・反映
            cx.update_input_caret_position(id);
            cx.mark_dirty(id);

            // これ以降の初期化を完全にスキップして早期リターン
            return;
        }

        // 最初のロード時、シグナルから現在値を取得して内部カーソルを末尾に合わせる
        let current_text = c.text.0.get();
        let current_len = current_text.encode_utf16().count();
        c.selected_range = current_len..current_len;

        cx.contents.cont_input_contents.insert(id, c);
        cx.topology.topo_active_masks[id].set(COMP_INPUT_CONTENT);
        cx.topology.topo_active_masks[id].set(COMP_TEXT_CONTENT);

        self.get_or_create_listeners(|l| {
            let mut existing_mouse = l.on_mouse_input.take();
            l.on_mouse_input = Some(Box::new(move |cx, button, modifiers, state| {
                if button == MouseButton::Left
                    && state == ElementState::Pressed
                    && let Some(pointer_pos) = cx.events.evt_current_pointer_position
                {
                    let rect = cx.rect(id).unwrap_or_default();
                    // 要素の境界枠（border + padding）を取得してローカル座標を算出
                    let (basic, flex, _) = cx.resolve_active_layouts(id);
                    let (border, padding) =
                        LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

                    let scroll = cx
                        .outputs
                        .out_scroll_offsets
                        .get(id)
                        .copied()
                        .unwrap_or(LayoutPoint::ZERO);

                    let text_size = if let Some(contents) = cx.contents.cont_input_contents.get(id)
                        && let Some(layout_rect) = contents.last_layout
                    {
                        LayoutSize::new(layout_rect.width, layout_rect.height)
                    } else {
                        LayoutSize::ZERO
                    };

                    let content_w =
                        (rect.width - border.left - border.right - padding.left - padding.right)
                            .max(0.0);
                    let align_offset_x = match flex.text_align {
                        TextAlign::Center => ((content_w - text_size.width) * 0.5).max(0.0),
                        TextAlign::Right => (content_w - text_size.width).max(0.0),
                        _ => 0.0,
                    };

                    let content_h =
                        (rect.height - border.top - border.bottom - padding.top - padding.bottom)
                            .max(0.0);
                    let align_offset_y = ((content_h - text_size.height) * 0.5).max(0.0);

                    // テキスト本来の描画領域に対する相対マウス座標
                    let local_x = pointer_pos.x
                        - (rect.x + border.left + padding.left + align_offset_x)
                        + scroll.x;
                    let local_y = pointer_pos.y
                        - (rect.y + border.top + padding.top + align_offset_y)
                        + scroll.y;

                    let mut update_rects_needed = false;

                    if let Some(contents) = cx.contents.cont_input_contents.get_mut(id) {
                        let text_val = contents.text.0.get();

                        // 逆引きレイアウト時もプレースホルダーは含まない
                        let editable_text_for_caret = if text_val.is_empty() {
                            if let Some(ref ime) = contents.ime_state
                                && !ime.composition_text.is_empty()
                            {
                                ime.composition_text.clone()
                            } else {
                                String::new()
                            }
                        } else if contents.is_password {
                            let mask = contents.mask_text.as_deref().unwrap_or("●");
                            mask.repeat(text_val.chars().count())
                        } else if let Some(ref ime) = contents.ime_state
                            && !ime.composition_text.is_empty()
                        {
                            crate::input_get_display_text(
                                &text_val,
                                contents.selected_range.start,
                                &ime.composition_text,
                            )
                        } else {
                            text_val.clone()
                        };

                        let default_visual = VisualProperty::default();
                        let visual = cx
                            .renders
                            .rnd_visual
                            .get(id)
                            .unwrap_or(&default_visual);
                        let font_size = visual.font_size.unwrap_or(16.0);
                        let font_family = visual.font_family.as_deref();
                        let font_weight = visual.font_weight;
                        let font_style = visual.font_style;

                        let spans = cx
                            .contents
                            .cont_text_spans
                            .get(id)
                            .map_or(&[][..], Vec::as_slice);

                        let layout = cx.system.sys_text_engine.create_layout(
                            &editable_text_for_caret,
                            font_size,
                            font_family,
                            font_weight,
                            font_style,
                            None,
                            spans,
                        );

                        // 物理クリック座標から文字インデックスを逆引き
                        let (new_caret, is_trailing) = cx
                            .system
                            .sys_text_engine
                            .hit_test_point(&layout, local_x, local_y);

                        let final_caret = if is_trailing {
                            new_caret + 1
                        } else {
                            new_caret
                        };

                        let editable_len = editable_text_for_caret.encode_utf16().count();
                        let final_caret_clamped = final_caret.min(editable_len);

                        // プレースホルダーが表示状態にあるか
                        let is_placeholder = text_val.is_empty()
                            && contents
                                .ime_state
                                .as_ref()
                                .is_none_or(|s| s.composition_text.is_empty());

                        // プレースホルダーではない、またはプレースホルダー選択が明示許可されていること
                        let allow_selection = !is_placeholder || contents.placeholder_select;

                        if modifiers.shift && allow_selection {
                            let anchor = cx
                                .outputs
                                .out_selection_start_index
                                .get(id)
                                .copied()
                                .unwrap_or(contents.selected_range.start);

                            if !cx.outputs.out_selection_start_index.contains_key(id) {
                                cx.outputs
                                    .out_selection_start_index
                                    .insert(id, contents.selected_range.start);
                            }

                            let range = if anchor <= final_caret_clamped {
                                contents.selection_reversed = false;
                                anchor..final_caret_clamped
                            } else {
                                contents.selection_reversed = true;
                                final_caret_clamped..anchor
                            };

                            contents.selected_range = range.clone();
                            cx.outputs.out_text_selections.insert(id, range);
                            update_rects_needed = true;
                        } else {
                            contents.selected_range = final_caret_clamped..final_caret_clamped;
                            cx.outputs
                                .out_text_selections
                                .insert(id, final_caret_clamped..final_caret_clamped);
                            cx.outputs
                                .out_selection_start_index
                                .insert(id, final_caret_clamped);
                            cx.outputs.out_selected_rects.remove(id);
                            contents.selection_reversed = false;
                        }

                        contents.last_interacted_time = Some(std::time::Instant::now());

                        // キャレットの絶対座標と表示情報を一括更新
                        cx.update_input_caret_position(id);
                    }

                    if update_rects_needed {
                        if let Some(layout) = cx.get_or_create_layout(id) {
                            cx.update_selection_rects(id, &layout);
                        }
                    }

                    cx.mark_render_dirty(id);
                }
                if let Some(ref mut ext) = existing_mouse {
                    ext(cx, button, modifiers, state);
                }
            }));

            let mut existing_focus = l.on_focus.take();
            // フォーカス取得（点滅カーソルの有効化等）
            l.on_focus = Some(Box::new(move |cx| {
                if let Some(contents) = cx.contents.cont_input_contents.get_mut(id) {
                    contents.is_selecting = false;
                    // フォーカス獲得時も操作時刻を記録して即座にキャレットを表示
                    contents.last_interacted_time = Some(std::time::Instant::now());
                }
                cx.mark_render_dirty(id);

                if let Some(ref mut ext) = existing_focus {
                    ext(cx);
                }
            }));

            let mut existing_char = l.on_char_input.take();
            // 確定した1文字の文字入力 (WM_CHAR)
            l.on_char_input = Some(Box::new(move |cx, mut ch| {
                // IME未変換の入力中 (composition_textがある間) は文字入力を無視
                let is_ime_active = cx
                    .contents
                    .cont_input_contents
                    .get(id)
                    .and_then(|c| c.ime_state.as_ref())
                    .is_some_and(|s| !s.composition_text.is_empty());

                if !is_ime_active {
                    let mut is_allowed = !ch.is_control();
                    let is_multiline = cx
                        .contents
                        .cont_input_contents
                        .get(id)
                        .is_some_and(|c| c.is_multiline);

                    // 複数行入力時に、Enterキー（'\r' / '\n'）が押された場合は改行コードとして許可
                    if is_multiline && (ch == '\r' || ch == '\n') {
                        ch = '\n';
                        is_allowed = true;
                    }

                    if is_allowed && let Some(contents) = cx.contents.cont_input_contents.get_mut(id) {
                        contents.last_interacted_time = Some(std::time::Instant::now());

                        let text_val = contents.text.0.get();

                        // 数値制限フィルター
                        if contents.numeric_only && !ch.is_numeric() && ch != '.' && ch != '-' {
                            return;
                        }

                        let range = contents.selected_range.clone();

                        // 変更発生前に現在の状態をセーブ
                        contents.record_undo(text_val.clone(), range.clone());

                        let u16_text: Vec<u16> = text_val.encode_utf16().collect();

                        // 選択範囲が削除された後の長さ
                        let u16_len_after_delete = u16_text.len()
                            - (range.end.min(u16_text.len()) - range.start.min(u16_text.len()));
                        let mut buf = [0u16; 2];
                        let ch_u16_slice = ch.encode_utf16(&mut buf);

                        // 文字数制限
                        if let Some(max) = contents.max_length
                            && u16_len_after_delete + ch_u16_slice.len() > max
                        {
                            return; // 制限を超えるため入力を中断
                        }

                        contents.last_interacted_time = Some(std::time::Instant::now());
                        contents.record_undo(text_val.clone(), range.clone());

                        let mut left = u16_text[..range.start.min(u16_text.len())].to_vec();
                        let right = u16_text[range.end.min(u16_text.len())..].to_vec();

                        left.extend_from_slice(ch_u16_slice);
                        left.extend_from_slice(&right);

                        let new_text = String::from_utf16_lossy(&left);
                        let new_caret = range.start + ch_u16_slice.len();

                        contents.selected_range = new_caret..new_caret;
                        cx.outputs.out_text_selections.insert(id, new_caret..new_caret); // 選択表示をリセット
                        cx.outputs.out_selected_rects.remove(id);
                        // タイピング編集が発生したため古い開始選択アンカーを消去
                        cx.outputs.out_selection_start_index.remove(id);
                        contents.text.1.set(new_text);
                        cx.mark_render_dirty(id);
                    }
                }
                if let Some(ref mut ext) = existing_char {
                    ext(cx, ch);
                }
            }));

            let mut existing_keyboard = l.on_keyboard_input.take();
            // 物理キーボード操作 (Backspace, Delete, 矢印キー)
            l.on_keyboard_input = Some(Box::new(move |cx, key, modifiers, state| {
                if state == ElementState::Pressed
                    && let Some(contents) = cx.contents.cont_input_contents.get_mut(id)
                {
                    let default_visual = VisualProperty::default();
                    let text_val = contents.text.0.get();
                    let u16_len = text_val.encode_utf16().count();

                    contents.selected_range = (contents.selected_range.start.min(u16_len))
                        ..(contents.selected_range.end.min(u16_len));

                    let raw_caret = if contents.selection_reversed {
                        contents.selected_range.start
                    } else {
                        contents.selected_range.end
                    };
                    let mut caret = raw_caret.min(u16_len);
                    let mut changed = false; // 状態変更フラグ

                    match key {
                        VirtualKey::BACK => {
                            let range = contents.selected_range.clone();
                            contents.record_undo(text_val.clone(), range.clone());

                            if range.start < range.end {
                                // 選択範囲を一撃で消去
                                let u16_text: Vec<u16> = text_val.encode_utf16().collect();
                                let mut left = u16_text[..range.start.min(u16_text.len())].to_vec();
                                let right = u16_text[range.end.min(u16_text.len())..].to_vec();
                                left.extend_from_slice(&right);

                                let new_text = String::from_utf16_lossy(&left);
                                contents.selected_range = range.start..range.start;
                                cx.outputs
                                    .out_text_selections
                                    .insert(id, range.start..range.start);
                                cx.outputs.out_selection_start_index.remove(id);
                                contents.text.1.set(new_text);
                            } else {
                                // 通常の1文字バックスペース
                                let new_text = crate::input_backspace(&text_val, &mut caret);
                                contents.selected_range = caret..caret;
                                // Context側の描画SoAにも最新のキャレット位置を強制同期
                                cx.outputs.out_text_selections.insert(id, caret..caret);
                                cx.outputs.out_selection_start_index.remove(id);
                                contents.text.1.set(new_text);
                            }
                            contents.last_interacted_time = Some(std::time::Instant::now());
                            changed = true;
                        }
                        VirtualKey::DELETE => {
                            let range = contents.selected_range.clone();
                            contents.record_undo(text_val.clone(), range.clone());
                            if range.start < range.end {
                                let u16_text: Vec<u16> = text_val.encode_utf16().collect();
                                let mut left = u16_text[..range.start.min(u16_text.len())].to_vec();
                                let right = u16_text[range.end.min(u16_text.len())..].to_vec();
                                left.extend_from_slice(&right);

                                let new_text = String::from_utf16_lossy(&left);
                                contents.selected_range = range.start..range.start;
                                cx.outputs
                                    .out_text_selections
                                    .insert(id, range.start..range.start);
                                cx.outputs.out_selection_start_index.remove(id);
                                contents.text.1.set(new_text);
                            } else {
                                // 通常の1文字デリート
                                let new_text = crate::input_delete(&text_val, caret);
                                contents.selected_range = caret..caret;
                                cx.outputs.out_text_selections.insert(id, caret..caret);
                                cx.outputs.out_selection_start_index.remove(id);
                                contents.text.1.set(new_text);
                            }
                            contents.last_interacted_time = Some(std::time::Instant::now());
                            changed = true;
                        }
                        VirtualKey::LEFT => {
                            let range = contents.selected_range.clone();
                            // 選択範囲が存在し、かつ Shiftキーが押されていない通常移動時
                            if range.start < range.end && !modifiers.shift {
                                // 選択範囲をすべて解除し、キャレットを左端（start）に収束
                                let new_caret = range.start;
                                contents.selected_range = new_caret..new_caret;
                                cx.outputs.out_text_selections.insert(id, new_caret..new_caret);
                                cx.outputs.out_selected_rects.remove(id);
                                cx.outputs.out_selection_start_index.remove(id);
                                contents.selection_reversed = false;
                                contents.last_interacted_time = Some(std::time::Instant::now());
                                changed = true;
                            } else if caret > 0 {
                                let new_caret = caret - 1;

                                if modifiers.shift {
                                    // Shiftキー押下中：選択の拡張
                                    let anchor = cx
                                        .outputs
                                        .out_selection_start_index
                                        .get(id)
                                        .copied()
                                        .unwrap_or(caret);
                                    if !cx.outputs.out_selection_start_index.contains_key(id) {
                                        cx.outputs.out_selection_start_index.insert(id, caret);
                                    }
                                    let range = if anchor <= new_caret {
                                        contents.selection_reversed = false;
                                        anchor..new_caret
                                    } else {
                                        contents.selection_reversed = true;
                                        new_caret..anchor
                                    };
                                    contents.selected_range = range.clone();
                                    cx.outputs.out_text_selections.insert(id, range);
                                } else {
                                    // Shiftキー非押下：選択解除して単なる移動
                                    contents.selected_range = new_caret..new_caret;
                                    cx.outputs.out_text_selections.insert(id, new_caret..new_caret);
                                    cx.outputs.out_selected_rects.remove(id);
                                    cx.outputs.out_selection_start_index.remove(id);
                                }
                                contents.last_interacted_time = Some(std::time::Instant::now());
                                changed = true;
                            }
                        }
                        VirtualKey::RIGHT => {
                            let range = contents.selected_range.clone();
                            // 選択範囲が存在し、かつ Shiftキーが押されていない通常移動時（全選択中での右移動に完全対応）
                            if range.start < range.end && !modifiers.shift {
                                let new_caret = range.end;
                                contents.selected_range = new_caret..new_caret;
                                cx.outputs.out_text_selections.insert(id, new_caret..new_caret);
                                cx.outputs.out_selected_rects.remove(id);
                                cx.outputs.out_selection_start_index.remove(id);
                                contents.selection_reversed = false;
                                contents.last_interacted_time = Some(std::time::Instant::now());
                                changed = true;
                            } else if caret < u16_len {
                                let new_caret = caret + 1;

                                if modifiers.shift {
                                    let anchor = cx
                                        .outputs
                                        .out_selection_start_index
                                        .get(id)
                                        .copied()
                                        .unwrap_or(caret);
                                    if !cx.outputs.out_selection_start_index.contains_key(id) {
                                        cx.outputs.out_selection_start_index.insert(id, caret);
                                    }
                                    let range = if anchor <= new_caret {
                                        contents.selection_reversed = false;
                                        anchor..new_caret
                                    } else {
                                        contents.selection_reversed = true;
                                        new_caret..anchor
                                    };
                                    contents.selected_range = range.clone();
                                    cx.outputs.out_text_selections.insert(id, range);
                                } else {
                                    contents.selected_range = new_caret..new_caret;
                                    cx.outputs.out_text_selections.insert(id, new_caret..new_caret);
                                    cx.outputs.out_selected_rects.remove(id);
                                    cx.outputs.out_selection_start_index.remove(id);
                                }
                                contents.last_interacted_time = Some(std::time::Instant::now());
                                changed = true;
                            }
                        }
                        VirtualKey::UP => {
                            if contents.is_multiline {
                                let visual = cx
                                    .renders
                                    .rnd_visual
                                    .get(id)
                                    .unwrap_or(&default_visual);
                                let font_size = visual.font_size.unwrap_or(16.0);
                                let font_family = visual.font_family.as_deref();
                                let font_weight = visual.font_weight;
                                let font_style = visual.font_style;

                                let spans = cx
                                    .contents
                                    .cont_text_spans
                                    .get(id)
                                    .map_or(&[][..], Vec::as_slice);

                                let layout = cx.system.sys_text_engine.create_layout(
                                    &text_val,
                                    font_size,
                                    font_family,
                                    font_weight,
                                    font_style,
                                    None,
                                    spans,
                                );

                                let (cx_offset, cy_offset, _) = cx
                                    .system
                                    .sys_text_engine
                                    .get_caret_position(&layout, caret, u16_len);

                                let line_height = font_size * 1.3;
                                let target_y = (cy_offset - line_height * 1.1).max(0.0); // 1行分＋マージン

                                let (new_caret, is_trailing) = cx
                                    .system
                                    .sys_text_engine
                                    .hit_test_point(&layout, cx_offset, target_y);
                                let final_caret = if is_trailing {
                                    new_caret + 1
                                } else {
                                    new_caret
                                };

                                if modifiers.shift {
                                    let anchor = cx
                                        .outputs
                                        .out_selection_start_index
                                        .get(id)
                                        .copied()
                                        .unwrap_or(caret);
                                    if !cx.outputs.out_selection_start_index.contains_key(id) {
                                        cx.outputs.out_selection_start_index.insert(id, caret);
                                    }
                                    let range = if anchor <= final_caret {
                                        contents.selection_reversed = false;
                                        anchor..final_caret
                                    } else {
                                        contents.selection_reversed = true;
                                        final_caret..anchor
                                    };
                                    contents.selected_range = range.clone();
                                    cx.outputs.out_text_selections.insert(id, range);
                                } else {
                                    contents.selected_range = final_caret..final_caret;
                                    cx.outputs
                                        .out_text_selections
                                        .insert(id, final_caret..final_caret);
                                    cx.outputs.out_selection_start_index.remove(id);
                                    contents.selection_reversed = false;
                                }

                                contents.last_interacted_time = Some(std::time::Instant::now());
                                changed = true;
                            }
                        }
                        VirtualKey::DOWN if contents.is_multiline => {
                            let visual = cx
                                .renders
                                .rnd_visual
                                .get(id)
                                .unwrap_or(&default_visual);
                            let font_size = visual.font_size.unwrap_or(16.0);
                            let font_family = visual.font_family.as_deref();
                            let font_weight = visual.font_weight;
                            let font_style = visual.font_style;

                            let spans = cx
                                .contents
                                .cont_text_spans
                                .get(id)
                                .map_or(&[][..], Vec::as_slice);

                            let layout = cx.system.sys_text_engine.create_layout(
                                &text_val,
                                font_size,
                                font_family,
                                font_weight,
                                font_style,
                                None,
                                spans,
                            );

                            let (cx_offset, cy_offset, _) = cx
                                .system
                                .sys_text_engine
                                .get_caret_position(&layout, caret, u16_len);

                            let line_height = font_size * 1.3;
                            let target_y = cy_offset + line_height * 1.5;
                            let (new_caret, is_trailing) = cx
                                .system
                                .sys_text_engine
                                .hit_test_point(&layout, cx_offset, target_y);
                            let final_caret = if is_trailing {
                                new_caret + 1
                            } else {
                                new_caret
                            };

                            if modifiers.shift {
                                let anchor = cx
                                    .outputs
                                    .out_selection_start_index
                                    .get(id)
                                    .copied()
                                    .unwrap_or(caret);
                                if !cx.outputs.out_selection_start_index.contains_key(id) {
                                    cx.outputs.out_selection_start_index.insert(id, caret);
                                }
                                let range = if anchor <= final_caret {
                                    contents.selection_reversed = false;
                                    anchor..final_caret
                                } else {
                                    contents.selection_reversed = true;
                                    final_caret..anchor
                                };
                                contents.selected_range = range.clone();
                                cx.outputs.out_text_selections.insert(id, range);
                            } else {
                                contents.selected_range = final_caret..final_caret;
                                cx.outputs
                                    .out_text_selections
                                    .insert(id, final_caret..final_caret);
                                cx.outputs.out_selection_start_index.remove(id);
                                contents.selection_reversed = false;
                            }

                            contents.last_interacted_time = Some(std::time::Instant::now());
                            changed = true;
                        }
                        _ => {}
                    }

                    if changed {
                        if let Some(layout) = cx.get_or_create_layout(id) {
                            cx.update_selection_rects(id, &layout);
                        }
                        cx.update_input_caret_position(id);
                        cx.mark_render_dirty(id);
                    }
                }
                if let Some(ref mut ext) = existing_keyboard {
                    ext(cx, key, modifiers, state);
                }
            }));

            let mut existing_ime = l.on_ime.take();
            // IME連動
            l.on_ime = Some(Box::new(move |cx, ime| {
                if let Some(contents) = cx.contents.cont_input_contents.get_mut(id) {
                    contents.last_interacted_time = Some(std::time::Instant::now());
                    contents.ime_state = Some(ime.clone());

                    // IME 確定文字の書き込み
                    if !ime.result_text.is_empty() {
                        let text_val = contents.text.0.get();
                        let mut caret = contents.selected_range.start;

                        // 確定した文字列を1文字ずつ安全に挿入
                        let mut temp_text = text_val;
                        for ch in ime.result_text.chars() {
                            temp_text = crate::input_insert_char(
                                &temp_text,
                                &mut caret,
                                ch,
                                contents.max_length,
                                contents.numeric_only,
                            );
                        }

                        contents.selected_range = caret..caret;
                        contents.text.1.set(temp_text);
                        contents.marked_range = None;
                    } else if !ime.composition_text.is_empty() {
                        // IME 未変換中
                        let caret = contents.selected_range.start;
                        let comp_len = ime.composition_text.encode_utf16().count();
                        contents.marked_range = Some(caret..(caret + comp_len));
                    } else {
                        contents.marked_range = None;
                    }

                    // IME の未確定状態（未確定波線、変換フォーカス太線/細線）を TextSpan に自動マッピング
                    if ime.composition_text.is_empty() {
                        cx.contents.cont_text_spans.remove(id);
                        cx.topology.topo_active_masks[id].unset(STYLE_TEXT_SPANS);
                    } else {
                        let mut spans = Vec::new();
                        let caret = contents.selected_range.start;

                        if ime.composition_attrs.is_empty() {
                            // 属性が取得できない場合のフォールバック（全体を未確定波線に設定）
                            let comp_len = ime.composition_text.encode_utf16().count();
                            spans.push(TextSpan {
                                range: caret..(caret + comp_len),
                                underline: Some(UnderlineStyle::Wave),
                                ..Default::default()
                            });
                        } else {
                            let attrs = &ime.composition_attrs;
                            let mut start_idx = 0;

                            // 同一のIME属性が連続する境界ごとに TextSpan を分割
                            while start_idx < attrs.len() {
                                let attr = attrs[start_idx];
                                let mut end_idx = start_idx + 1;
                                while end_idx < attrs.len() && attrs[end_idx] == attr {
                                    end_idx += 1;
                                }

                                // Windows IME 属性定数:
                                // ATTR_INPUT (0): 未変換入力 ➔ 波線 (Wave)
                                // ATTR_TARGET_CONVERTED (1): フォーカス（ターゲット）文節 ➔ 太実線 (Thick)
                                // ATTR_CONVERTED (2): 変換済み非フォーカス文節 ➔ 細実線 (Solid)
                                let underline_style = match attr {
                                    1 => Some(UnderlineStyle::Thick),
                                    2 => Some(UnderlineStyle::Solid),
                                    _ => Some(UnderlineStyle::Wave),
                                };

                                spans.push(TextSpan {
                                    range: (caret + start_idx)..(caret + end_idx),
                                    color: None,
                                    bg_color: None,
                                    font_size: None,
                                    font_family: None,
                                    font_weight: None,
                                    font_style: None,
                                    underline: underline_style,
                                    underline_color: None,
                                    strikethrough: None,
                                    strikethrough_color: None,
                                    link_id: None,
                                });

                                start_idx = end_idx;
                            }
                        }

                        cx.contents.cont_text_spans.insert(id, spans);
                        cx.topology.topo_active_masks[id].set(STYLE_TEXT_SPANS);
                    }

                    // IMEイベント終了（または変換中）に表示テキストとキャレット位置を再計算・同期させる
                    cx.update_input_caret_position(id);

                    cx.mark_render_dirty(id);
                }
                if let Some(ref mut ext) = existing_ime {
                    ext(cx, ime);
                }
            }));
        });

        cx.create_element_effect(id, EffectCategory::Text, move |cx| {
            if let Some(contents) = cx.contents.cont_input_contents.get(id) {
                let _base_text_val = contents.text.0.get();
            }
            cx.update_input_caret_position(id);
            cx.mark_dirty(id);
        });
    }

    /// 内部ヘルパー：この要素に対応する `EventListeners` が `SoA` 上に存在しない場合は新規に作成し、
    /// 可変参照を取得して渡されたクロージャを実行します。
    #[inline]
    fn get_or_create_listeners<R>(&self, f: impl FnOnce(&mut EventListeners) -> R) -> R {
        with_context(|cx| {
            // SparseSecondaryMap にキーが存在しない場合は Default (すべて None) で差し込む
            if !cx.events.evt_listeners.contains_key(self.id) {
                cx.events
                    .evt_listeners
                    .insert(self.id, EventListeners::default());
            }
            let listeners = cx.events.evt_listeners.get_mut(self.id).unwrap();
            f(listeners)
        })
    }

    /// 左クリックのリリース（押し下げ ➔ 同一要素上での離し）が成立した際に発火するイベントを登録します。
    /// 複数回呼ぶとイベントは追加され登録順に実行されます。
    #[must_use]
    #[inline]
    pub fn on_click<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_click_with(move |_cx| f())
    }

    #[must_use]
    #[inline]
    pub fn on_click_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_click.take() {
                let mut f = f;
                l.on_click = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_click = Some(Box::new(f));
            }
        });
        self
    }

    /// 右クリックのリリース（押し下げ ➔ 同一要素上での離し）が成立した際に発火するイベントを登録します。
    #[must_use]
    #[inline]
    pub fn on_right_click<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_right_click_with(move |_cx| f())
    }

    #[must_use]
    #[inline]
    pub fn on_right_click_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_right_click.take() {
                let mut f = f;
                l.on_right_click = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_right_click = Some(Box::new(f));
            }
        });
        self
    }

    /// マウスボタンの生入力（押し下げ、または離し）が発生した際に発火するイベントを登録します。
    #[must_use]
    #[inline]
    pub fn on_mouse_input<F>(self, mut f: F) -> Self
    where
        F: FnMut(MouseButton, Modifiers, ElementState) + 'static,
    {
        self.on_mouse_input_with(move |_cx, btn, mods, state| f(btn, mods, state))
    }

    #[must_use]
    #[inline]
    pub fn on_mouse_input_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, MouseButton, Modifiers, ElementState) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_mouse_input.take() {
                let mut f = f;
                l.on_mouse_input = Some(Box::new(move |cx, btn, mods, state| {
                    existing(cx, btn, mods, state);
                    f(cx, btn, mods, state);
                }));
            } else {
                l.on_mouse_input = Some(Box::new(f));
            }
        });
        self
    }

    /// マウスポインタが要素の可視境界内に入った（Enter）際に発火するイベントを登録します。
    #[must_use]
    #[inline]
    pub fn on_mouse_enter<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_mouse_enter_with(move |_cx| f())
    }

    #[must_use]
    #[inline]
    pub fn on_mouse_enter_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_mouse_enter.take() {
                let mut f = f;
                l.on_mouse_enter = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_mouse_enter = Some(Box::new(f));
            }
        });
        self
    }

    /// マウスポインタが要素の可視境界から外に出た（Leave）際に発火するイベントを登録します。
    #[must_use]
    #[inline]
    pub fn on_mouse_leave<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_mouse_leave_with(move |_cx| f())
    }

    #[must_use]
    #[inline]
    pub fn on_mouse_leave_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_mouse_leave.take() {
                let mut f = f;
                l.on_mouse_leave = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_mouse_leave = Some(Box::new(f));
            }
        });
        self
    }

    /// マウスポインタが要素内で移動した際に発火するイベントを登録します。
    /// コールバックには、要素の左上を原点 (0, 0) とする論理座標 `LayoutPoint` が伝播します。
    #[must_use]
    #[inline]
    pub fn on_cursor_moved<F>(self, mut f: F) -> Self
    where
        F: FnMut(LayoutPoint) + 'static,
    {
        self.on_cursor_moved_with(move |_cx, point| f(point))
    }

    #[must_use]
    #[inline]
    pub fn on_cursor_moved_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, LayoutPoint) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_cursor_moved.take() {
                let mut f = f;
                l.on_cursor_moved = Some(Box::new(move |cx, p| {
                    existing(cx, p);
                    f(cx, p);
                }));
            } else {
                l.on_cursor_moved = Some(Box::new(f));
            }
        });
        self
    }

    /// マウスホイールスクロールがこの要素上で検知された際のイベントをバインドします。
    /// コールバック引数には、論理ピクセル単位に換算された (`scroll_x`, `scroll_y`) が渡されます。
    #[must_use]
    #[inline]
    pub fn on_mouse_wheel<F>(self, mut f: F) -> Self
    where
        F: FnMut(f32, f32) + 'static,
    {
        self.on_mouse_wheel_with(move |_cx, sx, sy| f(sx, sy))
    }

    #[must_use]
    #[inline]
    pub fn on_mouse_wheel_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, f32, f32) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_mouse_wheel.take() {
                let mut f = f;
                l.on_mouse_wheel = Some(Box::new(move |cx, sx, sy| {
                    existing(cx, sx, sy);
                    f(cx, sx, sy);
                }));
            } else {
                l.on_mouse_wheel = Some(Box::new(f));
            }
        });
        self
    }

    /// スクロールコンテナの指定軸方向のオフセット（スクロール位置）を強制変更します。
    #[inline]
    #[must_use]
    pub fn scroll_to(self, x: f32, y: f32) -> Self {
        with_context(|cx| {
            cx.scroll_to(self.id, x, y);
        });
        self
    }

    /// スクロールコンテナを指定ピクセル分だけ相対移動させます。
    #[inline]
    #[must_use]
    pub fn scroll_by(self, dx: f32, dy: f32) -> Self {
        with_context(|cx| {
            cx.scroll_by(self.id, dx, dy);
        });
        self
    }

    /// このコンテナの現在のスクロール位置 (x, y) を安全に取得します。
    #[inline]
    #[must_use]
    pub fn scroll_offset(self) -> LayoutPoint {
        with_context(|cx| {
            cx.outputs
                .out_scroll_offsets
                .get(self.id)
                .copied()
                .unwrap_or(LayoutPoint::ZERO)
        })
    }

    /// 要素のドラッグ（左クリック押し下げ中のマウス移動）が発生した際に発火するイベントを登録します。
    /// コールバックには、前フレームからの移動差分である `LayoutPoint` が伝播します。
    #[must_use]
    #[inline]
    pub fn on_drag<F>(self, mut f: F) -> Self
    where
        F: FnMut(LayoutPoint) + 'static,
    {
        self.on_drag_with(move |_cx, delta| f(delta))
    }

    #[must_use]
    #[inline]
    pub fn on_drag_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, LayoutPoint) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_drag.take() {
                let mut f = f;
                l.on_drag = Some(Box::new(move |cx, p| {
                    existing(cx, p);
                    f(cx, p);
                }));
            } else {
                l.on_drag = Some(Box::new(f));
            }
        });
        self
    }

    /// マウスオーバーされた瞬間（`on_mouse_enter` と同時）に発火するイベントを登録します。
    #[must_use]
    #[inline]
    pub fn on_hover<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_hover_with(move |_cx| f())
    }

    #[must_use]
    #[inline]
    pub fn on_hover_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_hover.take() {
                let mut f = f;
                l.on_hover = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_hover = Some(Box::new(f));
            }
        });
        self
    }

    /// 物理キーボードキーの操作が発生した際に発火するイベントを登録します（フォーカス獲得時のみ有効）。
    #[must_use]
    #[inline]
    pub fn on_keyboard_input<F>(self, mut f: F) -> Self
    where
        F: FnMut(VirtualKey, Modifiers, ElementState) + 'static,
    {
        self.on_keyboard_input_with(move |_cx, key, mods, state| f(key, mods, state))
    }

    #[must_use]
    #[inline]
    pub fn on_keyboard_input_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, VirtualKey, Modifiers, ElementState) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_keyboard_input.take() {
                let mut f = f;
                l.on_keyboard_input = Some(Box::new(move |cx, key, mods, state| {
                    existing(cx, key, mods, state);
                    f(cx, key, mods, state);
                }));
            } else {
                l.on_keyboard_input = Some(Box::new(f));
            }
        });
        self
    }

    /// ローカライズやリピート処理が適用された確定1文字が入力された際に発火するイベントを登録します。
    #[must_use]
    #[inline]
    pub fn on_char_input<F>(self, mut f: F) -> Self
    where
        F: FnMut(char) + 'static,
    {
        self.on_char_input_with(move |_cx, c| f(c))
    }

    #[must_use]
    #[inline]
    pub fn on_char_input_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, char) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_char_input.take() {
                let mut f = f;
                l.on_char_input = Some(Box::new(move |cx, c| {
                    existing(cx, c);
                    f(cx, c);
                }));
            } else {
                l.on_char_input = Some(Box::new(f));
            }
        });
        self
    }

    /// IME（入力文字プロセッサ）による変換テキスト、キャレット、確定文字列の更新を捕捉するイベントを登録します。
    #[must_use]
    #[inline]
    pub fn on_ime<F>(self, mut f: F) -> Self
    where
        F: FnMut(ImeState) + 'static,
    {
        self.on_ime_with(move |_cx, state| f(state))
    }

    #[must_use]
    #[inline]
    pub fn on_ime_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, ImeState) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_ime.take() {
                let mut f = f;
                l.on_ime = Some(Box::new(move |cx, state| {
                    existing(cx, state.clone());
                    f(cx, state);
                }));
            } else {
                l.on_ime = Some(Box::new(f));
            }
        });
        self
    }

    /// OS上からファイルやフォルダーがこの要素へドラッグ＆ドロップされた際のイベントを登録します。
    #[must_use]
    #[inline]
    pub fn on_file_dropped<F>(self, mut f: F) -> Self
    where
        F: FnMut(Vec<PathBuf>) + 'static,
    {
        self.on_file_dropped_with(move |_cx, paths| f(paths))
    }

    #[must_use]
    #[inline]
    pub fn on_file_dropped_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, Vec<PathBuf>) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_file_dropped.take() {
                let mut f = f;
                l.on_file_dropped = Some(Box::new(move |cx, paths| {
                    existing(cx, paths.clone());
                    f(cx, paths);
                }));
            } else {
                l.on_file_dropped = Some(Box::new(f));
            }
        });
        self
    }

    /// ファイルが要素上にドラッグ侵入した際のイベント（シンプル版）
    #[must_use]
    #[inline]
    pub fn on_file_drag_enter<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_file_drag_enter_with(move |_cx| f())
    }

    /// ファイルが要素上にドラッグ侵入した際のイベント（エスケープハッチ版）
    #[must_use]
    #[inline]
    pub fn on_file_drag_enter_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_file_drag_enter.take() {
                let mut f = f;
                l.on_file_drag_enter = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_file_drag_enter = Some(Box::new(f));
            }
        });
        self
    }

    /// ファイルが要素上からドラッグ離脱した際のイベント（シンプル版）
    #[must_use]
    #[inline]
    pub fn on_file_drag_leave<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_file_drag_leave_with(move |_cx| f())
    }

    /// ファイルが要素上からドラッグ離脱した際のイベント（エスケープハッチ版）
    #[must_use]
    #[inline]
    pub fn on_file_drag_leave_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_file_drag_leave.take() {
                let mut f = f;
                l.on_file_drag_leave = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_file_drag_leave = Some(Box::new(f));
            }
        });
        self
    }

    /// 画像ファイルのロードが完了し、
    /// メタデータ（解像度、フォーマット、アニメーションの有無等）が取得可能になった時のイベントを登録します。
    #[must_use]
    #[inline]
    pub fn on_image_loaded<F>(self, mut f: F) -> Self
    where
        F: FnMut(ImageMetadata) + 'static,
    {
        self.on_image_loaded_with(move |_cx, img| f(img))
    }

    #[must_use]
    #[inline]
    pub fn on_image_loaded_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, ImageMetadata) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_image_loaded.take() {
                let mut f = f;
                l.on_image_loaded = Some(Box::new(move |cx, img| {
                    existing(cx, img.clone());
                    f(cx, img);
                }));
            } else {
                l.on_image_loaded = Some(Box::new(f));
            }
        });
        self
    }

    /// 動画ファイルがロードされ、メタデータ（解像度、FPS、ビットレート等）が取得可能になった時のイベントを登録します。
    #[must_use]
    #[inline]
    pub fn on_media_loaded<F>(self, mut f: F) -> Self
    where
        F: FnMut(MovieMetadata) + 'static,
    {
        self.on_media_loaded_with(move |_cx, movie| f(movie))
    }

    #[must_use]
    #[inline]
    pub fn on_media_loaded_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, MovieMetadata) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_media_loaded.take() {
                let mut f = f;
                l.on_media_loaded = Some(Box::new(move |cx, movie| {
                    existing(cx, movie.clone());
                    f(cx, movie);
                }));
            } else {
                l.on_media_loaded = Some(Box::new(f));
            }
        });
        self
    }

    /// 要素が新しく入力フォーカスを獲得した際に発火するイベントを登録します。
    #[must_use]
    #[inline]
    pub fn on_focus<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_focus_with(move |_cx| f())
    }

    #[must_use]
    #[inline]
    pub fn on_focus_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_focus.take() {
                let mut f = f;
                l.on_focus = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_focus = Some(Box::new(f));
            }
        });
        self
    }

    /// 他の要素がクリックされるなどして、フォーカスを喪失した際に発火するイベントを登録します。
    #[must_use]
    #[inline]
    pub fn on_blur<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_blur_with(move |_cx| f())
    }

    #[must_use]
    #[inline]
    pub fn on_blur_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_blur.take() {
                let mut f = f;
                l.on_blur = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_blur = Some(Box::new(f));
            }
        });
        self
    }

    /// 要素が無効化（Disabled）された瞬間に発火するイベントを登録します。
    #[must_use]
    #[inline]
    pub fn on_disable<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_disable_with(move |_cx| f())
    }

    #[must_use]
    #[inline]
    pub fn on_disable_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_disable.take() {
                let mut f = f;
                l.on_disable = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_disable = Some(Box::new(f));
            }
        });
        self
    }

    /// 要素がアクティブ（Actived）状態になった瞬間に発火するイベントを登録します。
    #[must_use]
    #[inline]
    pub fn on_active<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_active_with(move |_cx| f())
    }

    #[must_use]
    #[inline]
    pub fn on_active_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_active.take() {
                let mut f = f;
                l.on_active = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_active = Some(Box::new(f));
            }
        });
        self
    }

    /// チェックボックスやラジオボタンなどで、要素が選択（Selected）された瞬間に発火するイベントを登録します。
    #[must_use]
    #[inline]
    pub fn on_select<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_select_with(move |_cx| f())
    }

    #[must_use]
    #[inline]
    pub fn on_select_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_select.take() {
                let mut f = f;
                l.on_select = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_select = Some(Box::new(f));
            }
        });
        self
    }

    /// 実体（Entity）ドラッグ中に毎フレーム呼び出されるイベントを登録します。
    /// 引数: (ドラッグ元ID, 現在重なっているドロップ先ID)
    #[must_use]
    #[inline]
    pub fn on_dnd_element_drag<F>(self, mut f: F) -> Self
    where
        F: FnMut(Element, Option<Element>) + 'static,
    {
        self.on_dnd_element_drag_with(move |_cx, src, dst| f(src, dst))
    }

    #[must_use]
    #[inline]
    pub fn on_dnd_element_drag_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, Element, Option<Element>) + 'static,
    {
        self.get_or_create_listeners(|l| {
            l.on_dnd_entity_drag = Some(Box::new(f));
        });
        self
    }

    /// IDドラッグ中に毎フレーム呼び出されるイベントを登録します。
    #[must_use]
    #[inline]
    pub fn on_dnd_id_drag<F>(self, mut f: F) -> Self
    where
        F: FnMut(EntityId, Option<EntityId>) + 'static,
    {
        self.on_dnd_id_drag_with(move |_cx, src, dst| f(src, dst))
    }

    #[must_use]
    #[inline]
    pub fn on_dnd_id_drag_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, EntityId, Option<EntityId>) + 'static,
    {
        self.get_or_create_listeners(|l| {
            l.on_dnd_id_drag = Some(Box::new(f));
        });
        self
    }

    /// ドロップ完了時（成功またはエリア外での失敗時）に呼び出されるイベントを登録します。
    /// 引数: (ドラッグ元ID, ドロップされた先のID（失敗時はNone）)
    #[must_use]
    #[inline]
    pub fn on_dnd_element_drop<F>(self, mut f: F) -> Self
    where
        F: FnMut(Element, Option<Element>) + 'static,
    {
        self.on_dnd_element_drop_with(move |_cx, src, dst| f(src, dst))
    }

    #[must_use]
    #[inline]
    pub fn on_dnd_element_drop_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, Element, Option<Element>) + 'static,
    {
        self.get_or_create_listeners(|l| {
            l.on_dnd_entity_drop = Some(Box::new(f));
        });
        self
    }

    /// IDドロップ完了時（成功またはエリア外での失敗時）に呼び出されるイベントを登録します。
    #[must_use]
    #[inline]
    pub fn on_dnd_id_drop<F>(self, mut f: F) -> Self
    where
        F: FnMut(EntityId, Option<EntityId>) + 'static,
    {
        self.on_dnd_id_drop_with(move |_cx, src, dst| f(src, dst))
    }

    #[must_use]
    #[inline]
    pub fn on_dnd_id_drop_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, EntityId, Option<EntityId>) + 'static,
    {
        self.get_or_create_listeners(|l| {
            l.on_dnd_id_drop = Some(Box::new(f));
        });
        self
    }

    /// ドラッグ開始時（プレースホルダー生成の瞬間）に呼び出されるイベントを登録します。
    /// 引数: (元のオリジナル要素, 生成されたプレースホルダー要素)
    #[must_use]
    #[inline]
    pub fn on_dnd_drag_start<F>(self, mut f: F) -> Self
    where
        F: FnMut(Element, Element) + 'static,
    {
        self.on_dnd_drag_start_with(move |_cx, src, placeholder| f(src, placeholder))
    }

    #[must_use]
    #[inline]
    pub fn on_dnd_drag_start_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, Element, Element) + 'static,
    {
        self.get_or_create_listeners(|l| {
            l.on_dnd_drag_start = Some(Box::new(f));
        });
        self
    }

    /// シグナルやクロージャに基づいて要素の `STATE_ACTIVED`（アクティブ疑似スタイル）を自動的にマッピングします。
    #[must_use]
    pub fn active(self, active: impl Into<Prop<bool>>) -> Self {
        match active.into() {
            Prop::None => {}
            Prop::Static(val) => {
                with_context(|cx| {
                    cx.set_actived(self.id, val);
                });
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                with_context(|cx| {
                    cx.create_element_effect(id, EffectCategory::ActiveState, move |cx| {
                        let active_val = f();
                        cx.set_actived(id, active_val);
                    });
                });
            }
        }
        self
    }

    /// シグナルやクロージャに基づいて要素の `STATE_SELECTED`（選択疑似スタイル）を自動的にマッピングします。
    #[must_use]
    pub fn select(self, selected: impl Into<Prop<bool>>) -> Self {
        match selected.into() {
            Prop::None => {}
            Prop::Static(val) => {
                with_context(|cx| {
                    cx.set_selected(self.id, val);
                });
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                with_context(|cx| {
                    cx.create_element_effect(id, EffectCategory::SelectState, move |cx| {
                        let selected_val = f();
                        cx.set_selected(id, selected_val);
                    });
                });
            }
        }
        self
    }

    /// シグナルやクロージャに基づいて要素の `STATE_DISABLED`（無効疑似スタイル）を自動的にマッピングします。
    #[must_use]
    pub fn disable(self, disabled: impl Into<Prop<bool>>) -> Self {
        match disabled.into() {
            Prop::None => {}
            Prop::Static(val) => {
                with_context(|cx| {
                    cx.set_disabled(self.id, val);
                });
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                with_context(|cx| {
                    cx.create_element_effect(id, EffectCategory::DisableState, move |cx| {
                        let disabled_val = f();
                        cx.set_disabled(id, disabled_val);
                    });
                });
            }
        }
        self
    }

    /// シグナルやクロージャに基づいて要素の `STATE_FOCUSED`（フォーカス疑似スタイル）を自動的にマッピングします。
    #[must_use]
    pub fn focus(self, focused: impl Into<Prop<bool>>) -> Self {
        match focused.into() {
            Prop::None => {}
            Prop::Static(val) => {
                with_context(|cx| {
                    cx.set_focused(self.id, val);
                });
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                with_context(|cx| {
                    cx.create_element_effect(id, EffectCategory::FocusState, move |cx| {
                        let focused_val = f();
                        cx.set_focused(id, focused_val);
                    });
                });
            }
        }
        self
    }

    /// UI Automation のプロパティを生の ID (i32) を指定して直接登録します
    #[must_use]
    #[inline]
    pub fn uia_property(self, property_id: i32, value: impl Into<UiaValue>) -> Self {
        with_context(|cx| self.uia_property_internal(cx, property_id, value.into()));
        self
    }

    /// `スクリーンリーダーが読み上げる要素の「名前」を設定します（UIA_NamePropertyId` 互換）。
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
        let list = cx.system.sys_uia_properties.get_mut(self.id).unwrap();
        if let Some(pos) = list.iter().position(|(k, _)| *k == property_id) {
            list[pos].1 = value;
        } else {
            list.push((property_id, value));
        }
        cx.topology.topo_active_masks[self.id].set(COMP_UIA_CONTENT);
    }

    /// 自動テストフレームワークやデバッグで要素を特定するための「Automation `ID」を設定します（UIA_AutomationIdPropertyId` 互換）。
    #[inline]
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

    /// `この要素がどのようなコントロール（ボタン、チェックボックス、リスト等）として振る舞うかを定義します（UIA_ControlTypePropertyId` 互換）。
    #[inline]
    #[must_use]
    pub fn uia_control_type(self, control_type_id: i32) -> Self {
        self.uia_property(30003, control_type_id)
    }

    /// スクロールコンテナのスタイル設定に連動し、
    /// トラック・サムに相当する要素（Element）を遅延生成して親子関係にアタッチします。
    #[must_use]
    #[inline]
    pub(crate) fn ensure_scrollbar_elements(
        cx: &mut Context,
        id: EntityId,
        sb: &ScrollbarStyle,
        merge: bool,
    ) {
        if !cx.layouts.lay_scrollbar_styles.contains_key(id) {
            cx.layouts.lay_scrollbar_styles.insert(
                id,
                ScrollBarState {
                    style: sb.clone(),
                    ..Default::default()
                },
            );
        }

        let mut state = cx.layouts.lay_scrollbar_styles.get(id).cloned().unwrap();
        state.style = sb.clone();
        let mut changed = false;

        if sb.display != ScrollbarDisplay::None {
            // A. 縦スクロールバー (V-Track)
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

            // A-1. 縦つまみ (V-Thumb、V-Track の子要素としてアタッチ)
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

            // B. 横スクロールバー (H-Track)
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
            *cx.layouts.lay_scrollbar_styles.get_mut(id).unwrap() = state;
            cx.topology.topo_is_structure_dirty = true; // topo_flat_dfs_sequence の更新契機
        }
    }
}

#[cfg(test)]
mod tests;
