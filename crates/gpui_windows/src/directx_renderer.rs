use std::{
    cell::Cell,
    rc::Rc,
    slice,
    sync::{Arc, OnceLock},
};

use anyhow::{Context, Result};
use gpui_util::ResultExt;
use windows::{
    Win32::{
        Foundation::HWND,
        Graphics::{
            Direct3D::*,
            Direct3D11::*,
            DirectComposition::*,
            DirectWrite::*,
            Dxgi::{Common::*, *},
        },
    },
    core::{HSTRING, Interface},
};

use crate::directx_renderer::shader_resources::{RawShaderBytes, ShaderModule, ShaderTarget};
use crate::*;
use gpui::*;

pub(crate) const DISABLE_DIRECT_COMPOSITION: &str = "GPUI_DISABLE_DIRECT_COMPOSITION";
const RENDER_TARGET_FORMAT: DXGI_FORMAT = DXGI_FORMAT_B8G8R8A8_UNORM;
// This configuration is used for MSAA rendering on paths only, and it's guaranteed to be supported by DirectX 11.
const PATH_MULTISAMPLE_COUNT: u32 = 4;
const MAX_INSTANCE_BUFFER_SIZE: usize = 256 * 1024 * 1024;
/// How many full-viewport blur passes all the glass surfaces in one frame may
/// spend between them. How many one radius needs is
/// [`BackdropGlass::gaussian_pass_count`], which this renderer shares with
/// WGPU; how many a frame may afford is this renderer's own, because it is a
/// property of drawing them one at a time into the swap chain.
/// Sized so a scene holding [`MAX_LUMINANCE_PROBES`] surfaces at the themes'
/// standard blur — twelve blur passes each at 2x scale — still fits with room
/// to spare; a surface past the budget keeps its optics and its probe but
/// loses the blur.
const MAX_BACKDROP_GLASS_PASSES_PER_FRAME: usize = 256;

pub(crate) struct FontInfo {
    pub gamma_ratios: [f32; 4],
    pub grayscale_enhanced_contrast: f32,
    pub subpixel_enhanced_contrast: f32,
    pub is_bgr: bool,
}

pub(crate) struct DirectXRenderer {
    hwnd: HWND,
    atlas: Arc<DirectXAtlas>,
    devices: Option<DirectXRendererDevices>,
    resources: Option<DirectXResources>,
    overlay_resources: Option<OverlayResources>,
    globals: DirectXGlobalElements,
    pipelines: DirectXRenderPipelines,
    direct_composition: Option<DirectComposition>,
    font_info: &'static FontInfo,

    width: u32,
    height: u32,

    /// Whether we want to skip drwaing due to device lost events.
    ///
    /// In that case we want to discard the first frame that we draw as we got reset in the middle of a frame
    /// meaning we lost all the allocated gpu textures and scene resources.
    skip_draws: bool,

    /// The staging texture luminance probe texels are copied into, one row of
    /// [`LUMINANCE_PROBE_SAMPLES`] texels per slot, mapped a frame later.
    probe_staging: Option<ID3D11Texture2D>,
    /// The slots the frame currently being drawn copies probes for.
    probe_requests: Vec<u32>,
    /// The slots the previous frame copied, awaiting a map that never waits.
    probe_pending: Vec<u32>,
    probe_values: [Option<f32>; MAX_LUMINANCE_PROBES],
}

/// Direct3D objects
#[derive(Clone)]
pub(crate) struct DirectXRendererDevices {
    pub(crate) adapter: IDXGIAdapter1,
    pub(crate) dxgi_factory: IDXGIFactory6,
    pub(crate) device: ID3D11Device,
    pub(crate) device_context: ID3D11DeviceContext,
    dxgi_device: Option<IDXGIDevice>,
    annotation: Option<ID3DUserDefinedAnnotation>,
}

struct DirectXResources {
    // Direct3D rendering objects
    swap_chain: IDXGISwapChain1,
    render_target: Option<ID3D11Texture2D>,
    render_target_view: Option<ID3D11RenderTargetView>,

    // Path intermediate textures (with MSAA)
    path_intermediate_texture: ID3D11Texture2D,
    path_intermediate_srv: Option<ID3D11ShaderResourceView>,
    path_intermediate_msaa_texture: ID3D11Texture2D,
    path_intermediate_msaa_view: Option<ID3D11RenderTargetView>,

    // Backdrop glass scratch. `snapshot` is the copy of the render target the
    // blur reads, and the two scratch textures are ping-ponged through the
    // separable gaussian's axes. All three are full viewport: a glass surface
    // samples outside its own bounds, both because the blur's kernel reaches
    // past the rim and because refraction displaces the sample further.
    backdrop_snapshot: ID3D11Texture2D,
    backdrop_snapshot_srv: Option<ID3D11ShaderResourceView>,
    backdrop_scratch: [ID3D11Texture2D; 2],
    backdrop_scratch_srv: [Option<ID3D11ShaderResourceView>; 2],
    backdrop_scratch_view: [Option<ID3D11RenderTargetView>; 2],

    // Cached viewport
    viewport: D3D11_VIEWPORT,
}

struct OverlayResources {
    swap_chain: IDXGISwapChain1,
    render_target: Option<ID3D11Texture2D>,
    render_target_view: Option<ID3D11RenderTargetView>,
}

struct DirectXRenderPipelines {
    shadow_pipeline: PipelineState<Shadow>,
    quad_pipeline: PipelineState<Quad>,
    path_rasterization_pipeline: PipelineState<PathRasterizationSprite>,
    path_sprite_pipeline: PipelineState<PathSprite>,
    underline_pipeline: PipelineState<Underline>,
    mono_sprites: PipelineState<MonochromeSprite>,
    subpixel_sprites: PipelineState<SubpixelSprite>,
    poly_sprites: PipelineState<PolychromeSprite>,
    poly_additive_blend: ID3D11BlendState,
    poly_screen_blend: ID3D11BlendState,
    // The two backdrop passes carry no instance buffer: each draws one full
    // viewport strip and reads everything it needs from `b2`, so they are a
    // shader pair and a blend state rather than a `PipelineState`.
    backdrop_blur: BackdropPipeline,
    backdrop_glass: BackdropPipeline,
}

/// A shader pair that draws one full-viewport strip.
struct BackdropPipeline {
    vertex: ID3D11VertexShader,
    fragment: ID3D11PixelShader,
    blend_state: ID3D11BlendState,
}

impl BackdropPipeline {
    fn new(
        device: &ID3D11Device,
        shader_module: ShaderModule,
        blend_state: ID3D11BlendState,
    ) -> Result<Self> {
        let vertex = {
            let raw_shader = RawShaderBytes::new(shader_module, ShaderTarget::Vertex)?;
            create_vertex_shader(device, raw_shader.as_bytes())?
        };
        let fragment = {
            let raw_shader = RawShaderBytes::new(shader_module, ShaderTarget::Fragment)?;
            create_fragment_shader(device, raw_shader.as_bytes())?
        };
        Ok(Self {
            vertex,
            fragment,
            blend_state,
        })
    }

    fn draw(
        &self,
        device_context: &ID3D11DeviceContext,
        source: &[Option<ID3D11ShaderResourceView>],
        sampler: &[Option<ID3D11SamplerState>],
    ) {
        unsafe {
            device_context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
            device_context.VSSetShader(&self.vertex, None);
            device_context.PSSetShader(&self.fragment, None);
            device_context.OMSetBlendState(&self.blend_state, None, 0xFFFFFFFF);
            device_context.PSSetSamplers(0, Some(sampler));
            device_context.PSSetShaderResources(0, Some(source));
            device_context.DrawInstanced(4, 1, 0, 0);
        }
    }
}

struct DirectXGlobalElements {
    global_params_buffer: Option<ID3D11Buffer>,
    batch_params_buffer: Option<ID3D11Buffer>,
    backdrop_params_buffer: Option<ID3D11Buffer>,
    sampler: Option<ID3D11SamplerState>,
    /// Clamping, so a blur tap or a refracted sample that reaches past the
    /// edge of the viewport reads the edge rather than wrapping to the far
    /// side, which the wrapping sprite sampler would do.
    backdrop_sampler: Option<ID3D11SamplerState>,
}

struct Annotation<'a>(&'a ID3DUserDefinedAnnotation);

impl<'a> Annotation<'a> {
    fn new(annotation: &'a ID3DUserDefinedAnnotation, label: HSTRING) -> Self {
        unsafe { annotation.BeginEvent(&label) };
        Self(annotation)
    }
}

impl Drop for Annotation<'_> {
    fn drop(&mut self) {
        unsafe { self.0.EndEvent() };
    }
}

struct DirectComposition {
    comp_device: IDCompositionDevice,
    // Keep these COM objects alive for the lifetime of the visual tree. They
    // are not otherwise read after the tree is attached to the target.
    #[allow(dead_code)]
    comp_target: IDCompositionTarget,
    #[allow(dead_code)]
    root_visual: IDCompositionVisual,
    base_visual: IDCompositionVisual,
    portal_container: IDCompositionVisual,
    overlay_visual: IDCompositionVisual,
}

struct DirectCompositionPortal {
    comp_device: IDCompositionDevice,
    container: IDCompositionVisual,
    visual: IDCompositionVisual,
    clip: IDCompositionRectangleClip,
    visible: Cell<bool>,
}

impl DirectXRendererDevices {
    pub(crate) fn new(
        directx_devices: &DirectXDevices,
        disable_direct_composition: bool,
    ) -> Result<Self> {
        let DirectXDevices {
            adapter,
            dxgi_factory,
            device,
            device_context,
        } = directx_devices;
        let dxgi_device = if disable_direct_composition {
            None
        } else {
            Some(device.cast().context("Creating DXGI device")?)
        };
        let annotation = device_context.cast().ok();

        Ok(Self {
            adapter: adapter.clone(),
            dxgi_factory: dxgi_factory.clone(),
            device: device.clone(),
            device_context: device_context.clone(),
            dxgi_device,
            annotation,
        })
    }
}

impl DirectXRenderer {
    pub(crate) fn new(
        hwnd: HWND,
        directx_devices: &DirectXDevices,
        disable_direct_composition: bool,
    ) -> Result<Self> {
        if disable_direct_composition {
            log::info!("Direct Composition is disabled.");
        }

        let devices = DirectXRendererDevices::new(directx_devices, disable_direct_composition)
            .context("Creating DirectX devices")?;
        let atlas = Arc::new(DirectXAtlas::new(&devices.device, &devices.device_context));

        let resources = DirectXResources::new(&devices, 1, 1, hwnd, disable_direct_composition)
            .context("Creating DirectX resources")?;
        let globals = DirectXGlobalElements::new(&devices.device)
            .context("Creating DirectX global elements")?;
        let pipelines = DirectXRenderPipelines::new(&devices.device)
            .context("Creating DirectX render pipelines")?;

        let direct_composition = if disable_direct_composition {
            None
        } else {
            let composition = DirectComposition::new(
                devices
                    .dxgi_device
                    .as_ref()
                    .expect("required framework invariant must hold"),
                hwnd,
            )
            .context("Creating DirectComposition")?;
            composition
                .set_swap_chain(&resources.swap_chain)
                .context("Setting swap chain for DirectComposition")?;
            Some(composition)
        };

        Ok(DirectXRenderer {
            hwnd,
            atlas,
            devices: Some(devices),
            resources: Some(resources),
            overlay_resources: None,
            globals,
            pipelines,
            direct_composition,
            font_info: Self::get_font_info(),
            width: 1,
            height: 1,
            skip_draws: false,
            probe_staging: None,
            probe_requests: Vec::new(),
            probe_pending: Vec::new(),
            probe_values: [None; MAX_LUMINANCE_PROBES],
        })
    }

