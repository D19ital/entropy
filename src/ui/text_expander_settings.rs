use super::*;

impl EntropyApp {
    pub(super) fn draw_text_expander_settings_page(
        &mut self,
        ui: &mut egui::Ui,
        content_rect: egui::Rect,
    ) {
        let lang = self.app_settings.language;
        let dark = ui.visuals().dark_mode;
        let metrics = crate::ui_style::ResponsiveMetrics::from_ctx(ui.ctx());
        crate::ui_style::allocate_ui_at_rect(ui, content_rect, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(metrics.value(18.0));
                ui.label(
                    RichText::new(crate::i18n::tr_catalog(lang, "text_expander.title"))
                        .size(metrics.value(18.0))
                        .strong(),
                );
                ui.add_space(metrics.value(6.0));
                ui.add_sized(
                    Vec2::new(metrics.settings_content_width(), metrics.value(34.0)),
                    egui::Label::new(
                        RichText::new(crate::i18n::tr_catalog(lang, "text_expander.description"))
                            .size(metrics.value(13.0))
                            .color(app_muted_text(dark)),
                    )
                    .wrap()
                    .halign(egui::Align::Center),
                );
                ui.add_sized(
                    Vec2::new(metrics.settings_content_width(), metrics.value(28.0)),
                    egui::Label::new(
                        RichText::new(crate::i18n::tr_catalog(lang, "text_expander.quick_help"))
                            .size(metrics.value(11.5))
                            .color(app_muted_text(dark)),
                    )
                    .wrap()
                    .halign(egui::Align::Center),
                );
                ui.add_space(metrics.value(10.0));

                let rule_row_count = self.app_settings.text_expansion_rules.len().max(1);
                let backend_row_count = usize::from(cfg!(not(target_os = "windows")));
                let row_count = backend_row_count + 4 + rule_row_count;
                let list = allocate_adaptive_settings_list_viewport(
                    ui,
                    "text_expander_settings",
                    metrics,
                    row_count,
                    metrics.value(44.0),
                );
                crate::ui_style::allocate_ui_at_rect(ui, list.content_rect, |ui| {
                    ui.set_clip_rect(list.viewport);
                    ui.set_min_size(list.content_rect.size());
                    ui.spacing_mut().item_spacing.y = 0.0;
                    self.draw_text_expander_editor_content(
                        ui,
                        list.first_visible_row..list.last_visible_row,
                        list.row_content_width,
                        list.row_height,
                        metrics,
                        list.suppress_tooltips,
                    );
                });

                if list.has_scrollbar {
                    crate::ui_style::paint_floating_scrollbar_handle(
                        ui,
                        list.track_rect,
                        list.handle_height,
                        list.scroll_ratio,
                        list.track_hovered,
                    );
                }

                let button_size = metrics.size(126.0, 34.0);
                let button_gap = metrics.value(10.0);
                let actions_rect = fixed_settings_action_bar_rect(
                    list.viewport,
                    metrics,
                    button_size,
                    2,
                    button_gap,
                );
                crate::ui_style::allocate_ui_at_rect(ui, actions_rect, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = button_gap;
                        if crate::ui_style::modern_button(
                            ui,
                            crate::i18n::tr_catalog(lang, "text_expander.add_rule"),
                            button_size,
                            true,
                        )
                        .on_hover_text(crate::i18n::tr_catalog(
                            lang,
                            "text_expander.add_rule_tooltip",
                        ))
                        .clicked()
                        {
                            self.app_settings
                                .text_expansion_rules
                                .push(crate::text_expander::TextExpansionRule::default());
                            self.save_text_expander_settings();
                        }

                        let restore_enabled = !self.text_expander_deleted_rules.is_empty();
                        if crate::ui_style::modern_button(
                            ui,
                            crate::i18n::tr_catalog(lang, "text_expander.restore_deleted_rule"),
                            button_size,
                            restore_enabled,
                        )
                        .on_hover_text(crate::i18n::tr_catalog(
                            lang,
                            "text_expander.restore_deleted_rule_tooltip",
                        ))
                        .clicked()
                            && restore_enabled
                        {
                            if let Some((rule_idx, rule)) = self.text_expander_deleted_rules.pop() {
                                let insert_idx =
                                    rule_idx.min(self.app_settings.text_expansion_rules.len());
                                self.app_settings
                                    .text_expansion_rules
                                    .insert(insert_idx, rule);
                                self.save_text_expander_settings();
                            }
                        }
                    });
                });
            });
        });
    }

    #[cfg(not(target_os = "windows"))]
    pub(super) fn draw_text_expander_backend_settings_row(
        &mut self,
        ui: &mut egui::Ui,
        content_width: f32,
        row_height: f32,
        metrics: crate::ui_style::ResponsiveMetrics,
        lang: crate::i18n::Language,
        suppress_tooltips: bool,
    ) {
        let button_size = metrics.size(148.0, 30.0);
        #[cfg(target_os = "linux")]
        let registration = self.ibus_registration.get();
        #[cfg(target_os = "linux")]
        ui.ctx()
            .request_repaint_after(crate::linux_setup::IBUS_REGISTRATION_MAX_AGE);
        #[cfg(target_os = "linux")]
        let action = text_expander_ibus_action(registration);
        #[cfg(target_os = "linux")]
        let setup_running = self.linux_setup_task.is_some() || self.pending_ibus_reload.is_some();
        #[cfg(target_os = "linux")]
        let delete_size = metrics.size(78.0, 30.0);
        #[cfg(target_os = "linux")]
        let button_gap = metrics.value(8.0);
        #[cfg(target_os = "linux")]
        let control_width = button_size.x + button_gap + delete_size.x;
        #[cfg(not(target_os = "linux"))]
        let control_width = button_size.x;

        crate::ui_style::settings_list_row_with_tooltip(
            ui,
            content_width,
            row_height,
            crate::i18n::tr_catalog(lang, text_expander_backend_label_key()),
            true,
            (!suppress_tooltips).then_some(crate::i18n::tr_catalog(
                lang,
                text_expander_backend_hint_key(),
            )),
            control_width,
            |ui| {
                #[cfg(target_os = "linux")]
                {
                    ui.spacing_mut().item_spacing.x = button_gap;
                    let primary_key = text_expander_ibus_button_key(action, setup_running);
                    let primary_tooltip = match action {
                        TextExpanderIbusAction::Install => text_expander_backend_hint_key(),
                        TextExpanderIbusAction::Reinstall => "text_expander.reinstall_ibus_tooltip",
                        TextExpanderIbusAction::Reload => {
                            "universal_symbols_setup.reload_ibus_tooltip"
                        }
                    };
                    if crate::ui_style::modern_button(
                        ui,
                        crate::i18n::tr_catalog(lang, primary_key),
                        button_size,
                        !setup_running,
                    )
                    .on_hover_text(crate::i18n::tr_catalog(lang, primary_tooltip))
                    .clicked()
                    {
                        match action {
                            TextExpanderIbusAction::Install | TextExpanderIbusAction::Reinstall => {
                                self.run_linux_universal_symbols_setup(
                                    "linux/ibus/install-user.sh",
                                    "IBus",
                                );
                            }
                            TextExpanderIbusAction::Reload => self.start_linux_ibus_reload(),
                        }
                    }
                    if crate::ui_style::modern_button(
                        ui,
                        crate::i18n::tr_catalog(lang, "text_expander.delete_ibus_source"),
                        delete_size,
                        text_expander_ibus_delete_enabled(registration, setup_running),
                    )
                    .on_hover_text(crate::i18n::tr_catalog(
                        lang,
                        "universal_symbols_setup.remove_ibus_source_tooltip",
                    ))
                    .clicked()
                    {
                        self.run_linux_universal_symbols_setup(
                            "linux/ibus/uninstall-user.sh",
                            "IBus",
                        );
                    }
                }
                #[cfg(not(target_os = "linux"))]
                if crate::ui_style::modern_button(
                    ui,
                    crate::i18n::tr_catalog(lang, text_expander_backend_button_key()),
                    button_size,
                    true,
                )
                .on_hover_text(crate::i18n::tr_catalog(
                    lang,
                    text_expander_backend_hint_key(),
                ))
                .clicked()
                {
                    self.open_text_expander_setup_page();
                }
            },
        );
    }
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextExpanderIbusAction {
    Install,
    Reinstall,
    Reload,
}

