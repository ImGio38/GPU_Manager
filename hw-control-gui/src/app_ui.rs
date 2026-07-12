use eframe::egui;
use hw_control_core::{IpcRequest, IpcResponse, SystemStatus, GpuMode, CurvePoint};
use crate::socket_client::send_request;
use std::time::Instant;

pub struct AppUi {
    status: Option<SystemStatus>,
    error_message: Option<String>,
    active_tab: Tab,
    cpu_curve_edit: Vec<CurvePoint>,
    gpu_curve_edit: Vec<CurvePoint>,
    drag_index: Option<usize>,
    last_poll_time: Instant,
    unsaved_cpu_changes: bool,
    unsaved_gpu_changes: bool,
    daemon_connected: bool,
    confirm_uninstall: bool,
}

#[derive(PartialEq, Eq)]
enum Tab {
    CpuCurve,
    GpuCurve,
}

impl Default for AppUi {
    fn default() -> Self {
        Self {
            status: None,
            error_message: None,
            active_tab: Tab::CpuCurve,
            cpu_curve_edit: Vec::new(),
            gpu_curve_edit: Vec::new(),
            drag_index: None,
            last_poll_time: Instant::now() - std::time::Duration::from_secs(10), // force immediate poll
            unsaved_cpu_changes: false,
            unsaved_gpu_changes: false,
            daemon_connected: false,
            confirm_uninstall: false,
        }
    }
}

impl AppUi {
    pub fn new() -> Self {
        Self::default()
    }

    /// Poll daemon state via Unix Domain Socket
    fn poll_daemon(&mut self) {
        match send_request(&IpcRequest::GetStatus) {
            Ok(IpcResponse::Status(status)) => {
                // Only sync the edit curves if the user is not actively dragging or has unsaved changes
                if self.drag_index.is_none() {
                    if !self.unsaved_cpu_changes {
                        self.cpu_curve_edit = status.cpu_curve.clone();
                    }
                    if !self.unsaved_gpu_changes {
                        self.gpu_curve_edit = status.gpu_curve.clone();
                    }
                }
                self.status = Some(status);
                self.error_message = None;
                self.daemon_connected = true;
            }
            Ok(IpcResponse::Error(err)) => {
                self.error_message = Some(format!("Daemon reported error: {}", err));
                self.daemon_connected = false;
            }
            Ok(_) => {
                self.error_message = Some("Unexpected response from daemon".to_string());
                self.daemon_connected = false;
            }
            Err(err) => {
                self.error_message = Some(err);
                self.daemon_connected = false;
                self.status = None;
            }
        }
    }

    /// Set the GPU mode via IPC
    fn set_gpu_mode(&mut self, mode: GpuMode) {
        match send_request(&IpcRequest::SetGpuMode(mode)) {
            Ok(IpcResponse::Ok) => {
                self.poll_daemon();
            }
            Ok(IpcResponse::Error(err)) => {
                self.error_message = Some(format!("Failed to switch GPU mode: {}", err));
            }
            Ok(_) => {
                self.error_message = Some("Unexpected response from daemon".to_string());
            }
            Err(err) => {
                self.error_message = Some(format!("IPC connection failed: {}", err));
            }
        }
    }

    /// Apply active fan curve edits to daemon
    fn apply_fan_curve(&mut self, name: &str) {
        let points = if name == "cpu" {
            &self.cpu_curve_edit
        } else {
            &self.gpu_curve_edit
        };

        let request = IpcRequest::SetFanCurve {
            name: name.to_string(),
            points: points.clone(),
        };

        match send_request(&request) {
            Ok(IpcResponse::Ok) => {
                if name == "cpu" {
                    self.unsaved_cpu_changes = false;
                } else {
                    self.unsaved_gpu_changes = false;
                }
                self.poll_daemon();
            }
            Ok(IpcResponse::Error(err)) => {
                self.error_message = Some(format!("Failed to update fan curve: {}", err));
            }
            Ok(_) => {
                self.error_message = Some("Unexpected response from daemon".to_string());
            }
            Err(err) => {
                self.error_message = Some(format!("IPC connection failed: {}", err));
            }
        }
    }

    /// Discard current edits and pull curves from daemon
    fn discard_changes(&mut self) {
        self.unsaved_cpu_changes = false;
        self.unsaved_gpu_changes = false;
        self.poll_daemon();
    }
}

