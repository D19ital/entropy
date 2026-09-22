use super::*;

pub(super) fn device_dropdown_open_id() -> egui::Id {
    egui::Id::new("device_dropdown_open")
}

pub(super) fn device_layer_operations_submenu_open_id() -> egui::Id {
    egui::Id::new("device_layer_operations_submenu_open")
}

pub(super) fn advanced_dropdown_open_id() -> egui::Id {
    egui::Id::new("advanced_dropdown_open")
}

pub(super) fn settings_dropdown_open_id() -> egui::Id {
    egui::Id::new("settings_dropdown_open")
}

pub(super) fn top_dropdown_frame(dark: bool) -> egui::Frame {
    egui::Frame::new()
        .fill(app_surface_fill(dark))
        .stroke(crate::ui_style::modal_outline_stroke(dark))
        .corner_radius(12.0)
        .inner_margin(egui::Margin::symmetric(8, 6))
}

/// Height of a group divider row inside a top dropdown.
pub(super) const TOP_DROPDOWN_DIVIDER_HEIGHT: f32 = 9.0;
/// Extra row width taken by the icon column.
pub(super) const TOP_DROPDOWN_ICON_COLUMN: f32 = 24.0;
/// Left inset of the label in a row with an icon.
pub(super) const TOP_DROPDOWN_ICON_TEXT_LEFT: f32 = 10.0 + TOP_DROPDOWN_ICON_COLUMN;

// Named family that resolves symbols from Noto Emoji first.
// The proportional family would pick some glyphs from egui's bundled
// emoji fonts, which are drawn in a different style.
const TOP_MENU_ICON_FAMILY: &str = "emoji_preview";

/// Block of related rows in a top menu. Rows of one group share a tint,
/// so the color marks the block, not a single row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TopMenuGroup {
    Devices,
    Layers,
    Files,
    Lighting,
    Input,
    KeyBehavior,
    Service,
    HostTools,
    /// Rows about the app or the device itself.
    Meta,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TopMenuIcon {
    Device,
    KeyLegendOrder,
    LayerOperations,
    ImportLayout,
    ExportLayout,
    ExportImage,
    LayoutIndicator,
    AboutDevice,
    TextExpander,
    TypingTrainer,
    Macros,
    TapDance,
    Combo,
    AutoShift,
    KeyOverrides,
    Rgb,
    LayerLeds,
    Display,
    DisplayPresets,
    Encoders,
    Touchpad,
    Modules,
    Bluetooth,
    LiveFeatures,
    TapHold,
    Magic,
    MatrixTester,
    Lock,
    Unlock,
    AppSettings,
    AboutEntropy,
}