#[cfg(target_os = "linux")]
fn text_expander_ibus_action(
    registration: crate::linux_setup::IbusRegistration,
) -> TextExpanderIbusAction {
    if registration.system {
        TextExpanderIbusAction::Reload
    } else if registration.user {
        TextExpanderIbusAction::Reinstall
    } else {
        TextExpanderIbusAction::Install
    }
}

#[cfg(target_os = "linux")]
fn text_expander_ibus_delete_enabled(
    registration: crate::linux_setup::IbusRegistration,
    setup_running: bool,
) -> bool {
    registration.user && !setup_running
}

#[cfg(target_os = "linux")]
fn text_expander_ibus_button_key(
    action: TextExpanderIbusAction,
    setup_running: bool,
) -> &'static str {
    if setup_running {
        "text_expander.ibus_working"
    } else {
        match action {
            TextExpanderIbusAction::Install => "universal_symbols_setup.install_ibus",
            TextExpanderIbusAction::Reinstall => "text_expander.reinstall_ibus_source",
            TextExpanderIbusAction::Reload => "universal_symbols_setup.reload_ibus_button",
        }
    }
}

#[cfg(all(not(target_os = "windows"), not(target_os = "linux")))]
fn text_expander_backend_button_key() -> &'static str {
    "text_expander.open_backend_setup"
}

