use michiu::{
    prelude::*,
    ui::{ExternalMask, ExternalMaskMetadata, MaskImage},
};
use resvg::{tiny_skia, usvg};
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    path::Path,
    sync::Arc,
};

pub fn container() -> Entity {
    v_flex(ii().p(16.0).gap(16.0).r(3.0).w_full()).child(section_1())
}

fn section_1() -> Entity {
    const CLOSE: &[u8] = include_bytes!("../assets/close_24dp.svg");
    const SEARCH: &[u8] = include_bytes!("../assets/search_24dp.svg");
    const CHECK: &[u8] = include_bytes!("../assets/check_24dp.svg");
    const STAR: &[u8] = include_bytes!("../assets/star_24dp.svg");
    const FAVORITE: &[u8] = include_bytes!("../assets/favorite_24dp.svg");

    let mask_close = SvgMask::from_data(CLOSE).unwrap();
    let mask_search = SvgMask::from_data(SEARCH).unwrap();
    let mask_check = SvgMask::from_data(CHECK).unwrap();
    let mask_star = SvgMask::from_data(STAR).unwrap();
    let mask_favorite = SvgMask::from_data(FAVORITE).unwrap();

    h_flex(ii().w_full().gap(24.0)).children([
        external_mask(mask_close).modify(ii()),
        external_mask(mask_search).modify(ii().text_color(Color::GREEN)),
        external_mask(mask_check).modify(ii().size((30.0, 50.0))),
        external_mask(mask_star).modify(
            ii().opacity(0.2)
                .text_color(Color::YELLOW)
                .debug_border_red(),
        ),
        external_mask(mask_favorite).modify(
            ii().size(50.0)
                .p(10.0)
                .text_color(Color::RED)
                .hovered(ii().text_color(Color::BLUE))
                .debug_border_red(),
        ),
    ])
}

#[derive(Clone, Debug)]
pub struct SvgMask {
    tree: Arc<usvg::Tree>,
    metadata: ExternalMaskMetadata,
}

impl SvgMask {
    pub fn new(path: impl AsRef<Path>) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let path = path.as_ref();
        let svg_data = std::fs::read(path)?;
        Self::from_data(&svg_data)
    }

    pub fn from_data(svg_data: &[u8]) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let opt = usvg::Options::default();
        let tree = usvg::Tree::from_data(svg_data, &opt)?;

        let original_size = tree.size();
        let size = LayoutSize::new(original_size.width(), original_size.height());

        // パスからハッシュを計算してキャッシュキーにする（複数箇所で使い回すため）
        let mut hasher = DefaultHasher::new();
        svg_data.hash(&mut hasher);
        let cache_key = hasher.finish();

        let metadata = ExternalMaskMetadata::new(size).with_cache_key(cache_key);

        Ok(Self {
            tree: Arc::new(tree),
            metadata,
        })
    }
}

impl ExternalMask for SvgMask {
    fn metadata(&self) -> ExternalMaskMetadata {
        self.metadata
    }

    fn resolve_mask(&self, width: u32, height: u32) -> MaskImage {
        let w = width.max(1);
        let h = height.max(1);

        // ラスタライズ用 Pixmap を生成
        let mut pixmap = tiny_skia::Pixmap::new(w, h).expect("Failed to allocate pixmap");

        // SVG の元サイズから描画先サイズへのスケーリング行列を計算
        let orig_size = self.tree.size();
        let sx = w as f32 / orig_size.width();
        let sy = h as f32 / orig_size.height();
        let transform = tiny_skia::Transform::from_scale(sx, sy);

        // CPU ラスタライズ
        resvg::render(&self.tree, transform, &mut pixmap.as_mut());

        // tiny-skia (RGBA8 PMA) からアルファ値（Aチャンネル）のみを抽出して R8 バッファにする
        let bytes: Vec<u8> = pixmap.pixels().iter().map(|p| p.alpha()).collect();

        MaskImage {
            width: w,
            height: h,
            bytes,
        }
    }
}