    pub(crate) fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }

    fn pre_draw(
        &self,
        render_target_view: &Option<ID3D11RenderTargetView>,
        clear_color: &[f32; 4],
    ) -> Result<()> {
        let resources = self.resources.as_ref().expect("resources missing");
        let device_context = &self
            .devices
            .as_ref()
            .expect("devices missing")
            .device_context;
        update_buffer(
            device_context,
            self.globals
                .global_params_buffer
                .as_ref()
                .expect("required framework invariant must hold"),
            &[GlobalParams {
                gamma_ratios: self.font_info.gamma_ratios,
                viewport_size: [resources.viewport.Width, resources.viewport.Height],
                grayscale_enhanced_contrast: self.font_info.grayscale_enhanced_contrast,
                subpixel_enhanced_contrast: self.font_info.subpixel_enhanced_contrast,
                is_bgr: self.font_info.is_bgr as u32,
                _pad: [0; 3],
            }],
        )?;
        unsafe {
            device_context.ClearRenderTargetView(
                render_target_view
                    .as_ref()
                    .context("missing render target view")?,
                clear_color,
            );
            device_context.OMSetRenderTargets(Some(slice::from_ref(render_target_view)), None);
            device_context.RSSetViewports(Some(slice::from_ref(&resources.viewport)));
            device_context
                .VSSetConstantBuffers(0, Some(slice::from_ref(&self.globals.global_params_buffer)));
            device_context
                .VSSetConstantBuffers(1, Some(slice::from_ref(&self.globals.batch_params_buffer)));
            device_context
                .PSSetConstantBuffers(0, Some(slice::from_ref(&self.globals.global_params_buffer)));
        }
        Ok(())
    }

    #[inline]
    fn present(&mut self) -> Result<()> {
        let result = unsafe {
            self.resources
                .as_ref()
                .expect("resources missing")
                .swap_chain
                .Present(0, DXGI_PRESENT(0))
        };
        result.ok().context("Presenting swap chain failed")
    }

    pub(crate) fn handle_device_lost(&mut self, directx_devices: &DirectXDevices) -> Result<()> {
        try_to_recover_from_device_lost(|| {
            self.handle_device_lost_impl(directx_devices)
                .context("DirectXRenderer handling device lost")
        })
    }

    fn handle_device_lost_impl(&mut self, directx_devices: &DirectXDevices) -> Result<()> {
        let disable_direct_composition = self.direct_composition.is_none();
        let overlay_enabled = self.overlay_resources.is_some();

        unsafe {
            #[cfg(debug_assertions)]
            if let Some(devices) = &self.devices {
                report_live_objects(&devices.device)
                    .context("Failed to report live objects after device lost")
                    .log_err();
            }

            self.resources.take();
            self.overlay_resources.take();
            if let Some(devices) = &self.devices {
                devices.device_context.OMSetRenderTargets(None, None);
                devices.device_context.ClearState();
                devices.device_context.Flush();
                #[cfg(debug_assertions)]
                report_live_objects(&devices.device)
                    .context("Failed to report live objects after device lost")
                    .log_err();
            }

            self.direct_composition.take();
            self.devices.take();
        }

        let devices = DirectXRendererDevices::new(directx_devices, disable_direct_composition)
            .context("Recreating DirectX devices")?;
        let resources = DirectXResources::new(
            &devices,
            self.width,
            self.height,
            self.hwnd,
            disable_direct_composition,
        )
        .context("Creating DirectX resources")?;
        let globals = DirectXGlobalElements::new(&devices.device)
            .context("Creating DirectXGlobalElements")?;
        let pipelines = DirectXRenderPipelines::new(&devices.device)
            .context("Creating DirectXRenderPipelines")?;

        let direct_composition = if disable_direct_composition {
            None
        } else {
            let composition = DirectComposition::new(
                devices
                    .dxgi_device
                    .as_ref()
                    .expect("required framework invariant must hold"),
                self.hwnd,
            )?;
            composition.set_swap_chain(&resources.swap_chain)?;
            Some(composition)
        };
        let overlay_resources = if overlay_enabled {
            let overlay = OverlayResources::new(&devices, self.width, self.height)?;
            direct_composition
                .as_ref()
                .context("DirectComposition missing for overlay")?
                .set_overlay_swap_chain(&overlay.swap_chain)?;
            Some(overlay)
        } else {
            None
        };

        self.atlas
            .handle_device_lost(&devices.device, &devices.device_context);

        unsafe {
            devices
                .device_context
                .OMSetRenderTargets(Some(slice::from_ref(&resources.render_target_view)), None);
        }
        self.devices = Some(devices);
        self.resources = Some(resources);
        self.overlay_resources = overlay_resources;
        self.globals = globals;
        self.pipelines = pipelines;
        self.direct_composition = direct_composition;
        self.skip_draws = true;
        Ok(())
    }

    pub(crate) fn draw(
        &mut self,
        scene: &Scene,
        background_appearance: WindowBackgroundAppearance,
    ) -> Result<()> {
        if self.skip_draws {
            // skip drawing this frame, we just recovered from a device lost event
            // and so likely do not have the textures anymore that are required for drawing
            return Ok(());
        }
        let render_target_view = self
            .resources
            .as_ref()
            .context("resources missing")?
            .render_target_view
            .clone();
        self.pre_draw(
            &render_target_view,
            &match background_appearance {
                WindowBackgroundAppearance::Opaque => [1.0f32; 4],
                _ => [0.0f32; 4],
            },
        )?;
        self.draw_scene(scene)?;
        self.present()
    }

    /// Draw the scene and read the result back, without presenting it.
    ///
    /// This is what makes the DirectX renderer testable: everything below it,
    /// including the backdrop passes, runs exactly as it does for a real
    /// frame, and the pixels come back rather than going to the display. The
    /// frame is not presented, so the window keeps whatever it was showing.
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn render_to_image(&mut self, scene: &Scene) -> Result<image::RgbaImage> {
        anyhow::ensure!(
            !self.skip_draws,
            "the renderer is recovering from a lost device and has no textures to draw with"
        );
        let render_target_view = self
            .resources
            .as_ref()
            .context("resources missing")?
            .render_target_view
            .clone();
        self.pre_draw(&render_target_view, &[0.0f32; 4])?;
        self.draw_scene(scene)?;

        let devices = self.devices.as_ref().context("devices missing")?;
        let resources = self.resources.as_ref().context("resources missing")?;
        let render_target = resources
            .render_target
            .as_ref()
            .context("missing render target")?;
        let (width, height) = (self.width.max(1), self.height.max(1));

        // The render target lives in device memory the CPU cannot map, so the
        // readback goes through a staging texture, which is the only usage
        // D3D11 will let both the GPU write to and the CPU read.
        let staging = unsafe {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: RENDER_TARGET_FORMAT,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };
            let mut output = None;
            devices
                .device
                .CreateTexture2D(&desc, None, Some(&mut output))?;
            output.context("failed to create the readback texture")?
        };

        let mut image = image::RgbaImage::new(width, height);
        unsafe {
            devices.device_context.CopyResource(&staging, render_target);
            let mut mapped = std::mem::zeroed();
            devices
                .device_context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
            let pitch = mapped.RowPitch as usize;
            let source =
                std::slice::from_raw_parts(mapped.pData as *const u8, pitch * height as usize);
            for y in 0..height as usize {
                let row = &source[y * pitch..y * pitch + width as usize * 4];
                for x in 0..width as usize {
                    let pixel = &row[x * 4..x * 4 + 4];
                    // The render target is BGRA and the image is RGBA.
                    image.put_pixel(
                        x as u32,
                        y as u32,
                        image::Rgba([pixel[2], pixel[1], pixel[0], pixel[3]]),
                    );
                }
            }
            devices.device_context.Unmap(&staging, 0);
        }
        Ok(image)
    }

    pub(crate) fn draw_layered(
        &mut self,
        scene: &Scene,
        overlay_start: usize,
        background_appearance: WindowBackgroundAppearance,
    ) -> Result<()> {
        if self.overlay_resources.is_none() {
            return self.draw(scene, background_appearance);
        }
        if self.skip_draws {
            return Ok(());
        }

        let split = overlay_start.min(scene.len());
        let mut base_scene = Scene::default();
        base_scene.replay(0..split, scene);
        base_scene.finish();
        let mut overlay_scene = Scene::default();
        overlay_scene.replay(split..scene.len(), scene);
        overlay_scene.finish();

        let base_view = self
            .resources
            .as_ref()
            .context("resources missing")?
            .render_target_view
            .clone();
        self.pre_draw(
            &base_view,
            &match background_appearance {
                WindowBackgroundAppearance::Opaque => [1.0; 4],
                _ => [0.0; 4],
            },
        )?;
        self.draw_scene(&base_scene)?;

        let overlay_view = self
            .overlay_resources
            .as_ref()
            .context("overlay resources missing")?
            .render_target_view
            .clone();
        self.pre_draw(&overlay_view, &[0.0; 4])?;
        self.draw_scene(&overlay_scene)?;

        unsafe {
            self.resources
                .as_ref()
                .context("resources missing")?
                .swap_chain
                .Present(0, DXGI_PRESENT(0))
                .ok()
                .context("presenting base swap chain")?;
            self.overlay_resources
                .as_ref()
                .context("overlay resources missing")?
                .swap_chain
                .Present(0, DXGI_PRESENT(0))
                .ok()
                .context("presenting overlay swap chain")?;
        }
        Ok(())
    }

    pub(crate) fn enable_scene_overlay(&mut self) -> Result<()> {
        if self.overlay_resources.is_some() {
            return Ok(());
        }
        let devices = self.devices.as_ref().context("devices missing")?;
        let overlay = OverlayResources::new(devices, self.width, self.height)?;
        self.direct_composition
            .as_ref()
            .context("DirectComposition is disabled")?
            .set_overlay_swap_chain(&overlay.swap_chain)?;
        self.overlay_resources = Some(overlay);
        Ok(())
    }

    pub(crate) fn create_native_surface(&mut self) -> Result<Rc<dyn PlatformNativeSurface>> {
        self.enable_scene_overlay()?;
        Ok(Rc::new(
            self.direct_composition
                .as_ref()
                .context("DirectComposition is disabled")?
                .create_portal()?,
        ))
    }

    fn draw_scene(&mut self, scene: &Scene) -> Result<()> {
        self.collect_probes();
        self.upload_scene_buffers(scene)?;
        let annotation = self
            .devices
            .as_ref()
            .and_then(|devices| devices.annotation.clone())
            .filter(|annotation| unsafe { annotation.GetStatus().as_bool() });
        // A glass surface reads everything painted below its order, so it is
        // drawn between the batches rather than as one of them, on the same
        // schedule the WGPU renderer uses.
        let mut pending_glass = scene.backdrop_glass.iter().peekable();
        let mut remaining_backdrop_passes = MAX_BACKDROP_GLASS_PASSES_PER_FRAME;
        for batch in scene.batches() {
            let batch_order = backdrop_batch_first_order(scene, &batch);
            while pending_glass
                .peek()
                .is_some_and(|glass| glass.order <= batch_order)
            {
                let Some(glass) = pending_glass.next() else {
                    break;
                };
                self.draw_backdrop_glass(glass, &mut remaining_backdrop_passes)?;
            }
            let _annotation = annotation
                .as_ref()
                .map(|annotation| Annotation::new(annotation, HSTRING::from(batch.label())));
            match batch {
                PrimitiveBatch::Shadows(range) => self.draw_shadows(range.start, range.len()),
                PrimitiveBatch::Quads(range) => self.draw_quads(range.start, range.len()),
                PrimitiveBatch::Paths(range) => {
                    let paths = &scene.paths[range];
                    self.draw_paths_to_intermediate(paths)?;
                    self.draw_paths_from_intermediate(paths)
                }
                PrimitiveBatch::Underlines(range) => self.draw_underlines(range.start, range.len()),
                PrimitiveBatch::MonochromeSprites { texture_id, range } => {
                    self.draw_monochrome_sprites(texture_id, range.start, range.len())
                }
                PrimitiveBatch::SubpixelSprites { texture_id, range } => {
                    self.draw_subpixel_sprites(texture_id, range.start, range.len())
                }
                PrimitiveBatch::PolychromeSprites {
                    texture_id,
                    blend_mode,
                    range,
                } => {
                    self.draw_polychrome_sprites(
                        texture_id,
                        blend_mode,
                        range.start,
                        range.len(),
                    )
                }
                PrimitiveBatch::Surfaces(range) => self.draw_surfaces(&scene.surfaces[range]),
            }
            .with_context(|| {
                format!(
                    "scene too large:\
                    {} paths, {} shadows, {} quads, {} underlines, {} mono, {} subpixel, {} poly, {} surfaces",
                    scene.paths.len(),
                    scene.shadows.len(),
                    scene.quads.len(),
                    scene.underlines.len(),
                    scene.monochrome_sprites.len(),
                    scene.subpixel_sprites.len(),
                    scene.polychrome_sprites.len(),
                    scene.surfaces.len(),
                )
            })?;
        }
        // A surface ordered above everything painted has no batch after it to
        // trigger on, and still has a backdrop.
        for glass in pending_glass {
            self.draw_backdrop_glass(glass, &mut remaining_backdrop_passes)?;
        }
        if !self.probe_requests.is_empty() {
            self.probe_pending = std::mem::take(&mut self.probe_requests);
        }
        Ok(())
    }

    /// Snapshot what has been painted, blur it, and paint it back through the
    /// surface's shape and material.
    ///
    /// A surface whose blur needs more passes than the frame has left keeps
    /// its unblurred backdrop rather than being skipped, which is the same
    /// refusal the other renderers make and the reason it is a fallback
    /// rather than a hole: a legible surface beats a missing one.
    fn draw_backdrop_glass(
        &mut self,
        glass: &BackdropGlass,
        remaining_passes: &mut usize,
    ) -> Result<()> {
        let probe_slot = glass.material.probe;
        let takes_probe =
            probe_slot != NO_LUMINANCE_PROBE && (probe_slot as usize) < MAX_LUMINANCE_PROBES;
        if takes_probe {
            self.ensure_probe_staging()?;
        }
        let pass_count = glass.gaussian_pass_count().unwrap_or(0);
        let requested = pass_count as usize * 2;
        let pass_count = if pass_count > 0 && requested <= *remaining_passes {
            *remaining_passes -= requested;
            pass_count
        } else {
            0
        };

        let devices = self.devices.as_ref().context("devices missing")?;
        let resources = self.resources.as_ref().context("resources missing")?;
        let device_context = &devices.device_context;
        let render_target = resources
            .render_target
            .as_ref()
            .context("missing render target")?;
        let backdrop_params_buffer = self
            .globals
            .backdrop_params_buffer
            .as_ref()
            .context("backdrop params buffer missing")?;
        let sampler = slice::from_ref(&self.globals.backdrop_sampler);

        let mut params = BackdropGlassParams::from_glass(glass);

        unsafe {
            // Take the backdrop out of the render target. Everything painted
            // below this order is in it, and nothing above it has been drawn.
            device_context.CopyResource(&resources.backdrop_snapshot, render_target);
            device_context.RSSetViewports(Some(slice::from_ref(&resources.viewport)));
            device_context.PSSetConstantBuffers(
                2,
                Some(slice::from_ref(&self.globals.backdrop_params_buffer)),
            );
        }

        // The variances of several passes add, so a wide blur is several
        // narrow ones. `read` names the texture the next pass samples, and
        // `blurred` the texture behind it, which the luminance probe copies
        // its sample texels out of.
        let mut read = &resources.backdrop_snapshot_srv;
        let mut blurred = &resources.backdrop_snapshot;
        if pass_count > 0 {
            params.sigma = glass.blur_radius.0.max(1.0) / (pass_count as f32).sqrt();
            for _ in 0..pass_count {
                for (index, direction) in [[1.0, 0.0], [0.0, 1.0]].into_iter().enumerate() {
                    params.direction = direction;
                    update_buffer(device_context, backdrop_params_buffer, &[params])?;
                    unsafe {
                        // The texture being drawn to cannot also be bound for
                        // reading, and the previous pass left it bound.
                        device_context.PSSetShaderResources(0, Some(&[None]));
                        device_context.OMSetRenderTargets(
                            Some(slice::from_ref(&resources.backdrop_scratch_view[index])),
                            None,
                        );
                    }
                    self.pipelines.backdrop_blur.draw(
                        device_context,
                        slice::from_ref(read),
                        sampler,
                    );
                    read = &resources.backdrop_scratch_srv[index];
                    blurred = &resources.backdrop_scratch[index];
                }
            }
        }

        if takes_probe && let Some(staging) = &self.probe_staging {
            let points = glass.probe_sample_points(self.width as f32, self.height as f32);
            for (index, [x, y]) in points.into_iter().enumerate() {
                let source = D3D11_BOX {
                    left: x as u32,
                    top: y as u32,
                    front: 0,
                    right: x as u32 + 1,
                    bottom: y as u32 + 1,
                    back: 1,
                };
                unsafe {
                    device_context.CopySubresourceRegion(
                        staging,
                        0,
                        index as u32,
                        probe_slot,
                        0,
                        blurred,
                        0,
                        Some(&source),
                    );
                }
            }
        }

        // The composite reads the blurred backdrop and writes the surface
        // back into the render target it was taken from.
        params.direction = [0.0, 0.0];
        update_buffer(device_context, backdrop_params_buffer, &[params])?;
        unsafe {
            device_context.PSSetShaderResources(0, Some(&[None]));
            device_context
                .OMSetRenderTargets(Some(slice::from_ref(&resources.render_target_view)), None);
        }
        self.pipelines
            .backdrop_glass
            .draw(device_context, slice::from_ref(read), sampler);

        unsafe {
            device_context.PSSetShaderResources(0, Some(&[None]));
        }
        if takes_probe {
            self.probe_requests.push(probe_slot);
        }
        Ok(())
    }

    /// The staging texture the probe texels land in, created on first use and
    /// kept: its size depends only on the probe capacity, never the window's.
    fn ensure_probe_staging(&mut self) -> Result<()> {
        if self.probe_staging.is_some() {
            return Ok(());
        }
        let devices = self.devices.as_ref().context("devices missing")?;
        let staging = unsafe {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: LUMINANCE_PROBE_SAMPLES as u32,
                Height: MAX_LUMINANCE_PROBES as u32,
                MipLevels: 1,
                ArraySize: 1,
                Format: RENDER_TARGET_FORMAT,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };
            let mut output = None;
            devices
                .device
                .CreateTexture2D(&desc, None, Some(&mut output))?;
            output.context("failed to create the probe staging texture")?
        };
        self.probe_staging = Some(staging);
        Ok(())
    }

    /// Fold the previous frame's probe texels into the slot values.
    ///
    /// The map waits for the frame that wrote the staging texture if the GPU
    /// is still on it: the copy is already submitted, so the wait is bounded
    /// by work the renderer had to finish anyway, and on a slow adapter a
    /// reading that silently stayed one more frame behind would make the
    /// flip land at a different frame per machine.
    fn collect_probes(&mut self) {
        if self.probe_pending.is_empty() {
            return;
        }
        let Some(devices) = self.devices.as_ref() else {
            return;
        };
        let Some(staging) = self.probe_staging.as_ref() else {
            return;
        };
        let mut values = self.probe_values;
        let read = unsafe {
            let mut mapped = std::mem::zeroed();
            if devices
                .device_context
                .Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .is_err()
            {
                false
            } else {
                let pitch = mapped.RowPitch as usize;
                let data = std::slice::from_raw_parts(
                    mapped.pData as *const u8,
                    pitch * MAX_LUMINANCE_PROBES,
                );
                for &slot in &self.probe_pending {
                    let row = &data[slot as usize * pitch..];
                    let mut total = 0.0;
                    for index in 0..LUMINANCE_PROBE_SAMPLES {
                        // The render target, and so every texel here, is BGRA.
                        let texel = &row[index * 4..index * 4 + 4];
                        total += probe_sample_luminance(
                            texel[2] as f32 / 255.0,
                            texel[1] as f32 / 255.0,
                            texel[0] as f32 / 255.0,
                        );
                    }
                    values[slot as usize] = Some(total / LUMINANCE_PROBE_SAMPLES as f32);
                }
                devices.device_context.Unmap(staging, 0);
                true
            }
        };
        if read {
            self.probe_values = values;
            self.probe_pending.clear();
        }
    }

    /// The luminance the most recently completed frame read for this slot.
    pub(crate) fn backdrop_luminance(&mut self, slot: u32) -> Option<f32> {
        self.collect_probes();
        *self.probe_values.get(slot as usize)?
    }

    pub(crate) fn resize(&mut self, new_size: Size<DevicePixels>) -> Result<()> {
        let width = new_size.width.0.max(1) as u32;
        let height = new_size.height.0.max(1) as u32;
        if self.width == width && self.height == height {
            return Ok(());
        }
        self.width = width;
        self.height = height;

        // Clear the render target before resizing
        let devices = self.devices.as_ref().context("devices missing")?;
        unsafe { devices.device_context.OMSetRenderTargets(None, None) };
        let resources = self.resources.as_mut().context("resources missing")?;
        resources.render_target.take();
        resources.render_target_view.take();

        // Resizing the swap chain requires a call to the underlying DXGI adapter, which can return the device removed error.
        // The app might have moved to a monitor that's attached to a different graphics device.
        // When a graphics device is removed or reset, the desktop resolution often changes, resulting in a window size change.
        // But here we just return the error, because we are handling device lost scenarios elsewhere.
        unsafe {
            resources
                .swap_chain
                .ResizeBuffers(
                    BUFFER_COUNT as u32,
                    width,
                    height,
                    RENDER_TARGET_FORMAT,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )
                .context("Failed to resize swap chain")?;
        }

        resources.recreate_resources(devices, width, height)?;

        if let Some(overlay) = self.overlay_resources.as_mut() {
            overlay.resize(devices, width, height)?;
        }

        unsafe {
            devices
                .device_context
                .OMSetRenderTargets(Some(slice::from_ref(&resources.render_target_view)), None);
        }

        Ok(())
    }

    fn upload_scene_buffers(&mut self, scene: &Scene) -> Result<()> {
        let devices = self.devices.as_ref().context("devices missing")?;

        if !scene.shadows.is_empty() {
            self.pipelines.shadow_pipeline.update_buffer(
                &devices.device,
                &devices.device_context,
                &scene.shadows,
            )?;
        }

        if !scene.quads.is_empty() {
            self.pipelines.quad_pipeline.update_buffer(
                &devices.device,
                &devices.device_context,
                &scene.quads,
            )?;
        }

        if !scene.underlines.is_empty() {
            self.pipelines.underline_pipeline.update_buffer(
                &devices.device,
                &devices.device_context,
                &scene.underlines,
            )?;
        }

        if !scene.monochrome_sprites.is_empty() {
            self.pipelines.mono_sprites.update_buffer(
                &devices.device,
                &devices.device_context,
                &scene.monochrome_sprites,
            )?;
        }

        if !scene.subpixel_sprites.is_empty() {
            self.pipelines.subpixel_sprites.update_buffer(
                &devices.device,
                &devices.device_context,
                &scene.subpixel_sprites,
            )?;
        }

        if !scene.polychrome_sprites.is_empty() {
            self.pipelines.poly_sprites.update_buffer(
                &devices.device,
                &devices.device_context,
                &scene.polychrome_sprites,
            )?;
        }

        Ok(())
    }

    fn draw_shadows(&mut self, start: usize, len: usize) -> Result<()> {
        if len == 0 {
            return Ok(());
        }
        let devices = self.devices.as_ref().context("devices missing")?;
        self.pipelines.shadow_pipeline.draw_range(
            &devices.device_context,
            self.globals
                .batch_params_buffer
                .as_ref()
                .context("batch params buffer missing")?,
            start as u32,
            len as u32,
        )
    }

    fn draw_quads(&mut self, start: usize, len: usize) -> Result<()> {
        if len == 0 {
            return Ok(());
        }
        let devices = self.devices.as_ref().context("devices missing")?;
        self.pipelines.quad_pipeline.draw_range(
            &devices.device_context,
            self.globals
                .batch_params_buffer
                .as_ref()
                .context("batch params buffer missing")?,
            start as u32,
            len as u32,
        )
    }

    fn draw_paths_to_intermediate(&mut self, paths: &[Path<ScaledPixels>]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }

        let devices = self.devices.as_ref().context("devices missing")?;
        let resources = self.resources.as_ref().context("resources missing")?;
        // Clear intermediate MSAA texture
        unsafe {
            devices.device_context.ClearRenderTargetView(
                resources
                    .path_intermediate_msaa_view
                    .as_ref()
                    .expect("required framework invariant must hold"),
                &[0.0; 4],
            );
            // Set intermediate MSAA texture as render target
            devices.device_context.OMSetRenderTargets(
                Some(slice::from_ref(&resources.path_intermediate_msaa_view)),
                None,
            );
        }

        // Collect all vertices and sprites for a single draw call
        let mut vertices = Vec::new();

        for path in paths {
            vertices.extend(path.vertices.iter().map(|v| PathRasterizationSprite {
                xy_position: v.xy_position,
                st_position: v.st_position,
                color: path.color,
                bounds: path.clipped_bounds(),
            }));
        }

        self.pipelines.path_rasterization_pipeline.update_buffer(
            &devices.device,
            &devices.device_context,
            &vertices,
        )?;

        self.pipelines.path_rasterization_pipeline.draw(
            &devices.device_context,
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
            vertices.len() as u32,
            1,
        )?;

        // Resolve MSAA to non-MSAA intermediate texture
        unsafe {
            devices.device_context.ResolveSubresource(
                &resources.path_intermediate_texture,
                0,
                &resources.path_intermediate_msaa_texture,
                0,
                RENDER_TARGET_FORMAT,
            );
            // Restore main render target
            devices
                .device_context
                .OMSetRenderTargets(Some(slice::from_ref(&resources.render_target_view)), None);
        }

        Ok(())
    }

    fn draw_paths_from_intermediate(&mut self, paths: &[Path<ScaledPixels>]) -> Result<()> {
        let Some(first_path) = paths.first() else {
            return Ok(());
        };

        // When copying paths from the intermediate texture to the drawable,
        // each pixel must only be copied once, in case of transparent paths.
        //
        // If all paths have the same draw order, then their bounds are all
        // disjoint, so we can copy each path's bounds individually. If this
        // batch combines different draw orders, we perform a single copy
        // for a minimal spanning rect.
        let sprites = if paths
            .last()
            .expect("required framework invariant must hold")
            .order
            == first_path.order
        {
            paths
                .iter()
                .map(|path| PathSprite {
                    bounds: path.clipped_bounds(),
                })
                .collect::<Vec<_>>()
        } else {
            let mut bounds = first_path.clipped_bounds();
            for path in paths.iter().skip(1) {
                bounds = bounds.union(&path.clipped_bounds());
            }
            vec![PathSprite { bounds }]
        };

        let devices = self.devices.as_ref().context("devices missing")?;
        let resources = self.resources.as_ref().context("resources missing")?;
        self.pipelines.path_sprite_pipeline.update_buffer(
            &devices.device,
            &devices.device_context,
            &sprites,
        )?;

        // Draw the sprites with the path texture
        self.pipelines.path_sprite_pipeline.draw_with_texture(
            &devices.device_context,
            slice::from_ref(&resources.path_intermediate_srv),
            slice::from_ref(&self.globals.sampler),
            sprites.len() as u32,
        )
    }

    fn draw_underlines(&mut self, start: usize, len: usize) -> Result<()> {
        if len == 0 {
            return Ok(());
        }
        let devices = self.devices.as_ref().context("devices missing")?;
        self.pipelines.underline_pipeline.draw_range(
            &devices.device_context,
            self.globals
                .batch_params_buffer
                .as_ref()
                .context("batch params buffer missing")?,
            start as u32,
            len as u32,
        )
    }

    fn draw_monochrome_sprites(
        &mut self,
        texture_id: AtlasTextureId,
        start: usize,
        len: usize,
    ) -> Result<()> {
        if len == 0 {
            return Ok(());
        }
        let devices = self.devices.as_ref().context("devices missing")?;
        let texture_view = self.atlas.get_texture_view(texture_id);
        self.pipelines.mono_sprites.draw_range_with_texture(
            &devices.device_context,
            &texture_view,
            self.globals
                .batch_params_buffer
                .as_ref()
                .context("batch params buffer missing")?,
            slice::from_ref(&self.globals.sampler),
            start as u32,
            len as u32,
        )
    }

    fn draw_subpixel_sprites(
        &mut self,
        texture_id: AtlasTextureId,
        start: usize,
        len: usize,
    ) -> Result<()> {
        if len == 0 {
            return Ok(());
        }
        let devices = self.devices.as_ref().context("devices missing")?;
        let texture_view = self.atlas.get_texture_view(texture_id);
        self.pipelines.subpixel_sprites.draw_range_with_texture(
            &devices.device_context,
            &texture_view,
            self.globals
                .batch_params_buffer
                .as_ref()
                .context("batch params buffer missing")?,
            slice::from_ref(&self.globals.sampler),
            start as u32,
            len as u32,
        )
    }

    fn draw_polychrome_sprites(
        &mut self,
        texture_id: AtlasTextureId,
        blend_mode: SpriteBlendMode,
        start: usize,
        len: usize,
    ) -> Result<()> {
        if len == 0 {
            return Ok(());
        }
        let devices = self.devices.as_ref().context("devices missing")?;
        let texture_view = self.atlas.get_texture_view(texture_id);
        let blend_state = match blend_mode {
            SpriteBlendMode::Normal => &self.pipelines.poly_sprites.blend_state,
            SpriteBlendMode::Additive => &self.pipelines.poly_additive_blend,
            SpriteBlendMode::Screen => &self.pipelines.poly_screen_blend,
        };
        self.pipelines
            .poly_sprites
            .draw_range_with_texture_and_blend(
                &devices.device_context,
                &texture_view,
                self.globals
                    .batch_params_buffer
                    .as_ref()
                    .context("batch params buffer missing")?,
                slice::from_ref(&self.globals.sampler),
                start as u32,
                len as u32,
                blend_state,
            )
    }

    fn draw_surfaces(&mut self, surfaces: &[PaintSurface]) -> Result<()> {
        if surfaces.is_empty() {
            return Ok(());
        }
        Ok(())
    }

    pub(crate) fn gpu_specs(&self) -> Result<GpuSpecs> {
        let devices = self.devices.as_ref().context("devices missing")?;
        let desc = unsafe { devices.adapter.GetDesc1() }?;
        let is_software_emulated = (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0;
        let device_name = String::from_utf16_lossy(&desc.Description)
            .trim_matches(char::from(0))
            .to_string();
        let driver_name = match desc.VendorId {
            0x10DE => "NVIDIA Corporation".to_string(),
            0x1002 => "AMD Corporation".to_string(),
            0x8086 => "Intel Corporation".to_string(),
            id => format!("Unknown Vendor (ID: {:#X})", id),
        };
        let driver_version = match desc.VendorId {
            0x10DE => nvidia::get_driver_version(),
            0x1002 => amd::get_driver_version(),
            // For Intel and other vendors, we use the DXGI API to get the driver version.
            _ => dxgi::get_driver_version(&devices.adapter),
        }
        .context("Failed to get gpu driver info")
        .log_err()
        .unwrap_or("Unknown Driver".to_string());
        Ok(GpuSpecs {
            is_software_emulated,
            device_name,
            driver_name,
            driver_info: driver_version,
        })
    }

    pub(crate) fn get_font_info() -> &'static FontInfo {
        static CACHED_FONT_INFO: OnceLock<FontInfo> = OnceLock::new();
        CACHED_FONT_INFO.get_or_init(|| unsafe {
            let factory: IDWriteFactory5 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)
                .expect("required framework invariant must hold");
            let render_params: IDWriteRenderingParams1 = factory
                .CreateRenderingParams()
                .expect("required framework invariant must hold")
                .cast()
                .expect("required framework invariant must hold");
            FontInfo {
                gamma_ratios: gpui::get_gamma_correction_ratios(render_params.GetGamma()),
                grayscale_enhanced_contrast: render_params.GetGrayscaleEnhancedContrast(),
                subpixel_enhanced_contrast: render_params.GetEnhancedContrast(),
                is_bgr: render_params.GetPixelGeometry() == DWRITE_PIXEL_GEOMETRY_BGR,
            }
        })
    }

    pub(crate) fn mark_drawable(&mut self) {
        self.skip_draws = false;
    }
}