impl TopMenuIcon {
    // Glyphs must exist in the embedded fonts (Noto Emoji, Noto Sans
    // Symbols 2, DejaVu) and render as line art there. Keycap-style emoji
    // such as "📶" or "⏺" render as solid boxes and are avoided.
    pub(super) fn glyph(self) -> &'static str {
        match self {
            TopMenuIcon::Device => "⌨",
            TopMenuIcon::KeyLegendOrder => "🌐",
            TopMenuIcon::LayerOperations => "☰",
            TopMenuIcon::ImportLayout => "📥",
            TopMenuIcon::ExportLayout => "📤",
            TopMenuIcon::ExportImage => "🖼",
            TopMenuIcon::LayoutIndicator => "📌",
            TopMenuIcon::AboutDevice | TopMenuIcon::AboutEntropy => "🛈",
            TopMenuIcon::TextExpander => "📝",
            TopMenuIcon::TypingTrainer => "🎯",
            TopMenuIcon::Macros => "📜",
            TopMenuIcon::TapDance => "👆",
            TopMenuIcon::Combo => "🔗",
            TopMenuIcon::AutoShift => "⇧",
            TopMenuIcon::KeyOverrides => "⇄",
            TopMenuIcon::Rgb => "💡",
            TopMenuIcon::LayerLeds => "🚦",
            TopMenuIcon::Display => "🖥",
            TopMenuIcon::DisplayPresets => "🎞",
            TopMenuIcon::Encoders => "🎛",
            TopMenuIcon::Touchpad => "🖱",
            TopMenuIcon::Modules => "🧩",
            TopMenuIcon::Bluetooth => "📡",
            TopMenuIcon::LiveFeatures => "⚡",
            TopMenuIcon::TapHold => "⏱",
            TopMenuIcon::Magic => "🪄",
            TopMenuIcon::MatrixTester => "▦",
            TopMenuIcon::Lock => "🔒",
            TopMenuIcon::Unlock => "🔓",
            TopMenuIcon::AppSettings => "⚙",
        }
    }

    pub(super) fn group(self) -> TopMenuGroup {
        match self {
            TopMenuIcon::Device => TopMenuGroup::Devices,
            TopMenuIcon::KeyLegendOrder | TopMenuIcon::LayerOperations => TopMenuGroup::Layers,
            TopMenuIcon::ImportLayout | TopMenuIcon::ExportLayout | TopMenuIcon::ExportImage => {
                TopMenuGroup::Files
            }
            TopMenuIcon::TextExpander | TopMenuIcon::TypingTrainer => TopMenuGroup::HostTools,
            TopMenuIcon::Macros
            | TopMenuIcon::TapDance
            | TopMenuIcon::Combo
            | TopMenuIcon::AutoShift
            | TopMenuIcon::KeyOverrides
            | TopMenuIcon::TapHold
            | TopMenuIcon::Magic => TopMenuGroup::KeyBehavior,
            TopMenuIcon::Rgb
            | TopMenuIcon::LayerLeds
            | TopMenuIcon::Display
            | TopMenuIcon::DisplayPresets => TopMenuGroup::Lighting,
            TopMenuIcon::Encoders
            | TopMenuIcon::Touchpad
            | TopMenuIcon::Modules
            | TopMenuIcon::Bluetooth
            | TopMenuIcon::LiveFeatures => TopMenuGroup::Input,
            TopMenuIcon::MatrixTester | TopMenuIcon::Lock | TopMenuIcon::Unlock => {
                TopMenuGroup::Service
            }
            TopMenuIcon::LayoutIndicator
            | TopMenuIcon::AboutDevice
            | TopMenuIcon::AppSettings
            | TopMenuIcon::AboutEntropy => TopMenuGroup::Meta,
        }
    }
}

// Group tints come from the Okabe-Ito colorblind-safe palette, adjusted
// per theme for contrast. Color is a redundant cue on top of glyph and
// divider, never the only difference between rows.
pub(super) fn top_menu_group_tint(group: TopMenuGroup, dark: bool) -> Color32 {
    let ((dr, dg, db), (lr, lg, lb)) = match group {
        TopMenuGroup::Devices | TopMenuGroup::Input => ((86, 180, 233), (40, 116, 166)),
        TopMenuGroup::Layers | TopMenuGroup::KeyBehavior => ((179, 157, 219), (126, 87, 194)),
        TopMenuGroup::Files => ((77, 182, 172), (0, 121, 107)),
        TopMenuGroup::Lighting => ((233, 196, 76), (156, 126, 29)),
        TopMenuGroup::Service => ((240, 130, 79), (208, 90, 30)),
        TopMenuGroup::HostTools => ((60, 190, 142), (11, 138, 98)),
        TopMenuGroup::Meta => ((158, 154, 161), (117, 113, 122)),
    };
    if dark {
        Color32::from_rgb(dr, dg, db)
    } else {
        Color32::from_rgb(lr, lg, lb)
    }
}

/// Decides which menu groups get a divider after them, given the number
/// of visible rows per group. A group with a single row is too small to
/// stand alone: it joins the block before it, or the next block when it
/// opens the menu.
pub(super) fn top_menu_dividers<const N: usize>(group_sizes: [usize; N]) -> [bool; N] {
    // Each block is (index of its last group, visible rows).
    let mut blocks: Vec<(usize, usize)> = Vec::new();
    for (index, size) in group_sizes.into_iter().enumerate() {
        if size == 0 {
            continue;
        }
        match blocks.last_mut() {
            Some((last_group, rows)) if size < 2 || *rows < 2 => {
                *last_group = index;
                *rows += size;
            }
            _ => blocks.push((index, size)),
        }
    }
    let mut divider_after = [false; N];
    for (last_group, _) in blocks.iter().take(blocks.len().saturating_sub(1)) {
        divider_after[*last_group] = true;
    }
    divider_after
}

pub(super) fn top_dropdown_divider(ui: &mut egui::Ui, width: f32) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width, TOP_DROPDOWN_DIVIDER_HEIGHT),
        Sense::hover(),
    );
    if ui.is_rect_visible(rect) {
        ui.painter().hline(
            egui::Rangef::new(rect.left() + 10.0, rect.right() - 10.0),
            rect.center().y,
            crate::ui_style::modal_outline_stroke(ui.visuals().dark_mode),
        );
    }
}

