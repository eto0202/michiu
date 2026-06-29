use crate::{
    Context, EffectCategory, ElementState, EntityId, EventListeners, ImageMetadata, ImageSource,
    ImeState, LayoutPoint, LinearGradient, Modifiers, MouseButton, MovieMetadata, MovieProperty,
    ReadSignal, Transform, UiaValue, VirtualKey, WebView2Contents, bitmap::*, create_effect,
    style::ThisStyle,
};
use std::{borrow::Cow, cell::Cell, path::PathBuf};

thread_local! {
    // 現在構築中のUIコンテキストへの生ポインタを一時的にバインドするグローバルスレッド領域。
    // UI構築は常に単一のスレッド（メインスレッド）で行われるため、このアプローチは安全に機能。
    static ACTIVE_CONTEXT: Cell<Option<*mut Context>> = const { Cell::new(None) };
}

/// ユーザーがコンポーネントを評価する際に呼び出すグローバルラッパー
pub fn build_ui(cx: &mut Context, f: impl FnOnce() -> Element) -> Element {
    let old = ACTIVE_CONTEXT.get();
    ACTIVE_CONTEXT.set(Some(cx as *mut Context));
    let _guard = ContextGuard { old };

    let marker = cx.start_session();
    let result = f();
    // 戻り値に含まれるハンドルをルート要素として登録
    cx.register_root(result.id);
    // 親子関係に組み込まれなかった無駄な孤児を自動一掃
    cx.end_session(marker);
    result
}

/// スレッドローカルから安全にContextへのアクセスを解決する内部ヘルパー
#[inline(always)]
pub(crate) fn with_context<R>(f: impl FnOnce(&mut Context) -> R) -> R {
    let ptr = ACTIVE_CONTEXT
        .get()
        .expect("No active UI Context found in this thread context");
    // UIスレッドは単一かつ非同期にまたがらないため、ポインタの生存期間は保証される。
    unsafe { f(&mut *ptr) }
}

/// 構築が完了したUI要素を表す、軽量なハンドル
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Element {
    pub(crate) id: EntityId,
}

impl Default for Element {
    fn default() -> Self {
        Self::new()
    }
}

impl Element {
    /// 新規要素の構築を開始します
    #[inline]
    pub fn new() -> Self {
        // スレッドローカルの Context から安全に要素を spawn
        let id = with_context(|cx| cx.spawn(None));
        Self { id }
    }

    #[inline]
    pub fn id(&self) -> EntityId {
        self.id
    }

    /// スタイルを適用します（静的な値、Signal、またはクロージャ）。
    pub fn style(self, style: impl Into<Prop<ThisStyle>>) -> Self {
        match style.into() {
            Prop::None => {}
            Prop::Static(s) => with_context(|cx| self.style_internal(cx, s)),
            Prop::Dynamic(f) => {
                let id = self.id;
                let effect_id = create_effect(move |cx| {
                    let s = f();
                    let element = Element { id };
                    element.style_internal(cx, s);
                });
                with_context(|cx| cx.register_element_effect(id, EffectCategory::Style, effect_id));
            }
        }
        self
    }

    /// スタイルの適用（一括インライン展開）
    pub(crate) fn style_internal(self, cx: &mut Context, style: ThisStyle) {
        let inner = &style.inner;
        let mask = inner.mask;

        // active_masks にスタイル側のマスクをマージするが、
        // 動的なインタラクション状態フラグ（STYLE_INTERACTION_PROPERTY）は
        // 実行時にのみ制御されるべきなので、ここでは除外（マスクアウト）する
        let property_only_mask = mask.0 & !STYLE_INTERACTION_PROPERTY;
        cx.active_masks[self.id].0 |= property_only_mask;

        // 1. ベーススタイル（不変の基準値）として登録
        if mask.has_basic_layout() {
            cx.base_basic_layouts.insert(self.id, inner.basic_layout);
            // サイズ等の基本レイアウトが静的に指定されたことをマーク
            cx.mark_layout_dirty(self.id);
        }
        if mask.has_visual_property() {
            cx.base_visual_properties
                .insert(self.id, inner.visual_property.clone());
        }
        if mask.has_interaction_property() {
            cx.interaction_properties
                .insert(self.id, inner.interaction_styles.clone());
        }

        // 2. トランジションが関与しないプロパティは即時にマウント
        if mask.has_flex_layout() {
            if let Some(flex) = cx.flex_layouts.get_mut(self.id) {
                flex.override_with(&inner.flex_layout, mask);
            } else {
                cx.flex_layouts.insert(self.id, inner.flex_layout);
            }
            cx.mark_layout_dirty(self.id);
        }

        if mask.has_grid_layout()
            && let Some(ref grid) = inner.grid_layout
        {
            cx.grid_layouts.insert(self.id, grid.clone());
            cx.mark_layout_dirty(self.id);
        }

        // 3. 即座に「スタイル解決」を走り込ませ、トランジションやレイアウトマウントを自動処理！
        cx.resolve_element_style_state(self.id);
    }