impl DirectXResources {
    pub fn new(
        devices: &DirectXRendererDevices,
        width: u32,
        height: u32,
        hwnd: HWND,
        disable_direct_composition: bool,
    ) -> Result<Self> {
        let swap_chain = if disable_direct_composition {
            create_swap_chain(&devices.dxgi_factory, &devices.device, hwnd, width, height)?
        } else {
            create_swap_chain_for_composition(
                &devices.dxgi_factory,
                &devices.device,
                width,
                height,
            )?
        };

        let created = create_resources(devices, &swap_chain, width, height)?;
        set_rasterizer_state(&devices.device, &devices.device_context)?;

        Ok(Self {
            swap_chain,
            render_target: Some(created.render_target),
            render_target_view: created.render_target_view,
            path_intermediate_texture: created.path_intermediate_texture,
            path_intermediate_msaa_texture: created.path_intermediate_msaa_texture,
            path_intermediate_msaa_view: created.path_intermediate_msaa_view,
            path_intermediate_srv: created.path_intermediate_srv,
            backdrop_snapshot: created.backdrop_snapshot,
            backdrop_snapshot_srv: created.backdrop_snapshot_srv,
            backdrop_scratch: created.backdrop_scratch,
            backdrop_scratch_srv: created.backdrop_scratch_srv,
            backdrop_scratch_view: created.backdrop_scratch_view,
            viewport: created.viewport,
        })
    }

