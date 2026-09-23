use super::application_layout_runtime::app_layout_text;
use super::*;

#[derive(Debug, Clone, Copy)]
struct ApplicationPickerGeometry {
    window_size: egui::Vec2,
    content_width: f32,
    list_height: f32,
}

fn application_picker_geometry(viewport: egui::Vec2, scale: f32) -> ApplicationPickerGeometry {
    let horizontal_margin = 24.0 * scale;
    let vertical_margin = 24.0 * scale;
    let available_width = (viewport.x - horizontal_margin * 2.0).max(1.0);
    let available_height = (viewport.y - vertical_margin * 2.0).max(1.0);
    let window_width = (680.0 * scale).min(available_width);
    let window_height = (520.0 * scale).min(available_height);
    let content_width = (window_width - 60.0 * scale)
        .max(220.0 * scale)
        .min((window_width - 20.0 * scale).max(1.0));
    let list_height = (window_height - 360.0 * scale).clamp(58.0 * scale, 180.0 * scale);

    ApplicationPickerGeometry {
        window_size: egui::vec2(window_width, window_height),
        content_width,
        list_height,
    }
}

fn centered_application_picker_content_rect(
    available: egui::Rect,
    requested_width: f32,
) -> egui::Rect {
    let width = requested_width.min(available.width()).max(1.0);
    egui::Rect::from_min_max(
        egui::pos2(available.center().x - width / 2.0, available.top()),
        egui::pos2(available.center().x + width / 2.0, available.bottom()),
    )
}

fn application_picker_content_ui<R>(
    ui: &mut egui::Ui,
    window_center_x: f32,
    requested_width: f32,
    add_contents: impl FnOnce(&mut egui::Ui, f32) -> R,
) -> egui::InnerResponse<R> {
    let available = ui.available_rect_before_wrap();
    let width = requested_width.min(available.width()).max(1.0);
    let left = (window_center_x - width / 2.0).clamp(
        available.left(),
        (available.right() - width).max(available.left()),
    );
    let content_rect = egui::Rect::from_min_max(
        egui::pos2(left, available.top()),
        egui::pos2(left + width, available.bottom()),
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(content_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.set_min_width(content_rect.width());
            ui.set_max_width(content_rect.width());
            add_contents(ui, content_rect.width())
        },
    )
}

