use windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VirtualKey(pub u32);

impl VirtualKey {
    // --- マウス & システム制御 ---
    pub const LBUTTON: Self = Self(0x01); // 左マウスボタン
    pub const RBUTTON: Self = Self(0x02); // 右マウスボタン
    pub const CANCEL: Self = Self(0x03); // Control-break 処理
    pub const MBUTTON: Self = Self(0x04); // 中央マウスボタン
    pub const XBUTTON1: Self = Self(0x05); // X1 マウスボタン
    pub const XBUTTON2: Self = Self(0x06); // X2 マウスボタン

    // --- 主要な編集・入力制御 ---
    pub const BACK: Self = Self(0x08); // BACKSPACE
    pub const TAB: Self = Self(0x09); // TAB
    pub const CLEAR: Self = Self(0x0C); // CLEAR
    pub const RETURN: Self = Self(0x0D); // ENTER (Return)
    pub const SHIFT: Self = Self(0x10); // SHIFT
    pub const CONTROL: Self = Self(0x11); // CTRL
    pub const MENU: Self = Self(0x12); // ALT
    pub const PAUSE: Self = Self(0x13); // PAUSE
    pub const CAPITAL: Self = Self(0x14); // CAPS LOCK

    // --- 日本語/韓国語/IME制御キー ---
    pub const KANA: Self = Self(0x15); // かな入力 / ハングルモード (HANGULと同値)
    pub const HANGUL: Self = Self(0x15); // ハングルモード (KANAと同値)
    pub const IME_ON: Self = Self(0x16); // IME オン
    pub const JUNJA: Self = Self(0x17); // IME ジュンジャモード
    pub const FINAL: Self = Self(0x18); // IME ファイナルモード
    pub const HANJA: Self = Self(0x19); // 漢字モード / ハンジャモード (KANJIと同値)
    pub const KANJI: Self = Self(0x19); // 漢字モード (HANJAと同値)
    pub const IME_OFF: Self = Self(0x1A); // IME オフ

    // --- 画面制御・移動キー ---
    pub const ESCAPE: Self = Self(0x1B); // ESC
    pub const CONVERT: Self = Self(0x1C); // 変換 (IME)
    pub const NONCONVERT: Self = Self(0x1D); // 無変換 (IME)
    pub const ACCEPT: Self = Self(0x1E); // 確定 (IME)
    pub const MODECHANGE: Self = Self(0x1F); // モード変更要求 (IME)
    pub const SPACE: Self = Self(0x20); // スペースキー
    pub const PRIOR: Self = Self(0x21); // PAGE UP
    pub const NEXT: Self = Self(0x22); // PAGE DOWN
    pub const END: Self = Self(0x23); // END
    pub const HOME: Self = Self(0x24); // HOME
    pub const LEFT: Self = Self(0x25); // 左矢印
    pub const UP: Self = Self(0x26); // 上矢印
    pub const RIGHT: Self = Self(0x27); // 右矢印
    pub const DOWN: Self = Self(0x28); // 下矢印
    pub const SELECT: Self = Self(0x29); // SELECT
    pub const PRINT: Self = Self(0x2A); // PRINT
    pub const EXECUTE: Self = Self(0x2B); // EXECUTE
    pub const SNAPSHOT: Self = Self(0x2C); // PRINT SCREEN
    pub const INSERT: Self = Self(0x2D); // INSERT
    pub const DELETE: Self = Self(0x2E); // DELETE
    pub const HELP: Self = Self(0x2F); // HELP

    // --- 数字キー (0-9) ---
    pub const KEY_0: Self = Self(0x30);
    pub const KEY_1: Self = Self(0x31);
    pub const KEY_2: Self = Self(0x32);
    pub const KEY_3: Self = Self(0x33);
    pub const KEY_4: Self = Self(0x34);
    pub const KEY_5: Self = Self(0x35);
    pub const KEY_6: Self = Self(0x36);
    pub const KEY_7: Self = Self(0x37);
    pub const KEY_8: Self = Self(0x38);
    pub const KEY_9: Self = Self(0x39);