pub(super) fn top_dropdown_item(
    ui: &mut egui::Ui,
    width: f32,
    label: &str,
    enabled: bool,
    selected: bool,
) -> egui::Response {
    top_dropdown_item_with_accessory(
        ui,
        width,
        label,
        enabled,
        selected,
        TopDropdownItemAccessory::None,
        None,
    )
}

pub(super) fn top_dropdown_icon_item(
    ui: &mut egui::Ui,
    width: f32,
    icon: TopMenuIcon,
    label: &str,
    enabled: bool,
    selected: bool,
) -> egui::Response {
    top_dropdown_item_with_accessory(
        ui,
        width,
        label,
        enabled,
        selected,
        TopDropdownItemAccessory::None,
        Some(icon),
    )
}

pub(super) fn top_dropdown_icon_item_with_indicator(
    ui: &mut egui::Ui,
    width: f32,
    icon: TopMenuIcon,
    label: &str,
    enabled: bool,
    selected: bool,
    show_indicator: bool,
) -> egui::Response {
    top_dropdown_item_with_accessory(
        ui,
        width,
        label,
        enabled,
        selected,
        if show_indicator {
            TopDropdownItemAccessory::Indicator
        } else {
            TopDropdownItemAccessory::None
        },
        Some(icon),
    )
}

pub(super) fn top_dropdown_icon_submenu_item(
    ui: &mut egui::Ui,
    width: f32,
    icon: TopMenuIcon,
    label: &str,
    enabled: bool,
    submenu_open: bool,
) -> egui::Response {
    top_dropdown_item_with_accessory(
        ui,
        width,
        label,
        enabled,
        submenu_open,
        TopDropdownItemAccessory::Submenu,
        Some(icon),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TopDropdownItemAccessory {
    None,
    Indicator,
    Submenu,
}

fn top_dropdown_item_with_accessory(
    ui: &mut egui::Ui,
    width: f32,
    label: &str,
    enabled: bool,
    selected: bool,
    accessory: TopDropdownItemAccessory,
    icon: Option<TopMenuIcon>,
) -> egui::Response {
    let dark = ui.visuals().dark_mode;
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 30.0), sense);
    let hovered = resp.hovered() && enabled;
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        if selected || hovered {
            let fill = app_hover_fill(dark);
            ui.painter().rect_filled(rect, 8.0, fill);
        }

        let text_color = if !enabled {
            app_muted_text(dark)
        } else if selected {
            app_accent()
        } else {
            ui.visuals().text_color()
        };
        let reserve_right = selected || accessory == TopDropdownItemAccessory::Submenu;
        let text_clip = if reserve_right {
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.right() - 24.0, rect.bottom()))
        } else {
            rect
        };
        let text_left = rect.left()
            + if icon.is_some() {
                TOP_DROPDOWN_ICON_TEXT_LEFT
            } else {
                10.0
            };
        if let Some(icon) = icon {
            let icon_color = if !enabled {
                app_muted_text(dark)
            } else if selected {
                app_accent()
            } else {
                top_menu_group_tint(icon.group(), dark)
            };
            paint_top_menu_icon(
                ui,
                egui::pos2(
                    rect.left() + 8.0 + TOP_DROPDOWN_ICON_COLUMN * 0.5,
                    rect.center().y,
                ),
                icon,
                icon_color,
            );
        }
        ui.painter().with_clip_rect(text_clip).text(
            egui::pos2(text_left, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(13.0),
            text_color,
        );

        if accessory == TopDropdownItemAccessory::Indicator {
            let label_width = top_menu_text_width(ui, label, 13.0);
            let max_dot_x = rect.right() - if selected { 28.0 } else { 10.0 };
            let dot_x = (text_left + label_width + 8.0).min(max_dot_x);
            ui.painter()
                .circle_filled(egui::pos2(dot_x, rect.center().y), 2.5, app_accent());
        }

        if accessory == TopDropdownItemAccessory::Submenu {
            ui.painter().text(
                egui::pos2(rect.right() - 10.0, rect.center().y - 1.0),
                egui::Align2::RIGHT_CENTER,
                "›",
                egui::FontId::proportional(18.0),
                if !enabled {
                    app_muted_text(dark)
                } else if selected || hovered {
                    app_accent()
                } else {
                    ui.visuals().text_color()
                },
            );
        } else if selected {
            ui.painter().circle_filled(
                egui::pos2(rect.right() - 12.0, rect.center().y),
                2.5,
                app_accent(),
            );
        }
    }

    resp
}

