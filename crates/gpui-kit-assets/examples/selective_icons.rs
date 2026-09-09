//! A link-level fixture: exactly two SVG payloads, and no full Assets fallback.
use gpui::AssetSource;
use gpui_kit_assets::{Icon, icon_bundle};

fn main() {
    let assets = icon_bundle![Icon::Copy, Icon::Check.filled()];
    for path in assets.list("").expect("selected paths") {
        let bytes = assets
            .load(&path)
            .expect("asset lookup")
            .expect("selected asset");
        println!("{path}: {} bytes", std::hint::black_box(bytes).len());
    }
}
