use bevy::{
    camera::visibility::RenderLayers,
    prelude::*,
    render::{
        Extract,
        camera::ExtractedCamera,
        render_asset::RenderAssets,
        renderer::{RenderDevice, RenderQueue},
        sync_world::TemporaryRenderEntity,
        texture::GpuImage,
        view::ExtractedView,
    },
};
use kurbo::Affine;
use std::collections::HashSet;
use vello::{RenderParams, Scene};

use super::{VelloSvgAnchor, VelloSvgLayer2d, VelloUiSvgImage, asset::VelloSvg};
use crate::{
    prelude::*,
    render::{VelloEntityCountData, VelloRenderSettings, VelloRenderer, prepare::PreparedAffine},
};

#[derive(Component, Clone)]
pub struct ExtractedVelloSvg2d {
    pub asset: VelloSvg,
    pub asset_anchor: VelloSvgAnchor,
    pub transform: GlobalTransform,
    pub alpha: f32,
}

#[derive(Component, Clone)]
pub struct ExtractedUiVelloSvg {
    pub asset: VelloSvg,
    pub ui_transform: UiGlobalTransform,
    pub alpha: f32,
    pub ui_node: ComputedNode,
    pub render_image: Option<Handle<Image>>,
}

pub fn extract_world_svg_assets(
    mut commands: Commands,
    query_views: Query<
        (&ExtractedCamera, Option<&RenderLayers>),
        (With<Camera2d>, With<VelloView>),
    >,
    query_vectors: Extract<
        Query<
            (
                &VelloSvg2d,
                &VelloSvgAnchor,
                &GlobalTransform,
                Option<&RenderLayers>,
                &ViewVisibility,
                &InheritedVisibility,
            ),
            Without<Node>,
        >,
    >,
    assets: Extract<Res<Assets<VelloSvg>>>,
    mut frame_data: ResMut<VelloEntityCountData>,
) {
    let mut n_svgs = 0;

    // Sort cameras by rendering order
    let mut views: Vec<_> = query_views.iter().collect();
    views.sort_unstable_by_key(|(camera, _)| camera.order);

    for (
        asset_handle,
        asset_anchor,
        transform,
        render_layers,
        view_visibility,
        inherited_visibility,
    ) in query_vectors.iter()
    {
        // Skip if visibility conditions are not met
        if !view_visibility.get() || !inherited_visibility.get() {
            continue;
        }
        // Skip if asset isn't loaded.
        let Some(asset) = assets.get(asset_handle.id()) else {
            continue;
        };

        // Check if any camera renders this asset
        let asset_render_layers = render_layers.unwrap_or_default();
        if views.iter().any(|(_, camera_layers)| {
            asset_render_layers.intersects(camera_layers.unwrap_or_default())
        }) {
            commands
                .spawn(ExtractedVelloSvg2d {
                    asset: asset.to_owned(),
                    transform: *transform,
                    asset_anchor: *asset_anchor,
                    alpha: asset.alpha,
                })
                .insert(TemporaryRenderEntity);
            n_svgs += 1;
        }
    }

    frame_data.n_world_svgs = n_svgs;
}

pub fn extract_world_svg_layer_assets(
    mut commands: Commands,
    query_views: Query<
        (&ExtractedCamera, Option<&RenderLayers>),
        (With<Camera2d>, With<VelloView>),
    >,
    query_vectors: Extract<
        Query<
            (
                &VelloSvgLayer2d,
                &VelloSvgAnchor,
                &GlobalTransform,
                Option<&RenderLayers>,
                &ViewVisibility,
                &InheritedVisibility,
            ),
            Without<Node>,
        >,
    >,
    assets: Extract<Res<Assets<VelloSvg>>>,
    mut frame_data: ResMut<VelloEntityCountData>,
) {
    let mut n_svgs = frame_data.n_world_svgs;

    let mut views: Vec<_> = query_views.iter().collect();
    views.sort_unstable_by_key(|(camera, _)| camera.order);

    for (layer_ref, asset_anchor, transform, render_layers, view_visibility, inherited_visibility) in query_vectors.iter() {
        if !view_visibility.get() || !inherited_visibility.get() {
            continue;
        }

        let Some(asset) = assets.get(layer_ref.svg.id()) else {
            continue;
        };
        let Some(layer) = asset.layer(&layer_ref.layer) else {
            continue;
        };

        let asset_render_layers = render_layers.unwrap_or_default();
        if views.iter().any(|(_, camera_layers)| {
            asset_render_layers.intersects(camera_layers.unwrap_or_default())
        }) {
            commands
                .spawn(ExtractedVelloSvg2d {
                    asset: VelloSvg {
                        scene: layer.scene.clone(),
                        width: layer.width,
                        height: layer.height,
                        alpha: layer.alpha,
                        layers: Default::default(),
                    },
                    transform: *transform,
                    asset_anchor: *asset_anchor,
                    alpha: layer.alpha,
                })
                .insert(TemporaryRenderEntity);
            n_svgs += 1;
        }
    }

    frame_data.n_world_svgs = n_svgs;
}

