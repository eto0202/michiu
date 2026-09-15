use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq)]
pub enum WebView2Source {
    /// 外部のWebサイトやローカルのサーバー
    Url(Cow<'static, str>),
    /// 生のHTMLコード
    Html(Cow<'static, str>),
}

impl Default for WebView2Source {
    fn default() -> Self {
        Self::new()
    }
}

impl WebView2Source {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::Url("about:blank".into())
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq)]
pub struct WebView2Contents {
    pub source: WebView2Source,
    /// イベントフォワード（マウス/キーボード入力を受け付けるか）
    pub allow_interaction: bool,
    /// 右クリックのシステムデフォルトメニューを表示するか
    pub enable_context_menu: bool,
    /// F12で開発者ツールを起動できるか
    pub enable_dev_tools: bool,
    /// `JavaScriptを有効にするか`
    pub enable_scripts: bool,
    /// 起動時（ドキュメント読み込み前）に自動実行させるJavaScript
    pub user_scripts: Vec<Cow<'static, str>>,
    /// ユーザーが操作していなくても、常にコンポジションスレッドで再生し続けるか
    pub always_active: bool,
}

impl Default for WebView2Contents {
    fn default() -> Self {
        Self {
            source: WebView2Source::new(),
            allow_interaction: true,
            enable_context_menu: false, // デフォルトでは消してアプリ感を出す
            enable_dev_tools: false,    // デフォルトはオフ
            enable_scripts: true,
            user_scripts: Vec::new(),
            always_active: false,
        }
    }
}

impl WebView2Contents {
    #[inline]
    #[must_use]
    pub fn new(source: WebView2Source) -> Self {
        Self {
            source,
            ..Default::default()
        }
    }

    #[inline]
    pub fn from_url(url: impl Into<Cow<'static, str>>) -> Self {
        Self::new(WebView2Source::Url(url.into()))
    }

    #[inline]
    pub fn from_html(html: impl Into<Cow<'static, str>>) -> Self {
        Self::new(WebView2Source::Html(html.into()))
    }

    #[inline]
    #[must_use]
    pub fn url(mut self, url: impl Into<Cow<'static, str>>) -> Self {
        self.source = WebView2Source::Url(url.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn html(mut self, html: impl Into<Cow<'static, str>>) -> Self {
        self.source = WebView2Source::Html(html.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn allow_interaction(mut self, allow: bool) -> Self {
        self.allow_interaction = allow;
        self
    }

    #[inline]
    #[must_use]
    pub fn enable_context_menu(mut self, enable: bool) -> Self {
        self.enable_context_menu = enable;
        self
    }

    #[inline]
    #[must_use]
    pub fn enable_dev_tools(mut self, enable: bool) -> Self {
        self.enable_dev_tools = enable;
        self
    }

    #[inline]
    #[must_use]
    pub fn enable_scripts(mut self, enable: bool) -> Self {
        self.enable_scripts = enable;
        self
    }

    /// example
    /// `add_user_script(include_str!("example.js`"))
    #[inline]
    #[must_use]
    pub fn add_user_script(mut self, script: impl Into<Cow<'static, str>>) -> Self {
        self.user_scripts.push(script.into());
        self
    }

    /// 動画プレイヤーやWebGL、アニメーションがある場合、常時レンダリングを有効にする
    #[inline]
    #[must_use]
    pub fn always_active(mut self, always: bool) -> Self {
        self.always_active = always;
        self
    }
}
