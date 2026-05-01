use egui::{ClippedPrimitive, Context};
use egui_wgpu::renderer::ScreenDescriptor;

pub struct GuiHandler {
    ctx: egui::Context,
    pub renderer: egui_wgpu::Renderer,
    state: egui_winit::State,
}

impl GuiHandler {
    pub fn new(
        window: &winit::window::Window,
        format: wgpu::TextureFormat,
        device: &wgpu::Device,
    ) -> Self {
        let ctx = egui::Context::default();
        let mut state = egui_winit::State::new(&window);
        state.set_pixels_per_point(window.scale_factor() as f32);

        let renderer = egui_wgpu::Renderer::new(device, format, None, 1);

        Self {
            ctx,
            renderer,
            state,
        }
    }

    pub fn handle_event(&mut self, event: &winit::event::Event<()>) -> bool {
        match event {
            winit::event::Event::WindowEvent {
                window_id: _,
                event,
            } => self.state.on_event(&self.ctx, event).consumed,
            _ => false,
        }
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        window: &winit::window::Window,
        surface_size: [u32; 2],
        encoder: &mut wgpu::CommandEncoder,
        gui: &mut dyn FnMut(&Context),
    ) -> (Vec<ClippedPrimitive>, ScreenDescriptor) {
        // IMPORTANT: build the screen descriptor from the *actual* configured
        // surface size, not from `window.inner_size()`. When the user drags the
        // window between monitors with different DPI scale factors, winit may
        // report `inner_size()` at the new monitor's physical size before the
        // `Resized`/`ScaleFactorChanged` event has caused us to reconfigure
        // the wgpu surface. egui-wgpu uses `size_in_pixels` to clamp scissor
        // rects, and emits a final `set_scissor_rect(0, 0, w, h)`. If those
        // dimensions exceed the render target (the surface texture, sized to
        // the last `configure()` call), wgpu fires a validation error like:
        //   Scissor Rect { x:0, y:0, w:1600, h:902 } is not contained in the
        //   render target Extent3d { width:800, height:451, ... }
        // Sourcing from `surface_size` (= self.config.width/height) makes the
        // scissor coords match the render target by construction.
        let screen_descriptor = {
            let scale_factor = window.scale_factor() as f32;
            // Guard against a zero/negative scale factor that would cause
            // egui to divide by zero when computing logical sizes.
            let pixels_per_point = if scale_factor > 0.0 { scale_factor } else { 1.0 };
            egui_wgpu::renderer::ScreenDescriptor {
                size_in_pixels: surface_size,
                pixels_per_point,
            }
        };

        let raw_input: egui::RawInput = self.state.take_egui_input(window);
        self.ctx.begin_frame(raw_input);
        gui(&self.ctx);
        let full_output = self.ctx.end_frame();

        self.state
            .handle_platform_output(window, &self.ctx, full_output.platform_output);

        let clipped_primitives = self.ctx.tessellate(full_output.shapes);

        self.renderer.update_buffers(
            device,
            queue,
            encoder,
            &clipped_primitives,
            &screen_descriptor,
        );
        for (tex_id, img_delta) in full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, tex_id, &img_delta);
        }
        for tex_id in full_output.textures_delta.free {
            self.renderer.free_texture(&tex_id);
        }

        (clipped_primitives, screen_descriptor)
    }
}