#[cfg(not(target_os = "windows"))]
fn text_expander_backend_label_key() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "universal_symbols_setup.wayland_ibus"
    }
    #[cfg(not(target_os = "linux"))]
    {
        "universal_symbols_setup.current_backend"
    }
}

#[cfg(not(target_os = "windows"))]
fn text_expander_backend_hint_key() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "text_expander.backend_hint_linux_ibus"
    }
    #[cfg(target_os = "macos")]
    {
        "text_expander.backend_hint_macos"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "text_expander.backend_hint_unsupported"
    }
}

#[cfg(all(test, target_os = "linux"))]
mod ibus_action_tests {
    use super::*;
    use crate::linux_setup::IbusRegistration;

    #[test]
    fn installed_user_source_offers_reinstall_and_delete() {
        let missing = IbusRegistration {
            user: false,
            system: false,
        };
        let user = IbusRegistration {
            user: true,
            system: false,
        };
        let system = IbusRegistration {
            user: false,
            system: true,
        };
        let both = IbusRegistration {
            user: true,
            system: true,
        };

        assert_eq!(
            text_expander_ibus_action(missing),
            TextExpanderIbusAction::Install
        );
        assert_eq!(
            text_expander_ibus_action(user),
            TextExpanderIbusAction::Reinstall
        );
        assert_eq!(
            text_expander_ibus_action(system),
            TextExpanderIbusAction::Reload
        );
        assert_eq!(
            text_expander_ibus_action(both),
            TextExpanderIbusAction::Reload
        );
        assert!(!text_expander_ibus_delete_enabled(missing, false));
        assert!(text_expander_ibus_delete_enabled(user, false));
        assert!(!text_expander_ibus_delete_enabled(system, false));
        assert!(text_expander_ibus_delete_enabled(both, false));
        assert!(!text_expander_ibus_delete_enabled(user, true));
        assert_eq!(
            text_expander_ibus_button_key(TextExpanderIbusAction::Reinstall, false),
            "text_expander.reinstall_ibus_source"
        );
        assert_eq!(
            text_expander_ibus_button_key(TextExpanderIbusAction::Reinstall, true),
            "text_expander.ibus_working"
        );
    }

    #[test]
    fn source_management_labels_are_localized() {
        for language in crate::i18n::Language::ALL {
            for key in [
                "text_expander.reinstall_ibus_source",
                "text_expander.delete_ibus_source",
                "text_expander.reinstall_ibus_tooltip",
                "text_expander.ibus_working",
            ] {
                assert_ne!(crate::i18n::tr_catalog(language, key), key);
            }
        }
    }
}
