pub fn show_window(ctx: &mut egui::Context, pacer: &pacy::FramePacer) {
    egui::Window::new("Pacy Frame Pacer")
        .resizable(true)
        .show(ctx, |ui| {
            let internals = pacer.internals();
            if let Some(cpu_time) = internals.cpu_time_history.back() {
                ui.label(format!("CPU Time - {cpu_time:?}"));
            }
            if let Some(post_frame_time) = internals.cpu_post_frame_time_history.back() {
                ui.label(format!("Post Frame Time - {post_frame_time:?}"));
            }
            if let Some(sleep_time) = internals.cpu_sleep_time_history.back() {
                ui.label(format!("Sleep Time - {sleep_time:?}"));
            }
        });
}
