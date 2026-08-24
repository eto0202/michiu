#![allow(unused)]

pub use michiu_ui::prelude::*;
use smallvec::SmallVec;

use crate::app::{sidebar, theme::Theme};

pub type ChildrenVec = SmallVec<[Element; 10]>;

const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

pub fn container() -> Element {
    h_flex_d(|t: &Theme| {
        sidebar::scrollbar(t)
            .p(16.0)
            .r(4.0)
            .size_full()
            .overflow_scroll()
    })
    .children(element_1())
}

/*
 * 要素数はたぶん 260000 程度
 * それぞれ一桁の数値ラベルを持つ
 * 全ての子は Signal を使用しホバー時に背景色が変化。クリック時にフォントサイズが変化
 * container は常時スクロールバー（overlay）を表示。それ以外は hidden
 *
 * 1. container 直下は親 100（横並び） で開始
 */

#[inline]
fn element_1() -> Vec<Element> {
    let mut children = Vec::new();

    for _ in 0..100 {
        let el = v_flex(ts()).children(element_2());
        children.push(el);
    }
    children
}

#[inline]
fn base_el(i: i32) -> Element {
    let label_str = DIGITS.get(i as usize).copied().unwrap_or("");

    div(ts().hovered(ts().bg_color(dynamic(|t: &Theme| t.primary))))
        .label(
            label_str,
            ts().font_size(16.0)
                .text_color(dynamic(|t: &Theme| t.text))
                .pressed(ts().font_size(20.0)),
        )
        .on_click_with(|cx| println!("Current Element ID: {:?}", cx.current_element_id()))
}

#[inline]
fn element_2() -> ChildrenVec {
    let mut children = SmallVec::new();

    for _ in 0..5 {
        let el = v_flex(ts()).children(element_3());
        children.push(el);
    }
    children
}

#[inline]
fn element_3() -> ChildrenVec {
    let mut children = SmallVec::new();

    for _ in 0..5 {
        let el = v_flex(ts()).children(element_4());
        children.push(el);
    }
    children
}

#[inline]
fn element_4() -> ChildrenVec {
    let mut children = SmallVec::new();

    for _ in 0..5 {
        let el = v_flex(ts()).children(element_5());
        children.push(el);
    }
    children
}

#[inline]
fn element_5() -> ChildrenVec {
    let mut children = SmallVec::new();

    for i in 0..10 {
        let el = v_flex(ts()).child(base_el(i));
        children.push(el);
    }
    children
}