pub(super) fn top_menu_text_width(ui: &egui::Ui, label: &str, font_size: f32) -> f32 {
    ui.fonts_mut(|f| {
        f.layout_no_wrap(
            label.to_owned(),
            egui::FontId::proportional(font_size),
            ui.visuals().widgets.inactive.fg_stroke.color,
        )
        .size()
        .x
    })
}

fn top_menu_icon_family(ui: &egui::Ui) -> egui::FontFamily {
    let family = egui::FontFamily::Name(TOP_MENU_ICON_FAMILY.into());
    // egui panics on an unregistered family, so fall back to the UI font.
    if ui.fonts(|fonts| fonts.definitions().families.contains_key(&family)) {
        family
    } else {
        egui::FontFamily::Proportional
    }
}

// Glyphs come from fonts with different metrics. Size and position follow
// the drawn shape, not the font line box, so every icon gets the same
// optical size and sits on the row center.
fn paint_top_menu_icon(ui: &egui::Ui, center: egui::Pos2, icon: TopMenuIcon, color: Color32) {
    const BASE_FONT_SIZE: f32 = 15.0;
    const TARGET_EXTENT: f32 = 16.0;

    let family = top_menu_icon_family(ui);
    let layout = |size: f32| {
        ui.painter().layout_no_wrap(
            icon.glyph().to_owned(),
            egui::FontId::new(size, family.clone()),
            color,
        )
    };
    let extent =
        |galley: &egui::Galley| galley.mesh_bounds.width().max(galley.mesh_bounds.height());

    let mut galley = layout(BASE_FONT_SIZE);
    let base_extent = extent(&galley);
    if base_extent > 0.0 {
        let scale = (TARGET_EXTENT / base_extent).clamp(0.8, 1.5);
        if (scale - 1.0).abs() > 0.05 {
            galley = layout(BASE_FONT_SIZE * scale);
        }
    }
    let ink_center = galley.mesh_bounds.center();
    ui.painter()
        .galley(center - ink_center.to_vec2(), galley, color);
}

pub(super) fn top_menu_divider_stroke(dark: bool) -> egui::Stroke {
    let color = if dark {
        Color32::from_gray(105)
    } else {
        Color32::from_gray(170)
    };
    egui::Stroke::new(1.5_f32, color)
}

pub(super) fn adaptive_top_dropdown_width<'a>(
    ui: &egui::Ui,
    labels: impl IntoIterator<Item = &'a str>,
    min_width: f32,
) -> f32 {
    let text_width = labels
        .into_iter()
        .filter(|label| !label.is_empty())
        .map(|label| top_menu_text_width(ui, label, 13.0))
        .fold(0.0, f32::max);

    // 16px frame margins + 10px left text inset + selected-dot reserve + breathing room.
    (text_width + 56.0).max(min_width).min(360.0)
}

pub(super) fn adaptive_top_icon_dropdown_width<'a>(
    ui: &egui::Ui,
    labels: impl IntoIterator<Item = &'a str>,
    min_width: f32,
) -> f32 {
    adaptive_top_dropdown_width(ui, labels, min_width) + TOP_DROPDOWN_ICON_COLUMN
}

impl EntropyApp {
    pub(super) fn close_top_dropdowns(&self, ctx: &egui::Context) {
        ctx.data_mut(|d| {
            d.insert_temp(device_dropdown_open_id(), false);
            d.insert_temp(device_layer_operations_submenu_open_id(), false);
            d.insert_temp(advanced_dropdown_open_id(), false);
            d.insert_temp(settings_dropdown_open_id(), false);
        });
    }

