#![allow(unused)]

pub use michiu_ui::prelude::*;
use smallvec::SmallVec;

use crate::app::{sidebar, theme::Theme};

pub type ChildrenVec = SmallVec<[Entity; 10]>;

const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

pub fn container() -> Entity {
    h_flex_d(|t: &Theme| {
        sidebar::scrollbar(t)
            .p(16.0)
            .r(4.0)
            .size_full()
            .overflow_scroll()
    })
    .children(entity_1())
}

#[inline]
fn entity_1() -> Vec<Entity> {
    let mut children = Vec::new();

    for _ in 0..100 {
        let e = v_flex(ii()).children(entity_2());
        children.push(e);
    }
    children
}

#[inline]
fn base_el(i: i32) -> Entity {
    let label_str = DIGITS.get(i as usize).copied().unwrap_or("");

    flex(ii().hovered(ii().bg_color(dynamic(|t: &Theme| t.primary))))
        .label(
            label_str,
            ii().font_size(16.0)
                .text_color(dynamic(|t: &Theme| t.text))
                .pressed_parent(ii().font_size(20.0)),
        )
        .on_click_with(|cx| println!("Current ID: {:?}", cx.current()))
}

#[inline]
fn entity_2() -> ChildrenVec {
    let mut children = SmallVec::new();

    for _ in 0..5 {
        let e = v_flex(ii()).children(entity_3());
        children.push(e);
    }
    children
}

#[inline]
fn entity_3() -> ChildrenVec {
    let mut children = SmallVec::new();

    for _ in 0..5 {
        let e = v_flex(ii()).children(entity_4());
        children.push(e);
    }
    children
}

#[inline]
fn entity_4() -> ChildrenVec {
    let mut children = SmallVec::new();

    for _ in 0..5 {
        let e = v_flex(ii()).children(entity_5());
        children.push(e);
    }
    children
}

#[inline]
fn entity_5() -> ChildrenVec {
    let mut children = SmallVec::new();

    for i in 0..10 {
        let e = v_flex(ii()).child(base_el(i));
        children.push(e);
    }
    children
}