    /// 子要素を追加します（Element単体、Signal、またはクロージャ）。
    /// 動的な値が渡された場合、自動的にスロット要素が作成され、その中身がリアクティブに切り替わります。
    #[inline]
    pub fn child(self, element: impl Into<Prop<Element>>) -> Self {
        match element.into() {
            Prop::None => {}
            Prop::Static(child) => {
                with_context(|cx| cx.add_child(self.id, child.id));
            }
            Prop::Dynamic(f) => {
                // 動的な子の場合はスロット(div)を作成して追加し、その中身を set_content で管理する
                let slot = div_n();
                with_context(|cx| cx.add_child(self.id, slot.id));
                slot.set_contents(Prop::Dynamic(f));
            }
        }
        self
    }

    /// このコンテナの内容を差し替えます。以前の内容はすべて破棄されます。
    pub fn set_contents(self, contents: impl Into<Prop<Element>>) -> Self {
        match contents.into() {
            Prop::None => {}
            Prop::Static(new_child) => {
                with_context(|cx| self.set_contents_internal(cx, new_child));
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                let effect_id = create_effect(move |cx| {
                    let new_child = f();
                    let container = Element { id };
                    container.set_contents_internal(cx, new_child);
                });
                with_context(|cx| {
                    cx.register_element_effect(id, EffectCategory::Contents, effect_id)
                });
            }
        }
        self
    }

    /// 既存のクロージャをクリアせずに、子要素の差し替え（マウント）のみを実行する
    fn set_contents_internal(self, cx: &mut Context, new_child: Element) {
        let id = self.id;

        // 現在の子要素をすべて再帰的に despawn
        if let Some(children_list) = cx.children.get(id) {
            let old_children: Vec<EntityId> = children_list.iter().copied().collect();
            for child_id in old_children {
                cx.despawn_internal(child_id);
            }
        }

        // 新しい子要素を追加（親子トポロジーおよび Taffy ツリーの同期）
        cx.add_child(id, new_child.id);

        // レイアウトと描画の再計算を要求
        cx.mark_layout_dirty(id);
        cx.mark_render_dirty(id);
    }

    /// テキストを設定します。
    /// 引数には &str, String, ReadSignal<T>, またはクロージャを渡せます。
    #[inline]
    pub fn text(self, content: impl Into<Prop<Cow<'static, str>>>) -> Self {
        match content.into() {
            Prop::None => {}
            Prop::Static(val) => {
                with_context(|cx| {
                    cx.text_contents.insert(self.id, val);
                    cx.active_masks[self.id].set(COMP_TEXT_CONTENT);
                    cx.mark_layout_dirty(self.id);
                    cx.mark_render_dirty(self.id);
                });
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                let effect_id = create_effect(move |cx| {
                    let new_text = f();
                    cx.text_contents.insert(id, new_text);
                    cx.active_masks[id].set(COMP_TEXT_CONTENT);
                    cx.mark_layout_dirty(id);
                    cx.mark_render_dirty(id);
                });
                with_context(|cx| cx.register_element_effect(id, EffectCategory::Text, effect_id));
            }
        }
        self
    }