impl eframe::App for AppUi {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll status every 1.5 seconds
        if self.status.is_none() || self.last_poll_time.elapsed().as_secs_f32() > 1.5 {
            self.poll_daemon();
            self.last_poll_time = Instant::now();
        }

        // Apply dark mode visuals
        ctx.set_visuals(egui::Visuals::dark());

        // Request continuous repaint to show real-time temperature updates
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        // 1. Header Area
        egui::TopBottomPanel::top("header_panel").show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Linux Hardware Controller").font(egui::FontId::proportional(20.0)).strong().color(egui::Color32::from_rgb(0, 220, 255)));
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.daemon_connected {
                            ui.label(egui::RichText::new("CONNECTED").font(egui::FontId::proportional(11.0)).strong().color(egui::Color32::from_rgb(0, 230, 110)));
                            ui.painter().circle_filled(ui.available_rect_before_wrap().right_top() + egui::vec2(-8.0, 12.0), 5.0, egui::Color32::from_rgb(0, 230, 110));
                        } else {
                            ui.label(egui::RichText::new("DAEMON OFFLINE").font(egui::FontId::proportional(11.0)).strong().color(egui::Color32::from_rgb(255, 60, 60)));
                            ui.painter().circle_filled(ui.available_rect_before_wrap().right_top() + egui::vec2(-8.0, 12.0), 5.0, egui::Color32::from_rgb(255, 60, 60));
                        }
                    });
                });
                ui.add_space(8.0);
            });
        });

        // 2. Main Content
        egui::CentralPanel::default().show(ctx, |ui| {
            // Display error messages prominently
            if let Some(ref err) = self.error_message {
                ui.add_space(5.0);
                ui.group(|ui| {
                    ui.style_mut().visuals.extreme_bg_color = egui::Color32::from_rgb(50, 10, 10);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("⚠").font(egui::FontId::proportional(16.0)).color(egui::Color32::from_rgb(255, 80, 80)));
                        ui.label(egui::RichText::new(err).color(egui::Color32::from_rgb(255, 120, 120)).strong());
                    });
                });
            }

            ui.add_space(10.0);

            // Row 1: GPU Switching & Telemetry Sensors
            ui.columns(2, |columns| {
                // Column 1: GPU switching buttons
                columns[0].vertical(|ui| {
                    ui.label(egui::RichText::new("GPU SWITCHING MUX").font(egui::FontId::proportional(13.0)).strong());
                    ui.add_space(8.0);

                    let current_mode = self.status.as_ref().map(|s| s.current_gpu_mode).unwrap_or(GpuMode::Hybrid);

                    // 1. Integrated Button
                    let is_integrated = current_mode == GpuMode::Integrated;
                    let int_btn = ui.add_sized(
                        [ui.available_width(), 45.0],
                        egui::Button::new(
                            egui::RichText::new("Integrated Mode")
                                .font(egui::FontId::proportional(14.0))
                                .strong()
                        )
                        .selected(is_integrated)
                    );
                    if int_btn.clicked() {
                        self.set_gpu_mode(GpuMode::Integrated);
                    }
                    ui.label(egui::RichText::new("Disable dGPU completely. Unloads modules and triggers RTD3/power off to save power.").weak().font(egui::FontId::proportional(11.0)));
                    ui.add_space(12.0);

                    // 2. Hybrid Button
                    let is_hybrid = current_mode == GpuMode::Hybrid;
                    let hyb_btn = ui.add_sized(
                        [ui.available_width(), 45.0],
                        egui::Button::new(
                            egui::RichText::new("Hybrid Mode")
                                .font(egui::FontId::proportional(14.0))
                                .strong()
                        )
                        .selected(is_hybrid)
                    );
                    if hyb_btn.clicked() {
                        self.set_gpu_mode(GpuMode::Hybrid);
                    }
                    ui.label(egui::RichText::new("Default dynamic switching. Keep dGPU active but powered down when not rendering via PRIME.").weak().font(egui::FontId::proportional(11.0)));
                    ui.add_space(12.0);

                    // 3. Dedicated Button
                    let is_dedicated = current_mode == GpuMode::Dedicated;
                    let ded_btn = ui.add_sized(
                        [ui.available_width(), 45.0],
                        egui::Button::new(
                            egui::RichText::new("Dedicated Mode (MUX)")
                                .font(egui::FontId::proportional(14.0))
                                .strong()
                        )
                        .selected(is_dedicated)
                    );
                    if ded_btn.clicked() {
                        self.set_gpu_mode(GpuMode::Dedicated);
                    }
                    ui.label(egui::RichText::new("Force dGPU as primary display output. Sets hardware MUX ACPI controls for maximum FPS.").weak().font(egui::FontId::proportional(11.0)));
                });

                // Column 2: System Telemetry
                columns[1].vertical(|ui| {
                    ui.label(egui::RichText::new("SYSTEM TELEMETRY").font(egui::FontId::proportional(13.0)).strong());
                    ui.add_space(8.0);

                    let (cpu_temp, cpu_speed, gpu_temp, gpu_speed) = match &self.status {
                        Some(s) => (s.cpu_temp, s.cpu_fan_speed, s.gpu_temp, s.gpu_fan_speed),
                        None => (None, None, None, None),
                    };

                    // CPU Temperature Card
                    ui.group(|ui| {
                        ui.set_min_height(60.0);
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("CPU Core Temperature").strong().color(egui::Color32::from_rgb(0, 200, 255)));
                                if let Some(t) = cpu_temp {
                                    ui.label(egui::RichText::new(format!("{:.1}°C", t)).font(egui::FontId::proportional(22.0)).strong());
                                    // Visual color slider for temp
                                    let bar_color = if t > 80.0 { egui::Color32::from_rgb(255, 80, 80) } else if t > 60.0 { egui::Color32::from_rgb(255, 160, 0) } else { egui::Color32::from_rgb(0, 220, 120) };
                                    ui.add(egui::ProgressBar::new(t / 100.0).show_percentage().fill(bar_color));
                                } else {
                                    ui.label(egui::RichText::new("N/A").font(egui::FontId::proportional(22.0)).weak());
                                }
                            });
                        });
                    });

                    ui.add_space(10.0);

                    // CPU Fan Speed Card
                    ui.group(|ui| {
                        ui.set_min_height(60.0);
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("CPU Fan Speed").strong().color(egui::Color32::from_rgb(0, 200, 255)));
                                if let Some(s) = cpu_speed {
                                    let pct = (s as f32 / 255.0 * 100.0) as i32;
                                    ui.label(egui::RichText::new(format!("{} %", pct)).font(egui::FontId::proportional(22.0)).strong());
                                    ui.add(egui::ProgressBar::new(s as f32 / 255.0).fill(egui::Color32::from_rgb(0, 220, 255)));
                                } else {
                                    ui.label(egui::RichText::new("Auto / BIOS controlled").font(egui::FontId::proportional(15.0)).weak());
                                }
                            });
                        });
                    });

                    ui.add_space(10.0);

                    // GPU Temperature & Fan Card (If available)
                    ui.group(|ui| {
                        ui.set_min_height(60.0);
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("GPU Core & Fan Status").strong().color(egui::Color32::from_rgb(220, 0, 220)));
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new("Temperature").small().weak());
                                        if let Some(t) = gpu_temp {
                                            ui.label(egui::RichText::new(format!("{:.1}°C", t)).font(egui::FontId::proportional(16.0)).strong());
                                        } else {
                                            ui.label("N/A");
                                        }
                                    });
                                    ui.add_space(30.0);
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new("Fan Speed").small().weak());
                                        if let Some(s) = gpu_speed {
                                            let pct = (s as f32 / 255.0 * 100.0) as i32;
                                            ui.label(egui::RichText::new(format!("{} %", pct)).font(egui::FontId::proportional(16.0)).strong());
                                        } else {
                                            ui.label("Auto/OFF");
                                        }
                                    });
                                });
                            });
                        });
                    });
                });
            });

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(10.0);

            // Row 2: Fan Curve Canvas Editor
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("INTERACTIVE FAN CURVES").font(egui::FontId::proportional(13.0)).strong());
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.selectable_label(self.active_tab == Tab::GpuCurve, "GPU Curve").clicked() {
                        self.active_tab = Tab::GpuCurve;
                    }
                    if ui.selectable_label(self.active_tab == Tab::CpuCurve, "CPU Curve").clicked() {
                        self.active_tab = Tab::CpuCurve;
                    }
                });
            });

            ui.add_space(10.0);

            // Display active editor canvas
            match self.active_tab {
                Tab::CpuCurve => {
                    let stroke_color = egui::Color32::from_rgb(0, 180, 255); // Cyan
                    Self::draw_fan_curve_editor(
                        ui,
                        &mut self.cpu_curve_edit,
                        &mut self.drag_index,
                        &mut self.unsaved_cpu_changes,
                        stroke_color,
                    );
                    
                    ui.horizontal(|ui| {
                        if ui.add_enabled(self.unsaved_cpu_changes, egui::Button::new("Apply CPU Fan Curve").fill(egui::Color32::from_rgb(0, 120, 60))).clicked() {
                            self.apply_fan_curve("cpu");
                        }
                        if ui.add_enabled(self.unsaved_cpu_changes, egui::Button::new("Discard Edits")).clicked() {
                            self.discard_changes();
                        }
                        if self.unsaved_cpu_changes {
                            ui.label(egui::RichText::new("● You have unsaved CPU fan curve modifications").small().color(egui::Color32::from_rgb(255, 160, 0)));
                        }
                    });
                }
                Tab::GpuCurve => {
                    let stroke_color = egui::Color32::from_rgb(220, 0, 220); // Magenta
                    Self::draw_fan_curve_editor(
                        ui,
                        &mut self.gpu_curve_edit,
                        &mut self.drag_index,
                        &mut self.unsaved_gpu_changes,
                        stroke_color,
                    );

                    ui.horizontal(|ui| {
                        if ui.add_enabled(self.unsaved_gpu_changes, egui::Button::new("Apply GPU Fan Curve").fill(egui::Color32::from_rgb(100, 0, 100))).clicked() {
                            self.apply_fan_curve("gpu");
                        }
                        if ui.add_enabled(self.unsaved_gpu_changes, egui::Button::new("Discard Edits")).clicked() {
                            self.discard_changes();
                        }
                        if self.unsaved_gpu_changes {
                            ui.label(egui::RichText::new("● You have unsaved GPU fan curve modifications").small().color(egui::Color32::from_rgb(255, 160, 0)));
                        }
                    });
                }
            }

            ui.add_space(15.0);
            ui.separator();
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if !self.confirm_uninstall {
                    if ui.button(egui::RichText::new("Uninstall Application").color(egui::Color32::from_rgb(255, 100, 100))).clicked() {
                        self.confirm_uninstall = true;
                    }
                } else {
                    ui.label(egui::RichText::new("Are you sure? This completely removes the daemon, GUI launcher, and configurations.").color(egui::Color32::from_rgb(255, 120, 120)).strong());
                    let btn = egui::Button::new(egui::RichText::new("Yes, Uninstall").color(egui::Color32::WHITE).strong())
                        .fill(egui::Color32::from_rgb(180, 0, 0));
                    if ui.add(btn).clicked() {
                        let _ = send_request(&IpcRequest::Uninstall);
                        std::process::exit(0);
                    }
                    if ui.button("Cancel").clicked() {
                        self.confirm_uninstall = false;
                    }
                }
            });
        });
    }
}