    // --- 英字キー (A-Z) ---
    pub const A: Self = Self(0x41);
    pub const B: Self = Self(0x42);
    pub const C: Self = Self(0x43);
    pub const D: Self = Self(0x44);
    pub const E: Self = Self(0x45);
    pub const F: Self = Self(0x46);
    pub const G: Self = Self(0x47);
    pub const H: Self = Self(0x48);
    pub const I: Self = Self(0x49);
    pub const J: Self = Self(0x4A);
    pub const K: Self = Self(0x4B);
    pub const L: Self = Self(0x4C);
    pub const M: Self = Self(0x4D);
    pub const N: Self = Self(0x4E);
    pub const O: Self = Self(0x4F);
    pub const P: Self = Self(0x50);
    pub const Q: Self = Self(0x51);
    pub const R: Self = Self(0x52);
    pub const S: Self = Self(0x53);
    pub const T: Self = Self(0x54);
    pub const U: Self = Self(0x55);
    pub const V: Self = Self(0x56);
    pub const W: Self = Self(0x57);
    pub const X: Self = Self(0x58);
    pub const Y: Self = Self(0x59);
    pub const Z: Self = Self(0x5A);

    // --- Windows 固有キー ---
    pub const LWIN: Self = Self(0x5B); // 左 Windows キー
    pub const RWIN: Self = Self(0x5C); // 右 Windows キー
    pub const APPS: Self = Self(0x5D); // アプリケーションキー (メニューキー)
    pub const SLEEP: Self = Self(0x5F); // スリープキー

    // --- テンキー (Numpad) ---
    pub const NUMPAD0: Self = Self(0x60);
    pub const NUMPAD1: Self = Self(0x61);
    pub const NUMPAD2: Self = Self(0x62);
    pub const NUMPAD3: Self = Self(0x63);
    pub const NUMPAD4: Self = Self(0x64);
    pub const NUMPAD5: Self = Self(0x65);
    pub const NUMPAD6: Self = Self(0x66);
    pub const NUMPAD7: Self = Self(0x67);
    pub const NUMPAD8: Self = Self(0x68);
    pub const NUMPAD9: Self = Self(0x69);
    pub const MULTIPLY: Self = Self(0x6A); // 積算 (*)
    pub const ADD: Self = Self(0x6B); // 加算 (+)
    pub const SEPARATOR: Self = Self(0x6C); // セパレータキー
    pub const SUBTRACT: Self = Self(0x6D); // 減算 (-)
    pub const DECIMAL: Self = Self(0x6E); // 小数点 (.)
    pub const DIVIDE: Self = Self(0x6F); // 除算 (/)

    // --- ファンクションキー (F1-F24) ---
    pub const F1: Self = Self(0x70);
    pub const F2: Self = Self(0x71);
    pub const F3: Self = Self(0x72);
    pub const F4: Self = Self(0x73);
    pub const F5: Self = Self(0x74);
    pub const F6: Self = Self(0x75);
    pub const F7: Self = Self(0x76);
    pub const F8: Self = Self(0x77);
    pub const F9: Self = Self(0x78);
    pub const F10: Self = Self(0x79);
    pub const F11: Self = Self(0x7A);
    pub const F12: Self = Self(0x7B);
    pub const F13: Self = Self(0x7C);
    pub const F14: Self = Self(0x7D);
    pub const F15: Self = Self(0x7E);
    pub const F16: Self = Self(0x7F);
    pub const F17: Self = Self(0x80);
    pub const F18: Self = Self(0x81);
    pub const F19: Self = Self(0x82);
    pub const F20: Self = Self(0x83);
    pub const F21: Self = Self(0x84);
    pub const F22: Self = Self(0x85);
    pub const F23: Self = Self(0x86);
    pub const F24: Self = Self(0x87);