    /// 画像を設定します。
    pub fn image(self, content: impl Into<Prop<ImageSource>>) -> Self {
        match content.into() {
            Prop::None => {}
            Prop::Static(src) => {
                with_context(|cx| {
                    cx.image_sources.insert(self.id, src);
                    cx.active_masks[self.id].set(COMP_IMAGE_CONTENT);
                    cx.mark_layout_dirty(self.id);
                    cx.mark_render_dirty(self.id);
                });
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                let effect_id = create_effect(move |cx| {
                    let src = f();
                    cx.image_sources.insert(id, src);
                    cx.active_masks[id].set(COMP_IMAGE_CONTENT);
                    cx.mark_layout_dirty(id);
                    cx.mark_render_dirty(id);
                });
                with_context(|cx| cx.register_element_effect(id, EffectCategory::Image, effect_id));
            }
        }
        self
    }

    /// 動画を設定します。
    pub fn movie(self, content: impl Into<Prop<MovieProperty>>) -> Self {
        match content.into() {
            Prop::None => {}
            Prop::Static(p) => {
                with_context(|cx| {
                    cx.movie_properties.insert(self.id, p);
                    cx.active_masks[self.id].set(COMP_MOVIE_CONTENT);
                    cx.mark_layout_dirty(self.id);
                    cx.mark_render_dirty(self.id);
                });
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                let effect_id = create_effect(move |cx| {
                    let p = f();
                    cx.movie_properties.insert(id, p);
                    cx.active_masks[id].set(COMP_MOVIE_CONTENT);
                    cx.mark_layout_dirty(id);
                    cx.mark_render_dirty(id);
                });
                with_context(|cx| cx.register_element_effect(id, EffectCategory::Movie, effect_id));
            }
        }
        self
    }

    /// WebView2 コンポーネントを配置します（静的設定、またはSignal / クロージャに対応）。
    pub fn webview2(self, contents: impl Into<Prop<WebView2Contents>>) -> Self {
        match contents.into() {
            Prop::None => {}
            Prop::Static(contents) => {
                with_context(|cx| {
                    cx.webview_contents.insert(self.id, contents);
                    cx.active_masks[self.id].set(COMP_WEBVIEW_CONTENT);
                    cx.mark_layout_dirty(self.id);
                    cx.mark_render_dirty(self.id);
                });
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                let effect_id = create_effect(move |cx| {
                    let contents = f();
                    cx.webview_contents.insert(id, contents);
                    cx.active_masks[id].set(COMP_WEBVIEW_CONTENT);
                    cx.mark_layout_dirty(id);
                    cx.mark_render_dirty(id);
                });
                with_context(|cx| {
                    cx.register_element_effect(id, EffectCategory::WebView2, effect_id)
                });
            }
        }
        self
    }

    /// 内部ヘルパー：この要素に対応する `EventListeners` が SoA 上に存在しない場合は新規に作成し、
    /// 可変参照を取得して渡されたクロージャを実行します。
    #[inline(always)]
    fn get_or_create_listeners<R>(&self, f: impl FnOnce(&mut EventListeners) -> R) -> R {
        with_context(|cx| {
            // SparseSecondaryMap にキーが存在しない場合は Default (すべて None) で差し込む
            if !cx.event_listeners.contains_key(self.id) {
                cx.event_listeners
                    .insert(self.id, EventListeners::default());
            }
            let listeners = cx.event_listeners.get_mut(self.id).unwrap();
            f(listeners)
        })
    }