    #[inline]
    fn recreate_resources(
        &mut self,
        devices: &DirectXRendererDevices,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let created = create_resources(devices, &self.swap_chain, width, height)?;
        self.render_target = Some(created.render_target);
        self.render_target_view = created.render_target_view;
        self.path_intermediate_texture = created.path_intermediate_texture;
        self.path_intermediate_msaa_texture = created.path_intermediate_msaa_texture;
        self.path_intermediate_msaa_view = created.path_intermediate_msaa_view;
        self.path_intermediate_srv = created.path_intermediate_srv;
        self.backdrop_snapshot = created.backdrop_snapshot;
        self.backdrop_snapshot_srv = created.backdrop_snapshot_srv;
        self.backdrop_scratch = created.backdrop_scratch;
        self.backdrop_scratch_srv = created.backdrop_scratch_srv;
        self.backdrop_scratch_view = created.backdrop_scratch_view;
        self.viewport = created.viewport;
        Ok(())
    }
}

impl DirectXRenderPipelines {
    pub fn new(device: &ID3D11Device) -> Result<Self> {
        let shadow_pipeline = PipelineState::new(
            device,
            "shadow_pipeline",
            ShaderModule::Shadow,
            4,
            create_blend_state(device)?,
        )?;
        let quad_pipeline = PipelineState::new(
            device,
            "quad_pipeline",
            ShaderModule::Quad,
            64,
            create_blend_state(device)?,
        )?;
        let path_rasterization_pipeline = PipelineState::new(
            device,
            "path_rasterization_pipeline",
            ShaderModule::PathRasterization,
            32,
            create_blend_state_for_path_rasterization(device)?,
        )?;
        let path_sprite_pipeline = PipelineState::new(
            device,
            "path_sprite_pipeline",
            ShaderModule::PathSprite,
            4,
            create_blend_state_for_path_sprite(device)?,
        )?;
        let underline_pipeline = PipelineState::new(
            device,
            "underline_pipeline",
            ShaderModule::Underline,
            4,
            create_blend_state(device)?,
        )?;
        let mono_sprites = PipelineState::new(
            device,
            "monochrome_sprite_pipeline",
            ShaderModule::MonochromeSprite,
            512,
            create_blend_state(device)?,
        )?;
        let subpixel_sprites = PipelineState::new(
            device,
            "subpixel_sprite_pipeline",
            ShaderModule::SubpixelSprite,
            512,
            create_blend_state_for_subpixel_rendering(device)?,
        )?;
        let backdrop_blur = BackdropPipeline::new(
            device,
            ShaderModule::BackdropBlur,
            create_blend_state_for_backdrop(device)?,
        )?;
        let backdrop_glass = BackdropPipeline::new(
            device,
            ShaderModule::BackdropGlass,
            create_blend_state_for_backdrop(device)?,
        )?;
        let poly_sprites = PipelineState::new(
            device,
            "polychrome_sprite_pipeline",
            ShaderModule::PolychromeSprite,
            16,
            create_blend_state_for_composited_sprite(device, SpriteBlendMode::Normal)?,
        )?;
        let poly_additive_blend =
            create_blend_state_for_composited_sprite(device, SpriteBlendMode::Additive)?;
        let poly_screen_blend =
            create_blend_state_for_composited_sprite(device, SpriteBlendMode::Screen)?;

        Ok(Self {
            shadow_pipeline,
            quad_pipeline,
            path_rasterization_pipeline,
            path_sprite_pipeline,
            underline_pipeline,
            mono_sprites,
            subpixel_sprites,
            poly_sprites,
            poly_additive_blend,
            poly_screen_blend,
            backdrop_blur,
            backdrop_glass,
        })
    }
}

impl DirectComposition {
    pub fn new(dxgi_device: &IDXGIDevice, hwnd: HWND) -> Result<Self> {
        let comp_device = get_comp_device(dxgi_device)?;
        let comp_target = unsafe { comp_device.CreateTargetForHwnd(hwnd, true) }?;
        let root_visual = unsafe { comp_device.CreateVisual() }?;
        let base_visual = unsafe { comp_device.CreateVisual() }?;
        let portal_container = unsafe { comp_device.CreateVisual() }?;
        let overlay_visual = unsafe { comp_device.CreateVisual() }?;

        unsafe {
            root_visual.AddVisual(&base_visual, false, None)?;
            root_visual.AddVisual(&portal_container, true, &base_visual)?;
            root_visual.AddVisual(&overlay_visual, true, &portal_container)?;
            comp_target.SetRoot(&root_visual)?;
            comp_device.Commit()?;
        }

        Ok(Self {
            comp_device,
            comp_target,
            root_visual,
            base_visual,
            portal_container,
            overlay_visual,
        })
    }

    pub fn set_swap_chain(&self, swap_chain: &IDXGISwapChain1) -> Result<()> {
        unsafe {
            self.base_visual.SetContent(swap_chain)?;
            self.comp_device.Commit()?;
        }
        Ok(())
    }

