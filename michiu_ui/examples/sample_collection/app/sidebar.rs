use crate::app::{ComponentType, SidebarSortOrder, theme::Theme};
pub use michiu_ui::prelude::*;
use strum::IntoEnumIterator;

pub fn sidebar() -> Element {
    v_flex(
        ts().width(170.0)
            .h_full()
            .min_width(170.0)
            .max_width(pct(40.0))
            .shrink_0()
            .border_right(BorderStyle::Solid, 1.0)
            .border_color(dynamic(|t: &Theme| t.border))
            .resizable_right(true),
    )
    .children([
        controll_header().children([header_label(), sort_toggle()]),
        list_container(),
    ])
}

fn controll_header() -> Element {
    h_flex(
        ts().justify_between()
            .items_center()
            .shrink_0()
            .p(10.0)
            .border_bottom(BorderStyle::Solid, 1.0)
            .border_color(dynamic(|t: &Theme| t.border)),
    )
}

fn header_label() -> Element {
    text("Components").style_d(|t: &Theme| {
        ts().font_size(14.0)
            .text_color(t.text_muted)
            .font_family(t.font_family.clone())
    })
}

fn sort_toggle() -> Element {
    div_d(|t: &Theme| {
        ts().p_x(8.0)
            .p_y(2.0)
            .r(2.0)
            .bg_color(t.surface)
            .border_solid(1.0)
            .border_color(t.border)
            .cursor_pointer()
            .hovered(ts().bg_color(t.background_hover))
    })
    .on_click(move || {
        let order = use_provided::<SidebarSortOrder>();
        let set_order = use_provided_setter::<SidebarSortOrder>();
        let next = match order.get() {
            SidebarSortOrder::Ascending => SidebarSortOrder::Descending,
            SidebarSortOrder::Descending => SidebarSortOrder::Ascending,
        };
        set_order.set(next);
    })
    .child(toggle_label())
}

fn toggle_label() -> Element {
    text_d(|order: &SidebarSortOrder| match order {
        SidebarSortOrder::Ascending => "A-Z ↑",
        SidebarSortOrder::Descending => "Z-A ↓",
    })
    .style_d(|t: &Theme| {
        ts().text_color(t.text_muted)
            .font_size(14.0)
            .font_family(t.font_family.clone())
            .pointer_events_none()
    })
}

fn list_container() -> Element {
    v_flex_d(|t: &Theme| scrollbar(t).grow().overflow_y_scroll()).child(move || {
        let order = use_provided::<SidebarSortOrder>().get();
        let query = use_provided::<String>().get().to_lowercase();

        let mut comp_types: Vec<ComponentType> = ComponentType::iter().collect();

        // 検索フィルタリング
        if !query.is_empty() {
            comp_types.retain(|comp| comp.to_string().to_lowercase().contains(&query));
        }

        // ソート順制御
        match order {
            SidebarSortOrder::Ascending => {
                comp_types.sort_by_key(|a| a.to_string());
            }
            SidebarSortOrder::Descending => {
                comp_types.sort_by_key(|b| std::cmp::Reverse(b.to_string()));
            }
        }

        // リストアイテム
        let items: Vec<Element> = comp_types
            .into_iter()
            .map(|comp_type| sidebar_item(comp_type.to_string(), comp_type))
            .collect();

        v_flex(ts().w_full()).children(items)
    })
}

fn sidebar_item(name: String, item_type: ComponentType) -> Element {
    h_flex_d(move |t: &Theme| {
        let active_bg = t.border_hover;
        let hover_bg = t.background_hover;

        let current = use_provided::<ComponentType>().get();
        let is_active = current == item_type;

        let bg = if is_active {
            active_bg
        } else {
            Color::TRANSPARENT
        };

        let h_bg = if is_active { active_bg } else { hover_bg };

        let border_color = if is_active {
            t.primary_hover
        } else {
            Color::TRANSPARENT
        };

        ts().justify_center()
            .items_center()
            .p(10.0)
            .bg_color(bg)
            .border_left(BorderStyle::Solid, 4.0)
            .border_color(border_color)
            .hovered(ts().bg_color(h_bg))
    })
    .label_d(name, move |t: &Theme| {
        ts().text_color(t.text).font_size(t.font_size_base)
    })
    .on_click_with(move |cx| {
        let setter = cx.use_provided_setter::<ComponentType>();
        setter.set(item_type);
    })
}

pub fn scrollbar(t: &Theme) -> ThisStyle {
    ts().scrollbar_always()
        .scrollbar_mode(ScrollbarMode::Overlay)
        .scrollbar(
            ScrollbarStyle::new(10.0)
                .v_thumb(
                    ts().m_y(4.0)
                        .p_r(5.0)
                        .width(7.0)
                        .rounded_full()
                        .bg_color(t.border)
                        .hovered(ts().bg_color(t.border_hover)),
                )
                .h_thumb(
                    ts().m_x(4.0)
                        .p_b(5.0)
                        .height(7.0)
                        .rounded_full()
                        .bg_color(t.border)
                        .hovered(ts().bg_color(t.border_hover)),
                ),
        )
}