    /// 左クリックのリリース（押し下げ ➔ 同一要素上での離し）が成立した際に発火するイベントを登録します。
    #[inline]
    pub fn on_click<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_click_with(move |_cx| f())
    }

    #[inline]
    pub fn on_click_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_click = Some(Box::new(f)));
        self
    }

    /// 右クリックのリリース（押し下げ ➔ 同一要素上での離し）が成立した際に発火するイベントを登録します。
    #[inline]
    pub fn on_right_click<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_right_click_with(move |_cx| f())
    }

    #[inline]
    pub fn on_right_click_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_right_click = Some(Box::new(f)));
        self
    }

    /// マウスボタンの生入力（押し下げ、または離し）が発生した際に発火するイベントを登録します。
    #[inline]
    pub fn on_mouse_input<F>(self, mut f: F) -> Self
    where
        F: FnMut(MouseButton, Modifiers, ElementState) + 'static,
    {
        self.on_mouse_input_with(move |_cx, btn, mods, state| f(btn, mods, state))
    }

    #[inline]
    pub fn on_mouse_input_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, MouseButton, Modifiers, ElementState) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_mouse_input = Some(Box::new(f)));
        self
    }

    /// マウスポインタが要素の可視境界内に入った（Enter）際に発火するイベントを登録します。
    #[inline]
    pub fn on_mouse_enter<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_mouse_enter_with(move |_cx| f())
    }

    #[inline]
    pub fn on_mouse_enter_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_mouse_enter = Some(Box::new(f)));
        self
    }

    /// マウスポインタが要素の可視境界から外に出た（Leave）際に発火するイベントを登録します。
    #[inline]
    pub fn on_mouse_leave<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_mouse_leave_with(move |_cx| f())
    }

    #[inline]
    pub fn on_mouse_leave_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_mouse_leave = Some(Box::new(f)));
        self
    }

    /// マウスポインタが要素内で移動した際に発火するイベントを登録します。
    /// コールバックには、要素の左上を原点 (0, 0) とする論理座標 `LayoutPoint` が伝播します。
    #[inline]
    pub fn on_cursor_moved<F>(self, mut f: F) -> Self
    where
        F: FnMut(LayoutPoint) + 'static,
    {
        self.on_cursor_moved_with(move |_cx, point| f(point))
    }

    #[inline]
    pub fn on_cursor_moved_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, LayoutPoint) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_cursor_moved = Some(Box::new(f)));
        self
    }

    /// マウスホイール（縦スクロール）が回転した際に発火するイベントを登録します。
    #[inline]
    pub fn on_mouse_wheel<F>(self, mut f: F) -> Self
    where
        F: FnMut(f32) + 'static,
    {
        self.on_mouse_wheel_with(move |_cx, delta| f(delta))
    }

    #[inline]
    pub fn on_mouse_wheel_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, f32) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_mouse_wheel = Some(Box::new(f)));
        self
    }

    /// 要素のドラッグ（左クリック押し下げ中のマウス移動）が発生した際に発火するイベントを登録します。
    /// コールバックには、前フレームからの移動差分である `LayoutPoint` が伝播します。
    #[inline]
    pub fn on_drag<F>(self, mut f: F) -> Self
    where
        F: FnMut(LayoutPoint) + 'static,
    {
        self.on_drag_with(move |_cx, delta| f(delta))
    }

    #[inline]
    pub fn on_drag_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, LayoutPoint) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_drag = Some(Box::new(f)));
        self
    }

    /// マウスオーバーされた瞬間（`on_mouse_enter` と同時）に発火するイベントを登録します。
    #[inline]
    pub fn on_hover<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_hover_with(move |_cx| f())
    }

    #[inline]
    pub fn on_hover_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_hover = Some(Box::new(f)));
        self
    }

    /// 物理キーボードキーの操作が発生した際に発火するイベントを登録します（フォーカス獲得時のみ有効）。
    #[inline]
    pub fn on_keyboard_input<F>(self, mut f: F) -> Self
    where
        F: FnMut(VirtualKey, Modifiers, ElementState) + 'static,
    {
        self.on_keyboard_input_with(move |_cx, key, mods, state| f(key, mods, state))
    }

    #[inline]
    pub fn on_keyboard_input_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, VirtualKey, Modifiers, ElementState) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_keyboard_input = Some(Box::new(f)));
        self
    }

    /// ローカライズやリピート処理が適用された確定1文字が入力された際に発火するイベントを登録します。
    #[inline]
    pub fn on_char_input<F>(self, mut f: F) -> Self
    where
        F: FnMut(char) + 'static,
    {
        self.on_char_input_with(move |_cx, c| f(c))
    }

    #[inline]
    pub fn on_char_input_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, char) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_char_input = Some(Box::new(f)));
        self
    }

    /// IME（入力文字プロセッサ）による変換テキスト、キャレット、確定文字列の更新を捕捉するイベントを登録します。
    #[inline]
    pub fn on_ime<F>(self, mut f: F) -> Self
    where
        F: FnMut(ImeState) + 'static,
    {
        self.on_ime_with(move |_cx, state| f(state))
    }

    #[inline]
    pub fn on_ime_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, ImeState) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_ime = Some(Box::new(f)));
        self
    }

    /// OS上からファイルやフォルダーがこの要素へドラッグ＆ドロップされた際のイベントを登録します。
    #[inline]
    pub fn on_file_dropped<F>(self, mut f: F) -> Self
    where
        F: FnMut(Vec<PathBuf>) + 'static,
    {
        self.on_file_dropped_with(move |_cx, paths| f(paths))
    }

    #[inline]
    pub fn on_file_dropped_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, Vec<PathBuf>) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_file_dropped = Some(Box::new(f)));
        self
    }

    /// ファイルが要素上にドラッグ侵入した際のイベント（シンプル版）
    #[inline]
    pub fn on_file_drag_enter<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_file_drag_enter_with(move |_cx| f())
    }

    /// ファイルが要素上にドラッグ侵入した際のイベント（エスケープハッチ版）
    #[inline]
    pub fn on_file_drag_enter_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_file_drag_enter = Some(Box::new(f)));
        self
    }

    /// ファイルが要素上からドラッグ離脱した際のイベント（シンプル版）
    #[inline]
    pub fn on_file_drag_leave<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_file_drag_leave_with(move |_cx| f())
    }

    /// ファイルが要素上からドラッグ離脱した際のイベント（エスケープハッチ版）
    #[inline]
    pub fn on_file_drag_leave_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_file_drag_leave = Some(Box::new(f)));
        self
    }

    /// 画像ファイルのロードが完了し、
    /// メタデータ（解像度、フォーマット、アニメーションの有無等）が取得可能になった時のイベントを登録します。
    #[inline]
    pub fn on_image_loaded<F>(self, mut f: F) -> Self
    where
        F: FnMut(ImageMetadata) + 'static,
    {
        self.on_image_loaded_with(move |_cx, img| f(img))
    }

    #[inline]
    pub fn on_image_loaded_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, ImageMetadata) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_image_loaded = Some(Box::new(f)));
        self
    }

    /// 動画ファイルがロードされ、メタデータ（解像度、FPS、ビットレート等）が取得可能になった時のイベントを登録します。
    #[inline]
    pub fn on_media_loaded<F>(self, mut f: F) -> Self
    where
        F: FnMut(MovieMetadata) + 'static,
    {
        self.on_media_loaded_with(move |_cx, movie| f(movie))
    }

    #[inline]
    pub fn on_media_loaded_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, MovieMetadata) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_media_opened = Some(Box::new(f)));
        self
    }

    /// 要素が新しく入力フォーカスを獲得した際に発火するイベントを登録します。
    #[inline]
    pub fn on_focus<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_focus_with(move |_cx| f())
    }

    #[inline]
    pub fn on_focus_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_focus = Some(Box::new(f)));
        self
    }

    /// 他の要素がクリックされるなどして、フォーカスを喪失した際に発火するイベントを登録します。
    #[inline]
    pub fn on_blur<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_blur_with(move |_cx| f())
    }

    #[inline]
    pub fn on_blur_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_blur = Some(Box::new(f)));
        self
    }

    /// 要素が無効化（Disabled）された瞬間に発火するイベントを登録します。
    #[inline]
    pub fn on_disable<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_disable_with(move |_cx| f())
    }

    #[inline]
    pub fn on_disable_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_disable = Some(Box::new(f)));
        self
    }

    /// 要素がアクティブ（Actived）状態になった瞬間に発火するイベントを登録します。
    #[inline]
    pub fn on_active<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_active_with(move |_cx| f())
    }

    #[inline]
    pub fn on_active_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_active = Some(Box::new(f)));
        self
    }

    /// チェックボックスやラジオボタンなどで、要素が選択（Selected）された瞬間に発火するイベントを登録します。
    #[inline]
    pub fn on_select<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_select_with(move |_cx| f())
    }

    #[inline]
    pub fn on_select_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| l.on_select = Some(Box::new(f)));
        self
    }

    /// UI Automation のプロパティを生の ID (i32) を指定して直接登録します
    #[inline]
    pub fn uia_property(self, property_id: i32, value: impl Into<UiaValue>) -> Self {
        with_context(|cx| self.uia_property_internal(cx, property_id, value.into()));
        self
    }

    /// スクリーンリーダーが読み上げる要素の「名前」を設定します（UIA_NamePropertyId 互換）。
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
                    cx.register_element_effect(id, EffectCategory::UiaName, effect_id)
                });
                self
            }
        }
    }

    fn uia_property_internal(self, cx: &mut Context, property_id: i32, value: UiaValue) {
        if !cx.uia_properties.contains_key(self.id) {
            cx.uia_properties.insert(self.id, Vec::new());
        }
        let list = cx.uia_properties.get_mut(self.id).unwrap();
        if let Some(pos) = list.iter().position(|(k, _)| *k == property_id) {
            list[pos].1 = value;
        } else {
            list.push((property_id, value));
        }
        cx.active_masks[self.id].set(COMP_UIA_CONTENT);
    }

    /// 自動テストフレームワークやデバッグで要素を特定するための「Automation ID」を設定します（UIA_AutomationIdPropertyId 互換）。
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
                    cx.register_element_effect(id, EffectCategory::UiaAutomationId, effect_id)
                });
                self
            }
        }
    }

    /// この要素がどのようなコントロール（ボタン、チェックボックス、リスト等）として振る舞うかを定義します（UIA_ControlTypePropertyId 互換）。
    #[inline]
    pub fn uia_control_type(self, control_type_id: i32) -> Self {
        self.uia_property(30003, control_type_id)
    }
}