    pub fn set_overlay_swap_chain(&self, swap_chain: &IDXGISwapChain1) -> Result<()> {
        unsafe {
            self.overlay_visual.SetContent(swap_chain)?;
            self.comp_device.Commit()?;
        }
        Ok(())
    }

    fn create_portal(&self) -> Result<DirectCompositionPortal> {
        let visual = unsafe { self.comp_device.CreateVisual() }?;
        let clip = unsafe { self.comp_device.CreateRectangleClip() }?;
        unsafe {
            visual.SetClip(&clip)?;
            self.portal_container.AddVisual(&visual, true, None)?;
            self.comp_device.Commit()?;
        }
        Ok(DirectCompositionPortal {
            comp_device: self.comp_device.clone(),
            container: self.portal_container.clone(),
            visual,
            clip,
            visible: Cell::new(true),
        })
    }
}

impl PlatformNativeSurface for DirectCompositionPortal {
    fn set_bounds(&self, bounds: Bounds<DevicePixels>) -> Result<()> {
        let x = bounds.origin.x.0 as f32;
        let y = bounds.origin.y.0 as f32;
        let width = bounds.size.width.0.max(0) as f32;
        let height = bounds.size.height.0.max(0) as f32;
        unsafe {
            self.visual.SetOffsetX2(x)?;
            self.visual.SetOffsetY2(y)?;
            self.clip.SetLeft2(0.0)?;
            self.clip.SetTop2(0.0)?;
            self.clip.SetRight2(width)?;
            self.clip.SetBottom2(height)?;
            self.comp_device.Commit()?;
        }
        Ok(())
    }

    fn set_visible(&self, visible: bool) -> Result<()> {
        if self.visible.get() != visible {
            unsafe {
                if visible {
                    self.container.AddVisual(&self.visual, true, None)?;
                } else {
                    self.container.RemoveVisual(&self.visual)?;
                }
                self.comp_device.Commit()?;
            }
            self.visible.set(visible);
        }
        Ok(())
    }

    fn platform_handle(&self) -> Box<dyn std::any::Any> {
        Box::new(
            self.visual
                .cast::<windows::core::IUnknown>()
                .expect("IDCompositionVisual must implement IUnknown"),
        )
    }
}

impl Drop for DirectCompositionPortal {
    fn drop(&mut self) {
        unsafe {
            self.container.RemoveVisual(&self.visual).ok();
            self.comp_device.Commit().ok();
        }
    }
}

impl OverlayResources {
    fn new(devices: &DirectXRendererDevices, width: u32, height: u32) -> Result<Self> {
        let swap_chain = create_swap_chain_for_composition(
            &devices.dxgi_factory,
            &devices.device,
            width,
            height,
        )?;
        let (render_target, render_target_view) =
            create_render_target_and_its_view(&swap_chain, &devices.device)?;
        Ok(Self {
            swap_chain,
            render_target: Some(render_target),
            render_target_view,
        })
    }

    fn resize(&mut self, devices: &DirectXRendererDevices, width: u32, height: u32) -> Result<()> {
        self.render_target.take();
        self.render_target_view.take();
        unsafe {
            self.swap_chain.ResizeBuffers(
                BUFFER_COUNT as u32,
                width,
                height,
                RENDER_TARGET_FORMAT,
                DXGI_SWAP_CHAIN_FLAG(0),
            )?;
        }
        let (render_target, render_target_view) =
            create_render_target_and_its_view(&self.swap_chain, &devices.device)?;
        self.render_target = Some(render_target);
        self.render_target_view = render_target_view;
        Ok(())
    }
}

impl DirectXGlobalElements {
    pub fn new(device: &ID3D11Device) -> Result<Self> {
        let global_params_buffer = create_constant_buffer::<GlobalParams>(device)?;
        let batch_params_buffer = create_constant_buffer::<BatchParams>(device)?;
        let backdrop_params_buffer = create_constant_buffer::<BackdropGlassParams>(device)?;

        let sampler = unsafe {
            let desc = D3D11_SAMPLER_DESC {
                Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                AddressU: D3D11_TEXTURE_ADDRESS_WRAP,
                AddressV: D3D11_TEXTURE_ADDRESS_WRAP,
                AddressW: D3D11_TEXTURE_ADDRESS_WRAP,
                MipLODBias: 0.0,
                MaxAnisotropy: 1,
                ComparisonFunc: D3D11_COMPARISON_ALWAYS,
                BorderColor: [0.0; 4],
                MinLOD: 0.0,
                MaxLOD: D3D11_FLOAT32_MAX,
            };
            let mut output = None;
            device.CreateSamplerState(&desc, Some(&mut output))?;
            output
        };

        let backdrop_sampler = unsafe {
            let desc = D3D11_SAMPLER_DESC {
                Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                MipLODBias: 0.0,
                MaxAnisotropy: 1,
                ComparisonFunc: D3D11_COMPARISON_ALWAYS,
                BorderColor: [0.0; 4],
                MinLOD: 0.0,
                MaxLOD: D3D11_FLOAT32_MAX,
            };
            let mut output = None;
            device.CreateSamplerState(&desc, Some(&mut output))?;
            output
        };

        Ok(Self {
            global_params_buffer,
            batch_params_buffer,
            backdrop_params_buffer,
            sampler,
            backdrop_sampler,
        })
    }
}

/// What one backdrop pass reads, matching `BackdropGlassParams` in
/// `shaders.hlsl`.
///
/// The lobes are flattened into vectors rather than kept as a struct array
/// because a constant buffer packs an array element to sixteen bytes, and a
/// lobe is two of them: writing the pairing out here is what keeps the Rust
/// layout and the HLSL layout the same thing rather than two things that
/// happen to agree today.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C, align(16))]
struct BackdropGlassParams {
    direction: [f32; 2],
    sigma: f32,
    bevel: f32,
    bounds: [f32; 4],
    radii: [f32; 4],
    mask: [f32; 4],
    refraction: f32,
    dispersion: f32,
    specular: f32,
    light_angle: f32,
    specular_sharpness: f32,
    smoothing: f32,
    lobe_count: u32,
    _pad: u32,
    lobes: [[f32; 4]; MAX_GLASS_LOBES * 2],
}

// Six sixteen-byte registers of header, then two per lobe. HLSL packs a
// constant buffer into sixteen-byte registers that a member may not straddle,
// which is what the groupings above are chosen to respect.
const _: () = assert!(
    std::mem::size_of::<BackdropGlassParams>() == 96 + MAX_GLASS_LOBES * 32,
    "the backdrop parameter buffer must match the cbuffer in shaders.hlsl"
);

impl BackdropGlassParams {
    fn from_glass(glass: &BackdropGlass) -> Self {
        let bounds = |bounds: Bounds<ScaledPixels>| {
            [
                bounds.origin.x.0,
                bounds.origin.y.0,
                bounds.size.width.0,
                bounds.size.height.0,
            ]
        };
        let radii = |radii: Corners<ScaledPixels>| {
            [
                radii.top_left.0,
                radii.top_right.0,
                radii.bottom_right.0,
                radii.bottom_left.0,
            ]
        };

        let mut lobes = [[0.0; 4]; MAX_GLASS_LOBES * 2];
        let lobe_count = (glass.lobe_count as usize).min(MAX_GLASS_LOBES);
        for (index, lobe) in glass.lobes[..lobe_count].iter().enumerate() {
            lobes[index * 2] = bounds(lobe.bounds);
            lobes[index * 2 + 1] = radii(lobe.corner_radii);
        }

        Self {
            direction: [0.0, 0.0],
            sigma: 1.0,
            bevel: glass.material.bevel.0,
            bounds: bounds(glass.bounds),
            radii: radii(glass.corner_radii),
            mask: bounds(glass.content_mask.bounds),
            refraction: glass.material.refraction,
            dispersion: glass.material.dispersion,
            specular: glass.material.specular,
            light_angle: glass.material.light_angle,
            specular_sharpness: glass.material.specular_sharpness,
            smoothing: glass.material.smoothing.0,
            lobe_count: lobe_count as u32,
            _pad: 0,
            lobes,
        }
    }
}

#[derive(Debug, Default)]
#[repr(C)]
struct GlobalParams {
    gamma_ratios: [f32; 4],
    viewport_size: [f32; 2],
    grayscale_enhanced_contrast: f32,
    subpixel_enhanced_contrast: f32,
    is_bgr: u32,
    _pad: [u32; 3],
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C, align(16))]
struct BatchParams {
    start_index: u32,
    _padding: [u32; 3],
}

const _: () = assert!(std::mem::size_of::<BatchParams>() == 16);

struct PipelineState<T> {
    label: &'static str,
    vertex: ID3D11VertexShader,
    fragment: ID3D11PixelShader,
    buffer: ID3D11Buffer,
    buffer_size: usize,
    view: Option<ID3D11ShaderResourceView>,
    blend_state: ID3D11BlendState,
    _marker: std::marker::PhantomData<T>,
}

impl<T> PipelineState<T> {
    fn new(
        device: &ID3D11Device,
        label: &'static str,
        shader_module: ShaderModule,
        buffer_size: usize,
        blend_state: ID3D11BlendState,
    ) -> Result<Self> {
        let vertex = {
            let raw_shader = RawShaderBytes::new(shader_module, ShaderTarget::Vertex)?;
            create_vertex_shader(device, raw_shader.as_bytes())?
        };
        let fragment = {
            let raw_shader = RawShaderBytes::new(shader_module, ShaderTarget::Fragment)?;
            create_fragment_shader(device, raw_shader.as_bytes())?
        };
        let buffer = create_buffer(device, std::mem::size_of::<T>(), buffer_size)?;
        let view = create_buffer_view(device, &buffer)?;

        Ok(PipelineState {
            label,
            vertex,
            fragment,
            buffer,
            buffer_size,
            view,
            blend_state,
            _marker: std::marker::PhantomData,
        })
    }

    fn update_buffer(
        &mut self,
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
        data: &[T],
    ) -> Result<()> {
        if self.buffer_size < data.len() {
            let element_size = std::mem::size_of::<T>();
            let required_size = std::mem::size_of_val(data);
            anyhow::ensure!(
                required_size <= MAX_INSTANCE_BUFFER_SIZE,
                "{} buffer needs {required_size} bytes, above the maximum of {MAX_INSTANCE_BUFFER_SIZE}",
                self.label
            );
            let new_buffer_size = data
                .len()
                .next_power_of_two()
                .min(MAX_INSTANCE_BUFFER_SIZE / element_size);
            log::debug!(
                "Updating {} buffer size from {} to {}",
                self.label,
                self.buffer_size,
                new_buffer_size
            );
            let buffer = create_buffer(device, std::mem::size_of::<T>(), new_buffer_size)?;
            let view = create_buffer_view(device, &buffer)?;
            self.buffer = buffer;
            self.view = view;
            self.buffer_size = new_buffer_size;
        }
        update_buffer(device_context, &self.buffer, data)
    }

    fn draw(
        &self,
        device_context: &ID3D11DeviceContext,
        topology: D3D_PRIMITIVE_TOPOLOGY,
        vertex_count: u32,
        instance_count: u32,
    ) -> Result<()> {
        set_pipeline_state(
            device_context,
            slice::from_ref(&self.view),
            topology,
            &self.vertex,
            &self.fragment,
            &self.blend_state,
        );
        unsafe {
            device_context.DrawInstanced(vertex_count, instance_count, 0, 0);
        }
        Ok(())
    }

    fn draw_with_texture(
        &self,
        device_context: &ID3D11DeviceContext,
        texture: &[Option<ID3D11ShaderResourceView>],
        sampler: &[Option<ID3D11SamplerState>],
        instance_count: u32,
    ) -> Result<()> {
        set_pipeline_state(
            device_context,
            slice::from_ref(&self.view),
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            &self.vertex,
            &self.fragment,
            &self.blend_state,
        );
        unsafe {
            device_context.PSSetSamplers(0, Some(sampler));
            device_context.VSSetShaderResources(0, Some(texture));
            device_context.PSSetShaderResources(0, Some(texture));

            device_context.DrawInstanced(4, instance_count, 0, 0);
        }
        Ok(())
    }

    fn draw_range(
        &self,
        device_context: &ID3D11DeviceContext,
        batch_params_buffer: &ID3D11Buffer,
        first_instance: u32,
        instance_count: u32,
    ) -> Result<()> {
        anyhow::ensure!(
            first_instance as usize + instance_count as usize <= self.buffer_size,
            "DirectX instance range exceeds the {} buffer",
            self.label
        );
        update_batch_start(device_context, batch_params_buffer, first_instance)?;
        set_pipeline_state(
            device_context,
            slice::from_ref(&self.view),
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            &self.vertex,
            &self.fragment,
            &self.blend_state,
        );
        unsafe {
            device_context.DrawInstanced(4, instance_count, 0, 0);
        }
        Ok(())
    }

