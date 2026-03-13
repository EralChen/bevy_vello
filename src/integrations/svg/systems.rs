use crate::prelude::*;
use bevy::{
    camera::primitives::Aabb,
    image::ToExtents,
    prelude::*,
    render::render_resource::{
        Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    },
    ui::{ContentSize, NodeMeasure},
};

use super::{VelloSvgLayer2d, VelloUiSvgImage};

fn helper_calculate_aabb(width: f32, height: f32, anchor: &VelloSvgAnchor) -> Aabb {
    let half_size = Vec3::new(width / 2.0, height / 2.0, 0.0);
    let (dx, dy) = {
        match anchor {
            VelloSvgAnchor::TopLeft => (half_size.x, -half_size.y),
            VelloSvgAnchor::Left => (half_size.x, 0.0),
            VelloSvgAnchor::BottomLeft => (half_size.x, half_size.y),
            VelloSvgAnchor::Top => (0.0, -half_size.y),
            VelloSvgAnchor::Center => (0.0, 0.0),
            VelloSvgAnchor::Bottom => (0.0, half_size.y),
            VelloSvgAnchor::TopRight => (-half_size.x, -half_size.y),
            VelloSvgAnchor::Right => (-half_size.x, 0.0),
            VelloSvgAnchor::BottomRight => (-half_size.x, half_size.y),
        }
    };
    let adjustment = Vec3::new(dx, dy, 0.0);
    let min = -half_size + adjustment;
    let max = half_size + adjustment;
    Aabb::from_min_max(min, max)
}

pub fn update_svg_2d_aabb_on_asset_load(
    mut asset_events: MessageReader<AssetEvent<VelloSvg>>,
    mut world_svgs: Query<(&mut Aabb, &VelloSvg2d, &VelloSvgAnchor)>,
    svgs: Res<Assets<VelloSvg>>,
) {
    for event in asset_events.read() {
        let id = if let AssetEvent::LoadedWithDependencies { id } = event {
            *id
        } else {
            continue;
        };
        let Some(svg) = svgs.get(id) else {
            // Not yet loaded
            continue;
        };
        for (mut aabb, _, anchor) in world_svgs.iter_mut().filter(|(_, svg, _)| svg.id() == id) {
            let new_aabb = helper_calculate_aabb(svg.width, svg.height, anchor);
            *aabb = new_aabb;
        }
    }
}

pub fn update_svg_layer_2d_aabb_on_asset_load(
    mut asset_events: MessageReader<AssetEvent<VelloSvg>>,
    mut world_svgs: Query<(&mut Aabb, &VelloSvgLayer2d, &VelloSvgAnchor)>,
    svgs: Res<Assets<VelloSvg>>,
) {
    for event in asset_events.read() {
        let id = if let AssetEvent::LoadedWithDependencies { id } = event {
            *id
        } else {
            continue;
        };
        let Some(svg) = svgs.get(id) else {
            continue;
        };

        for (mut aabb, layer_ref, anchor) in world_svgs.iter_mut().filter(|(_, layer_ref, _)| layer_ref.svg.id() == id) {
            let Some(layer) = svg.layer(&layer_ref.layer) else {
                continue;
            };

            *aabb = helper_calculate_aabb(layer.width, layer.height, anchor);
        }
    }
}

pub fn update_svg_2d_aabb_on_change(
    mut world_svgs: Query<(&mut Aabb, &mut VelloSvg2d, &VelloSvgAnchor), Changed<VelloSvg2d>>,
    svgs: Res<Assets<VelloSvg>>,
) {
    for (mut aabb, svg, anchor) in world_svgs.iter_mut() {
        let Some(svg) = svgs.get(&svg.0) else {
            // Not yet loaded
            continue;
        };
        let new_aabb = helper_calculate_aabb(svg.width, svg.height, anchor);
        *aabb = new_aabb;
    }
}

pub fn update_svg_layer_2d_aabb_on_change(
    mut world_svgs: Query<(&mut Aabb, &VelloSvgLayer2d, &VelloSvgAnchor), Changed<VelloSvgLayer2d>>,
    svgs: Res<Assets<VelloSvg>>,
) {
    for (mut aabb, layer_ref, anchor) in world_svgs.iter_mut() {
        let Some(svg) = svgs.get(&layer_ref.svg) else {
            continue;
        };
        let Some(layer) = svg.layer(&layer_ref.layer) else {
            continue;
        };

        *aabb = helper_calculate_aabb(layer.width, layer.height, anchor);
    }
}

pub fn update_ui_svg_content_size_on_change(
    mut text_q: Query<
        (&mut ContentSize, &ComputedNode, &mut UiVelloSvg),
        (Or<(Changed<UiVelloSvg>, Changed<ComputedNode>)>, Without<VelloUiSvgImage>),
    >,
    svgs: Res<Assets<VelloSvg>>,
) {
    for (mut content_size, node, svg) in text_q.iter_mut() {
        let Some(svg) = svgs.get(&svg.0) else {
            // Not yet loaded
            continue;
        };

        let size = Vec2::new(svg.width, svg.height) / node.inverse_scale_factor();
        let measure = NodeMeasure::Fixed(bevy::ui::FixedMeasure { size });
        content_size.set(measure);
    }
}

/// Creates per-entity render images for `UiVelloSvg` entities and inserts `ImageNode`
/// so the SVG participates in normal Bevy UI z-ordering.
pub fn manage_ui_svg_render_images(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    svgs: Res<Assets<VelloSvg>>,
    query: Query<(
        Entity,
        &UiVelloSvg,
        Option<&VelloUiSvgImage>,
    )>,
) {
    for (entity, svg_handle, existing) in query.iter() {
        let Some(svg) = svgs.get(&svg_handle.0) else {
            continue;
        };

        let target_w = (svg.width.ceil() as u32).max(1);
        let target_h = (svg.height.ceil() as u32).max(1);

        // Skip if already set up with the same SVG handle and matching image size.
        // When the SVG handle changes (e.g. normal→selected card frame), svg_id
        // will differ and a new image is created — the new AssetId is absent from
        // UiSvgRenderCache so it gets re-rendered next frame automatically.
        if let Some(existing) = existing {
            if existing.svg_id == svg_handle.id() {
                if let Some(img) = images.get(existing.image.id()) {
                    let ext = img.size().to_extents();
                    if ext.width == target_w && ext.height == target_h {
                        continue;
                    }
                }
            }
        }

        // Create a render target image
        let image_handle = create_vello_ui_render_image(&mut images, target_w, target_h);
        commands.entity(entity).insert((
            VelloUiSvgImage {
                image: image_handle.clone(),
                svg_id: svg_handle.id(),
            },
            ImageNode::new(image_handle),
        ));
    }
}

fn create_vello_ui_render_image(
    images: &mut Assets<Image>,
    width: u32,
    height: u32,
) -> Handle<Image> {
    let size = Extent3d {
        width,
        height,
        ..default()
    };
    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: None,
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        },
        ..default()
    };
    image.resize(size);
    images.add(image)
}