/// スタイルを適用して生成するコンテナ。
/// 静的な ThisStyle、ReadSignal<ThisStyle>、クロージャ、または None (Option) を受け入れます。
#[inline]
pub fn div(style: impl Into<Prop<ThisStyle>>) -> Element {
    let el = Element::new();
    el.style(style)
}

pub const NO_STYLE: Option<ThisStyle> = None;

/// 現時点ではスタイルを適用しないことを明示したコンテナ。
#[inline]
pub fn div_n() -> Element {
    div(NO_STYLE)
}

#[inline]
pub fn text(content: impl Into<Prop<Cow<'static, str>>>) -> Element {
    div_n().text(content)
}

#[inline]
pub fn img(source: impl Into<Prop<ImageSource>>) -> Element {
    div_n().image(source)
}

#[inline]
pub fn video(property: impl Into<Prop<MovieProperty>>) -> Element {
    div_n().movie(property)
}

#[inline]
pub fn webview2(contents: impl Into<Prop<WebView2Contents>>) -> Element {
    div_n().webview2(contents)
}

// コンテキストを復元するための一時的なガード構造体
pub(crate) struct ContextGuard {
    old: Option<*mut Context>,
}

/// 現在のスレッドローカル（ACTIVE_CONTEXT）に Context を一時的にバインドします。
/// 戻り値のガードオブジェクト（ContextGuard）がスコープを抜ける際、自動的に元のコンテキストに復元されます。
#[inline(always)]
pub(crate) fn bind_context(cx: &Context) -> ContextGuard {
    let old = ACTIVE_CONTEXT.get();
    // 借用チェッカーと衝突しないよう、生ポインタキャストを行ってスレッドローカルに格納
    ACTIVE_CONTEXT.set(Some(cx as *const Context as *mut Context));
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

impl<T> From<Option<T>> for Prop<T> {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => Self::Static(v),
            None => Self::None,
        }
    }
}