    // --- ロック制御 ---
    pub const NUMLOCK: Self = Self(0x90); // NUM LOCK
    pub const SCROLL: Self = Self(0x91); // SCROLL LOCK

    // --- 左右別 Modifier キー ---
    pub const LSHIFT: Self = Self(0xA0); // 左 SHIFT
    pub const RSHIFT: Self = Self(0xA1); // 右 SHIFT
    pub const LCONTROL: Self = Self(0xA2); // 左 CTRL
    pub const RCONTROL: Self = Self(0xA3); // 右 CTRL
    pub const LMENU: Self = Self(0xA4); // 左 ALT (Left Menu)
    pub const RMENU: Self = Self(0xA5); // 右 ALT (Right Menu)

    // --- ブラウザ制御 ---
    pub const BROWSER_BACK: Self = Self(0xA6); // 戻る
    pub const BROWSER_FORWARD: Self = Self(0xA7); // 進む
    pub const BROWSER_REFRESH: Self = Self(0xA8); // 更新
    pub const BROWSER_STOP: Self = Self(0xA9); // 中止
    pub const BROWSER_SEARCH: Self = Self(0xAA); // 検索
    pub const BROWSER_FAVORITES: Self = Self(0xAB); // お気に入り
    pub const BROWSER_HOME: Self = Self(0xAC); // ホーム/スタート

    // --- 音量・メディア制御 ---
    pub const VOLUME_MUTE: Self = Self(0xAD); // ミュート
    pub const VOLUME_DOWN: Self = Self(0xAE); // 音量下げ
    pub const VOLUME_UP: Self = Self(0xAF); // 音量上げ
    pub const MEDIA_NEXT_TRACK: Self = Self(0xB0); // 次のトラック
    pub const MEDIA_PREV_TRACK: Self = Self(0xB1); // 前のトラック
    pub const MEDIA_STOP: Self = Self(0xB2); // 停止
    pub const MEDIA_PLAY_PAUSE: Self = Self(0xB3); // 再生/一時停止

    // --- アプリケーション起動 ---
    pub const LAUNCH_MAIL: Self = Self(0xB4); // メーラー起動
    pub const LAUNCH_MEDIA_SELECT: Self = Self(0xB5); // メディアプレーヤー起動
    pub const LAUNCH_APP1: Self = Self(0xB6); // アプリケーション1起動
    pub const LAUNCH_APP2: Self = Self(0xB7); // アプリケーション2起動

    // --- OEM 特有・記号類 (キーボードレイアウトにより変動) ---
    pub const OEM_1: Self = Self(0xBA); // US: ';:', JP: ';:' (仕様により異なる場合あり)
    pub const OEM_PLUS: Self = Self(0xBB); // どの国でも '+' キー
    pub const OEM_COMMA: Self = Self(0xBC); // どの国でも ',' キー
    pub const OEM_MINUS: Self = Self(0xBD); // どの国でも '-' キー
    pub const OEM_PERIOD: Self = Self(0xBE); // どの国でも '.' キー
    pub const OEM_2: Self = Self(0xBF); // US: '/?', JP: '/?'
    pub const OEM_3: Self = Self(0xC0); // US: '`~', JP: '`~' (半角/全角にマップされる等)

