use crate::*;
use slotmap::{SecondaryMap, SparseSecondaryMap};

pub struct OutputStore {
    pub(crate) rects: SecondaryMap<EntityId, LayoutRect>,
    pub(crate) clip_rects: SecondaryMap<EntityId, LayoutRect>,
    pub(crate) scroll_offsets: SecondaryMap<EntityId, LayoutPoint>,
    pub(crate) prev_rects: SecondaryMap<EntityId, LayoutRect>,
    pub(crate) prev_clip_rects: SecondaryMap<EntityId, LayoutRect>,
    pub(crate) selected_rects: SparseSecondaryMap<EntityId, Vec<LayoutRect>>,
}

impl Default for OutputStore {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputStore {
    #[inline]
    pub fn new() -> Self {
        Self {
            rects: SecondaryMap::new(),
            clip_rects: SecondaryMap::new(),
            scroll_offsets: SecondaryMap::new(),
            prev_rects: SecondaryMap::new(),
            prev_clip_rects: SecondaryMap::new(),
            selected_rects: SparseSecondaryMap::new(),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.rects.clear();
        self.clip_rects.clear();
        self.scroll_offsets.clear();
        self.prev_rects.clear();
        self.prev_clip_rects.clear();
        self.selected_rects.clear();
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.rects.remove(id);
        self.clip_rects.remove(id);
        self.scroll_offsets.remove(id);
        self.prev_rects.remove(id);
        self.prev_clip_rects.remove(id);
        self.selected_rects.remove(id);
    }
}

impl OutputStore {
    /// 指定した要素の画面上の絶対座標（LayoutRect）を取得します。
    pub fn rect(&self, handle: Element) -> Option<LayoutRect> {
        self.rects.get(handle.id).copied()
    }

    /// 指定した要素の画面上のクリップ境界（LayoutRect）を取得します。
    pub fn clip_rect(&self, handle: Element) -> Option<LayoutRect> {
        self.clip_rects.get(handle.id).copied()
    }
}