    fn draw_range_with_texture(
        &self,
        device_context: &ID3D11DeviceContext,
        texture: &[Option<ID3D11ShaderResourceView>],
        batch_params_buffer: &ID3D11Buffer,
        sampler: &[Option<ID3D11SamplerState>],
        first_instance: u32,
        instance_count: u32,
    ) -> Result<()> {
        anyhow::ensure!(
            first_instance as usize + instance_count as usize <= self.buffer_size,
            "DirectX instance range exceeds the {} buffer",
            self.label
        );
        update_batch_start(device_context, batch_params_buffer, first_instance)?;
        set_pipeline_state(
            device_context,
            slice::from_ref(&self.view),
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            &self.vertex,
            &self.fragment,
            &self.blend_state,
        );
        unsafe {
            device_context.PSSetSamplers(0, Some(sampler));
            device_context.VSSetShaderResources(0, Some(texture));
            device_context.PSSetShaderResources(0, Some(texture));
            device_context.DrawInstanced(4, instance_count, 0, 0);
        }
        Ok(())
    }

    fn draw_range_with_texture_and_blend(
        &self,
        device_context: &ID3D11DeviceContext,
        texture: &[Option<ID3D11ShaderResourceView>],
        batch_params_buffer: &ID3D11Buffer,
        sampler: &[Option<ID3D11SamplerState>],
        first_instance: u32,
        instance_count: u32,
        blend_state: &ID3D11BlendState,
    ) -> Result<()> {
        anyhow::ensure!(
            first_instance as usize + instance_count as usize <= self.buffer_size,
            "DirectX instance range exceeds the {} buffer",
            self.label
        );
        update_batch_start(device_context, batch_params_buffer, first_instance)?;
        set_pipeline_state(
            device_context,
            slice::from_ref(&self.view),
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            &self.vertex,
            &self.fragment,
            blend_state,
        );
        unsafe {
            device_context.PSSetSamplers(0, Some(sampler));
            device_context.VSSetShaderResources(0, Some(texture));
            device_context.PSSetShaderResources(0, Some(texture));
            device_context.DrawInstanced(4, instance_count, 0, 0);
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
struct PathRasterizationSprite {
    xy_position: Point<ScaledPixels>,
    st_position: Point<f32>,
    color: Background,
    bounds: Bounds<ScaledPixels>,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct PathSprite {
    bounds: Bounds<ScaledPixels>,
}

impl Drop for DirectXRenderer {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        if let Some(devices) = &self.devices {
            report_live_objects(&devices.device).ok();
        }
    }
}

#[inline]
fn get_comp_device(dxgi_device: &IDXGIDevice) -> Result<IDCompositionDevice> {
    Ok(unsafe { DCompositionCreateDevice(dxgi_device)? })
}

fn create_swap_chain_for_composition(
    dxgi_factory: &IDXGIFactory6,
    device: &ID3D11Device,
    width: u32,
    height: u32,
) -> Result<IDXGISwapChain1> {
    let desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: width,
        Height: height,
        Format: RENDER_TARGET_FORMAT,
        Stereo: false.into(),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: BUFFER_COUNT as u32,
        // Composition SwapChains only support the DXGI_SCALING_STRETCH Scaling.
        Scaling: DXGI_SCALING_STRETCH,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
        Flags: 0,
    };
    Ok(unsafe { dxgi_factory.CreateSwapChainForComposition(device, &desc, None)? })
}

fn create_swap_chain(
    dxgi_factory: &IDXGIFactory6,
    device: &ID3D11Device,
    hwnd: HWND,
    width: u32,
    height: u32,
) -> Result<IDXGISwapChain1> {
    use windows::Win32::Graphics::Dxgi::DXGI_MWA_NO_ALT_ENTER;

    let desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: width,
        Height: height,
        Format: RENDER_TARGET_FORMAT,
        Stereo: false.into(),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: BUFFER_COUNT as u32,
        Scaling: DXGI_SCALING_NONE,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        AlphaMode: DXGI_ALPHA_MODE_IGNORE,
        Flags: 0,
    };
    let swap_chain =
        unsafe { dxgi_factory.CreateSwapChainForHwnd(device, hwnd, &desc, None, None) }?;
    unsafe { dxgi_factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER) }?;
    Ok(swap_chain)
}

struct CreatedResources {
    render_target: ID3D11Texture2D,
    render_target_view: Option<ID3D11RenderTargetView>,
    path_intermediate_texture: ID3D11Texture2D,
    path_intermediate_srv: Option<ID3D11ShaderResourceView>,
    path_intermediate_msaa_texture: ID3D11Texture2D,
    path_intermediate_msaa_view: Option<ID3D11RenderTargetView>,
    backdrop_snapshot: ID3D11Texture2D,
    backdrop_snapshot_srv: Option<ID3D11ShaderResourceView>,
    backdrop_scratch: [ID3D11Texture2D; 2],
    backdrop_scratch_srv: [Option<ID3D11ShaderResourceView>; 2],
    backdrop_scratch_view: [Option<ID3D11RenderTargetView>; 2],
    viewport: D3D11_VIEWPORT,
}

#[inline]
fn create_resources(
    devices: &DirectXRendererDevices,
    swap_chain: &IDXGISwapChain1,
    width: u32,
    height: u32,
) -> Result<CreatedResources> {
    let (render_target, render_target_view) =
        create_render_target_and_its_view(swap_chain, &devices.device)?;
    let (path_intermediate_texture, path_intermediate_srv) =
        create_path_intermediate_texture(&devices.device, width, height)?;
    let (path_intermediate_msaa_texture, path_intermediate_msaa_view) =
        create_path_intermediate_msaa_texture_and_view(&devices.device, width, height)?;
    let (backdrop_snapshot, backdrop_snapshot_srv) =
        create_backdrop_texture(&devices.device, width, height, false)?;
    let (first_scratch, first_srv, first_view) =
        create_backdrop_scratch(&devices.device, width, height)?;
    let (second_scratch, second_srv, second_view) =
        create_backdrop_scratch(&devices.device, width, height)?;
    let viewport = D3D11_VIEWPORT {
        TopLeftX: 0.0,
        TopLeftY: 0.0,
        Width: width as f32,
        Height: height as f32,
        MinDepth: 0.0,
        MaxDepth: 1.0,
    };
    Ok(CreatedResources {
        render_target,
        render_target_view,
        path_intermediate_texture,
        path_intermediate_srv,
        path_intermediate_msaa_texture,
        path_intermediate_msaa_view,
        backdrop_snapshot,
        backdrop_snapshot_srv,
        backdrop_scratch: [first_scratch, second_scratch],
        backdrop_scratch_srv: [first_srv, second_srv],
        backdrop_scratch_view: [first_view, second_view],
        viewport,
    })
}

/// A full-viewport texture the backdrop passes read, and optionally draw to.
///
/// The snapshot only needs to be readable, because it is filled by copying
/// the render target rather than by drawing; the two scratch textures are
/// both, because the separable gaussian writes one axis while reading the
/// other.
#[inline]
fn create_backdrop_texture(
    device: &ID3D11Device,
    width: u32,
    height: u32,
    renderable: bool,
) -> Result<(ID3D11Texture2D, Option<ID3D11ShaderResourceView>)> {
    let bind_flags = if renderable {
        (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32
    } else {
        D3D11_BIND_SHADER_RESOURCE.0 as u32
    };
    let texture = unsafe {
        let mut output = None;
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width.max(1),
            Height: height.max(1),
            MipLevels: 1,
            ArraySize: 1,
            Format: RENDER_TARGET_FORMAT,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: bind_flags,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        device.CreateTexture2D(&desc, None, Some(&mut output))?;
        output.expect("required framework invariant must hold")
    };
    let mut view = None;
    unsafe { device.CreateShaderResourceView(&texture, None, Some(&mut view))? };
    Ok((texture, view))
}

#[inline]
fn create_backdrop_scratch(
    device: &ID3D11Device,
    width: u32,
    height: u32,
) -> Result<(
    ID3D11Texture2D,
    Option<ID3D11ShaderResourceView>,
    Option<ID3D11RenderTargetView>,
)> {
    let (texture, srv) = create_backdrop_texture(device, width, height, true)?;
    let mut view = None;
    unsafe { device.CreateRenderTargetView(&texture, None, Some(&mut view))? };
    Ok((texture, srv, view))
}

#[inline]
fn create_render_target_and_its_view(
    swap_chain: &IDXGISwapChain1,
    device: &ID3D11Device,
) -> Result<(ID3D11Texture2D, Option<ID3D11RenderTargetView>)> {
    let render_target: ID3D11Texture2D = unsafe { swap_chain.GetBuffer(0) }?;
    let mut render_target_view = None;
    unsafe { device.CreateRenderTargetView(&render_target, None, Some(&mut render_target_view))? };
    Ok((render_target, render_target_view))
}

#[inline]
fn create_path_intermediate_texture(
    device: &ID3D11Device,
    width: u32,
    height: u32,
) -> Result<(ID3D11Texture2D, Option<ID3D11ShaderResourceView>)> {
    let texture = unsafe {
        let mut output = None;
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: RENDER_TARGET_FORMAT,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        device.CreateTexture2D(&desc, None, Some(&mut output))?;
        output.expect("required framework invariant must hold")
    };

    let mut shader_resource_view = None;
    unsafe { device.CreateShaderResourceView(&texture, None, Some(&mut shader_resource_view))? };

    Ok((
        texture,
        Some(shader_resource_view.expect("required framework invariant must hold")),
    ))
}

#[inline]
fn create_path_intermediate_msaa_texture_and_view(
    device: &ID3D11Device,
    width: u32,
    height: u32,
) -> Result<(ID3D11Texture2D, Option<ID3D11RenderTargetView>)> {
    let msaa_texture = unsafe {
        let mut output = None;
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: RENDER_TARGET_FORMAT,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: PATH_MULTISAMPLE_COUNT,
                Quality: D3D11_STANDARD_MULTISAMPLE_PATTERN.0 as u32,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        device.CreateTexture2D(&desc, None, Some(&mut output))?;
        output.expect("required framework invariant must hold")
    };
    let mut msaa_view = None;
    unsafe { device.CreateRenderTargetView(&msaa_texture, None, Some(&mut msaa_view))? };
    Ok((
        msaa_texture,
        Some(msaa_view.expect("required framework invariant must hold")),
    ))
}

#[inline]
fn set_rasterizer_state(device: &ID3D11Device, device_context: &ID3D11DeviceContext) -> Result<()> {
    let desc = D3D11_RASTERIZER_DESC {
        FillMode: D3D11_FILL_SOLID,
        CullMode: D3D11_CULL_NONE,
        FrontCounterClockwise: false.into(),
        DepthBias: 0,
        DepthBiasClamp: 0.0,
        SlopeScaledDepthBias: 0.0,
        DepthClipEnable: true.into(),
        ScissorEnable: false.into(),
        MultisampleEnable: true.into(),
        AntialiasedLineEnable: false.into(),
    };
    let rasterizer_state = unsafe {
        let mut state = None;
        device.CreateRasterizerState(&desc, Some(&mut state))?;
        state.expect("required framework invariant must hold")
    };
    unsafe { device_context.RSSetState(&rasterizer_state) };
    Ok(())
}

// https://learn.microsoft.com/en-us/windows/win32/api/d3d11/ns-d3d11-d3d11_blend_desc
#[inline]
fn create_blend_state(device: &ID3D11Device) -> Result<ID3D11BlendState> {
    let mut desc = D3D11_BLEND_DESC::default();
    desc.RenderTarget[0].BlendEnable = true.into();
    desc.RenderTarget[0].BlendOp = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].BlendOpAlpha = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].SrcBlend = D3D11_BLEND_SRC_ALPHA;
    desc.RenderTarget[0].SrcBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].DestBlend = D3D11_BLEND_INV_SRC_ALPHA;
    desc.RenderTarget[0].DestBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8;
    unsafe {
        let mut state = None;
        device.CreateBlendState(&desc, Some(&mut state))?;
        Ok(state.expect("required framework invariant must hold"))
    }
}