    // --- ゲームパッド関連 (Windows 10/11 Xboxコントローラ等) ---
    pub const GAMEPAD_A: Self = Self(0xC3);
    pub const GAMEPAD_B: Self = Self(0xC4);
    pub const GAMEPAD_X: Self = Self(0xC5);
    pub const GAMEPAD_Y: Self = Self(0xC6);
    pub const GAMEPAD_RIGHT_SHOULDER: Self = Self(0xC7);
    pub const GAMEPAD_LEFT_SHOULDER: Self = Self(0xC8);
    pub const GAMEPAD_LEFT_TRIGGER: Self = Self(0xC9);
    pub const GAMEPAD_RIGHT_TRIGGER: Self = Self(0xCA);
    pub const GAMEPAD_DPAD_UP: Self = Self(0xCB);
    pub const GAMEPAD_DPAD_DOWN: Self = Self(0xCC);
    pub const GAMEPAD_DPAD_LEFT: Self = Self(0xCD);
    pub const GAMEPAD_DPAD_RIGHT: Self = Self(0xCE);
    pub const GAMEPAD_MENU: Self = Self(0xCF);
    pub const GAMEPAD_VIEW: Self = Self(0xD0);
    pub const GAMEPAD_LEFT_THUMBSTICK_BUTTON: Self = Self(0xD1);
    pub const GAMEPAD_RIGHT_THUMBSTICK_BUTTON: Self = Self(0xD2);
    pub const GAMEPAD_LEFT_THUMBSTICK_UP: Self = Self(0xD3);
    pub const GAMEPAD_LEFT_THUMBSTICK_DOWN: Self = Self(0xD4);
    pub const GAMEPAD_LEFT_THUMBSTICK_RIGHT: Self = Self(0xD5);
    pub const GAMEPAD_LEFT_THUMBSTICK_LEFT: Self = Self(0xD6);
    pub const GAMEPAD_RIGHT_THUMBSTICK_UP: Self = Self(0xD7);
    pub const GAMEPAD_RIGHT_THUMBSTICK_DOWN: Self = Self(0xD8);
    pub const GAMEPAD_RIGHT_THUMBSTICK_RIGHT: Self = Self(0xD9);
    pub const GAMEPAD_RIGHT_THUMBSTICK_LEFT: Self = Self(0xDA);

    // --- OEM 特有・その他記号 ---
    pub const OEM_4: Self = Self(0xDB); // US: '[{', JP: '[{'
    pub const OEM_5: Self = Self(0xDC); // US: '\|', JP: '\|'
    pub const OEM_6: Self = Self(0xDD); // US: ']}', JP: ']}'
    pub const OEM_7: Self = Self(0xDE); // US: ''"', JP: '^~'
    pub const OEM_8: Self = Self(0xDF); // 各国キーボードにより異なる
    pub const OEM_102: Self = Self(0xE2); // 102キーまたはJPキーボードのバックスラッシュなど

    // --- 特殊システムおよびIME処理キー ---
    pub const PROCESSKEY: Self = Self(0xE5); // IME PROCESS
    pub const PACKET: Self = Self(0xE7); // Unicode文字をキーストロークとして渡す仮想値
    pub const ATTN: Self = Self(0xF6); // Attn
    pub const CRSEL: Self = Self(0xF7); // CrSel
    pub const EXSEL: Self = Self(0xF8); // ExSel
    pub const EREOF: Self = Self(0xF9); // Erase EOF
    pub const PLAY: Self = Self(0xFA); // Play
    pub const ZOOM: Self = Self(0xFB); // Zoom
    pub const NONAME: Self = Self(0xFC); // Reserved
    pub const PA1: Self = Self(0xFD); // PA1
    pub const OEM_CLEAR: Self = Self(0xFE); // Clear
}

impl From<VirtualKey> for VIRTUAL_KEY {
    #[inline]
    fn from(vk: VirtualKey) -> Self {
        Self(vk.0 as u16)
    }
}

impl From<VIRTUAL_KEY> for VirtualKey {
    #[inline]
    fn from(vk: VIRTUAL_KEY) -> Self {
        Self(vk.0 as u32)
    }
}

impl VirtualKey {
    #[inline]
    pub fn to_windows(self) -> VIRTUAL_KEY {
        VIRTUAL_KEY(self.0 as u16)
    }

    #[inline]
    pub fn from_windows(vk: VIRTUAL_KEY) -> Self {
        Self(vk.0 as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_key_conversion() {
        let original = VirtualKey::A;
        let win_vk = original.to_windows();

        // 1. 正しい値（0x41）にキャストされているか
        assert_eq!(win_vk.0, 0x41);
        // 2. 逆変換した際に元の値に完全に戻るか
        assert_eq!(VirtualKey::from_windows(win_vk), original);
    }
}