    pub(super) fn top_dropdown_open(&self, ctx: &egui::Context) -> bool {
        ctx.data(|d| {
            d.get_temp::<bool>(device_dropdown_open_id())
                .unwrap_or(false)
                || d.get_temp::<bool>(advanced_dropdown_open_id())
                    .unwrap_or(false)
                || d.get_temp::<bool>(settings_dropdown_open_id())
                    .unwrap_or(false)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAYOUT_MENU_ICONS: [TopMenuIcon; 8] = [
        TopMenuIcon::Device,
        TopMenuIcon::KeyLegendOrder,
        TopMenuIcon::LayerOperations,
        TopMenuIcon::ImportLayout,
        TopMenuIcon::ExportLayout,
        TopMenuIcon::ExportImage,
        TopMenuIcon::LayoutIndicator,
        TopMenuIcon::AboutDevice,
    ];
    const ADVANCED_MENU_ICONS: [TopMenuIcon; 7] = [
        TopMenuIcon::TextExpander,
        TopMenuIcon::TypingTrainer,
        TopMenuIcon::Macros,
        TopMenuIcon::TapDance,
        TopMenuIcon::Combo,
        TopMenuIcon::AutoShift,
        TopMenuIcon::KeyOverrides,
    ];
    const CONFIG_MENU_ICONS: [TopMenuIcon; 16] = [
        TopMenuIcon::Rgb,
        TopMenuIcon::LayerLeds,
        TopMenuIcon::Display,
        TopMenuIcon::DisplayPresets,
        TopMenuIcon::Encoders,
        TopMenuIcon::Touchpad,
        TopMenuIcon::Modules,
        TopMenuIcon::Bluetooth,
        TopMenuIcon::LiveFeatures,
        TopMenuIcon::TapHold,
        TopMenuIcon::Magic,
        TopMenuIcon::MatrixTester,
        TopMenuIcon::Lock,
        TopMenuIcon::Unlock,
        TopMenuIcon::AppSettings,
        TopMenuIcon::AboutEntropy,
    ];

    #[test]
    fn menu_icons_are_unique_within_each_menu() {
        for menu in [
            LAYOUT_MENU_ICONS.as_slice(),
            ADVANCED_MENU_ICONS.as_slice(),
            CONFIG_MENU_ICONS.as_slice(),
        ] {
            let mut seen = std::collections::HashSet::new();
            for icon in menu {
                assert!(seen.insert(icon.glyph()), "duplicate glyph for {icon:?}");
            }
        }
    }

    #[test]
    fn menu_icon_glyphs_exist_in_the_icon_font_family() {
        use ab_glyph::Font as _;

        // Same fonts and order as the "emoji_preview" family.
        let fonts = [
            include_bytes!("../../assets/NotoEmoji-Regular.ttf").as_slice(),
            include_bytes!("../../assets/NotoSansSymbols2-Regular.ttf").as_slice(),
            include_bytes!("../../assets/DejaVuSans.ttf").as_slice(),
        ]
        .map(|bytes| ab_glyph::FontRef::try_from_slice(bytes).expect("embedded font parses"));

        for icon in LAYOUT_MENU_ICONS
            .iter()
            .chain(&ADVANCED_MENU_ICONS)
            .chain(&CONFIG_MENU_ICONS)
        {
            for ch in icon.glyph().chars() {
                assert!(
                    fonts.iter().any(|font| font.glyph_id(ch).0 != 0),
                    "{icon:?} glyph U+{:04X} is missing from the embedded fonts",
                    ch as u32
                );
            }
        }
    }

    #[test]
    fn full_menu_separates_every_group() {
        assert_eq!(
            top_menu_dividers([4, 5, 2, 2, 2]),
            [true, true, true, true, false]
        );
    }

    #[test]
    fn single_row_group_joins_the_previous_block() {
        // Layer LEDs | Modules, Bluetooth | Tap-Hold | Matrix, Unlock | App, About
        assert_eq!(
            top_menu_dividers([1, 2, 1, 2, 2]),
            [false, false, true, true, false]
        );
        assert_eq!(
            top_menu_dividers([2, 1, 0, 0, 2]),
            [false, true, false, false, false]
        );
    }

    #[test]
    fn leading_single_row_group_joins_the_next_block() {
        assert_eq!(
            top_menu_dividers([0, 0, 1, 2, 2]),
            [false, false, false, true, false]
        );
    }

    #[test]
    fn short_menu_has_no_dividers() {
        assert_eq!(top_menu_dividers([0, 0, 0, 0, 2]), [false; 5]);
        assert_eq!(top_menu_dividers([0, 0, 0, 1, 2]), [false; 5]);
        assert_eq!(top_menu_dividers([2, 0]), [false; 2]);
        assert_eq!(top_menu_dividers([2, 1]), [false; 2]);
    }

    #[test]
    fn shared_dropdown_state_is_visible_to_background_lifecycle() {
        let ctx = egui::Context::default();
        let creation_context = eframe::CreationContext::_new_kittest(ctx.clone());
        let app = EntropyApp::new(&creation_context);

        ctx.data_mut(|d| d.insert_temp(device_dropdown_open_id(), true));
        assert!(app.top_dropdown_open(&ctx));

        app.close_top_dropdowns(&ctx);
        assert!(!app.top_dropdown_open(&ctx));
    }
}