impl EntropyApp {
    pub(super) fn draw_application_layouts_settings_page(
        &mut self,
        ui: &mut egui::Ui,
        content_rect: egui::Rect,
    ) {
        #[cfg(target_os = "linux")]
        self.poll_gnome_integration_install(ui.ctx());

        let language = self.app_settings.language;
        let metrics = crate::ui_style::ResponsiveMetrics::from_ctx(ui.ctx());
        let content_width = metrics.value(620.0);
        let title_y = content_rect.top() + metrics.value(30.0);
        let description_y = title_y + metrics.value(28.0);
        let body_top = description_y + metrics.value(26.0);
        let center_x = content_rect.center().x;
        let body_rect = egui::Rect::from_min_max(
            egui::pos2(center_x - content_width / 2.0, body_top),
            egui::pos2(center_x + content_width / 2.0, content_rect.bottom()),
        );
        let dark = ui.visuals().dark_mode;

        ui.painter().text(
            egui::pos2(center_x, title_y),
            egui::Align2::CENTER_CENTER,
            app_layout_text(language, "Раскладки приложений", "Application layouts"),
            egui::FontId::proportional(metrics.value(18.0)),
            ui.visuals().text_color(),
        );
        ui.painter().text(
            egui::pos2(center_x, description_y),
            egui::Align2::CENTER_CENTER,
            app_layout_text(
                language,
                "Автоматически меняйте клавиши и энкодер для приложения в фокусе",
                "Automatically switch keys and encoder for the focused application",
            ),
            egui::FontId::proportional(metrics.value(13.0)),
            app_muted_text(dark),
        );

        crate::ui_style::allocate_ui_at_rect(ui, content_rect, |ui| {
            if !self.application_layouts_supported() {
                ui.vertical_centered(|ui| {
                    ui.add_space(metrics.value(150.0));
                    ui.label(app_layout_text(
                        language,
                        "Подключите M4CR0Pad v3, чтобы настроить раскладки приложений.",
                        "Connect M4CR0Pad v3 to configure application layouts.",
                    ));
                });
                return;
            }

            self.ensure_application_layout_settings();
            crate::ui_style::allocate_ui_at_rect(ui, body_rect, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("application_layouts_settings")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        crate::ui_style::modal_content(
                            ui,
                            crate::ui_style::ModalLayout::new(content_width)
                                .with_top_padding(metrics.value(4.0)),
                            |ui| self.draw_application_layouts_editor(ui, metrics),
                        );
                    });
            });
        });
        self.draw_application_picker_v2(ui.ctx());
    }

    fn ensure_application_layout_settings(&mut self) {
        let Some(key) = self.application_layout_device_key() else {
            return;
        };
        let changed = self
            .app_settings
            .application_layouts
            .entry(key)
            .or_default()
            .normalize();
        if changed {
            save_app_settings(&self.app_settings);
        }
    }

    fn draw_application_layouts_editor(
        &mut self,
        ui: &mut egui::Ui,
        metrics: crate::ui_style::ResponsiveMetrics,
    ) {
        let language = self.app_settings.language;
        let Some(device_key) = self.application_layout_device_key() else {
            return;
        };
        let Some(snapshot) = self.app_settings.application_layouts.get(&device_key) else {
            return;
        };
        let mut selected_id = snapshot.editor_layout_id.clone();
        let mut layouts = snapshot
            .layouts
            .values()
            .map(|layout| (layout.id.clone(), layout.name.clone()))
            .collect::<Vec<_>>();
        layouts.sort_by(|left, right| {
            let left_default = left.0 == crate::application_layouts::DEFAULT_APPLICATION_LAYOUT_ID;
            let right_default =
                right.0 == crate::application_layouts::DEFAULT_APPLICATION_LAYOUT_ID;
            right_default
                .cmp(&left_default)
                .then_with(|| left.1.to_lowercase().cmp(&right.1.to_lowercase()))
        });
        let selected_name = snapshot
            .editor_layout()
            .map(|layout| layout.name.clone())
            .unwrap_or_else(|| "Default".to_owned());
        let mut automatically_return_to_default = snapshot.automatically_return_to_default;

        let row_width = metrics.value(602.0);
        let row_height = metrics.settings_row_height();
        let control_width = metrics.value(260.0);
        let control_height = metrics.settings_control_height();
        let control_font = metrics.settings_control_font_size();
        ui.spacing_mut().item_spacing.y = 0.0;

        self.draw_application_layouts_section(
            ui,
            row_width,
            row_height,
            app_layout_text(language, "Общие настройки", "General settings"),
        );

        crate::ui_style::settings_list_row_with_tooltip(
            ui,
            row_width,
            row_height,
            app_layout_text(
                language,
                "Автоматически возвращаться к Default",
                "Automatically return to Default",
            ),
            true,
            Some(app_layout_text(
                language,
                "Общая настройка для всех раскладок. Настроенные приложения всегда переключаются на свои раскладки. Если включено, любое другое окно возвращает Default. Если выключено, сохраняется последняя распознанная раскладка.",
                "Global setting for all layouts. Configured applications always switch to their own layouts. When enabled, any other window returns to Default. When disabled, the last recognized layout stays active.",
            )),
            metrics.value(46.0),
            |ui| {
                crate::ui_style::settings_switch_sized_stable(
                    ui,
                    "application_layout_automatic_return_default",
                    &mut automatically_return_to_default,
                    metrics.size(46.0, 24.0),
                );
            },
        );
        let mut changed = false;
        if let Some(settings) = self.app_settings.application_layouts.get_mut(&device_key) {
            if settings.automatically_return_to_default != automatically_return_to_default {
                settings.automatically_return_to_default = automatically_return_to_default;
                changed = true;
            }
        }

        self.draw_application_layouts_section(
            ui,
            row_width,
            row_height,
            app_layout_text(language, "Раскладки", "Layouts"),
        );

        crate::ui_style::settings_list_row(
            ui,
            row_width,
            row_height,
            app_layout_text(language, "Раскладка", "Layout"),
            true,
            control_width,
            |ui| {
                let dropdown_id = ui.make_persistent_id("application_layout_selector");
                let dropdown = crate::ui_style::modern_dropdown_button_sized(
                    ui,
                    dropdown_id,
                    &selected_name,
                    ui.visuals().text_color(),
                    control_width,
                    control_height,
                    control_font,
                );
                crate::ui_style::popup_below_widget(
                    ui,
                    dropdown_id,
                    &dropdown,
                    egui::PopupCloseBehavior::CloseOnClickOutside,
                    |ui| {
                        ui.set_min_width(control_width);
                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
                        for (id, name) in &layouts {
                            if ui.selectable_label(selected_id == *id, name).clicked() {
                                selected_id = id.clone();
                                egui::Popup::close_id(ui.ctx(), dropdown_id);
                            }
                        }
                    },
                );
            },
        );

        if let Some(settings) = self.app_settings.application_layouts.get_mut(&device_key) {
            if settings.editor_layout_id != selected_id {
                settings.editor_layout_id = selected_id.clone();
                changed = true;
            }
        }

        let selected = self
            .app_settings
            .application_layouts
            .get(&device_key)
            .and_then(|settings| settings.editor_layout())
            .cloned();
        if let Some(selected) = selected {
            let is_default =
                selected.id == crate::application_layouts::DEFAULT_APPLICATION_LAYOUT_ID;
            let executable = selected.executable.clone();
            let mut title_contains = selected.title_contains.clone();
            let mut automatic = selected.automatic_switching;

            crate::ui_style::settings_list_row(
                ui,
                row_width,
                row_height,
                app_layout_text(language, "Действия", "Actions"),
                true,
                control_width,
                |ui| {
                    let gap = metrics.value(8.0);
                    let button_width = (control_width - gap) / 2.0;
                    if crate::ui_style::modern_button(
                        ui,
                        app_layout_text(language, "Добавить…", "Add…"),
                        egui::vec2(button_width, control_height),
                        true,
                    )
                    .clicked()
                    {
                        self.open_application_picker(false);
                    }
                    ui.add_space(gap);
                    if crate::ui_style::modern_button(
                        ui,
                        app_layout_text(language, "Удалить", "Delete"),
                        egui::vec2(button_width, control_height),
                        !is_default,
                    )
                    .clicked()
                    {
                        if let Some(settings) =
                            self.app_settings.application_layouts.get_mut(&device_key)
                        {
                            changed |= settings.remove(&selected.id);
                        }
                    }
                },
            );

            if !is_default {
                self.draw_application_layouts_section(
                    ui,
                    row_width,
                    row_height,
                    &format!(
                        "{} · {}",
                        app_layout_text(language, "Настройки приложения", "Application settings"),
                        selected.name.as_str()
                    ),
                );
            }

            crate::ui_style::settings_list_row_with_tooltip(
                ui,
                row_width,
                row_height,
                app_layout_text(language, "Автопереключение", "Automatic switching"),
                !is_default,
                Some(app_layout_text(
                    language,
                    "Включать эту раскладку, когда связанное приложение находится в фокусе",
                    "Activate this layout when its associated application is focused",
                )),
                metrics.value(46.0),
                |ui| {
                    crate::ui_style::settings_switch_sized_stable(
                        ui,
                        "application_layout_automatic_switching",
                        &mut automatic,
                        metrics.size(46.0, 24.0),
                    );
                },
            );

            let mut application_display = if is_default {
                app_layout_text(
                    language,
                    "Для остальных приложений",
                    "All other applications",
                )
                .to_owned()
            } else {
                executable.clone()
            };
            crate::ui_style::settings_list_row(
                ui,
                row_width,
                row_height,
                app_layout_text(language, "Приложение", "Application"),
                !is_default,
                control_width,
                |ui| {
                    let gap = metrics.value(8.0);
                    let choose_width = metrics.value(82.0);
                    let field_width = control_width - choose_width - gap;
                    crate::ui_style::modern_text_field_interactive(
                        ui,
                        ui.make_persistent_id("application_layout_executable"),
                        &mut application_display,
                        field_width,
                        app_layout_text(language, "Исполняемый файл", "Executable"),
                        120,
                        egui::Align::Min,
                        false,
                    );
                    ui.add_space(gap);
                    if crate::ui_style::modern_button(
                        ui,
                        app_layout_text(language, "Изменить", "Edit"),
                        metrics.size(82.0, 32.0),
                        !is_default,
                    )
                    .clicked()
                    {
                        self.open_application_picker(true);
                    }
                },
            );

            crate::ui_style::settings_list_row_with_tooltip(
                ui,
                row_width,
                row_height,
                app_layout_text(
                    language,
                    "Фрагмент заголовка окна",
                    "Window title fragment",
                ),
                !is_default,
                Some(app_layout_text(
                    language,
                    "Необязательно. Используйте, только если одному приложению нужны разные раскладки — например, для разных проектов VS Code",
                    "Optional. Use only when one application needs separate layouts, for example for different VS Code projects",
                )),
                control_width,
                |ui| {
                    crate::ui_style::modern_text_field_interactive(
                        ui,
                        ui.make_persistent_id("application_layout_window_title"),
                        &mut title_contains,
                        control_width,
                        app_layout_text(language, "Необязательно", "Optional"),
                        120,
                        egui::Align::Min,
                        false,
                    );
                },
            );

            if let Some(settings) = self.app_settings.application_layouts.get_mut(&device_key) {
                if let Some(layout) = settings.layouts.get_mut(&selected.id) {
                    if !is_default && layout.automatic_switching != automatic {
                        layout.automatic_switching = automatic;
                        layout.bump_revision();
                        changed = true;
                    }
                }
            }
        }

        let detector = self.application_discovery.foreground_status.clone();
        let (detector_text, detector_ok) = match &detector.state {
            crate::app_discovery::ForegroundState::BackendUnavailable(error) => {
                (format!("{} — {error}", detector.backend), false)
            }
            crate::app_discovery::ForegroundState::Focused(_)
            | crate::app_discovery::ForegroundState::UnidentifiedWindow(_)
            | crate::app_discovery::ForegroundState::NoFocusedWindow => (
                format!(
                    "{} — {}",
                    detector.backend,
                    app_layout_text(language, "работает", "running")
                ),
                true,
            ),
        };
        crate::ui_style::settings_list_row(
            ui,
            row_width,
            row_height,
            app_layout_text(language, "Детектор окон", "Window detector"),
            true,
            control_width,
            |ui| {
                ui.add_sized(
                    [control_width, control_height],
                    egui::Label::new(RichText::new(detector_text).size(control_font).color(
                        if detector_ok {
                            app_muted_text(ui.visuals().dark_mode)
                        } else {
                            egui::Color32::from_rgb(220, 92, 76)
                        },
                    ))
                    .truncate(),
                );
            },
        );

        #[cfg(target_os = "linux")]
        let show_gnome_integration = crate::app_discovery::gnome_shell_integration_needed()
            && (matches!(
                detector.state,
                crate::app_discovery::ForegroundState::BackendUnavailable(_)
            ) || self.gnome_integration_install_task.is_some()
                || self.gnome_integration_install_result.is_some());
        #[cfg(not(target_os = "linux"))]
        let show_gnome_integration = false;

        if show_gnome_integration {
            crate::ui_style::settings_list_row(
                ui,
                row_width,
                row_height,
                app_layout_text(language, "Интеграция GNOME", "GNOME integration"),
                true,
                control_width,
                |ui| {
                    #[cfg(target_os = "linux")]
                    if self.gnome_integration_install_task.is_some() {
                        ui.allocate_ui_with_layout(
                            egui::vec2(control_width, control_height),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.spinner();
                                ui.label(app_layout_text(
                                    language,
                                    "Установка и проверка…",
                                    "Installing and verifying…",
                                ));
                            },
                        );
                    } else if crate::ui_style::modern_button(
                        ui,
                        app_layout_text(language, "Установить и включить", "Install and enable"),
                        egui::vec2(control_width, control_height),
                        true,
                    )
                    .clicked()
                    {
                        #[cfg(target_os = "linux")]
                        self.start_gnome_integration_install();
                    }
                },
            );

            #[cfg(target_os = "linux")]
            if self.gnome_integration_install_task.is_some()
                || self.gnome_integration_install_result.is_some()
            {
                let (message, color) =
                    self.gnome_integration_install_feedback(language, ui.visuals().dark_mode);
                crate::ui_style::settings_list_row(
                    ui,
                    row_width,
                    row_height,
                    app_layout_text(language, "Статус установки", "Installation status"),
                    true,
                    control_width,
                    |ui| {
                        ui.add_sized(
                            [control_width, control_height],
                            egui::Label::new(
                                RichText::new(&message).size(control_font).color(color),
                            )
                            .truncate(),
                        )
                        .on_hover_text(message);
                    },
                );
            }
        }

        let foreground = match &detector.state {
            crate::app_discovery::ForegroundState::Focused(application) => application.label(),
            crate::app_discovery::ForegroundState::UnidentifiedWindow(title) => {
                if title.trim().is_empty() {
                    app_layout_text(language, "неизвестное окно", "unidentified window").to_owned()
                } else {
                    format!(
                        "{} — {}",
                        app_layout_text(language, "неизвестное окно", "unidentified window"),
                        title.trim()
                    )
                }
            }
            crate::app_discovery::ForegroundState::NoFocusedWindow => {
                app_layout_text(language, "нет активного окна", "no focused window").to_owned()
            }
            crate::app_discovery::ForegroundState::BackendUnavailable(_) => {
                app_layout_text(language, "детектор недоступен", "detector unavailable").to_owned()
            }
        };
        crate::ui_style::settings_list_row(
            ui,
            row_width,
            row_height,
            app_layout_text(language, "Приложение в фокусе", "Focused application"),
            true,
            control_width,
            |ui| {
                ui.add_sized(
                    [control_width, control_height],
                    egui::Label::new(
                        RichText::new(foreground)
                            .size(control_font)
                            .color(app_muted_text(ui.visuals().dark_mode)),
                    )
                    .truncate(),
                );
            },
        );

        let active_layout = self
            .app_settings
            .application_layouts
            .get(&device_key)
            .and_then(|settings| settings.active_layout())
            .map(|layout| layout.name.clone())
            .unwrap_or_else(|| "Default".to_owned());
        crate::ui_style::settings_list_row(
            ui,
            row_width,
            row_height,
            app_layout_text(language, "Активная раскладка", "Active layout"),
            true,
            control_width,
            |ui| {
                ui.add_sized(
                    [control_width, control_height],
                    egui::Label::new(
                        RichText::new(active_layout)
                            .size(control_font)
                            .color(app_muted_text(ui.visuals().dark_mode)),
                    )
                    .truncate(),
                );
            },
        );

        if changed {
            save_app_settings(&self.app_settings);
        }
    }

    #[cfg(target_os = "linux")]
    fn start_gnome_integration_install(&mut self) {
        self.gnome_integration_install_result = None;
        match crate::app_discovery::start_gnome_shell_integration_install() {
            Ok(task) => self.gnome_integration_install_task = Some(task),
            Err(error) => self.gnome_integration_install_result = Some(Err(error)),
        }
    }

    #[cfg(target_os = "linux")]
    fn poll_gnome_integration_install(&mut self, ctx: &egui::Context) {
        let outcome = self
            .gnome_integration_install_task
            .as_ref()
            .map(crate::app_discovery::GnomeIntegrationInstallTask::try_recv);
        match outcome {
            Some(Ok(result)) => {
                self.gnome_integration_install_task = None;
                self.status_msg = match &result {
                    Ok(report) => report.message.clone(),
                    Err(error) => format!("GNOME integration: {error}"),
                };
                self.gnome_integration_install_result = Some(result);
                crate::app_discovery::refresh_application_discovery();
                ctx.request_repaint();
            }
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                self.gnome_integration_install_task = None;
                let error = "GNOME integration installer stopped without a result".to_owned();
                self.status_msg = error.clone();
                self.gnome_integration_install_result = Some(Err(error));
                ctx.request_repaint();
            }
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
            None => {}
        }
    }

    #[cfg(target_os = "linux")]
    fn gnome_integration_install_feedback(
        &self,
        language: crate::i18n::Language,
        dark: bool,
    ) -> (String, egui::Color32) {
        if self.gnome_integration_install_task.is_some() {
            return (
                app_layout_text(
                    language,
                    "Устанавливаю файлы и проверяю GNOME…",
                    "Installing files and checking GNOME…",
                )
                .to_owned(),
                app_muted_text(dark),
            );
        }
        match self.gnome_integration_install_result.as_ref() {
            Some(Ok(report)) if report.active => (
                app_layout_text(language, "Установлено и запущено", "Installed and running")
                    .to_owned(),
                egui::Color32::from_rgb(74, 170, 108),
            ),
            Some(Ok(report)) if report.restart_required => (
                app_layout_text(
                    language,
                    "Установлено. Выйдите из Ubuntu и войдите снова",
                    "Installed. Sign out of Ubuntu and sign back in",
                )
                .to_owned(),
                egui::Color32::from_rgb(218, 164, 70),
            ),
            Some(Ok(report)) if report.enabled => (
                report.message.clone(),
                egui::Color32::from_rgb(218, 164, 70),
            ),
            Some(Ok(report)) => (report.message.clone(), app_muted_text(dark)),
            Some(Err(error)) => (
                format!(
                    "{}: {error}",
                    app_layout_text(language, "Ошибка установки", "Installation failed")
                ),
                egui::Color32::from_rgb(220, 92, 76),
            ),
            None => (String::new(), app_muted_text(dark)),
        }
    }

    fn draw_application_layouts_section(
        &self,
        ui: &mut egui::Ui,
        width: f32,
        height: f32,
        title: &str,
    ) {
        let dark = ui.visuals().dark_mode;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            egui::Stroke::new(1.0, crate::ui_style::border_color(dark)),
        );
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            title,
            egui::FontId::proportional(12.5),
            app_muted_text(dark),
        );
    }

    fn open_application_picker(&mut self, assign_existing: bool) {
        self.application_picker_assign_existing = assign_existing;
        self.application_picker_open = true;
        self.application_picker_search.clear();
        self.application_manual_executable.clear();
        self.application_picker_selected = None;
        self.application_picker_layout_name.clear();
        self.application_picker_title_contains.clear();

        if assign_existing {
            let selected = self
                .application_layout_device_key()
                .and_then(|key| self.app_settings.application_layouts.get(&key))
                .and_then(|settings| settings.editor_layout())
                .filter(|layout| {
                    layout.id != crate::application_layouts::DEFAULT_APPLICATION_LAYOUT_ID
                })
                .cloned();
            if let Some(layout) = selected {
                self.application_picker_layout_name = layout.name.clone();
                self.application_picker_title_contains = layout.title_contains.clone();
                self.application_picker_selected =
                    Some(crate::application_layouts::DetectedApplication {
                        executable: layout.executable,
                        identities: layout.application_identities,
                        display_name: layout.name,
                        window_title: String::new(),
                    });
            }
        }
        crate::app_discovery::refresh_application_discovery();
    }

    fn application_picker_validation(&self) -> (bool, bool) {
        let Some(device_key) = self.application_layout_device_key() else {
            return (false, false);
        };
        let Some(settings) = self.app_settings.application_layouts.get(&device_key) else {
            return (false, false);
        };
        let excluding_id = self
            .application_picker_assign_existing
            .then_some(settings.editor_layout_id.as_str());
        let duplicate_name =
            settings.layout_name_exists(&self.application_picker_layout_name, excluding_id);
        let duplicate_rule = self
            .application_picker_selected
            .as_ref()
            .is_some_and(|application| {
                settings.application_rule_exists(
                    application,
                    &self.application_picker_title_contains,
                    excluding_id,
                )
            });
        (duplicate_name, duplicate_rule)
    }

    fn draw_application_picker_v2(&mut self, ctx: &egui::Context) {
        if !self.application_picker_open {
            return;
        }
        let language = self.app_settings.language;
        let metrics = crate::ui_style::ResponsiveMetrics::from_ctx(ctx);
        let geometry = application_picker_geometry(ctx.content_rect().size(), metrics.scale);
        let mut open = self.application_picker_open;
        let mut confirm = false;
        let mut cancel = false;
        let applications = self.application_discovery.available.clone();
        let search = self.application_picker_search.trim().to_ascii_lowercase();

        let picker_window_id = egui::Id::new("application_picker_v2");
        let window_center_x = ctx.content_rect().center().x;
        crate::ui_style::centered_modal_window(
            ctx,
            if self.application_picker_assign_existing {
                app_layout_text(language, "Изменить приложение", "Edit application")
            } else {
                app_layout_text(language, "Добавить приложение", "Add application")
            },
            picker_window_id,
            &mut open,
            geometry.window_size,
        )
        .frame(
            crate::ui_style::modal_window_frame(
                ctx.global_style().as_ref(),
                ctx.global_style().visuals.dark_mode,
            )
            .inner_margin(egui::Margin::symmetric(30, 10)),
        )
        .movable(false)
        .show(ctx, |ui| {
            application_picker_content_ui(
                ui,
                window_center_x,
                geometry.content_width,
                |ui, content_width| {
                    ui.add_space(metrics.value(4.0));
                    let (duplicate_name, duplicate_rule) = self.application_picker_validation();
                    ui.label(
                        RichText::new(app_layout_text(
                            language,
                            "Название раскладки",
                            "Layout name",
                        ))
                        .size(metrics.value(12.0))
                        .color(app_muted_text(ui.visuals().dark_mode)),
                    );
                    ui.add_space(metrics.value(6.0));
                    ui.scope(|ui| {
                        if duplicate_name {
                            ui.visuals_mut().override_text_color =
                                Some(egui::Color32::from_rgb(220, 92, 76));
                        }
                        crate::ui_style::modern_text_field_sized(
                            ui,
                            ui.make_persistent_id("application_picker_layout_name_v2"),
                            &mut self.application_picker_layout_name,
                            content_width,
                            metrics.settings_control_height(),
                            app_layout_text(language, "Название приложения", "Application name"),
                            64,
                            egui::Align::Min,
                        );
                    });
                    if duplicate_name {
                        ui.label(
                            RichText::new(app_layout_text(
                                language,
                                "Такое название уже существует",
                                "This name already exists",
                            ))
                            .size(metrics.value(11.0))
                            .color(egui::Color32::from_rgb(220, 92, 76)),
                        );
                    }

                    ui.add_space(metrics.value(10.0));
                    ui.label(
                        RichText::new(app_layout_text(
                            language,
                            "Фрагмент заголовка окна — необязательно",
                            "Window title fragment — optional",
                        ))
                        .size(metrics.value(12.0))
                        .color(app_muted_text(ui.visuals().dark_mode)),
                    );
                    ui.add_space(metrics.value(6.0));
                    crate::ui_style::modern_text_field_sized(
                        ui,
                        ui.make_persistent_id("application_picker_title_v2"),
                        &mut self.application_picker_title_contains,
                        content_width,
                        metrics.settings_control_height(),
                        app_layout_text(language, "Например: Прошивка", "For example: Firmware"),
                        120,
                        egui::Align::Min,
                    );
                    if duplicate_rule {
                        ui.label(
                            RichText::new(app_layout_text(
                                language,
                                "Для этого приложения уже есть раскладка с таким фильтром",
                                "A layout for this application and title filter already exists",
                            ))
                            .size(metrics.value(11.0))
                            .color(egui::Color32::from_rgb(220, 92, 76)),
                        );
                    }

                    ui.add_space(metrics.value(12.0));
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let refresh_width = metrics.value(104.0);
                        let gap = metrics.value(10.0);
                        crate::ui_style::modern_text_field_sized(
                            ui,
                            ui.make_persistent_id("application_picker_search_v2"),
                            &mut self.application_picker_search,
                            content_width - refresh_width - gap,
                            metrics.settings_control_height(),
                            app_layout_text(language, "Поиск приложения", "Search applications"),
                            120,
                            egui::Align::Min,
                        );
                        ui.add_space(gap);
                        if crate::ui_style::modern_button(
                            ui,
                            app_layout_text(language, "Обновить", "Refresh"),
                            metrics.size(104.0, 32.0),
                            true,
                        )
                        .clicked()
                        {
                            crate::app_discovery::refresh_application_discovery();
                        }
                    });
                    ui.add_space(metrics.value(10.0));

                    let filtered = applications
                        .iter()
                        .filter(|application| {
                            search.is_empty()
                                || application.label().to_ascii_lowercase().contains(&search)
                                || application
                                    .executable
                                    .to_ascii_lowercase()
                                    .contains(&search)
                        })
                        .collect::<Vec<_>>();
                    egui::Frame::new()
                        .fill(app_surface_fill(ui.visuals().dark_mode))
                        .stroke(crate::ui_style::modal_outline_stroke(
                            ui.visuals().dark_mode,
                        ))
                        .corner_radius(metrics.value(10.0))
                        .inner_margin(metrics.value(8.0))
                        .show(ui, |ui| {
                            ui.set_width(content_width - metrics.value(16.0));
                            ui.set_height(geometry.list_height);
                            egui::ScrollArea::vertical()
                                .id_salt("application_picker_list_v2")
                                .max_height(geometry.list_height)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    if filtered.is_empty() {
                                        crate::ui_style::modal_empty_state(
                                            ui,
                                            app_layout_text(
                                                language,
                                                "Приложения не найдены",
                                                "No applications found",
                                            ),
                                            None,
                                        );
                                    }
                                    for application in filtered {
                                        let (rect, response) = ui.allocate_exact_size(
                                            egui::vec2(ui.available_width(), metrics.value(58.0)),
                                            egui::Sense::click(),
                                        );
                                        if response.hovered() {
                                            ui.painter().rect_filled(
                                                rect,
                                                metrics.value(8.0),
                                                app_hover_fill(ui.visuals().dark_mode),
                                            );
                                            ui.ctx()
                                                .set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }
                                        let selected = self
                                            .application_picker_selected
                                            .as_ref()
                                            .is_some_and(|selected| {
                                                selected.executable == application.executable
                                                    && selected.identities == application.identities
                                            });
                                        if selected {
                                            ui.painter().rect_stroke(
                                                rect.shrink(metrics.value(1.0)),
                                                metrics.value(8.0),
                                                egui::Stroke::new(
                                                    metrics.value(1.5),
                                                    egui::Color32::from_rgb(218, 164, 70),
                                                ),
                                                egui::StrokeKind::Inside,
                                            );
                                        }
                                        let left = rect.left() + metrics.value(12.0);
                                        ui.painter().text(
                                            egui::pos2(left, rect.top() + metrics.value(17.0)),
                                            egui::Align2::LEFT_CENTER,
                                            application.label(),
                                            egui::FontId::proportional(metrics.value(13.0)),
                                            ui.visuals().text_color(),
                                        );
                                        ui.painter().text(
                                            egui::pos2(left, rect.top() + metrics.value(39.0)),
                                            egui::Align2::LEFT_CENTER,
                                            &application.executable,
                                            egui::FontId::proportional(metrics.value(11.0)),
                                            app_muted_text(ui.visuals().dark_mode),
                                        );
                                        if response.clicked() {
                                            self.application_picker_selected =
                                                Some((*application).clone());
                                            self.application_picker_layout_name =
                                                if application.display_name.trim().is_empty() {
                                                    application.executable.trim().to_owned()
                                                } else {
                                                    application.display_name.trim().to_owned()
                                                };
                                            ui.ctx().request_repaint();
                                        }
                                    }
                                });
                        });

                    ui.add_space(metrics.value(10.0));
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let use_width = metrics.value(104.0);
                        let gap = metrics.value(10.0);
                        crate::ui_style::modern_text_field_sized(
                            ui,
                            ui.make_persistent_id("application_picker_manual_v2"),
                            &mut self.application_manual_executable,
                            content_width - use_width - gap,
                            metrics.settings_control_height(),
                            app_layout_text(
                                language,
                                "Исполняемый файл: figma, code, blender",
                                "Executable: figma, code, blender",
                            ),
                            120,
                            egui::Align::Min,
                        );
                        ui.add_space(gap);
                        if crate::ui_style::modern_button(
                            ui,
                            app_layout_text(language, "Использовать", "Use"),
                            metrics.size(104.0, 32.0),
                            !self.application_manual_executable.trim().is_empty(),
                        )
                        .clicked()
                        {
                            let executable = self.application_manual_executable.trim().to_owned();
                            self.application_picker_selected =
                                Some(crate::application_layouts::DetectedApplication {
                                    display_name: executable.clone(),
                                    identities: vec![executable.clone()],
                                    executable: executable.clone(),
                                    window_title: String::new(),
                                });
                            self.application_picker_layout_name = executable;
                            ui.ctx().request_repaint();
                        }
                    });

                    ui.add_space(metrics.value(12.0));
                    let (duplicate_name, duplicate_rule) = self.application_picker_validation();
                    let can_confirm = self.application_picker_selected.is_some()
                        && !self.application_picker_layout_name.trim().is_empty()
                        && !duplicate_name
                        && !duplicate_rule;
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        confirm = crate::ui_style::modern_button(
                            ui,
                            if self.application_picker_assign_existing {
                                app_layout_text(language, "Сохранить", "Save")
                            } else {
                                app_layout_text(language, "Добавить", "Add")
                            },
                            metrics.size(120.0, 32.0),
                            can_confirm,
                        )
                        .clicked();
                        ui.add_space(metrics.value(8.0));
                        cancel = crate::ui_style::modern_button(
                            ui,
                            app_layout_text(language, "Отмена", "Cancel"),
                            metrics.size(104.0, 32.0),
                            true,
                        )
                        .clicked();
                    });
                },
            );
        });

        if confirm {
            if let Some(application) = self.application_picker_selected.clone() {
                self.apply_picker_selection(
                    application,
                    self.application_picker_layout_name.clone(),
                    self.application_picker_title_contains.clone(),
                );
            }
            open = false;
        }
        if cancel {
            open = false;
        }
        if !open {
            self.application_picker_selected = None;
            self.application_picker_layout_name.clear();
            self.application_picker_title_contains.clear();
            self.application_manual_executable.clear();
        }
        self.application_picker_open = open;
    }

    fn apply_picker_selection(
        &mut self,
        application: crate::application_layouts::DetectedApplication,
        name: String,
        title_contains: String,
    ) {
        let Some(device_key) = self.application_layout_device_key() else {
            return;
        };
        let settings = self
            .app_settings
            .application_layouts
            .entry(device_key)
            .or_default();
        if self.application_picker_assign_existing
            && settings.editor_layout_id
                != crate::application_layouts::DEFAULT_APPLICATION_LAYOUT_ID
        {
            if let Some(layout) = settings.editor_layout_mut() {
                layout.name = name.trim().to_owned();
                layout.executable = application.executable.clone();
                layout.application_identities =
                    crate::application_layouts::normalized_identity_values(
                        std::iter::once(application.executable.as_str())
                            .chain(application.identities.iter().map(String::as_str)),
                    );
                layout.title_contains = title_contains.trim().to_owned();
                layout.bump_revision();
            }
        } else {
            settings.create_for_application_named(&application, Some(&name), &title_contains);
        }
        save_app_settings(&self.app_settings);
    }

    #[allow(dead_code)]
    fn draw_application_picker(&mut self, ctx: &egui::Context) {
        if !self.application_picker_open {
            return;
        }
        let language = self.app_settings.language;
        let metrics = crate::ui_style::ResponsiveMetrics::from_ctx(ctx);
        let mut open = self.application_picker_open;
        let mut picked: Option<crate::application_layouts::DetectedApplication> = None;
        let mut manual_add = false;
        let applications = self.application_discovery.available.clone();
        let search = self.application_picker_search.trim().to_ascii_lowercase();

        let modal_size = metrics.size(680.0, 540.0);
        crate::ui_style::centered_modal_window(
            ctx,
            app_layout_text(language, "Выбор приложения", "Choose application"),
            egui::Id::new("application_picker"),
            &mut open,
            modal_size,
        )
        .show(ctx, |ui| {
            let content_width = metrics.value(620.0);
            crate::ui_style::modal_content(
                ui,
                crate::ui_style::ModalLayout::new(content_width)
                    .with_top_padding(metrics.value(8.0)),
                |ui| {
                    ui.label(
                        RichText::new(app_layout_text(
                            language,
                            "Выберите установленное или запущенное приложение",
                            "Select an installed or running application",
                        ))
                        .size(metrics.value(12.5))
                        .color(app_muted_text(ui.visuals().dark_mode)),
                    );
                    ui.add_space(metrics.value(12.0));
                    ui.horizontal(|ui| {
                        let refresh_width = metrics.value(104.0);
                        let gap = metrics.value(10.0);
                        let search_width = content_width - refresh_width - gap;
                        let search_id = ui.make_persistent_id("application_picker_search");
                        crate::ui_style::modern_text_field_sized(
                            ui,
                            search_id,
                            &mut self.application_picker_search,
                            search_width,
                            metrics.settings_control_height(),
                            app_layout_text(language, "Поиск приложения", "Search applications"),
                            120,
                            egui::Align::Min,
                        );
                        ui.add_space(gap);
                        if crate::ui_style::modern_button(
                            ui,
                            app_layout_text(language, "Обновить", "Refresh"),
                            metrics.size(104.0, 32.0),
                            true,
                        )
                        .clicked()
                        {
                            crate::app_discovery::refresh_application_discovery();
                        }
                    });
                    ui.add_space(metrics.value(12.0));

                    let filtered = applications
                        .iter()
                        .filter(|application| {
                            search.is_empty()
                                || application.label().to_ascii_lowercase().contains(&search)
                                || application
                                    .executable
                                    .to_ascii_lowercase()
                                    .contains(&search)
                        })
                        .collect::<Vec<_>>();
                    egui::Frame::new()
                        .fill(app_surface_fill(ui.visuals().dark_mode))
                        .stroke(crate::ui_style::modal_outline_stroke(
                            ui.visuals().dark_mode,
                        ))
                        .corner_radius(metrics.value(10.0))
                        .inner_margin(metrics.value(8.0))
                        .show(ui, |ui| {
                            ui.set_width(content_width - metrics.value(18.0));
                            egui::ScrollArea::vertical()
                                .id_salt("application_picker_list")
                                .max_height(metrics.value(292.0))
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    if filtered.is_empty() {
                                        crate::ui_style::modal_empty_state(
                                            ui,
                                            app_layout_text(
                                                language,
                                                "Приложения не найдены",
                                                "No applications found",
                                            ),
                                            None,
                                        );
                                    }
                                    for application in filtered {
                                        let row_width = ui.available_width();
                                        let row_height = metrics.value(58.0);
                                        let (rect, response) = ui.allocate_exact_size(
                                            egui::vec2(row_width, row_height),
                                            egui::Sense::click(),
                                        );
                                        if response.hovered() {
                                            ui.painter().rect_filled(
                                                rect,
                                                metrics.value(8.0),
                                                app_hover_fill(ui.visuals().dark_mode),
                                            );
                                            ui.ctx()
                                                .set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }
                                        let text_left = rect.left() + metrics.value(12.0);
                                        ui.painter().text(
                                            egui::pos2(text_left, rect.top() + metrics.value(17.0)),
                                            egui::Align2::LEFT_CENTER,
                                            application.label(),
                                            egui::FontId::proportional(metrics.value(13.0)),
                                            ui.visuals().text_color(),
                                        );
                                        let detail = if application.window_title.trim().is_empty() {
                                            application.executable.clone()
                                        } else {
                                            format!(
                                                "{}  ·  {}",
                                                application.executable,
                                                application.window_title.trim()
                                            )
                                        };
                                        ui.painter()
                                            .with_clip_rect(rect.shrink(metrics.value(8.0)))
                                            .text(
                                                egui::pos2(
                                                    text_left,
                                                    rect.top() + metrics.value(39.0),
                                                ),
                                                egui::Align2::LEFT_CENTER,
                                                detail,
                                                egui::FontId::proportional(metrics.value(11.0)),
                                                app_muted_text(ui.visuals().dark_mode),
                                            );
                                        ui.painter().line_segment(
                                            [rect.left_bottom(), rect.right_bottom()],
                                            egui::Stroke::new(
                                                1.0_f32,
                                                crate::ui_style::border_color(
                                                    ui.visuals().dark_mode,
                                                ),
                                            ),
                                        );
                                        if response.clicked() {
                                            picked = Some((*application).clone());
                                        }
                                    }
                                });
                        });

                    ui.add_space(metrics.value(14.0));
                    ui.label(
                        RichText::new(app_layout_text(
                            language,
                            "Нет в списке? Укажите имя исполняемого файла",
                            "Not listed? Enter the executable name",
                        ))
                        .size(metrics.value(12.0))
                        .color(app_muted_text(ui.visuals().dark_mode)),
                    );
                    ui.add_space(metrics.value(8.0));
                    ui.horizontal(|ui| {
                        let use_width = metrics.value(104.0);
                        let gap = metrics.value(10.0);
                        let field_width = content_width - use_width - gap;
                        let manual_id = ui.make_persistent_id("application_picker_manual");
                        crate::ui_style::modern_text_field_sized(
                            ui,
                            manual_id,
                            &mut self.application_manual_executable,
                            field_width,
                            metrics.settings_control_height(),
                            "figma, code, blender",
                            120,
                            egui::Align::Min,
                        );
                        ui.add_space(gap);
                        manual_add = crate::ui_style::modern_button(
                            ui,
                            app_layout_text(language, "Использовать", "Use"),
                            metrics.size(104.0, 32.0),
                            !self.application_manual_executable.trim().is_empty(),
                        )
                        .clicked();
                    });
                },
            );
        });

        if manual_add {
            let executable = self.application_manual_executable.trim().to_owned();
            picked = Some(crate::application_layouts::DetectedApplication {
                display_name: executable.clone(),
                identities: vec![executable.clone()],
                executable,
                window_title: String::new(),
            });
        }
        if let Some(application) = picked {
            self.apply_picked_application(application);
            self.application_manual_executable.clear();
            open = false;
        }
        self.application_picker_open = open;
    }

    fn apply_picked_application(
        &mut self,
        application: crate::application_layouts::DetectedApplication,
    ) {
        let Some(device_key) = self.application_layout_device_key() else {
            return;
        };
        let settings = self
            .app_settings
            .application_layouts
            .entry(device_key)
            .or_default();
        if self.application_picker_assign_existing
            && settings.editor_layout_id
                != crate::application_layouts::DEFAULT_APPLICATION_LAYOUT_ID
        {
            if let Some(layout) = settings.editor_layout_mut() {
                layout.executable = application.executable.clone();
                layout.application_identities =
                    crate::application_layouts::normalized_identity_values(
                        std::iter::once(application.executable.as_str())
                            .chain(application.identities.iter().map(String::as_str)),
                    );
                if layout.name.trim().is_empty() {
                    layout.name = application.display_name;
                }
                layout.bump_revision();
            }
        } else {
            settings.create_for_application(&application);
        }
        save_app_settings(&self.app_settings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_picker_fits_reference_viewport_without_touching_edges() {
        let geometry = application_picker_geometry(egui::vec2(700.0, 720.0), 1.0);

        assert_eq!(geometry.window_size, egui::vec2(652.0, 520.0));
        assert_eq!(geometry.content_width, 592.0);
        assert_eq!(geometry.list_height, 160.0);
    }

    #[test]
    fn application_picker_shrinks_list_before_clipping_window() {
        let geometry = application_picker_geometry(egui::vec2(480.0, 480.0), 1.0);

        assert_eq!(geometry.window_size, egui::vec2(432.0, 432.0));
        assert_eq!(geometry.content_width, 372.0);
        assert_eq!(geometry.list_height, 72.0);
    }

    #[test]
    fn application_picker_caps_large_desktop_size() {
        let geometry = application_picker_geometry(egui::vec2(1_920.0, 1_080.0), 1.0);

        assert_eq!(geometry.window_size, egui::vec2(680.0, 520.0));
        assert_eq!(geometry.content_width, 620.0);
        assert_eq!(geometry.list_height, 160.0);
    }

    #[test]
    fn application_picker_content_has_equal_side_margins() {
        let available = egui::Rect::from_min_max(egui::pos2(10.0, 30.0), egui::pos2(662.0, 510.0));
        let content = centered_application_picker_content_rect(available, 592.0);

        assert_eq!(content.width(), 592.0);
        assert_eq!(content.left() - available.left(), 30.0);
        assert_eq!(available.right() - content.right(), 30.0);
    }

    #[test]
    fn application_picker_content_clamps_symmetrically_on_narrow_width() {
        let available = egui::Rect::from_min_max(egui::pos2(10.0, 30.0), egui::pos2(310.0, 510.0));
        let content = centered_application_picker_content_rect(available, 592.0);

        assert_eq!(content, available);
    }

    #[test]
    fn application_picker_real_window_places_widgets_symmetrically() {
        let ctx = egui::Context::default();
        let measured = std::cell::Cell::new(None);
        let mut open = true;

        for _ in 0..3 {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(726.0, 620.0),
            ));
            let _ = ctx.run_ui(input, |_ui| {
                let id = egui::Id::new("application_picker_real_window_test");
                let response = crate::ui_style::centered_modal_window(
                    &ctx,
                    "Add application",
                    id,
                    &mut open,
                    egui::vec2(652.0, 520.0),
                )
                .frame(
                    crate::ui_style::modal_window_frame(
                        ctx.global_style().as_ref(),
                        ctx.global_style().visuals.dark_mode,
                    )
                    .inner_margin(egui::Margin::symmetric(30, 10)),
                )
                .movable(false)
                .show(&ctx, |ui| {
                    application_picker_content_ui(
                        ui,
                        ctx.content_rect().center().x,
                        592.0,
                        |ui, width| {
                            let (field, _) = ui
                                .allocate_exact_size(egui::vec2(width, 32.0), egui::Sense::hover());
                            measured.set(Some(field));
                        },
                    );
                });
                let _ = response;
            });
        }

        let window = ctx
            .memory(|memory| memory.area_rect(egui::Id::new("application_picker_real_window_test")))
            .expect("modal window should render");
        let field = measured.get().expect("modal content should render");
        let left = field.left() - window.left();
        let right = window.right() - field.right();
        assert!(
            (left - right).abs() <= 0.5,
            "left={left}, right={right}, window={window:?}"
        );
        assert!((left - 30.0).abs() <= 0.5, "left={left}");
    }
}