pub fn extract_ui_svg_assets(
    mut commands: Commands,
    query_views: Query<
        (&ExtractedCamera, Option<&RenderLayers>),
        (With<Camera2d>, With<VelloView>),
    >,
    query_vectors: Extract<
        Query<(
            &UiVelloSvg,
            &UiGlobalTransform,
            &ComputedNode,
            Option<&RenderLayers>,
            &InheritedVisibility,
            Option<&VelloUiSvgImage>,
        )>,
    >,
    assets: Extract<Res<Assets<VelloSvg>>>,
    mut frame_data: ResMut<VelloEntityCountData>,
) {
    let mut n_svgs = 0;

    // Sort cameras by rendering order
    let mut views: Vec<_> = query_views.iter().collect();
    views.sort_unstable_by_key(|(camera, _)| camera.order);

    for (asset_handle, ui_transform, ui_node, render_layers, inherited_visibility, render_image) in
        query_vectors.iter()
    {
        // Skip if visibility conditions are not met.
        // UI does not check view visibility, only inherited visibility.
        if !inherited_visibility.get() {
            continue;
        }
        // Skip if asset isn't loaded.
        let Some(asset) = assets.get(asset_handle.id()) else {
            continue;
        };

        // Check if any camera renders this asset
        let asset_render_layers = render_layers.unwrap_or_default();
        if views.iter().any(|(_, camera_layers)| {
            asset_render_layers.intersects(camera_layers.unwrap_or_default())
        }) {
            commands
                .spawn(ExtractedUiVelloSvg {
                    asset: asset.to_owned(),
                    ui_transform: *ui_transform,
                    ui_node: *ui_node,
                    alpha: asset.alpha,
                    render_image: render_image.map(|r| r.image.clone()),
                })
                .insert(TemporaryRenderEntity);
            n_svgs += 1;
        }
    }

    frame_data.n_ui_svgs = n_svgs;
}