// 文字列リテラル用
impl From<&'static str> for Prop<Cow<'static, str>> {
    fn from(s: &'static str) -> Self {
        Self::Static(s.into())
    }
}

// String用
impl From<String> for Prop<Cow<'static, str>> {
    fn from(s: String) -> Self {
        Self::Static(s.into())
    }
}

// Cowそのもの
impl From<Cow<'static, str>> for Prop<Cow<'static, str>> {
    fn from(s: Cow<'static, str>) -> Self {
        Self::Static(s)
    }
}

// Displayを実装している型のSignal (u32, i32など)
impl<T: std::fmt::Display + Clone + Send + 'static> From<ReadSignal<T>>
    for Prop<Cow<'static, str>>
{
    fn from(sig: ReadSignal<T>) -> Self {
        Self::Dynamic(Box::new(move || sig.get().to_string().into()))
    }
}

// クロージャ用 (戻り値が Cow に変換可能なもの)
impl<F, S> From<F> for Prop<Cow<'static, str>>
where
    F: Fn() -> S + 'static,
    S: Into<Cow<'static, str>>,
{
    fn from(f: F) -> Self {
        Self::Dynamic(Box::new(move || f().into()))
    }
}

impl From<ThisStyle> for Prop<ThisStyle> {
    fn from(s: ThisStyle) -> Self {
        Self::Static(s)
    }
}

