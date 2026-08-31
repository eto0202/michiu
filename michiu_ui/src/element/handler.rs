use std::path::PathBuf;

use crate::{
    Context, EffectCategory, Element, ElementState, EntityId, EventListeners, ImageMetadata,
    ImeState, LayoutPoint, Modifiers, MouseButton, MovieMetadata, Prop, StateFlag, VirtualKey,
    with_context,
};

impl Element {
    /// この要素に対応する `EventListeners` が `SoA` 上に存在しない場合は新規に作成し、
    /// 可変参照を取得して渡されたクロージャを実行。
    #[inline]
    pub(crate) fn get_or_create_listeners<R>(self, f: impl FnOnce(&mut EventListeners) -> R) -> R {
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
    pub fn scroll_offset(self) -> Option<LayoutPoint> {
        with_context(|cx| cx.outputs.out_scroll_offsets.get(self.id).copied())
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
    #[inline]
    #[must_use]
    pub fn active(self, active: impl Into<Prop<bool>>) -> Self {
        self.bind_prop(active, EffectCategory::ActiveState, |cx, id, val| {
            cx.set_states(id, &StateFlag::Actived, val);
        })
    }

    /// シグナルやクロージャに基づいて要素の `STATE_SELECTED`（選択疑似スタイル）を自動的にマッピングします。
    #[inline]
    #[must_use]
    pub fn select(self, selected: impl Into<Prop<bool>>) -> Self {
        self.bind_prop(selected, EffectCategory::SelectState, |cx, id, val| {
            cx.set_states(id, &StateFlag::Selected, val);
        })
    }

    /// シグナルやクロージャに基づいて要素の `STATE_DISABLED`（無効疑似スタイル）を自動的にマッピングします。
    #[inline]
    #[must_use]
    pub fn disable(self, disabled: impl Into<Prop<bool>>) -> Self {
        self.bind_prop(disabled, EffectCategory::DisableState, |cx, id, val| {
            cx.set_states(id, &StateFlag::Disabled, val);
        })
    }

    /// シグナルやクロージャに基づいて要素の `STATE_FOCUSED`（フォーカス疑似スタイル）を自動的にマッピングします。
    #[inline]
    #[must_use]
    pub fn focus(self, focused: impl Into<Prop<bool>>) -> Self {
        self.bind_prop(focused, EffectCategory::FocusState, |cx, id, val| {
            cx.set_states(id, &StateFlag::Focused, val);
        })
    }
}
