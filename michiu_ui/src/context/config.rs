#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityConfig {
    pub sys_text_buffers: usize,
    pub sys_uia_properties: usize,
    pub react_signals: usize,
    pub react_effects: usize,
    pub react_subscribers: usize,
    pub react_element_effects: usize,
    pub react_effect_to_element: usize,
    pub react_pending_element_effects: usize,
    pub react_providers: usize,
    pub evt_listeners: usize,
    pub dnd_drag_properties: usize,
    pub dnd_drop_properties: usize,
    pub cont_text_contents: usize,
    pub cont_text_spans: usize,
    pub cont_input_contents: usize,
    pub cont_image_sources: usize,
    pub cont_movie_properties: usize,
    pub cont_webview_contents: usize,
    pub cont_external_textures: usize,
    pub topo_entities: usize,
    pub topo_parents: usize,
    pub topo_children: usize,
    pub topo_active_masks: usize,
    pub topo_active_entities: usize,
    pub topo_session_spawned: usize,
    pub topo_session_roots: usize,
    pub topo_flat_dfs_sequence: usize,
    pub topo_sorted_entities: usize,
    pub topo_effective_z_indices: usize,
    pub topo_dfs_indices: usize,
    pub topo_sort_cache: usize,
    pub topo_webview_entities: usize,
    pub topo_despawned_queue: usize,
    pub lay_basic: usize,
    pub lay_flex: usize,
    pub lay_grid: usize,
    pub lay_base_basic: usize,
    pub lay_base_flex: usize,
    pub bar_styles: usize,
    pub lay_taffy_nodes: usize,
    pub lay_taffy_tree: usize,
    pub lay_dirty_entities: usize,
    pub lay_resolved_basic: usize,
    pub lay_resolved_flex: usize,
    pub lay_resolved_grid: usize,
    pub rnd_visual: usize,
    pub rnd_interaction: usize,
    pub rnd_base_visual: usize,
    pub rnd_dirty_entities: usize,
    pub rnd_active_transitions: usize,
    pub rnd_active_animations: usize,
    pub rnd_active_webviews: usize,
    pub out_rects: usize,
    pub out_clip_rects: usize,
    pub sc_offsets: usize,
    pub sc_sizes: usize,
    pub out_prev_rects: usize,
    pub out_prev_clip_rects: usize,
    pub edit_selected_rects: usize,
    pub edit_selections: usize,
    pub edit_selection_start_index: usize,
}

impl CapacityConfig {
    #[must_use]
    #[inline]
    /// 想定する最大ノード数を基準に、各配列のキャパシティを傾斜配分して生成
    pub fn from_base_nodes(base: usize) -> Self {
        Self {
            // Entity数とほぼ1対1
            topo_entities: base,
            topo_parents: base,
            topo_children: base,
            topo_active_masks: base,
            topo_active_entities: base,
            topo_session_spawned: base,
            topo_flat_dfs_sequence: base,
            topo_sorted_entities: base,
            topo_effective_z_indices: base,
            topo_dfs_indices: base,
            topo_sort_cache: base,
            lay_basic: base,
            lay_base_basic: base,
            lay_taffy_nodes: base,
            lay_taffy_tree: base,
            rnd_visual: base,
            rnd_interaction: base,
            out_rects: base,
            out_clip_rects: base,
            out_prev_rects: base,

            react_signals: base * 2,
            react_effects: base * 2,
            react_subscribers: base * 4, // 依存関係は掛け算になりやすい
            react_element_effects: base,
            react_effect_to_element: base,
            react_pending_element_effects: base / 10, // 保留中のものは少ない

            evt_listeners: base / 2,

            cont_text_contents: base / 3,   // 3割程度がテキスト表示
            cont_text_spans: base / 2,      // スパンはテキストよりやや多め
            cont_input_contents: base / 20, // 入力欄は少ない
            cont_image_sources: base / 10,
            cont_movie_properties: (base / 1000).max(1),
            cont_external_textures: base / 10,

            lay_flex: base / 2,  // 2つに1つはFlexbox
            lay_grid: base / 10, // Gridは少なめ
            lay_base_flex: base / 2,
            lay_resolved_basic: base / 2,
            lay_resolved_flex: base / 2,

            rnd_active_transitions: base / 10, // アニメーション等は1割程度
            rnd_active_animations: base / 20,

            sys_text_buffers: base / 3,
            sys_uia_properties: base / 10,

            ..Self::ZERO
        }
    }

    pub const ZERO: Self = Self {
        sys_text_buffers: 0,
        sys_uia_properties: 0,
        react_signals: 0,
        react_effects: 0,
        react_subscribers: 0,
        react_element_effects: 0,
        react_effect_to_element: 0,
        react_pending_element_effects: 0,
        react_providers: 0,
        evt_listeners: 0,
        dnd_drag_properties: 0,
        dnd_drop_properties: 0,
        cont_text_contents: 0,
        cont_text_spans: 0,
        cont_input_contents: 0,
        cont_image_sources: 0,
        cont_movie_properties: 0,
        cont_webview_contents: 0,
        cont_external_textures: 0,
        topo_entities: 0,
        topo_parents: 0,
        topo_children: 0,
        topo_active_masks: 0,
        topo_active_entities: 0,
        topo_session_spawned: 0,
        topo_session_roots: 0,
        topo_flat_dfs_sequence: 0,
        topo_sorted_entities: 0,
        topo_effective_z_indices: 0,
        topo_dfs_indices: 0,
        topo_sort_cache: 0,
        topo_webview_entities: 0,
        topo_despawned_queue: 0,
        lay_basic: 0,
        lay_flex: 0,
        lay_grid: 0,
        lay_base_basic: 0,
        lay_base_flex: 0,
        bar_styles: 0,
        lay_taffy_nodes: 0,
        lay_taffy_tree: 0,
        lay_dirty_entities: 0,
        lay_resolved_basic: 0,
        lay_resolved_flex: 0,
        lay_resolved_grid: 0,
        rnd_visual: 0,
        rnd_interaction: 0,
        rnd_base_visual: 0,
        rnd_dirty_entities: 0,
        rnd_active_transitions: 0,
        rnd_active_animations: 0,
        rnd_active_webviews: 0,
        out_rects: 0,
        out_clip_rects: 0,
        sc_offsets: 0,
        sc_sizes: 0,
        out_prev_rects: 0,
        out_prev_clip_rects: 0,
        edit_selected_rects: 0,
        edit_selections: 0,
        edit_selection_start_index: 0,
    };
}

impl Default for CapacityConfig {
    fn default() -> Self {
        Self::ZERO
    }
}