pub fn prepare_asset_affines(
    mut commands: Commands,
    views: Query<(&ExtractedCamera, &ExtractedView), (With<Camera2d>, With<VelloView>)>,
    render_entities: Query<(Entity, &ExtractedVelloSvg2d)>,
    render_ui_entities: Query<(Entity, &ExtractedUiVelloSvg)>,
) {
    for (camera, view) in views.iter() {
        // Render UI
        for (entity, render_entity) in render_ui_entities.iter() {
            let ui_transform = render_entity.ui_transform;

            // A transposed (flipped over its diagonal) PostScript matrix
            // | a c e |
            // | b d f |
            // | 0 0 1 |
            //
            // Components
            // | scale_x skew_x translate_x |
            // | skew_y scale_y translate_y |
            // | skew_z skew_z scale_z |
            //
            // rotate (z)
            // | cos(θ) -sin(θ) translate_x |
            // | sin(θ) cos(θ) translate_y |
            // | skew_z skew_z scale_z |
            //
            // The order of operations is important, as it affects the final transformation matrix.
            //
            // Order of operations:
            // 1. Scale
            // 2. Rotate
            // 3. Translate
            let transform: [f64; 6] = {
                // Convert UiGlobalTransform to Mat4
                let mat2 = ui_transform.matrix2;
                let translation = ui_transform.translation;
                let model_matrix = Mat4::from_cols_array_2d(&[
                    [mat2.x_axis.x, mat2.x_axis.y, 0.0, 0.0],
                    [mat2.y_axis.x, mat2.y_axis.y, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [translation.x, translation.y, 0.0, 1.0],
                ]);
                let local_center_matrix = Transform::from_translation(Vec3 {
                    x: render_entity.asset.width / 2.0,
                    y: render_entity.asset.height / 2.0,
                    z: 0.0,
                })
                .to_matrix()
                .inverse();
                // Fill the bevy_ui Node with the asset size
                let aspect_fill_matrix = {
                    let asset_size =
                        Vec2::new(render_entity.asset.width, render_entity.asset.height);
                    let fill_scale = render_entity.ui_node.size() / asset_size;
                    let scale_factor = fill_scale.x.min(fill_scale.y); // Maintain aspect ratio
                    Mat4::from_scale(Vec3::new(scale_factor, scale_factor, 1.0))
                };

                // Transform chain: ui_transform (in logical px) → aspect_fill → local_center
                let raw_transform = model_matrix * aspect_fill_matrix * local_center_matrix;
                let transform = raw_transform.to_cols_array();
                [
                    transform[0] as f64,  // a // scale_x
                    transform[1] as f64,  // b // skew_y
                    transform[4] as f64,  // c // skew_x
                    transform[5] as f64,  // d // scale_y
                    transform[12] as f64, // e // translate_x
                    transform[13] as f64, // f // translate_y
                ]
            };

            commands
                .entity(entity)
                .insert(PreparedAffine(Affine::new(transform)));
        }

        // Render World
        for (entity, render_entity) in render_entities.iter() {
            // A transposed (flipped over its diagonal) PostScript matrix
            // | a c e |
            // | b d f |
            // | 0 0 1 |
            //
            // Components
            // | scale_x skew_x translate_x |
            // | skew_y scale_y translate_y |
            // | skew_z skew_z scale_z |
            //
            // rotate (z)
            // | cos(θ) -sin(θ) translate_x |
            // | sin(θ) cos(θ) translate_y |
            // | skew_z skew_z scale_z |
            //
            // The order of operations is important, as it affects the final transformation matrix.
            //
            // Order of operations:
            // 1. Scale
            // 2. Rotate
            // 3. Translate
            let transform: [f64; 6] = {
                // Get the base world transform
                let world_transform = render_entity.transform.compute_transform();
                let Transform {
                    translation,
                    rotation,
                    scale,
                } = world_transform;

                // Calculate anchor offset in local space (Vello's top-left origin)
                let anchor_local = match render_entity.asset_anchor {
                    VelloSvgAnchor::TopLeft => Vec3::ZERO,
                    VelloSvgAnchor::Left => Vec3::new(0.0, render_entity.asset.height / 2.0, 0.0),
                    VelloSvgAnchor::BottomLeft => Vec3::new(0.0, render_entity.asset.height, 0.0),
                    VelloSvgAnchor::Top => Vec3::new(render_entity.asset.width / 2.0, 0.0, 0.0),
                    VelloSvgAnchor::Center => Vec3::new(
                        render_entity.asset.width / 2.0,
                        render_entity.asset.height / 2.0,
                        0.0,
                    ),
                    VelloSvgAnchor::Bottom => Vec3::new(
                        render_entity.asset.width / 2.0,
                        render_entity.asset.height,
                        0.0,
                    ),
                    VelloSvgAnchor::TopRight => Vec3::new(render_entity.asset.width, 0.0, 0.0),
                    VelloSvgAnchor::Right => Vec3::new(
                        render_entity.asset.width,
                        render_entity.asset.height / 2.0,
                        0.0,
                    ),
                    VelloSvgAnchor::BottomRight => {
                        Vec3::new(render_entity.asset.width, render_entity.asset.height, 0.0)
                    }
                };
                let mut anchor_matrix = Mat4::from_translation(-anchor_local);
                // The anchor offset is in Vello's y-down coordinate space, but needs to be applied
                // in the transform chain that operates in Bevy's y-up space. This y-flip compensates
                // for the coordinate system difference before the final model_matrix y-flip (below).
                anchor_matrix.w_axis.y *= -1.0;

                let ndc_to_pixels_matrix = {
                    let size_pixels: UVec2 = camera.physical_viewport_size.unwrap();
                    let (pixels_x, pixels_y) = (size_pixels.x as f32, size_pixels.y as f32);
                    Mat4::from_cols_array_2d(&[
                        [pixels_x / 2.0, 0.0, 0.0, pixels_x / 2.0],
                        [0.0, pixels_y / 2.0, 0.0, pixels_y / 2.0],
                        [0.0, 0.0, 1.0, 0.0],
                        [0.0, 0.0, 0.0, 1.0],
                    ])
                    .transpose()
                };
                let view_proj_matrix = {
                    let mut view_mat = view.world_from_view.to_matrix();
                    // Flip Y-axis to match Vello's y-down coordinate space
                    view_mat.w_axis.y *= -1.0;
                    let proj_mat = view.clip_from_view;
                    proj_mat * view_mat.inverse()
                };

                // Build the model matrix with proper anchor handling
                let translation_matrix = Mat4::from_translation(translation);
                let rotation_matrix = Mat4::from_quat(rotation);
                let scale_matrix = Mat4::from_scale(scale);

                // Build model matrix: translate → rotate → scale → world_scale → camera_scale → anchor offset
                let mut model_matrix =
                    translation_matrix * rotation_matrix * scale_matrix * anchor_matrix;

                // Flip Y-axis to match Vello's y-down coordinate space
                model_matrix.w_axis.y *= -1.0;

                // Transform chain: world → world_scale → camera_scale → anchor → y-flip → view → projection → NDC → pixels
                let raw_transform = ndc_to_pixels_matrix * view_proj_matrix * model_matrix;
                let transform = raw_transform.to_cols_array();

                // Negate skew_x and skew_y to match rotation of the Bevy's y-up world
                [
                    transform[0] as f64,  // a // scale_x
                    -transform[1] as f64, // b // skew_y
                    -transform[4] as f64, // c // skew_x
                    transform[5] as f64,  // d // scale_y
                    transform[12] as f64, // e // translate_x
                    transform[13] as f64, // f // translate_y
                ]
            };

            commands
                .entity(entity)
                .insert(PreparedAffine(Affine::new(transform)));
        }
    }
}

/// Cache tracking which render images have already been rendered.
/// Keyed by image `AssetId` — when the main world creates a new image
/// (due to resize or asset change), the new ID won't be in the cache
/// and the SVG will be re-rendered.
#[derive(Resource, Default)]
pub struct UiSvgRenderCache(HashSet<AssetId<Image>>);

/// Renders each `UiVelloSvg` that has a per-entity render image to its own GPU texture.
/// Skips already-rendered images (static SVGs only need one render).
pub fn render_ui_svgs_to_textures(
    gpu_images: Res<RenderAssets<GpuImage>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    renderer: Res<VelloRenderer>,
    render_settings: Res<VelloRenderSettings>,
    ui_svgs: Query<&ExtractedUiVelloSvg>,
    mut cache: ResMut<UiSvgRenderCache>,
) {
    for svg in ui_svgs.iter() {
        let Some(ref render_image_handle) = svg.render_image else {
            continue;
        };

        // Skip if already rendered this image
        if cache.0.contains(&render_image_handle.id()) {
            continue;
        }

        let Some(gpu_image) = gpu_images.get(render_image_handle.id()) else {
            continue;
        };

        if svg.alpha <= 0.0 {
            cache.0.insert(render_image_handle.id());
            continue;
        }

        let mut scene = Scene::new();

        // Scale SVG to fit the render image while maintaining aspect ratio
        let scale_x = gpu_image.size.width as f64 / svg.asset.width as f64;
        let scale_y = gpu_image.size.height as f64 / svg.asset.height as f64;
        let scale = scale_x.min(scale_y);
        let affine = Affine::scale(scale);

        if svg.alpha < 1.0 {
            scene.push_layer(
                vello::peniko::Fill::NonZero,
                vello::peniko::Mix::Normal,
                svg.alpha,
                affine,
                &vello::kurbo::Rect::new(
                    0.0,
                    0.0,
                    svg.asset.width as f64,
                    svg.asset.height as f64,
                ),
            );
        }
        scene.append(&svg.asset.scene, Some(affine));
        if svg.alpha < 1.0 {
            scene.pop_layer();
        }

        renderer
            .lock()
            .unwrap()
            .render_to_texture(
                device.wgpu_device(),
                &queue,
                &scene,
                &gpu_image.texture_view,
                &RenderParams {
                    base_color: vello::peniko::Color::TRANSPARENT,
                    width: gpu_image.size.width,
                    height: gpu_image.size.height,
                    antialiasing_method: render_settings.antialiasing,
                },
            )
            .unwrap();

        cache.0.insert(render_image_handle.id());
    }
}
