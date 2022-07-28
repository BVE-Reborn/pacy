use std::time::Duration;

use egui::{
    plot::{GridMark, Line, Plot, Value, Values},
    Vec2,
};

pub fn show_window(ctx: &mut egui::Context, pacer: &mut pacy::FramePacer) {
    egui::Window::new("Pacy Frame Pacer")
        .resizable(true)
        .show(ctx, |ui| {
            ui.checkbox(&mut pacer.options.enabled, "Enable");
            let internals = pacer.internals();
            for (i, stage) in internals.frame_stages.iter().enumerate() {
                let estimated_time = stage.estimate_time_for_completion();
                ui.label(format!("Stage {i} - {}", ms_dur(estimated_time)));
            }
            if let Some(&sleep_time) = internals.sleep_history.back() {
                ui.label(format!("Sleep Time - {}", ms_dur(sleep_time)));
            }

            let max_value = internals.frame_stages[0]
                .duration_history
                .iter()
                .rev()
                .take(100)
                .max()
                .unwrap_or(&Duration::ZERO)
                .as_secs_f32();

            let seconds_per_frame = internals.monitor.reported_frequency.recip();
            let rounded_max_ms =
                ((max_value / seconds_per_frame).ceil() * seconds_per_frame) * 1000.0 * 1.2;
            let ms_per_frame = seconds_per_frame * 1000.0;

            Plot::new("Plot")
                .width(400.0)
                .height(200.0)
                .allow_drag(false)
                .allow_zoom(false)
                .allow_scroll(false)
                .include_y(rounded_max_ms)
                .include_y(0.0)
                .set_margin_fraction(Vec2::ZERO)
                .y_grid_spacer(move |_| {
                    let mut marks = Vec::new();
                    // Po2 multiples of frame rate
                    let mut ms = ms_per_frame;
                    while ms < rounded_max_ms {
                        marks.push(GridMark {
                            value: ms as f64,
                            step_size: rounded_max_ms as f64,
                        });
                        ms *= 2.0
                    }
                    // Any multiples of the frame rate
                    let mut ms = ms_per_frame;
                    while ms < rounded_max_ms {
                        marks.push(GridMark {
                            value: ms as f64,
                            step_size: 2.0,
                        });
                        ms += ms_per_frame
                    }
                    // Fixed ms
                    let mut ms = 0.0;
                    while ms < rounded_max_ms {
                        marks.push(GridMark {
                            value: ms as f64,
                            step_size: 1.0,
                        });
                        ms += 1.0
                    }

                    marks
                })
                .show(ui, |plot| {
                    let contiguous_history: Vec<_> =
                        internals.frame_stages[0].duration_history.iter().collect();
                    let iter = contiguous_history
                        .iter()
                        .copied()
                        .rev()
                        .take(100)
                        .enumerate()
                        .map(|(rev_index, duration)| Value {
                            x: 99.0 - rev_index as f64,
                            y: duration.as_secs_f64() * 1000.0,
                        });
                    plot.line(Line::new(Values::from_values_iter(iter)));
                    let iter2 = contiguous_history
                        .windows(40)
                        .map(|window| {
                            let sum: Duration = window.iter().copied().sum();
                            sum.as_secs_f64() / window.len() as f64
                        })
                        .rev()
                        .take(100)
                        .enumerate()
                        .map(|(rev_index, duration)| Value {
                            x: 99.0 - rev_index as f64,
                            y: duration * 1000.0,
                        });
                    plot.line(Line::new(Values::from_values_iter(iter2)));
                })
        });
}

fn ms_dur(duration: Duration) -> String {
    let ms = duration.as_secs_f32() * 1_000.0;
    format!("{ms:.3}ms")
}
