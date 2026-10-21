use std::fs;
use std::path::Path;

pub fn rasterize_svg(input: &Path, output_png: &Path) -> Result<(), String> {
    let data = fs::read(input).map_err(|_| "这张图无法读取".to_string())?;
    let mut options = usvg::Options::default();
    options.default_size = usvg::Size::from_wh(1024.0, 1024.0).unwrap();
    let tree = usvg::Tree::from_data(&data, &options).map_err(|_| "这张图无法读取".to_string())?;
    let size = tree.size();
    let width = size.width().round().max(1.0) as u32;
    let height = size.height().round().max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or("这张图无法读取")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap
        .save_png(output_png)
        .map_err(|_| "这张图无法读取".to_string())
}

pub fn thumbnail_svg(input: &Path) -> Result<Vec<u8>, String> {
    let data = fs::read(input).map_err(|_| "无法预览".to_string())?;
    let mut options = usvg::Options::default();
    options.default_size = usvg::Size::from_wh(96.0, 96.0).unwrap();
    let tree = usvg::Tree::from_data(&data, &options).map_err(|_| "无法预览".to_string())?;
    let size = tree.size();
    let longest = size.width().max(size.height()).max(1.0);
    let scale = (96.0 / longest).min(1.0);
    let width = (size.width() * scale).round().max(1.0) as u32;
    let height = (size.height() * scale).round().max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or("无法预览")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(|_| "无法预览".to_string())
}
