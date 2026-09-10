pub mod handler;
pub mod input_func;

use smallvec::SmallVec;

use crate::{
    BasicLayout, ComponentMask, Context, EffectCategory, EntityId, ExternalTexture, MichiuSoA,
    ReadSignal, ScrollBarState, ScrollbarDisplay, ScrollbarStyle, StyleTarget, ThisStyle, UiaValue,
    Val, WebView2Contents, create_effect, div_n,
};
use std::{borrow::Cow, cell::Cell, rc::Rc, sync::Arc};

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
#[repr(u8)]
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

    /// 要素が現在保持している親要素のハンドルを安全に取得します。
    #[inline]
    #[must_use]
    pub fn parent_element(self) -> Option<Element> {
        with_context(|cx| cx.parent_element(self))
    }

    /// 要素が現在保持している子要素のハンドルリストを安全に取得します。
    #[inline]
    #[must_use]
    pub fn children_list(self) -> Option<Vec<Element>> {
        with_context(|cx| cx.children_list(self))
    }

    /// この要素に対して、型 T のコンテキスト（シグナル）を提供（Provide）します。
    /// この要素、およびそのすべての子孫要素のエフェクトから `use_provided::<T>()` で取得可能になります。
    #[must_use]
    pub fn provide<T: Send + 'static>(self, read_signal: ReadSignal<T>) -> Self {
        with_context(|cx| {
            cx.provide::<T>(Some(self.id), read_signal);
        });
        self
    }

    /// 静的な値、または動的に変化する Prop を、該当する `EffectCategory` を通じて自動バインド
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

    /// プロバイダー `P` から動的に `ThisStyle` を解決してスタイルを適用します。
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

    /// スタイルの適用（一括インライン展開）
    pub(crate) fn style_internal(cx: &mut Context, id: EntityId, style: &ThisStyle, merge: bool) {
        let inner = &style.inner;
        let mask = inner.mask;

        // topo_active_masks にスタイル側のマスクをマージするが、
        // 動的なインタラクション状態フラグ（STYLE_INTERACTION_PROPERTY）は
        // 実行時にのみ制御されるべきなのでここでは除外する
        let property_only_mask = mask.0 & !ComponentMask::STYLE_INTERACTION_PROPERTY;

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
            cx.mark_layout_dirty(id);
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
            cx.mark_layout_dirty(id);
        }

        if mask.has_grid_layout()
            && let Some(ref grid) = inner.grid_layout
        {
            cx.layouts.lay_grid.insert(id, grid.clone());
            cx.mark_layout_dirty(id);
        }

        if mask.has(ComponentMask::STYLE_SCROLLBAR)
            && let Some(ref sb) = inner.scrollbar_style
        {
            Element::ensure_scrollbar_elements(cx, id, sb, merge);
            cx.mark_layout_dirty(id);
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
            let signal = with_context(|cx| cx.use_provided::<P>());
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
    // children_c を持つコンテナには他の静的子要素を混在させない
    #[must_use]
    #[inline]
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
                let new_elements: Vec<Element> = f(&val)
                    .into_iter()
                    .map(|e| match e.into() {
                        Prop::Static(el) => el,
                        _ => panic!("Dynamic nested elements inside children_c are not supported"),
                    })
                    .collect();

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
            let signal = with_context(|cx| cx.use_provided::<P>());
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
                    if let Some(effects) = cx.reactive.react_element_effects.get_mut(id)
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
        if let Some(sb) = cx.layouts.scrollbar.bar_styles.get(id) {
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

    /// テキストを設定します。
    /// 引数には &str, String, `ReadSignal`<T>, またはクロージャを渡せます。
    /// 1ノードパターン
    #[must_use]
    #[inline]
    pub fn text(self, content: impl Into<Prop<Cow<'static, str>>>) -> Self {
        self.bind_prop(content, EffectCategory::Text, |cx, id, val| {
            cx.contents.cont_text_contents.insert(id, val.into());
            cx.topology
                .topo_active_masks
                .at_mut(id)
                .set(ComponentMask::COMP_TEXT_CONTENT);
            cx.clear_layout_cache(id);
            cx.mark_dirty(id);
        })
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
            let signal = with_context(|cx| cx.use_provided::<P>());
            let val = signal.get();
            f(&val).into()
        }));
        self.text(dynamic_prop)
    }

    /// 外部画像や動画をwgpuで描画するためのテクスチャプロバイダ（`ExternalTexture`）をバインドします。
    ///
    /// ### 動的なテクスチャの更新（動画やゲーム画面など）
    /// 毎フレームテクスチャの中身が更新されるような動的要件は、描画直前に `ExternalTexture::resolve_view()`
    /// が毎回呼び出される仕様になっているため、プロバイダの内部処理だけで自動的に完結します。
    ///
    /// ### ソース自体の動的な切り替え（動画から静止画への変更など）
    /// 「動画から静止画へ切り替える」といった、テクスチャのソース自体を動的に変更したい場合は、以下のいずれかの方法を選択してください。
    ///
    /// 1. **トポロジーを差し替える**: 対象のUI要素を一度 `despawn` し、新しいソースを指定した要素として再生成する（不要になったリソースをVRAMから安全に解放できます）。
    /// 2. **プロバイダの内部で切り替える**: `ExternalTexture` を実装したオブジェクト自体は維持し、内部のデコーダ等に命令を送ることで、`resolve_view()` が返すテクスチャ（`TextureView`）を動的に切り替える。
    ///
    /// ### 設計上の注意（パフォーマンス）
    /// `BindGroup` の作成・再生成は処理負荷が高いため、本メソッドは軽量な `Prop<T>`（リアクティブなプロパティ）を介したテクスチャの動的差し替えをサポートしていません。
    /// 仮に `Prop<T>` による差し替えを可能にすると、リアクティブエフェクト内で毎フレームプロバイダを新規生成するような、パフォーマンスを著しく低下させるコードを容易に記述できてしまうためです。
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

    /// `WebView2` コンポーネントを配置します（静的設定、またはSignal / クロージャに対応）。
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

    /// プロバイダー `P` `から動的にWebView2設定を解決してアタッチします`。
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

    /// 自動テストフレームワークやデバッグで要素を特定するための「Automation `ID」を設定します（UIA_AutomationIdPropertyId` 互換）。
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

    /// `この要素がどのようなコントロール（ボタン、チェックボックス、リスト等）として振る舞うかを定義します（UIA_ControlTypePropertyId` 互換）。
    #[inline]
    #[must_use]
    pub fn uia_control_type(self, control_type_id: i32) -> Self {
        self.uia_property(30003, control_type_id)
    }

    /// スクロールコンテナのスタイル設定に連動し、
    /// トラック・サムに相当する要素（Element）を遅延生成して親子関係にアタッチします。
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

        let mut state = cx.layouts.scrollbar.bar_styles.get(id).cloned().unwrap();
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
            *cx.layouts.scrollbar.bar_styles.get_mut(id).unwrap() = state;
            cx.topology.topo_is_structure_dirty = true; // topo_flat_dfs_sequence の更新契機
            cx.topology.topo_is_sort_dirty = true;
        }
    }
}

#[cfg(test)]
mod tests;
