use std::time::Instant;

use flume::Receiver;
use winit::window::Window;

struct Event {
    winit: winit::event::Event<'static, ()>,
    #[allow(unused)]
    time: Instant,
}

fn main() {
    #[cfg(feature = "tracy")]
    tracy_client::Client::start();
    profiling::register_thread!("main event thread");

    let backends = wgpu::util::backend_bits_from_env().unwrap_or_else(wgpu::Backends::all);
    let instance = wgpu::Instance::new(backends);

    let el = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
        .with_title("Pacy Stress Test")
        .build(&el)
        .unwrap();

    let surface = unsafe { instance.create_surface(&window) };

    let (sender, reciever) = flume::unbounded();

    std::thread::spawn(move || {
        profiling::register_thread!("game thread");
        graphics_main(instance, backends, window, surface, reciever);
    });

    el.run(move |event, _el_wt, control_flow| {
        *control_flow = winit::event_loop::ControlFlow::Wait;

        if let Some(winit) = event.to_static() {
            sender
                .send(Event {
                    winit,
                    time: Instant::now(),
                })
                .unwrap();
        }
    });
}

fn graphics_main(
    instance: wgpu::Instance,
    backends: wgpu::Backends,
    window: Window,
    surface: wgpu::Surface,
    reciever: Receiver<Event>,
) {
    let adapter = pollster::block_on(wgpu::util::initialize_adapter_from_env_or_default(
        &instance,
        backends,
        Some(&surface),
    ))
    .unwrap();

    let features = adapter.features() & (wgpu::Features::TIMESTAMP_QUERY);

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            features,
            limits: wgpu::Limits::downlevel_webgl2_defaults(),
        },
        None,
    ))
    .unwrap();

    let mut size = window.inner_size();
    let scale_factor = window.scale_factor() as f32;
    let preferred_swapchain_format = surface.get_supported_formats(&adapter)[0];
    surface.configure(
        &device,
        &wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: preferred_swapchain_format,
            width: size.width,
            height: size.height,
            // FIFO is _always_ supported
            present_mode: wgpu::PresentMode::Fifo,
        },
    );

    let mut egui_platform =
        egui_winit_platform::Platform::new(egui_winit_platform::PlatformDescriptor {
            physical_width: size.width,
            physical_height: size.height,
            scale_factor: scale_factor as f64,
            font_definitions: egui::FontDefinitions::default(),
            style: egui::Style::default(),
        });

    let mut egui_renderpass =
        egui_wgpu_backend::RenderPass::new(&device, preferred_swapchain_format, 1);

    let mut pacer = pacy::FramePacer::new(get_monitor_frequency(&window));
    let cpu_stage = pacer.create_frame_stage(Instant::now());

    loop {
        while let Ok(event) = reciever.try_recv() {
            egui_platform.handle_event(&event.winit);

            if egui_platform.captures_event(&event.winit) {
                continue;
            }

            match event.winit {
                winit::event::Event::WindowEvent {
                    event: window_event,
                    ..
                } => match window_event {
                    winit::event::WindowEvent::Moved(..) => {
                        pacer.set_monitor_frequency(get_monitor_frequency(&window));
                    }
                    winit::event::WindowEvent::Resized(new_size) => {
                        size = new_size;
                        surface.configure(
                            &device,
                            &wgpu::SurfaceConfiguration {
                                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                                format: preferred_swapchain_format,
                                width: size.width,
                                height: size.height,
                                // FIFO is _always_ supported
                                present_mode: wgpu::PresentMode::Fifo,
                            },
                        );
                    }
                    _ => {}
                },
                // TODO: resume/suspend
                _ => {}
            }
        }

        profiling::scope!("Main Processing");
        pacer.begin_frame_stage(cpu_stage, Instant::now());

        egui_platform.begin_frame();

        let mut egui_ctx = egui_platform.context();
        pacy_egui::show_window(&mut egui_ctx, &mut pacer);

        let egui_output = egui_platform.end_frame(Some(&window));
        let egui_primitives = egui_ctx.tessellate(egui_output.shapes);

        let egui_screen_descriptor = egui_wgpu_backend::ScreenDescriptor {
            physical_width: size.width,
            physical_height: size.height,
            scale_factor,
        };
        egui_renderpass
            .add_textures(&device, &queue, &egui_output.textures_delta)
            .unwrap();
        egui_renderpass.update_buffers(&device, &queue, &egui_primitives, &egui_screen_descriptor);

        let image = surface.get_current_texture().unwrap();
        let image_view = image
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("primary"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &image_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        egui_renderpass
            .execute_with_renderpass(&mut rpass, &egui_primitives, &egui_screen_descriptor)
            .unwrap();

        drop(rpass);

        egui_renderpass
            .remove_textures(egui_output.textures_delta)
            .unwrap();

        profiling::finish_frame!();
        queue.submit(Some(encoder.finish()));

        image.present();

        pacer.end_frame_stage(cpu_stage, Instant::now());

        pacer.wait_for_frame();
    }
}

fn get_monitor_frequency(window: &winit::window::Window) -> f32 {
    window
        .current_monitor()
        .unwrap()
        .video_modes()
        .next()
        .unwrap()
        .refresh_rate() as f32
}
