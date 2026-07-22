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

    pub(crate) fn swap_output_rect(outputs: &mut OutputStore) {
        std::mem::swap(&mut outputs.rects, &mut outputs.prev_rects);
        std::mem::swap(&mut outputs.clip_rects, &mut outputs.prev_clip_rects);

        outputs.rects.clear();
        outputs.clip_rects.clear();
    }

    pub(crate) fn parent_changed(
        id: EntityId,
        outputs: &OutputStore,
        parents: &SecondaryMap<EntityId, Option<EntityId>>,
        active_masks: &SecondaryMap<EntityId, ComponentMask>,
    ) -> bool {
        let parent_id_opt = parents.get(id).copied().flatten();

        let mut parent_changed = false;

        if let Some(parent_id) = parent_id_opt {
            let prev_parent_rect = outputs.prev_rects.get(parent_id);
            let curr_parent_rect = outputs.rects.get(parent_id);
            let prev_parent_clip = outputs.prev_clip_rects.get(parent_id);
            let curr_parent_clip = outputs.clip_rects.get(parent_id);
            let is_parent_dirty = active_masks[parent_id].has(STATE_QUEUED_LAYOUT);

            // 親が動いた、サイズが変わった、クリップが変わった、または親にレイアウト変更がある
            if prev_parent_rect != curr_parent_rect
                || prev_parent_clip != curr_parent_clip
                || is_parent_dirty
            {
                parent_changed = true;
            }
        }

        parent_changed
    }

    pub(crate) fn calc_local_rect(
        id: EntityId,
        outputs: &OutputStore,
        layouts: &LayoutStore,
        parents: &SecondaryMap<EntityId, Option<EntityId>>,
        window_size: LayoutSize,
    ) -> (LayoutRect, LayoutRect) {
        let initial_clip = LayoutRect::new(0.0, 0.0, window_size.width, window_size.height);

        // Taffyから実データを引き出す
        let local_rect = LayoutStore::local_rect_from_taffy(id, layouts);

        let parent_id_opt = parents.get(id).copied().flatten();
        let (abs_rect, parent_clip) = if let Some(parent_id) = parent_id_opt {
            let parent_rect = outputs.rects[parent_id];
            let parent_clip = outputs.clip_rects[parent_id];

            let is_absolute = layouts
                .basic_layouts
                .get(id)
                .map(|l| l.position == Position::Absolute)
                .unwrap_or(false);

            let parent_scroll = if is_absolute {
                LayoutPoint::ZERO
            } else {
                outputs
                    .scroll_offsets
                    .get(parent_id)
                    .copied()
                    .unwrap_or(LayoutPoint::ZERO)
            };

            let abs_x = parent_rect.x + local_rect.x - parent_scroll.x;
            let abs_y = parent_rect.y + local_rect.y - parent_scroll.y;

            (
                LayoutRect::new(abs_x, abs_y, local_rect.width, local_rect.height),
                parent_clip,
            )
        } else {
            (
                LayoutRect::new(
                    local_rect.x,
                    local_rect.y,
                    local_rect.width,
                    local_rect.height,
                ),
                initial_clip,
            )
        };

        (abs_rect, parent_clip)
    }
}