impl AppUi {
    /// Paint the interactive coordinate grid canvas for the fan curve
    fn draw_fan_curve_editor(
        ui: &mut egui::Ui,
        curve: &mut Vec<CurvePoint>,
        drag_index: &mut Option<usize>,
        unsaved_changes: &mut bool,
        stroke_color: egui::Color32,
    ) {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Drag points: Temperature (X-axis: 0-100°C) vs Fan Speed (Y-axis: 0-100%).").weak().font(egui::FontId::proportional(11.0)));
            ui.add_space(4.0);

            // Allocate painting space
            let canvas_width = ui.available_width();
            let canvas_size = egui::vec2(canvas_width, 210.0);
            
            // Adjust layouts and spacing for labels
            let (response, painter) = ui.allocate_painter(canvas_size, egui::Sense::drag());
            let rect = response.rect;

            // Draw dark background panel
            painter.rect_filled(rect, 6.0, egui::Color32::from_rgb(15, 20, 30));

            // Grid parameters
            let grid_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 10));
            let text_color = egui::Color32::from_rgb(130, 140, 155);
            let font = egui::FontId::proportional(9.0);

            // Draw vertical grid lines (every 10°C)
            for i in 0..=10 {
                let temp = i as f32 * 10.0;
                let x = rect.left() + (temp / 100.0) * rect.width();
                painter.line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], grid_stroke);

                // Draw X-axis label
                if i % 2 == 0 {
                    let label_pos = egui::pos2(x, rect.bottom() + 4.0);
                    painter.text(label_pos, egui::Align2::CENTER_TOP, format!("{}°C", temp as i32), font.clone(), text_color);
                }
            }

            // Draw horizontal grid lines (every 20% speed)
            for i in 0..=5 {
                let pct = i as f32 * 20.0;
                let y = rect.bottom() - (pct / 100.0) * rect.height();
                painter.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)], grid_stroke);

                // Draw Y-axis label
                let label_pos = egui::pos2(rect.left() + 5.0, y - 5.0);
                painter.text(label_pos, egui::Align2::LEFT_CENTER, format!("{}%", pct as i32), font.clone(), text_color);
            }

            if curve.is_empty() {
                // If config was not successfully read from daemon yet, draw a placeholder loading notice
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, "No curve coordinates. Waiting for daemon...", egui::FontId::proportional(14.0), egui::Color32::GRAY);
                return;
            }

            let pointer_pos = response.interact_pointer_pos();

            // Drag start detection
            if response.drag_started() {
                if let Some(m_pos) = pointer_pos {
                    let mut closest_idx = None;
                    let mut min_dist = 12.0; // max pixel distance to grab a handle
                    
                    for (idx, pt) in curve.iter().enumerate() {
                        let pt_x = rect.left() + (pt.temp / 100.0) * rect.width();
                        let pt_y = rect.bottom() - (pt.speed as f32 / 255.0) * rect.height();
                        let pt_screen = egui::pos2(pt_x, pt_y);
                        let dist = m_pos.distance(pt_screen);
                        if dist < min_dist {
                            min_dist = dist;
                            closest_idx = Some(idx);
                        }
                    }
                    *drag_index = closest_idx;
                }
            }

            // Continuous dragging update
            if response.dragged() {
                if let Some(idx) = *drag_index {
                    if let Some(m_pos) = pointer_pos {
                        let raw_temp = ((m_pos.x - rect.left()) / rect.width()) * 100.0;
                        let raw_speed = ((rect.bottom() - m_pos.y) / rect.height()) * 255.0;

                        // Ensure points remain sorted by temperature
                        let min_temp = if idx > 0 { curve[idx - 1].temp + 1.0 } else { 0.0 };
                        let max_temp = if idx < curve.len() - 1 { curve[idx + 1].temp - 1.0 } else { 100.0 };

                        curve[idx].temp = raw_temp.clamp(min_temp, max_temp);
                        curve[idx].speed = raw_speed.clamp(0.0, 255.0) as u8;
                        *unsaved_changes = true;
                    }
                }
            }

            // Drag end
            if response.drag_stopped() {
                *drag_index = None;
            }

            // Draw line segments connecting coordinates
            for i in 0..curve.len() - 1 {
                let p1 = &curve[i];
                let p2 = &curve[i + 1];
                let x1 = rect.left() + (p1.temp / 100.0) * rect.width();
                let y1 = rect.bottom() - (p1.speed as f32 / 255.0) * rect.height();
                let x2 = rect.left() + (p2.temp / 100.0) * rect.width();
                let y2 = rect.bottom() - (p2.speed as f32 / 255.0) * rect.height();

                painter.line_segment([egui::pos2(x1, y1), egui::pos2(x2, y2)], egui::Stroke::new(3.0, stroke_color));
            }

            // Draw point handles
            for (idx, pt) in curve.iter().enumerate() {
                let x = rect.left() + (pt.temp / 100.0) * rect.width();
                let y = rect.bottom() - (pt.speed as f32 / 255.0) * rect.height();
                let pt_screen = egui::pos2(x, y);

                let is_hovered = pointer_pos.map_or(false, |m_pos| m_pos.distance(pt_screen) < 8.0);
                let is_dragging = *drag_index == Some(idx);

                let color = if is_dragging {
                    egui::Color32::WHITE
                } else if is_hovered {
                    egui::Color32::from_rgb(
                        stroke_color.r().saturating_add(45),
                        stroke_color.g().saturating_add(45),
                        stroke_color.b().saturating_add(45),
                    )
                } else {
                    stroke_color
                };

                let radius = if is_hovered || is_dragging { 7.0 } else { 5.0 };

                // Draw outer glowing halo
                painter.circle_filled(pt_screen, radius + 2.0, color.linear_multiply(0.35));
                // Draw inner solid handle
                painter.circle_filled(pt_screen, radius, color);
                // Highlight core dot
                painter.circle_filled(pt_screen, 1.8, egui::Color32::WHITE);
            }

            ui.add_space(14.0); // Margin spacing for X-axis labels
        });
    }
}
