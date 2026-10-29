#![expect(unused)]

#[cfg(test)]
use crate::{Context, Entity, LayoutSize};

#[cfg(test)]
pub struct TestContext {
    pub ctx: Context,
    // 必要に応じてタスク受信確認用などのテスト用ハンドルを保持
}

#[cfg(test)]
impl TestContext {
    pub fn new() -> Self {
        Self {
            ctx: Context::new(),
        }
    }

    // よく使う初期状態のプリセット（例: ウィンドウサイズ設定済み）

    // テスト用のダミーエンティティを簡易生成するヘルパー
}

#[cfg(test)]
impl std::ops::Deref for TestContext {
    type Target = Context;
    fn deref(&self) -> &Self::Target {
        &self.ctx
    }
}

#[cfg(test)]
impl std::ops::DerefMut for TestContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ctx
    }
}