impl From<ReadSignal<ThisStyle>> for Prop<ThisStyle> {
    fn from(sig: ReadSignal<ThisStyle>) -> Self {
        Self::Dynamic(Box::new(move || sig.get()))
    }
}

impl<F> From<F> for Prop<ThisStyle>
where
    F: Fn() -> ThisStyle + 'static,
{
    fn from(f: F) -> Self {
        Self::Dynamic(Box::new(f))
    }
}

impl From<Element> for Prop<Element> {
    fn from(el: Element) -> Self {
        Self::Static(el)
    }
}

impl From<ReadSignal<Element>> for Prop<Element> {
    fn from(sig: ReadSignal<Element>) -> Self {
        Self::Dynamic(Box::new(move || sig.get()))
    }
}

impl<F> From<F> for Prop<Element>
where
    F: Fn() -> Element + 'static,
{
    fn from(f: F) -> Self {
        Self::Dynamic(Box::new(f))
    }
}

impl From<ReadSignal<ImageSource>> for Prop<ImageSource> {
    #[inline]
    fn from(sig: ReadSignal<ImageSource>) -> Self {
        Self::Dynamic(Box::new(move || sig.get()))
    }
}
impl From<ReadSignal<MovieProperty>> for Prop<MovieProperty> {
    #[inline]
    fn from(sig: ReadSignal<MovieProperty>) -> Self {
        Self::Dynamic(Box::new(move || sig.get()))
    }
}

impl<F, S> From<F> for Prop<ImageSource>
where
    F: Fn() -> S + 'static,
    S: Into<ImageSource>,
{
    #[inline]
    fn from(f: F) -> Self {
        Self::Dynamic(Box::new(move || f().into()))
    }
}
impl<F, S> From<F> for Prop<MovieProperty>
where
    F: Fn() -> S + 'static,
    S: Into<MovieProperty>,
{
    #[inline]
    fn from(f: F) -> Self {
        Self::Dynamic(Box::new(move || f().into()))
    }
}

impl From<Transform> for Prop<Transform> {
    #[inline]
    fn from(t: Transform) -> Self {
        Self::Static(t)
    }
}
impl From<[[f32; 4]; 4]> for Prop<Transform> {
    #[inline]
    fn from(m: [[f32; 4]; 4]) -> Self {
        Self::Static(Transform { matrix: m })
    }
}
impl From<ReadSignal<Transform>> for Prop<Transform> {
    #[inline]
    fn from(sig: ReadSignal<Transform>) -> Self {
        Self::Dynamic(Box::new(move || sig.get()))
    }
}
impl<F> From<F> for Prop<Transform>
where
    F: Fn() -> Transform + 'static,
{
    #[inline]
    fn from(f: F) -> Self {
        Self::Dynamic(Box::new(f))
    }
}

impl From<LinearGradient> for Prop<LinearGradient> {
    #[inline]
    fn from(g: LinearGradient) -> Self {
        Self::Static(g)
    }
}
impl From<ReadSignal<LinearGradient>> for Prop<LinearGradient> {
    #[inline]
    fn from(sig: ReadSignal<LinearGradient>) -> Self {
        Self::Dynamic(Box::new(move || sig.get()))
    }
}
impl<F> From<F> for Prop<LinearGradient>
where
    F: Fn() -> LinearGradient + 'static,
{
    #[inline]
    fn from(f: F) -> Self {
        Self::Dynamic(Box::new(f))
    }
}

impl From<ReadSignal<u32>> for Prop<u32> {
    #[inline]
    fn from(sig: ReadSignal<u32>) -> Self {
        Self::Dynamic(Box::new(move || sig.get()))
    }
}
impl<F> From<F> for Prop<u32>
where
    F: Fn() -> u32 + 'static,
{
    #[inline]
    fn from(f: F) -> Self {
        Self::Dynamic(Box::new(f))
    }
}

impl From<WebView2Contents> for Prop<WebView2Contents> {
    fn from(c: WebView2Contents) -> Self {
        Self::Static(c)
    }
}
impl From<ReadSignal<WebView2Contents>> for Prop<WebView2Contents> {
    fn from(sig: ReadSignal<WebView2Contents>) -> Self {
        Self::Dynamic(Box::new(move || sig.get()))
    }
}

#[cfg(test)]
mod tests;