#[inline]
fn create_blend_state_for_composited_sprite(
    device: &ID3D11Device,
    blend_mode: SpriteBlendMode,
) -> Result<ID3D11BlendState> {
    let mut desc = D3D11_BLEND_DESC::default();
    desc.RenderTarget[0].BlendEnable = true.into();
    desc.RenderTarget[0].BlendOp = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].BlendOpAlpha = D3D11_BLEND_OP_ADD;
    match blend_mode {
        SpriteBlendMode::Normal => {
            desc.RenderTarget[0].SrcBlend = D3D11_BLEND_SRC_ALPHA;
            desc.RenderTarget[0].DestBlend = D3D11_BLEND_INV_SRC_ALPHA;
        }
        SpriteBlendMode::Additive => {
            desc.RenderTarget[0].SrcBlend = D3D11_BLEND_SRC_ALPHA;
            desc.RenderTarget[0].DestBlend = D3D11_BLEND_ONE;
        }
        SpriteBlendMode::Screen => {
            desc.RenderTarget[0].SrcBlend = D3D11_BLEND_ONE;
            desc.RenderTarget[0].DestBlend = D3D11_BLEND_INV_SRC_COLOR;
        }
    }
    desc.RenderTarget[0].SrcBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].DestBlendAlpha = D3D11_BLEND_INV_SRC_ALPHA;
    desc.RenderTarget[0].RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8;
    unsafe {
        let mut state = None;
        device.CreateBlendState(&desc, Some(&mut state))?;
        Ok(state.expect("required framework invariant must hold"))
    }
}

/// The backdrop passes replace what they cover rather than compositing over
/// it: the blur's output is the whole of the scratch texture, and the glass
/// surface is the backdrop it just blurred, painted back where it was. A
/// fragment outside the shape discards instead of writing a transparent
/// pixel, which is why blending is off rather than set to source-over.
#[inline]
fn create_blend_state_for_backdrop(device: &ID3D11Device) -> Result<ID3D11BlendState> {
    let mut desc = D3D11_BLEND_DESC::default();
    desc.RenderTarget[0].BlendEnable = false.into();
    desc.RenderTarget[0].RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8;
    unsafe {
        let mut state = None;
        device.CreateBlendState(&desc, Some(&mut state))?;
        Ok(state.expect("required framework invariant must hold"))
    }
}

#[inline]
fn create_blend_state_for_subpixel_rendering(device: &ID3D11Device) -> Result<ID3D11BlendState> {
    let mut desc = D3D11_BLEND_DESC::default();
    desc.RenderTarget[0].BlendEnable = true.into();
    desc.RenderTarget[0].BlendOp = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].BlendOpAlpha = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].SrcBlend = D3D11_BLEND_SRC1_COLOR;
    desc.RenderTarget[0].DestBlend = D3D11_BLEND_INV_SRC1_COLOR;
    // It does not make sense to draw transparent subpixel-rendered text, since it cannot be meaningfully alpha-blended onto anything else.
    desc.RenderTarget[0].SrcBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].DestBlendAlpha = D3D11_BLEND_ZERO;
    desc.RenderTarget[0].RenderTargetWriteMask =
        D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8 & !D3D11_COLOR_WRITE_ENABLE_ALPHA.0 as u8;

    unsafe {
        let mut state = None;
        device.CreateBlendState(&desc, Some(&mut state))?;
        Ok(state.expect("required framework invariant must hold"))
    }
}

#[inline]
fn create_blend_state_for_path_rasterization(device: &ID3D11Device) -> Result<ID3D11BlendState> {
    // If the feature level is set to greater than D3D_FEATURE_LEVEL_9_3, the display
    // device performs the blend in linear space, which is ideal.
    let mut desc = D3D11_BLEND_DESC::default();
    desc.RenderTarget[0].BlendEnable = true.into();
    desc.RenderTarget[0].BlendOp = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].BlendOpAlpha = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].SrcBlend = D3D11_BLEND_ONE;
    desc.RenderTarget[0].SrcBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].DestBlend = D3D11_BLEND_INV_SRC_ALPHA;
    desc.RenderTarget[0].DestBlendAlpha = D3D11_BLEND_INV_SRC_ALPHA;
    desc.RenderTarget[0].RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8;
    unsafe {
        let mut state = None;
        device.CreateBlendState(&desc, Some(&mut state))?;
        Ok(state.expect("required framework invariant must hold"))
    }
}

#[inline]
fn create_blend_state_for_path_sprite(device: &ID3D11Device) -> Result<ID3D11BlendState> {
    // If the feature level is set to greater than D3D_FEATURE_LEVEL_9_3, the display
    // device performs the blend in linear space, which is ideal.
    let mut desc = D3D11_BLEND_DESC::default();
    desc.RenderTarget[0].BlendEnable = true.into();
    desc.RenderTarget[0].BlendOp = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].BlendOpAlpha = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].SrcBlend = D3D11_BLEND_ONE;
    desc.RenderTarget[0].SrcBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].DestBlend = D3D11_BLEND_INV_SRC_ALPHA;
    desc.RenderTarget[0].DestBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8;
    unsafe {
        let mut state = None;
        device.CreateBlendState(&desc, Some(&mut state))?;
        Ok(state.expect("required framework invariant must hold"))
    }
}

#[inline]
fn create_vertex_shader(device: &ID3D11Device, bytes: &[u8]) -> Result<ID3D11VertexShader> {
    unsafe {
        let mut shader = None;
        device.CreateVertexShader(bytes, None, Some(&mut shader))?;
        Ok(shader.expect("required framework invariant must hold"))
    }
}

#[inline]
fn create_fragment_shader(device: &ID3D11Device, bytes: &[u8]) -> Result<ID3D11PixelShader> {
    unsafe {
        let mut shader = None;
        device.CreatePixelShader(bytes, None, Some(&mut shader))?;
        Ok(shader.expect("required framework invariant must hold"))
    }
}

#[inline]
fn create_constant_buffer<T>(device: &ID3D11Device) -> Result<Option<ID3D11Buffer>> {
    const { assert!(std::mem::size_of::<T>() != 0 && std::mem::size_of::<T>().is_multiple_of(16)) };
    let desc = D3D11_BUFFER_DESC {
        ByteWidth: std::mem::size_of::<T>() as u32,
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
        MiscFlags: 0,
        StructureByteStride: 0,
    };
    let mut buffer = None;
    unsafe { device.CreateBuffer(&desc, None, Some(&mut buffer)) }?;
    Ok(buffer)
}

#[inline]
fn create_buffer(
    device: &ID3D11Device,
    element_size: usize,
    buffer_size: usize,
) -> Result<ID3D11Buffer> {
    let desc = D3D11_BUFFER_DESC {
        ByteWidth: (element_size * buffer_size) as u32,
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
        MiscFlags: D3D11_RESOURCE_MISC_BUFFER_STRUCTURED.0 as u32,
        StructureByteStride: element_size as u32,
    };
    let mut buffer = None;
    unsafe { device.CreateBuffer(&desc, None, Some(&mut buffer)) }?;
    Ok(buffer.expect("required framework invariant must hold"))
}

#[inline]
fn create_buffer_view(
    device: &ID3D11Device,
    buffer: &ID3D11Buffer,
) -> Result<Option<ID3D11ShaderResourceView>> {
    let mut view = None;
    unsafe { device.CreateShaderResourceView(buffer, None, Some(&mut view)) }?;
    Ok(view)
}

#[inline]
fn update_buffer<T>(
    device_context: &ID3D11DeviceContext,
    buffer: &ID3D11Buffer,
    data: &[T],
) -> Result<()> {
    unsafe {
        let mut dest = std::mem::zeroed();
        device_context.Map(buffer, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut dest))?;
        std::ptr::copy_nonoverlapping(data.as_ptr(), dest.pData as _, data.len());
        device_context.Unmap(buffer, 0);
    }
    Ok(())
}

#[inline]
fn update_batch_start(
    device_context: &ID3D11DeviceContext,
    buffer: &ID3D11Buffer,
    first_instance: u32,
) -> Result<()> {
    update_buffer(
        device_context,
        buffer,
        &[BatchParams {
            start_index: first_instance,
            _padding: [0; 3],
        }],
    )
}

#[inline]
fn set_pipeline_state(
    device_context: &ID3D11DeviceContext,
    buffer_view: &[Option<ID3D11ShaderResourceView>],
    topology: D3D_PRIMITIVE_TOPOLOGY,
    vertex_shader: &ID3D11VertexShader,
    fragment_shader: &ID3D11PixelShader,
    blend_state: &ID3D11BlendState,
) {
    unsafe {
        device_context.VSSetShaderResources(1, Some(buffer_view));
        device_context.PSSetShaderResources(1, Some(buffer_view));
        device_context.IASetPrimitiveTopology(topology);
        device_context.VSSetShader(vertex_shader, None);
        device_context.PSSetShader(fragment_shader, None);
        device_context.OMSetBlendState(blend_state, None, 0xFFFFFFFF);
    }
}

/// The order of the first primitive in a batch, which is what decides whether
/// a pending glass surface belongs before it.
fn backdrop_batch_first_order(scene: &Scene, batch: &PrimitiveBatch) -> DrawOrder {
    match batch {
        PrimitiveBatch::Shadows(range) => scene.shadows[range.start].order,
        PrimitiveBatch::Quads(range) => scene.quads[range.start].order,
        PrimitiveBatch::Paths(range) => scene.paths[range.start].order,
        PrimitiveBatch::Underlines(range) => scene.underlines[range.start].order,
        PrimitiveBatch::MonochromeSprites { range, .. } => {
            scene.monochrome_sprites[range.start].order
        }
        PrimitiveBatch::SubpixelSprites { range, .. } => scene.subpixel_sprites[range.start].order,
        PrimitiveBatch::PolychromeSprites { range, .. } => {
            scene.polychrome_sprites[range.start].order
        }
        PrimitiveBatch::Surfaces(range) => scene.surfaces[range.start].order,
    }
}

#[cfg(debug_assertions)]
fn report_live_objects(device: &ID3D11Device) -> Result<()> {
    let debug_device: ID3D11Debug = device.cast()?;
    unsafe {
        debug_device.ReportLiveDeviceObjects(D3D11_RLDO_DETAIL)?;
    }
    Ok(())
}

const BUFFER_COUNT: usize = 3;

pub(crate) mod shader_resources {
    use anyhow::Result;

    #[cfg(debug_assertions)]
    use windows::{
        Win32::Graphics::Direct3D::{
            Fxc::{D3DCOMPILE_DEBUG, D3DCOMPILE_SKIP_OPTIMIZATION, D3DCompileFromFile},
            ID3DBlob,
        },
        core::{HSTRING, PCSTR},
    };

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub(crate) enum ShaderModule {
        Quad,
        Shadow,
        Underline,
        PathRasterization,
        PathSprite,
        MonochromeSprite,
        SubpixelSprite,
        PolychromeSprite,
        EmojiRasterization,
        BackdropBlur,
        BackdropGlass,
    }

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub(crate) enum ShaderTarget {
        Vertex,
        Fragment,
    }

    pub(crate) struct RawShaderBytes<'t> {
        inner: &'t [u8],

