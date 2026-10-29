#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum GridAutoFlow {
    #[default]
    Row,
    Column,
    RowDense,
    ColumnDense,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum GridPlacement<S>
where
    S: ToString,
{
    #[default]
    Auto,
    Line(S),
    NamedLine(S),
    Span(S),
    NamedSpan(S),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridLine<T> {
    pub start: T,
    pub end: T,
}

impl Modifier {
    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_template_rows(
        mut self,
        value: impl IntoModValue<Vec<GridTemplateComponent<String>>>,
    ) -> Self {
        match value.into_mod_value() {
            ModValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_template_rows = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            ModValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == ModTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_template_rows = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_template_columns(
        mut self,
        value: impl IntoModValue<Vec<GridTemplateComponent<String>>>,
    ) -> Self {
        match value.into_mod_value() {
            ModValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_template_columns = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            ModValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == ModTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_template_columns = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_auto_rows(mut self, value: impl IntoModValue<Vec<TrackSizingFunction>>) -> Self {
        match value.into_mod_value() {
            ModValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_auto_rows = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            ModValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == ModTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_auto_rows = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_auto_columns(mut self, value: impl IntoModValue<Vec<TrackSizingFunction>>) -> Self {
        match value.into_mod_value() {
            ModValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_auto_columns = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            ModValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == ModTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_auto_columns = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_auto_flow(mut self, value: impl IntoModValue<GridAutoFlow>) -> Self {
        match value.into_mod_value() {
            ModValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_auto_flow = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            ModValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == ModTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_auto_flow = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_template_areas(
        mut self,
        value: impl IntoModValue<Option<GridTemplateAreas<String>>>,
    ) -> Self {
        match value.into_mod_value() {
            ModValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_template_areas = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            ModValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == ModTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_template_areas = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_template_column_names(
        mut self,
        value: impl IntoModValue<Vec<Vec<String>>>,
    ) -> Self {
        match value.into_mod_value() {
            ModValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_template_column_names = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            ModValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == ModTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_template_column_names = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_template_row_names(mut self, value: impl IntoModValue<Vec<Vec<String>>>) -> Self {
        match value.into_mod_value() {
            ModValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_template_row_names = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            ModValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == ModTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_template_row_names = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_row(mut self, value: impl IntoModValue<GridLine<GridPlacement<String>>>) -> Self {
        match value.into_mod_value() {
            ModValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_row = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            ModValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == ModTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_row = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }

    /// Not implemented
    #[inline]
    #[must_use]
    pub fn grid_column(
        mut self,
        value: impl IntoModValue<GridLine<GridPlacement<String>>>,
    ) -> Self {
        match value.into_mod_value() {
            ModValue::Static(v) => {
                let inner = Arc::make_mut(&mut self.inner);
                let grid = inner.grid_layout.get_or_insert_with(GridLayout::default);
                grid.grid_column = v;
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
            }
            ModValue::Dynamic(getter) => {
                let inner = Arc::make_mut(&mut self.inner);
                inner.mask.set(ComponentMask::STYLE_GRID_LAYOUT);
                inner.dynamic_setters.push(Arc::new(move |cx, id, target| {
                    if target == ModTarget::Base {
                        let val = getter();
                        if !cx.layouts.lay_grid.contains_key(id) {
                            cx.layouts.lay_grid.insert(id, GridLayout::default());
                        }
                        cx.layouts.lay_grid.at_mut(id).grid_column = val;
                        cx.mark_layout_dirty(id);
                    }
                }));
            }
        }
        self
    }
}

// GridLayout (Gridレイアウト：10プロパティ) - コールドデータ
/// Gridコンテナ、およびGrid子要素に適用される複雑な情報。
/// `Vec`（動的配列）を多数含み、メモリ上で重いため `Copy` は不可能（`Clone` のみ）。
/// 滅多に使われないため、`SparseSecondaryMap` で隔離し、未使用の要素には一切メモリを消費させない。
#[derive(Debug, Clone, PartialEq)]
pub struct GridLayout {
    pub grid_template_rows: Vec<taffy::GridTemplateComponent<String>>,
    pub grid_template_columns: Vec<taffy::GridTemplateComponent<String>>,
    pub grid_auto_rows: Vec<taffy::TrackSizingFunction>,
    pub grid_auto_columns: Vec<taffy::TrackSizingFunction>,
    pub grid_auto_flow: GridAutoFlow,
    pub grid_template_areas: Option<taffy::GridTemplateAreas<String>>,
    pub grid_template_column_names: Vec<Vec<String>>,
    pub grid_template_row_names: Vec<Vec<String>>,
    pub grid_row: GridLine<GridPlacement<String>>,
    pub grid_column: GridLine<GridPlacement<String>>,
}

unsafe impl Send for GridLayout {}
unsafe impl Sync for GridLayout {}

impl Default for GridLayout {
    fn default() -> Self {
        // Taffyデフォルト値と完全に一致するように空のVecで初期化
        Self {
            grid_template_rows: Vec::new(),
            grid_template_columns: Vec::new(),
            grid_auto_rows: Vec::new(),
            grid_auto_columns: Vec::new(),
            grid_auto_flow: GridAutoFlow::Row,
            grid_template_areas: None,
            grid_template_column_names: Vec::new(),
            grid_template_row_names: Vec::new(),
            grid_row: GridLine {
                start: GridPlacement::Auto,
                end: GridPlacement::Auto,
            },
            grid_column: GridLine {
                start: GridPlacement::Auto,
                end: GridPlacement::Auto,
            },
        }
    }
}

define_sparse_secondary!(pub struct GridLayoutsSparse(GridLayout));
define_sparse_secondary!(pub struct ResolvedGridSparse(GridLayout));
