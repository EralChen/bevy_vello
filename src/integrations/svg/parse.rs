use std::sync::Arc;

use bevy::prelude::Vec2;
use vello::Scene;
use vello::kurbo::Affine;
use vello::peniko::{BlendMode, Fill};
use vello_svg::usvg::{self};

use super::asset::{VelloSvg, VelloSvgLayer};
use crate::integrations::VectorLoaderError;

/// Deserialize an SVG file from bytes.
pub fn load_svg_from_bytes(bytes: &[u8]) -> Result<VelloSvg, VectorLoaderError> {
    let svg_str = std::str::from_utf8(bytes)?;

    // Parse SVG
    let tree =
        usvg::Tree::from_str(svg_str, &usvg::Options::default()).map_err(vello_svg::Error::Svg)?;

    // Process the loaded SVG into Vello-compatible data
    let scene = vello_svg::render_tree(&tree);
    let width = tree.size().width();
    let height = tree.size().height();
    let layers = extract_named_group_layers(&tree, width, height);

    let asset = VelloSvg {
        scene: Arc::new(scene),
        width,
        height,
        alpha: 1.0,
        layers: Arc::new(layers),
    };

    Ok(asset)
}

/// Deserialize an SVG file from a string slice.
pub fn load_svg_from_str(svg_str: &str) -> Result<VelloSvg, VectorLoaderError> {
    let bytes = svg_str.as_bytes();

    load_svg_from_bytes(bytes)
}

fn extract_named_group_layers(tree: &usvg::Tree, svg_width: f32, svg_height: f32) -> Vec<VelloSvgLayer> {
    let mut layers = Vec::new();
    collect_named_group_layers(tree.root(), svg_width, svg_height, &mut layers);
    layers
}

fn collect_named_group_layers(
    group: &usvg::Group,
    svg_width: f32,
    svg_height: f32,
    layers: &mut Vec<VelloSvgLayer>,
) {
    for node in group.children() {
        if let usvg::Node::Group(child) = node {
            if !child.id().is_empty() {
                layers.push(render_named_group_layer(child, svg_width, svg_height));
            }

            collect_named_group_layers(child, svg_width, svg_height, layers);
        }
    }
}

fn render_named_group_layer(group: &usvg::Group, svg_width: f32, svg_height: f32) -> VelloSvgLayer {
    let bounds = group.abs_layer_bounding_box();
    let width = bounds.width();
    let height = bounds.height();
    let center_x = bounds.x() + width * 0.5;
    let center_y = bounds.y() + height * 0.5;

    let mut scene = Scene::new();
    let offset = Affine::translate((-f64::from(bounds.x()), -f64::from(bounds.y())));
    render_group(&mut scene, group, offset, &mut vello_svg::util::default_error_handler);

    VelloSvgLayer {
        id: group.id().to_string(),
        scene: Arc::new(scene),
        width,
        height,
        offset: Vec2::new(center_x - svg_width * 0.5, svg_height * 0.5 - center_y),
        alpha: group.opacity().get(),
    }
}

fn render_group<F: FnMut(&mut Scene, &usvg::Node)>(
    scene: &mut Scene,
    group: &usvg::Group,
    transform: Affine,
    error_handler: &mut F,
) {
    for node in group.children() {
        let transform = transform * vello_svg::util::to_affine(&node.abs_transform());
        match node {
            usvg::Node::Group(child) => {
                let alpha = child.opacity().get();
                let blend_mode: BlendMode = match child.blend_mode() {
                    usvg::BlendMode::Normal => vello::peniko::Mix::Normal.into(),
                    usvg::BlendMode::Multiply => vello::peniko::Mix::Multiply.into(),
                    usvg::BlendMode::Screen => vello::peniko::Mix::Screen.into(),
                    usvg::BlendMode::Overlay => vello::peniko::Mix::Overlay.into(),
                    usvg::BlendMode::Darken => vello::peniko::Mix::Darken.into(),
                    usvg::BlendMode::Lighten => vello::peniko::Mix::Lighten.into(),
                    usvg::BlendMode::ColorDodge => vello::peniko::Mix::ColorDodge.into(),
                    usvg::BlendMode::ColorBurn => vello::peniko::Mix::ColorBurn.into(),
                    usvg::BlendMode::HardLight => vello::peniko::Mix::HardLight.into(),
                    usvg::BlendMode::SoftLight => vello::peniko::Mix::SoftLight.into(),
                    usvg::BlendMode::Difference => vello::peniko::Mix::Difference.into(),
                    usvg::BlendMode::Exclusion => vello::peniko::Mix::Exclusion.into(),
                    usvg::BlendMode::Hue => vello::peniko::Mix::Hue.into(),
                    usvg::BlendMode::Saturation => vello::peniko::Mix::Saturation.into(),
                    usvg::BlendMode::Color => vello::peniko::Mix::Color.into(),
                    usvg::BlendMode::Luminosity => vello::peniko::Mix::Luminosity.into(),
                };

                let clipped = match child
                    .clip_path()
                    .and_then(|path| path.root().children().first())
                {
                    Some(usvg::Node::Path(clip_path)) => {
                        let local_path = vello_svg::util::to_bez_path(clip_path);
                        scene.push_layer(Fill::NonZero, blend_mode, alpha, transform, &local_path);
                        true
                    }
                    _ if child.should_isolate() => {
                        let bounding_box = child.layer_bounding_box();
                        let rect = vello::kurbo::Rect::from_origin_size(
                            (bounding_box.x(), bounding_box.y()),
                            (bounding_box.width() as f64, bounding_box.height() as f64),
                        );
                        scene.push_layer(Fill::NonZero, blend_mode, alpha, transform, &rect);
                        true
                    }
                    _ => false,
                };

                render_group(scene, child, Affine::IDENTITY, error_handler);

                if clipped {
                    scene.pop_layer();
                }
            }
            usvg::Node::Path(path) => {
                if !path.is_visible() {
                    continue;
                }

                let local_path = vello_svg::util::to_bez_path(path);

                if let Some(fill) = path.fill()
                    && let Some((brush, brush_transform)) =
                        vello_svg::util::to_brush(fill.paint(), fill.opacity())
                {
                    scene.fill(
                        match fill.rule() {
                            usvg::FillRule::NonZero => Fill::NonZero,
                            usvg::FillRule::EvenOdd => Fill::EvenOdd,
                        },
                        transform,
                        &brush,
                        Some(brush_transform),
                        &local_path,
                    );
                }

                if let Some(stroke) = path.stroke() {
                    if let Some((brush, brush_transform)) =
                        vello_svg::util::to_brush(stroke.paint(), stroke.opacity())
                    {
                        scene.stroke(
                            &vello_svg::util::to_stroke(stroke),
                            transform,
                            &brush,
                            Some(brush_transform),
                            &local_path,
                        );
                    }
                }
            }
            usvg::Node::Image(image) => match image.kind() {
                usvg::ImageKind::SVG(svg) => render_group(scene, svg.root(), transform, error_handler),
                _ => error_handler(scene, node),
            },
            usvg::Node::Text(text) => {
                render_group(scene, text.flattened(), transform, error_handler);
            }
        }
    }
}