        #[cfg(debug_assertions)]
        _blob: ID3DBlob,
    }

    impl<'t> RawShaderBytes<'t> {
        pub(crate) fn new(module: ShaderModule, target: ShaderTarget) -> Result<Self> {
            #[cfg(not(debug_assertions))]
            {
                Ok(Self::from_bytes(module, target))
            }
            #[cfg(debug_assertions)]
            {
                let blob = build_shader_blob(module, target)?;
                let inner = unsafe {
                    std::slice::from_raw_parts(
                        blob.GetBufferPointer() as *const u8,
                        blob.GetBufferSize(),
                    )
                };
                Ok(Self { inner, _blob: blob })
            }
        }

        pub(crate) fn as_bytes(&'t self) -> &'t [u8] {
            self.inner
        }

        #[cfg(not(debug_assertions))]
        fn from_bytes(module: ShaderModule, target: ShaderTarget) -> Self {
            let bytes = match module {
                ShaderModule::Quad => match target {
                    ShaderTarget::Vertex => QUAD_VERTEX_BYTES,
                    ShaderTarget::Fragment => QUAD_FRAGMENT_BYTES,
                },
                ShaderModule::Shadow => match target {
                    ShaderTarget::Vertex => SHADOW_VERTEX_BYTES,
                    ShaderTarget::Fragment => SHADOW_FRAGMENT_BYTES,
                },
                ShaderModule::Underline => match target {
                    ShaderTarget::Vertex => UNDERLINE_VERTEX_BYTES,
                    ShaderTarget::Fragment => UNDERLINE_FRAGMENT_BYTES,
                },
                ShaderModule::PathRasterization => match target {
                    ShaderTarget::Vertex => PATH_RASTERIZATION_VERTEX_BYTES,
                    ShaderTarget::Fragment => PATH_RASTERIZATION_FRAGMENT_BYTES,
                },
                ShaderModule::PathSprite => match target {
                    ShaderTarget::Vertex => PATH_SPRITE_VERTEX_BYTES,
                    ShaderTarget::Fragment => PATH_SPRITE_FRAGMENT_BYTES,
                },
                ShaderModule::MonochromeSprite => match target {
                    ShaderTarget::Vertex => MONOCHROME_SPRITE_VERTEX_BYTES,
                    ShaderTarget::Fragment => MONOCHROME_SPRITE_FRAGMENT_BYTES,
                },
                ShaderModule::SubpixelSprite => match target {
                    ShaderTarget::Vertex => SUBPIXEL_SPRITE_VERTEX_BYTES,
                    ShaderTarget::Fragment => SUBPIXEL_SPRITE_FRAGMENT_BYTES,
                },
                ShaderModule::PolychromeSprite => match target {
                    ShaderTarget::Vertex => POLYCHROME_SPRITE_VERTEX_BYTES,
                    ShaderTarget::Fragment => POLYCHROME_SPRITE_FRAGMENT_BYTES,
                },
                ShaderModule::EmojiRasterization => match target {
                    ShaderTarget::Vertex => EMOJI_RASTERIZATION_VERTEX_BYTES,
                    ShaderTarget::Fragment => EMOJI_RASTERIZATION_FRAGMENT_BYTES,
                },
                ShaderModule::BackdropBlur => match target {
                    ShaderTarget::Vertex => BACKDROP_BLUR_VERTEX_BYTES,
                    ShaderTarget::Fragment => BACKDROP_BLUR_FRAGMENT_BYTES,
                },
                ShaderModule::BackdropGlass => match target {
                    ShaderTarget::Vertex => BACKDROP_GLASS_VERTEX_BYTES,
                    ShaderTarget::Fragment => BACKDROP_GLASS_FRAGMENT_BYTES,
                },
            };
            Self { inner: bytes }
        }
    }

    #[cfg(debug_assertions)]
    pub(super) fn build_shader_blob(entry: ShaderModule, target: ShaderTarget) -> Result<ID3DBlob> {
        unsafe {
            use windows::Win32::Graphics::{
                Direct3D::ID3DInclude, Hlsl::D3D_COMPILE_STANDARD_FILE_INCLUDE,
            };

            let shader_name = if matches!(entry, ShaderModule::EmojiRasterization) {
                "color_text_raster.hlsl"
            } else {
                "shaders.hlsl"
            };

            let entry = format!(
                "{}_{}\0",
                entry.as_str(),
                match target {
                    ShaderTarget::Vertex => "vertex",
                    ShaderTarget::Fragment => "fragment",
                }
            );
            let target = match target {
                ShaderTarget::Vertex => "vs_4_1\0",
                ShaderTarget::Fragment => "ps_4_1\0",
            };

            let mut compile_blob = None;
            let mut error_blob = None;
            let shader_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("src/{}", shader_name))
                .canonicalize()?;

            let entry_point = PCSTR::from_raw(entry.as_ptr());
            let target_cstr = PCSTR::from_raw(target.as_ptr());

            // really dirty trick because winapi bindings are unhappy otherwise
            let include_handler = &std::mem::transmute::<usize, ID3DInclude>(
                D3D_COMPILE_STANDARD_FILE_INCLUDE as usize,
            );

            let ret = D3DCompileFromFile(
                &HSTRING::from(
                    shader_path
                        .to_str()
                        .expect("required framework invariant must hold"),
                ),
                None,
                include_handler,
                entry_point,
                target_cstr,
                D3DCOMPILE_DEBUG | D3DCOMPILE_SKIP_OPTIMIZATION,
                0,
                &mut compile_blob,
                Some(&mut error_blob),
            );
            if ret.is_err() {
                let Some(error_blob) = error_blob else {
                    return Err(anyhow::anyhow!("{ret:?}"));
                };

                let error_string =
                    std::ffi::CStr::from_ptr(error_blob.GetBufferPointer() as *const i8)
                        .to_string_lossy();
                log::error!("Shader compile error: {}", error_string);
                return Err(anyhow::anyhow!("Compile error: {}", error_string));
            }
            Ok(compile_blob.expect("required framework invariant must hold"))
        }
    }

    #[cfg(not(debug_assertions))]
    include!(concat!(env!("OUT_DIR"), "/shaders_bytes.rs"));

    #[cfg(debug_assertions)]
    impl ShaderModule {
        pub fn as_str(self) -> &'static str {
            match self {
                ShaderModule::Quad => "quad",
                ShaderModule::Shadow => "shadow",
                ShaderModule::Underline => "underline",
                ShaderModule::PathRasterization => "path_rasterization",
                ShaderModule::PathSprite => "path_sprite",
                ShaderModule::MonochromeSprite => "monochrome_sprite",
                ShaderModule::SubpixelSprite => "subpixel_sprite",
                ShaderModule::PolychromeSprite => "polychrome_sprite",
                ShaderModule::EmojiRasterization => "emoji_rasterization",
                // Both backdrop passes share one vertex shader, which draws
                // the same full-viewport strip: only the fragment differs.
                ShaderModule::BackdropBlur => "backdrop_blur",
                ShaderModule::BackdropGlass => "backdrop_glass",
            }
        }
    }
}

mod nvidia {
    use std::{
        ffi::CStr,
        os::raw::{c_char, c_int, c_uint},
    };

    use anyhow::Result;
    use windows::{Win32::System::LibraryLoader::GetProcAddress, core::s};

    use crate::with_dll_library;

    // https://github.com/NVIDIA/nvapi/blob/7cb76fce2f52de818b3da497af646af1ec16ce27/nvapi_lite_common.h#L180
    const NVAPI_SHORT_STRING_MAX: usize = 64;

    // https://github.com/NVIDIA/nvapi/blob/7cb76fce2f52de818b3da497af646af1ec16ce27/nvapi_lite_common.h#L235
    #[allow(non_camel_case_types)]
    type NvAPI_ShortString = [c_char; NVAPI_SHORT_STRING_MAX];

    // https://github.com/NVIDIA/nvapi/blob/7cb76fce2f52de818b3da497af646af1ec16ce27/nvapi_lite_common.h#L447
    #[allow(non_camel_case_types)]
    type NvAPI_SYS_GetDriverAndBranchVersion_t = unsafe extern "C" fn(
        driver_version: *mut c_uint,
        build_branch_string: *mut NvAPI_ShortString,
    ) -> c_int;

    pub(super) fn get_driver_version() -> Result<String> {
        #[cfg(target_pointer_width = "64")]
        let nvidia_dll_name = s!("nvapi64.dll");
        #[cfg(target_pointer_width = "32")]
        let nvidia_dll_name = s!("nvapi.dll");

        with_dll_library(nvidia_dll_name, |nvidia_dll| unsafe {
            let nvapi_query_addr = GetProcAddress(nvidia_dll, s!("nvapi_QueryInterface"))
                .ok_or_else(|| anyhow::anyhow!("Failed to get nvapi_QueryInterface address"))?;
            let nvapi_query: extern "C" fn(u32) -> *mut () = std::mem::transmute(nvapi_query_addr);

            // https://github.com/NVIDIA/nvapi/blob/7cb76fce2f52de818b3da497af646af1ec16ce27/nvapi_interface.h#L41
            let nvapi_get_driver_version_ptr = nvapi_query(0x2926aaad);
            if nvapi_get_driver_version_ptr.is_null() {
                anyhow::bail!("Failed to get NVIDIA driver version function pointer");
            }
            let nvapi_get_driver_version: NvAPI_SYS_GetDriverAndBranchVersion_t =
                std::mem::transmute(nvapi_get_driver_version_ptr);

            let mut driver_version: c_uint = 0;
            let mut build_branch_string: NvAPI_ShortString = [0; NVAPI_SHORT_STRING_MAX];
            let result = nvapi_get_driver_version(
                &mut driver_version as *mut c_uint,
                &mut build_branch_string as *mut NvAPI_ShortString,
            );

            if result != 0 {
                anyhow::bail!(
                    "Failed to get NVIDIA driver version, error code: {}",
                    result
                );
            }
            let major = driver_version / 100;
            let minor = driver_version % 100;
            let branch_string = CStr::from_ptr(build_branch_string.as_ptr());
            Ok(format!(
                "{}.{} {}",
                major,
                minor,
                branch_string.to_string_lossy()
            ))
        })
    }
}

mod amd {
    use std::os::raw::{c_char, c_int, c_void};

    use anyhow::Result;
    use windows::{Win32::System::LibraryLoader::GetProcAddress, core::s};

    use crate::with_dll_library;

    // https://github.com/GPUOpen-LibrariesAndSDKs/AGS_SDK/blob/5d8812d703d0335741b6f7ffc37838eeb8b967f7/ags_lib/inc/amd_ags.h#L145
    const AGS_CURRENT_VERSION: i32 = (6 << 22) | (3 << 12);

    // https://github.com/GPUOpen-LibrariesAndSDKs/AGS_SDK/blob/5d8812d703d0335741b6f7ffc37838eeb8b967f7/ags_lib/inc/amd_ags.h#L204
    // This is an opaque type, using struct to represent it properly for FFI
    #[repr(C)]
    struct AGSContext {
        _private: [u8; 0],
    }

    #[repr(C)]
    pub struct AGSGPUInfo {
        pub driver_version: *const c_char,
        pub radeon_software_version: *const c_char,
        pub num_devices: c_int,
        pub devices: *mut c_void,
    }

    // https://github.com/GPUOpen-LibrariesAndSDKs/AGS_SDK/blob/5d8812d703d0335741b6f7ffc37838eeb8b967f7/ags_lib/inc/amd_ags.h#L429
    #[allow(non_camel_case_types)]
    type agsInitialize_t = unsafe extern "C" fn(
        version: c_int,
        config: *const c_void,
        context: *mut *mut AGSContext,
        gpu_info: *mut AGSGPUInfo,
    ) -> c_int;

    // https://github.com/GPUOpen-LibrariesAndSDKs/AGS_SDK/blob/5d8812d703d0335741b6f7ffc37838eeb8b967f7/ags_lib/inc/amd_ags.h#L436
    #[allow(non_camel_case_types)]
    type agsDeInitialize_t = unsafe extern "C" fn(context: *mut AGSContext) -> c_int;

    pub(super) fn get_driver_version() -> Result<String> {
        #[cfg(target_pointer_width = "64")]
        let amd_dll_name = s!("amd_ags_x64.dll");
        #[cfg(target_pointer_width = "32")]
        let amd_dll_name = s!("amd_ags_x86.dll");

        with_dll_library(amd_dll_name, |amd_dll| unsafe {
            let ags_initialize_addr = GetProcAddress(amd_dll, s!("agsInitialize"))
                .ok_or_else(|| anyhow::anyhow!("Failed to get agsInitialize address"))?;
            let ags_deinitialize_addr = GetProcAddress(amd_dll, s!("agsDeInitialize"))
                .ok_or_else(|| anyhow::anyhow!("Failed to get agsDeInitialize address"))?;

            let ags_initialize: agsInitialize_t = std::mem::transmute(ags_initialize_addr);
            let ags_deinitialize: agsDeInitialize_t = std::mem::transmute(ags_deinitialize_addr);

            let mut context: *mut AGSContext = std::ptr::null_mut();
            let mut gpu_info: AGSGPUInfo = AGSGPUInfo {
                driver_version: std::ptr::null(),
                radeon_software_version: std::ptr::null(),
                num_devices: 0,
                devices: std::ptr::null_mut(),
            };

            let result = ags_initialize(
                AGS_CURRENT_VERSION,
                std::ptr::null(),
                &mut context,
                &mut gpu_info,
            );
            if result != 0 {
                anyhow::bail!("Failed to initialize AMD AGS, error code: {}", result);
            }

            // Vulkan actually returns this as the driver version
            let software_version = if !gpu_info.radeon_software_version.is_null() {
                std::ffi::CStr::from_ptr(gpu_info.radeon_software_version)
                    .to_string_lossy()
                    .into_owned()
            } else {
                "Unknown Radeon Software Version".to_string()
            };

            let driver_version = if !gpu_info.driver_version.is_null() {
                std::ffi::CStr::from_ptr(gpu_info.driver_version)
                    .to_string_lossy()
                    .into_owned()
            } else {
                "Unknown Radeon Driver Version".to_string()
            };

            ags_deinitialize(context);
            Ok(format!("{} ({})", software_version, driver_version))
        })
    }
}

mod dxgi {
    use windows::{
        Win32::Graphics::Dxgi::{IDXGIAdapter1, IDXGIDevice},
        core::Interface,
    };

    pub(super) fn get_driver_version(adapter: &IDXGIAdapter1) -> anyhow::Result<String> {
        let number = unsafe { adapter.CheckInterfaceSupport(&IDXGIDevice::IID as _) }?;
        Ok(format!(
            "{}.{}.{}.{}",
            number >> 48,
            (number >> 32) & 0xFFFF,
            (number >> 16) & 0xFFFF,
            number & 0xFFFF
        ))
    }
}
