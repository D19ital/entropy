use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) const APPLICATION_LAYOUT_CONTROL_COUNT: usize = 15;
pub(crate) const APPLICATION_LAYOUT_KEY_COUNT: usize = 13;
pub(crate) const APPLICATION_LAYOUT_LAYER_COUNT: usize = 16;
pub(crate) const APPLICATION_LAYOUT_UNSET_KEYCODE: u16 = u16::MAX;
pub(crate) const DEFAULT_APPLICATION_LAYOUT_ID: &str = "default";
pub(crate) const APPLICATION_LAYOUT_PROTOCOL_VERSION: u8 = 5;
pub(crate) const APPLICATION_LAYOUT_NAME_BYTES: usize = 22;

// Display transfers already occupy 0xC0..=0xD6 in the upstream firmware.
// Keep application-layout traffic in its own non-overlapping command range.
pub(crate) const HID_APPLICATION_LAYOUT_BEGIN: u8 = 0xE1;
pub(crate) const HID_APPLICATION_LAYOUT_KEYCODES: u8 = 0xE2;
pub(crate) const HID_APPLICATION_LAYOUT_COMMIT: u8 = 0xE3;
pub(crate) const HID_APPLICATION_LAYOUT_LAYER_NAME: u8 = 0xE4;
pub(crate) const HID_APPLICATION_LAYOUT_KEEPALIVE: u8 = 0xE5;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DetectedApplication {
    pub(crate) executable: String,
    /// Stable identities reported by launchers and window systems for the
    /// same application (desktop id, Exec, StartupWMClass, process, app_id).
    #[serde(default)]
    pub(crate) identities: Vec<String>,
    #[serde(default)]
    pub(crate) display_name: String,
    #[serde(default)]
    pub(crate) window_title: String,
}

impl DetectedApplication {
    pub(crate) fn label(&self) -> String {
        let name = if self.display_name.trim().is_empty() {
            self.executable.trim()
        } else {
            self.display_name.trim()
        };
        if self.window_title.trim().is_empty() || self.window_title.trim() == name {
            name.to_owned()
        } else {
            format!("{name} — {}", self.window_title.trim())
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ApplicationLayout {
    pub(crate) id: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) executable: String,
    /// All known launcher/window identities captured when the application is
    /// selected. This keeps matching data-driven instead of app-specific.
    #[serde(default)]
    pub(crate) application_identities: Vec<String>,
    #[serde(default)]
    pub(crate) title_contains: String,
    #[serde(default = "default_true")]
    pub(crate) automatic_switching: bool,
    /// Compatibility seed for application-layouts v1. New builds persist
    /// `layers`; the old single-layer keycodes are read once during normalize.
    #[serde(rename = "keycodes", default = "default_keycodes")]
    #[serde(skip_serializing)]
    legacy_keycodes: [u16; APPLICATION_LAYOUT_CONTROL_COUNT],
    #[serde(default)]
    pub(crate) layers: Vec<[u16; APPLICATION_LAYOUT_CONTROL_COUNT]>,
    #[serde(default = "default_layer_names")]
    pub(crate) layer_names: Vec<String>,
    #[serde(default = "default_revision")]
    pub(crate) revision: u32,
}

impl ApplicationLayout {
    fn default_layout() -> Self {
        Self {
            id: DEFAULT_APPLICATION_LAYOUT_ID.to_owned(),
            name: "Default".to_owned(),
            executable: String::new(),
            application_identities: Vec::new(),
            title_contains: String::new(),
            automatic_switching: false,
            legacy_keycodes: default_keycodes(),
            layers: default_layer_keycodes(),
            layer_names: default_layer_names(),
            revision: 1,
        }
    }

    fn normalize(&mut self) -> bool {
        let mut changed = false;
        let identities = normalized_identity_values(
            std::iter::once(self.executable.as_str())
                .chain(self.application_identities.iter().map(String::as_str)),
        );
        if self.application_identities != identities {
            self.application_identities = identities;
            changed = true;
        }
        if self.layers.is_empty() {
            self.layers = default_layer_keycodes();
            self.layers[0] = self.legacy_keycodes;
            changed = true;
        }
        if self.layers.len() < APPLICATION_LAYOUT_LAYER_COUNT {
            self.layers.extend(
                (self.layers.len()..APPLICATION_LAYOUT_LAYER_COUNT).map(|_| default_keycodes()),
            );
            changed = true;
        } else if self.layers.len() > APPLICATION_LAYOUT_LAYER_COUNT {
            self.layers.truncate(APPLICATION_LAYOUT_LAYER_COUNT);
            changed = true;
        }
        if self.layer_names.len() < APPLICATION_LAYOUT_LAYER_COUNT {
            let defaults = default_layer_names();
            self.layer_names.extend(
                (self.layer_names.len()..APPLICATION_LAYOUT_LAYER_COUNT)
                    .map(|index| defaults[index].clone()),
            );
            changed = true;
        } else if self.layer_names.len() > APPLICATION_LAYOUT_LAYER_COUNT {
            self.layer_names.truncate(APPLICATION_LAYOUT_LAYER_COUNT);
            changed = true;
        }
        changed
    }

    pub(crate) fn matches(&self, application: &DetectedApplication) -> bool {
        if self.id == DEFAULT_APPLICATION_LAYOUT_ID || self.executable.trim().is_empty() {
            return false;
        }
        application_identities_match(
            std::iter::once(self.executable.as_str())
                .chain(self.application_identities.iter().map(String::as_str)),
            std::iter::once(application.executable.as_str())
                .chain(application.identities.iter().map(String::as_str)),
        ) && (self.title_contains.trim().is_empty()
            || application
                .window_title
                .to_lowercase()
                .contains(&self.title_contains.trim().to_lowercase()))
    }

    pub(crate) fn set_keycode(&mut self, layer: usize, control: usize, keycode: u16) -> bool {
        let Some(slot) = self
            .layers
            .get_mut(layer)
            .and_then(|keycodes| keycodes.get_mut(control))
        else {
            return false;
        };
        if *slot == keycode {
            return false;
        }
        *slot = keycode;
        self.bump_revision();
        true
    }

    pub(crate) fn set_layer_name(&mut self, layer: usize, name: String) -> bool {
        let Some(slot) = self.layer_names.get_mut(layer) else {
            return false;
        };
        let name = if name.trim().is_empty() {
            default_layer_names()
                .get(layer)
                .cloned()
                .unwrap_or_else(|| format!("Layer {layer}"))
        } else {
            name.trim().to_owned()
        };
        if *slot == name {
            return false;
        }
        *slot = name;
        self.bump_revision();
        true
    }

    pub(crate) fn seed_unset_keycodes(
        &mut self,
        layers: [[u16; APPLICATION_LAYOUT_CONTROL_COUNT]; APPLICATION_LAYOUT_LAYER_COUNT],
    ) -> bool {
        let mut changed = false;
        self.normalize();
        for (target_layer, source_layer) in self.layers.iter_mut().zip(layers) {
            for (slot, source) in target_layer.iter_mut().zip(source_layer) {
                if *slot == APPLICATION_LAYOUT_UNSET_KEYCODE {
                    *slot = source;
                    changed = true;
                }
            }
        }
        if changed {
            self.bump_revision();
        }
        changed
    }

    pub(crate) fn bump_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1).max(1);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DeviceApplicationLayouts {
    #[serde(default = "default_layouts")]
    pub(crate) layouts: BTreeMap<String, ApplicationLayout>,
    /// Compatibility bridge for application-layouts v1/v2, where one global
    /// switch controlled every application. It is consumed once by normalize.
    #[serde(default, rename = "automatic_switching", skip_serializing)]
    legacy_automatic_switching: Option<bool>,
    #[serde(default = "default_layout_id")]
    pub(crate) editor_layout_id: String,
    #[serde(default = "default_layout_id")]
    pub(crate) active_layout_id: String,
    /// Automatically return to Default when the focused window does not match
    /// a configured application. A known application with automatic switching
    /// disabled preserves the current manually selected layout.
    #[serde(
        default = "default_true",
        rename = "automatically_return_to_default",
        alias = "unknown_window_uses_default"
    )]
    pub(crate) automatically_return_to_default: bool,
    #[serde(default = "default_next_id")]
    next_id: u32,
}

impl Default for DeviceApplicationLayouts {
    fn default() -> Self {
        Self {
            layouts: default_layouts(),
            legacy_automatic_switching: None,
            editor_layout_id: default_layout_id(),
            active_layout_id: default_layout_id(),
            automatically_return_to_default: true,
            next_id: default_next_id(),
        }
    }
}

impl DeviceApplicationLayouts {
    pub(crate) fn normalize(&mut self) -> bool {
        let mut changed = false;
        if let Some(automatic_switching) = self.legacy_automatic_switching.take() {
            for layout in self.layouts.values_mut() {
                if layout.id != DEFAULT_APPLICATION_LAYOUT_ID {
                    layout.automatic_switching = automatic_switching;
                }
            }
            changed = true;
        }
        if !self.layouts.contains_key(DEFAULT_APPLICATION_LAYOUT_ID) {
            self.layouts.insert(
                DEFAULT_APPLICATION_LAYOUT_ID.to_owned(),
                ApplicationLayout::default_layout(),
            );
            changed = true;
        }
        if let Some(default) = self.layouts.get_mut(DEFAULT_APPLICATION_LAYOUT_ID) {
            let stock_names = default_layer_names();
            let legacy_names = legacy_default_layer_names();
            let mut migrated_names = false;
            for (index, name) in default.layer_names.iter_mut().enumerate() {
                if legacy_names.get(index).is_some_and(|legacy| name == legacy) {
                    *name = stock_names[index].clone();
                    migrated_names = true;
                }
            }
            if migrated_names {
                default.bump_revision();
                changed = true;
            }
            if default.name != "Default"
                || !default.executable.is_empty()
                || !default.application_identities.is_empty()
                || !default.title_contains.is_empty()
                || default.automatic_switching
            {
                default.name = "Default".to_owned();
                default.executable.clear();
                default.application_identities.clear();
                default.title_contains.clear();
                default.automatic_switching = false;
                default.bump_revision();
                changed = true;
            }
        }
        // The map key is the stable profile identity. Older or partially
        // written settings could contain a stale `layout.id`; UI actions used
        // that inner value and could therefore edit/delete a neighbouring
        // profile. Repair the invariant before any profile is exposed.
        for (stable_id, layout) in &mut self.layouts {
            if layout.id != *stable_id {
                layout.id = stable_id.clone();
                changed = true;
            }
            changed |= layout.normalize();
        }
        let next_unused_id = self
            .layouts
            .keys()
            .filter_map(|id| id.rsplit_once('_')?.1.parse::<u32>().ok())
            .max()
            .map(|value| value.saturating_add(1))
            .unwrap_or(1);
        if self.next_id < next_unused_id {
            self.next_id = next_unused_id;
            changed = true;
        }
        if !self.layouts.contains_key(&self.editor_layout_id) {
            self.editor_layout_id = default_layout_id();
            changed = true;
        }
        if !self.layouts.contains_key(&self.active_layout_id) {
            self.active_layout_id = default_layout_id();
            changed = true;
        }
        changed
    }

    pub(crate) fn editor_layout(&self) -> Option<&ApplicationLayout> {
        self.layouts.get(&self.editor_layout_id)
    }

    pub(crate) fn editor_layout_mut(&mut self) -> Option<&mut ApplicationLayout> {
        self.layouts.get_mut(&self.editor_layout_id)
    }

    pub(crate) fn active_layout(&self) -> Option<&ApplicationLayout> {
        self.layouts
            .get(&self.active_layout_id)
            .or_else(|| self.layouts.get(DEFAULT_APPLICATION_LAYOUT_ID))
    }

    pub(crate) fn create_for_application(&mut self, application: &DetectedApplication) -> String {
        self.create_for_application_named(application, None, "")
    }

    pub(crate) fn create_for_application_named(
        &mut self,
        application: &DetectedApplication,
        requested_name: Option<&str>,
        title_contains: &str,
    ) -> String {
        let id = self.unique_id(&application.executable);
        let default_name = if application.display_name.trim().is_empty() {
            application.executable.trim()
        } else {
            application.display_name.trim()
        };
        let name = requested_name
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(default_name);
        let seed_layers = self
            .layouts
            .get(DEFAULT_APPLICATION_LAYOUT_ID)
            .map(|layout| layout.layers.clone())
            .unwrap_or_else(default_layer_keycodes);
        let seed_names = self
            .layouts
            .get(DEFAULT_APPLICATION_LAYOUT_ID)
            .map(|layout| layout.layer_names.clone())
            .unwrap_or_else(default_layer_names);
        self.layouts.insert(
            id.clone(),
            ApplicationLayout {
                id: id.clone(),
                name: if name.is_empty() {
                    "Application".to_owned()
                } else {
                    name.to_owned()
                },
                executable: application.executable.trim().to_owned(),
                application_identities: normalized_identity_values(
                    std::iter::once(application.executable.as_str())
                        .chain(application.identities.iter().map(String::as_str)),
                ),
                title_contains: title_contains.trim().to_owned(),
                automatic_switching: true,
                legacy_keycodes: default_keycodes(),
                layers: seed_layers,
                layer_names: seed_names,
                revision: 1,
            },
        );
        self.editor_layout_id = id.clone();
        id
    }

    pub(crate) fn layout_name_exists(&self, name: &str, excluding_id: Option<&str>) -> bool {
        let name = name.trim();
        !name.is_empty()
            && self.layouts.values().any(|layout| {
                Some(layout.id.as_str()) != excluding_id
                    && layout.name.trim().eq_ignore_ascii_case(name)
            })
    }

    pub(crate) fn rename_layout(&mut self, id: &str, name: &str) -> bool {
        let name = name.trim();
        if id == DEFAULT_APPLICATION_LAYOUT_ID
            || name.is_empty()
            || self.layout_name_exists(name, Some(id))
        {
            return false;
        }
        let Some(layout) = self.layouts.get_mut(id) else {
            return false;
        };
        if layout.name == name {
            return false;
        }
        layout.name = name.to_owned();
        layout.bump_revision();
        true
    }

    pub(crate) fn application_rule_exists(
        &self,
        application: &DetectedApplication,
        title_contains: &str,
        excluding_id: Option<&str>,
    ) -> bool {
        let title_contains = title_contains.trim();
        self.layouts.values().any(|layout| {
            layout.id != DEFAULT_APPLICATION_LAYOUT_ID
                && Some(layout.id.as_str()) != excluding_id
                && layout
                    .title_contains
                    .trim()
                    .eq_ignore_ascii_case(title_contains)
                && application_identities_match(
                    std::iter::once(layout.executable.as_str())
                        .chain(layout.application_identities.iter().map(String::as_str)),
                    std::iter::once(application.executable.as_str())
                        .chain(application.identities.iter().map(String::as_str)),
                )
        })
    }

    pub(crate) fn update_application_rule(
        &mut self,
        id: &str,
        application: &DetectedApplication,
        name: &str,
        title_contains: &str,
    ) -> bool {
        if id == DEFAULT_APPLICATION_LAYOUT_ID {
            return false;
        }
        let Some(layout) = self.layouts.get_mut(id) else {
            return false;
        };
        let identities = normalized_identity_values(
            std::iter::once(application.executable.as_str())
                .chain(application.identities.iter().map(String::as_str)),
        );
        let name = name.trim();
        let name = if name.is_empty() {
            application.display_name.trim()
        } else {
            name
        };
        let name = if name.is_empty() { "Application" } else { name };
        let executable = application.executable.trim();
        let title_contains = title_contains.trim();
        if layout.name == name
            && layout.executable == executable
            && layout.application_identities == identities
            && layout.title_contains == title_contains
        {
            return false;
        }
        layout.name = name.to_owned();
        layout.executable = executable.to_owned();
        layout.application_identities = identities;
        layout.title_contains = title_contains.to_owned();
        layout.bump_revision();
        true
    }

    pub(crate) fn remove(&mut self, id: &str) -> bool {
        if id == DEFAULT_APPLICATION_LAYOUT_ID || self.layouts.remove(id).is_none() {
            return false;
        }
        if self.editor_layout_id == id {
            self.editor_layout_id = default_layout_id();
        }
        if self.active_layout_id == id {
            self.active_layout_id = default_layout_id();
        }
        true
    }

    pub(crate) fn resolve(&self, application: Option<&DetectedApplication>) -> String {
        let active_or_default = || {
            if self.layouts.contains_key(&self.active_layout_id) {
                self.active_layout_id.clone()
            } else {
                default_layout_id()
            }
        };
        let matched = application.and_then(|application| {
            self.layouts
                .values()
                .filter(|layout| layout.matches(application))
                .max_by(|left, right| {
                    let rank = |layout: &&ApplicationLayout| {
                        (
                            !layout.title_contains.trim().is_empty(),
                            layout.title_contains.len(),
                            application_identity_match_score(
                                std::iter::once(layout.executable.as_str()).chain(
                                    layout.application_identities.iter().map(String::as_str),
                                ),
                                std::iter::once(application.executable.as_str())
                                    .chain(application.identities.iter().map(String::as_str)),
                            ),
                        )
                    };
                    rank(&left)
                        .cmp(&rank(&right))
                        // Stable fallback for legacy configurations that already contain
                        // ambiguous duplicate rules. Lower ids win deterministically.
                        .then_with(|| right.id.cmp(&left.id))
                })
                .map(|layout| {
                    if layout.automatic_switching {
                        layout.id.clone()
                    } else {
                        active_or_default()
                    }
                })
        });
        matched.unwrap_or_else(|| {
            if self.automatically_return_to_default {
                default_layout_id()
            } else {
                active_or_default()
            }
        })
    }

    /// Enrich layouts created by older builds with identities from the
    /// current launcher/window catalog. Upgrades then work without forcing
    /// users to delete and re-add every application.
    pub(crate) fn enrich_application_identities(
        &mut self,
        applications: &[DetectedApplication],
    ) -> bool {
        let mut changed = false;
        for layout in self.layouts.values_mut().filter(|layout| {
            layout.id != DEFAULT_APPLICATION_LAYOUT_ID && !layout.executable.trim().is_empty()
        }) {
            let Some(application) = applications.iter().max_by_key(|application| {
                application_identity_match_score(
                    std::iter::once(layout.executable.as_str())
                        .chain(layout.application_identities.iter().map(String::as_str)),
                    std::iter::once(application.executable.as_str())
                        .chain(application.identities.iter().map(String::as_str)),
                )
            }) else {
                continue;
            };
            if !application_identities_match(
                std::iter::once(layout.executable.as_str())
                    .chain(layout.application_identities.iter().map(String::as_str)),
                std::iter::once(application.executable.as_str())
                    .chain(application.identities.iter().map(String::as_str)),
            ) {
                continue;
            }
            let identities = normalized_identity_values(
                std::iter::once(layout.executable.as_str())
                    .chain(layout.application_identities.iter().map(String::as_str))
                    .chain(std::iter::once(application.executable.as_str()))
                    .chain(application.identities.iter().map(String::as_str)),
            );
            if layout.application_identities != identities {
                layout.application_identities = identities;
                layout.bump_revision();
                changed = true;
            }
        }
        changed
    }

    fn unique_id(&mut self, executable: &str) -> String {
        let base = executable
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() {
                    character.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .trim_matches('_')
            .to_owned();
        let base = if base.is_empty() {
            "application"
        } else {
            &base
        };
        loop {
            let suffix = self.next_id;
            self.next_id = self.next_id.wrapping_add(1).max(1);
            let candidate = format!("{base}_{suffix}");
            if !self.layouts.contains_key(&candidate) {
                return candidate;
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ApplicationLayoutSnapshot {
    pub(crate) active: bool,
    pub(crate) revision: u32,
    pub(crate) name: String,
    pub(crate) layer_names: [String; APPLICATION_LAYOUT_LAYER_COUNT],
    pub(crate) layers: [[u16; APPLICATION_LAYOUT_CONTROL_COUNT]; APPLICATION_LAYOUT_LAYER_COUNT],
}

impl ApplicationLayoutSnapshot {
    pub(crate) fn from_layout(layout: &ApplicationLayout) -> Self {
        Self {
            active: true,
            revision: layout.revision,
            name: layout.name.trim().to_owned(),
            layer_names: std::array::from_fn(|layer| {
                layout
                    .layer_names
                    .get(layer)
                    .cloned()
                    .unwrap_or_else(|| format!("Layer {layer}"))
            }),
            layers: std::array::from_fn(|layer| {
                layout
                    .layers
                    .get(layer)
                    .copied()
                    .unwrap_or_else(default_keycodes)
            }),
        }
    }

    pub(crate) fn inactive() -> Self {
        Self {
            active: false,
            revision: 0,
            name: String::new(),
            layer_names: std::array::from_fn(|_| String::new()),
            layers: [default_keycodes(); APPLICATION_LAYOUT_LAYER_COUNT],
        }
    }

    pub(crate) fn packets(&self) -> Vec<[u8; 32]> {
        let mut begin = [0u8; 32];
        begin[0] = HID_APPLICATION_LAYOUT_BEGIN;
        begin[1] = APPLICATION_LAYOUT_PROTOCOL_VERSION;
        begin[2] = u8::from(self.active);
        begin[3..7].copy_from_slice(&self.revision.to_le_bytes());
        begin[7] = APPLICATION_LAYOUT_CONTROL_COUNT as u8;
        begin[8] = APPLICATION_LAYOUT_LAYER_COUNT as u8;
        let name = application_layout_name_bytes(&self.name);
        begin[9] = name.len() as u8;
        begin[10..10 + name.len()].copy_from_slice(name);

        let mut packets = vec![begin];
        for (layer, keycodes) in self.layers.iter().enumerate() {
            for (start, count) in [
                (0, APPLICATION_LAYOUT_KEY_COUNT),
                (APPLICATION_LAYOUT_KEY_COUNT, 2),
            ] {
                let mut packet = [0u8; 32];
                packet[0] = HID_APPLICATION_LAYOUT_KEYCODES;
                packet[1] = APPLICATION_LAYOUT_PROTOCOL_VERSION;
                packet[2] = layer as u8;
                packet[3] = start as u8;
                packet[4] = count as u8;
                for (index, keycode) in keycodes[start..start + count].iter().enumerate() {
                    let offset = 5 + index * 2;
                    packet[offset..offset + 2].copy_from_slice(&keycode.to_le_bytes());
                }
                packets.push(packet);
            }
        }

        for (layer, layer_name) in self.layer_names.iter().enumerate() {
            let mut packet = [0u8; 32];
            let encoded = application_layout_name_bytes(layer_name);
            packet[0] = HID_APPLICATION_LAYOUT_LAYER_NAME;
            packet[1] = APPLICATION_LAYOUT_PROTOCOL_VERSION;
            packet[2] = layer as u8;
            packet[3] = encoded.len() as u8;
            packet[4..4 + encoded.len()].copy_from_slice(encoded);
            packets.push(packet);
        }

        let mut commit = [0u8; 32];
        commit[0] = HID_APPLICATION_LAYOUT_COMMIT;
        commit[1] = APPLICATION_LAYOUT_PROTOCOL_VERSION;
        commit[2] = u8::from(self.active);
        commit[3..7].copy_from_slice(&self.revision.to_le_bytes());
        commit[7..9]
            .copy_from_slice(&crc16_snapshot(&self.layers, name, &self.layer_names).to_le_bytes());
        packets.push(commit);
        packets
    }

    pub(crate) fn keepalive_packet(&self) -> [u8; 32] {
        let mut packet = [0u8; 32];
        packet[0] = HID_APPLICATION_LAYOUT_KEEPALIVE;
        packet[1] = APPLICATION_LAYOUT_PROTOCOL_VERSION;
        packet[2] = u8::from(self.active);
        packet[3..7].copy_from_slice(&self.revision.to_le_bytes());
        packet
    }
}

fn application_layout_name_bytes(name: &str) -> &[u8] {
    let mut end = name.len().min(APPLICATION_LAYOUT_NAME_BYTES);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    &name.as_bytes()[..end]
}

pub(crate) fn normalize_executable(value: &str) -> String {
    let value = value.trim().replace('\\', "/");
    let file = value
        .rsplit('/')
        .next()
        .unwrap_or(&value)
        .to_ascii_lowercase();
    let normalized = file
        .strip_suffix(".exe")
        .unwrap_or(&file)
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>();
    normalized
}

pub(crate) fn executables_match(left: &str, right: &str) -> bool {
    application_identities_match([left], [right])
}

pub(crate) fn application_identities_match<'a>(
    left: impl IntoIterator<Item = &'a str>,
    right: impl IntoIterator<Item = &'a str>,
) -> bool {
    application_identity_match_score(left, right) > 0
}

pub(crate) fn application_identity_match_score<'a>(
    left: impl IntoIterator<Item = &'a str>,
    right: impl IntoIterator<Item = &'a str>,
) -> usize {
    let left = left.into_iter().collect::<Vec<_>>();
    let right = right.into_iter().collect::<Vec<_>>();
    let left_bundle_ids = authoritative_bundle_ids(left.iter().copied());
    let right_bundle_ids = authoritative_bundle_ids(right.iter().copied());

    // NSWorkspace gives us a stable bundle identifier. When both records have
    // one, paths, localized names and helper-process aliases must never make
    // two different applications equal (for example Finder and another
    // process containing the word `finder`). Fall back to aliases only when a
    // stable identifier is absent on at least one side.
    if !left_bundle_ids.is_empty() && !right_bundle_ids.is_empty() {
        return left_bundle_ids
            .iter()
            .filter(|bundle_id| right_bundle_ids.contains(bundle_id))
            .map(String::len)
            .max()
            .unwrap_or(0);
    }

    let left = normalized_identity_values(left);
    let right = normalized_identity_values(right);
    left.iter()
        .filter(|identity| right.contains(identity))
        .map(String::len)
        .max()
        .unwrap_or(0)
}

fn authoritative_bundle_ids<'a>(values: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut bundle_ids = values
        .into_iter()
        .filter_map(|value| {
            let value = value.trim();
            if value.is_empty()
                || value
                    .chars()
                    .any(|character| matches!(character, '/' | '\\' | ' '))
                || value.ends_with(".exe")
            {
                return None;
            }
            let parts = value.split('.').collect::<Vec<_>>();
            if parts.len() < 3
                || !matches!(
                    parts[0].to_ascii_lowercase().as_str(),
                    "com" | "org" | "net" | "io" | "app" | "ru"
                )
                || parts.iter().any(|part| {
                    part.is_empty()
                        || !part
                            .chars()
                            .all(|character| character.is_ascii_alphanumeric() || character == '-')
                })
            {
                return None;
            }
            Some(value.to_ascii_lowercase())
        })
        .collect::<Vec<_>>();
    bundle_ids.sort();
    bundle_ids.dedup();
    bundle_ids
}

pub(crate) fn normalized_identity_values<'a>(
    values: impl IntoIterator<Item = &'a str>,
) -> Vec<String> {
    let mut identities = values
        .into_iter()
        .flat_map(application_identity_aliases)
        .collect::<Vec<_>>();
    identities.sort();
    identities.dedup();
    identities
}

fn application_identity_aliases(value: &str) -> Vec<String> {
    let value = value.trim().replace('\\', "/");
    let file = value.rsplit('/').next().unwrap_or(&value);
    let file = file
        .strip_suffix(".desktop")
        .or_else(|| file.strip_suffix(".exe"))
        .unwrap_or(file);
    let parts = file
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return Vec::new();
    }

    let mut aliases = vec![parts.concat()];
    let mut semantic = parts.as_slice();
    if semantic
        .first()
        .is_some_and(|part| matches!(part.as_str(), "com" | "org" | "net" | "io" | "app"))
    {
        semantic = &semantic[1..];
        if !semantic.is_empty() {
            aliases.push(semantic.concat());
        }
    }
    while semantic.last().is_some_and(|part| {
        matches!(
            part.as_str(),
            "desktop"
                | "application"
                | "app"
                | "launcher"
                | "server"
                | "client"
                | "bin"
                | "binary"
                | "wrapper"
                | "process"
        )
    }) {
        semantic = &semantic[..semantic.len() - 1];
        if !semantic.is_empty() {
            aliases.push(semantic.concat());
        }
    }
    if semantic.len() >= 2
        && semantic[semantic.len() - 2] == "url"
        && semantic[semantic.len() - 1] == "handler"
    {
        let base = &semantic[..semantic.len() - 2];
        if !base.is_empty() {
            aliases.push(base.concat());
            semantic = base;
        }
    }
    if semantic.len() > 1
        && semantic.last().is_some_and(|part| {
            !matches!(
                part.as_str(),
                "handler"
                    | "helper"
                    | "desktop"
                    | "application"
                    | "app"
                    | "launcher"
                    | "mac"
                    | "macos"
                    | "windows"
                    | "linux"
            )
        })
    {
        aliases.push(semantic.last().cloned().unwrap_or_default());
    }
    // Older builds could persist launcher/runtime implementation names as
    // identities. Values such as `desktop`, `electron`, or `snap` are shared
    // by unrelated applications and turn one rule (notably Telegram) into a
    // catch-all. Filter them both for new records and while normalizing old
    // settings so a poisoned profile repairs itself on the next load.
    aliases.retain(|alias| alias.len() >= 2 && !generic_application_identity(alias));
    aliases.sort();
    aliases.dedup();
    aliases
}

fn generic_application_identity(value: &str) -> bool {
    matches!(
        value,
        "app"
            | "application"
            | "bin"
            | "binary"
            | "chromium"
            | "client"
            | "desktop"
            | "electron"
            | "flatpak"
            | "handler"
            | "helper"
            | "launcher"
            | "linux"
            | "mac"
            | "macos"
            | "process"
            | "server"
            | "snap"
            | "wayland"
            | "windows"
            | "wrapper"
            | "x11"
    )
}

fn crc16_snapshot(
    layers: &[[u16; APPLICATION_LAYOUT_CONTROL_COUNT]; APPLICATION_LAYOUT_LAYER_COUNT],
    name: &[u8],
    layer_names: &[String; APPLICATION_LAYOUT_LAYER_COUNT],
) -> u16 {
    let mut crc = 0xFFFFu16;
    let bytes = layers
        .iter()
        .flatten()
        .flat_map(|value| value.to_le_bytes())
        .chain(std::iter::once(name.len() as u8))
        .chain(name.iter().copied())
        .chain(layer_names.iter().flat_map(|layer_name| {
            let encoded = application_layout_name_bytes(layer_name);
            std::iter::once(encoded.len() as u8).chain(encoded.iter().copied())
        }));
    for byte in bytes {
        crc ^= u16::from(byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

fn default_keycodes() -> [u16; APPLICATION_LAYOUT_CONTROL_COUNT] {
    [APPLICATION_LAYOUT_UNSET_KEYCODE; APPLICATION_LAYOUT_CONTROL_COUNT]
}

fn default_layer_keycodes() -> Vec<[u16; APPLICATION_LAYOUT_CONTROL_COUNT]> {
    vec![default_keycodes(); APPLICATION_LAYOUT_LAYER_COUNT]
}

pub(crate) fn default_layer_names() -> Vec<String> {
    [
        "Numbers",
        "Navigation",
        "Mouse",
        "Media",
        "Four",
        "Five",
        "Six",
        "Seven",
        "Eight",
        "Nine",
        "Ten",
        "Eleven",
        "Twelve",
        "Thirteen",
        "Fourteen",
        "Fifteen",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn legacy_default_layer_names() -> Vec<String> {
    (0..APPLICATION_LAYOUT_LAYER_COUNT)
        .map(|layer| {
            if layer == 0 {
                "Base".to_owned()
            } else {
                format!("Layer {layer}")
            }
        })
        .collect()
}

fn default_layouts() -> BTreeMap<String, ApplicationLayout> {
    let layout = ApplicationLayout::default_layout();
    [(layout.id.clone(), layout)].into_iter().collect()
}

fn default_layout_id() -> String {
    DEFAULT_APPLICATION_LAYOUT_ID.to_owned()
}

const fn default_revision() -> u32 {
    1
}

const fn default_next_id() -> u32 {
    1
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(executable: &str, title: &str) -> DetectedApplication {
        DetectedApplication {
            executable: executable.to_owned(),
            identities: Vec::new(),
            display_name: executable.to_owned(),
            window_title: title.to_owned(),
        }
    }

    #[test]
    fn default_is_always_present_and_cannot_be_removed() {
        let mut settings = DeviceApplicationLayouts::default();
        assert!(settings.layouts.contains_key(DEFAULT_APPLICATION_LAYOUT_ID));
        assert!(!settings.remove(DEFAULT_APPLICATION_LAYOUT_ID));
        assert_eq!(
            settings.layouts[DEFAULT_APPLICATION_LAYOUT_ID].layer_names,
            [
                "Numbers",
                "Navigation",
                "Mouse",
                "Media",
                "Four",
                "Five",
                "Six",
                "Seven",
                "Eight",
                "Nine",
                "Ten",
                "Eleven",
                "Twelve",
                "Thirteen",
                "Fourteen",
                "Fifteen",
            ]
        );
    }

    #[test]
    fn generated_v4_default_names_migrate_to_stock_firmware_names() {
        let mut settings = DeviceApplicationLayouts::default();
        settings
            .layouts
            .get_mut(DEFAULT_APPLICATION_LAYOUT_ID)
            .unwrap()
            .layer_names = legacy_default_layer_names();

        assert!(settings.normalize());
        assert_eq!(
            settings.layouts[DEFAULT_APPLICATION_LAYOUT_ID].layer_names,
            default_layer_names()
        );
    }

    #[test]
    fn executable_matching_is_case_and_extension_independent() {
        let mut settings = DeviceApplicationLayouts::default();
        settings.create_for_application(&app("Figma.exe", "Draft"));
        assert_ne!(
            settings.resolve(Some(&app("figma", "Other"))),
            DEFAULT_APPLICATION_LAYOUT_ID
        );
    }

    #[test]
    fn generic_identity_aliases_match_desktop_ids_processes_and_window_classes() {
        assert!(executables_match(
            "org.telegram.desktop",
            "telegram-desktop"
        ));
        assert!(executables_match("telegram-desktop", "Telegram"));
        assert!(executables_match("org.telegram.desktop", "Telegram"));
        assert!(!executables_match("telegram-desktop", "kotatogram-desktop"));
        assert!(executables_match("Arduino IDE", "arduino-ide"));
        assert!(!executables_match("code", "codeblocks"));
        assert!(executables_match(
            "org.gnome.Terminal.desktop",
            "gnome-terminal-server"
        ));
        assert!(executables_match("org.kde.konsole", "konsole"));
        assert!(executables_match("ticktick", "TickTick"));
        assert!(executables_match("ticktick", "ticktick_ticktick.desktop"));
    }

    #[test]
    fn generic_runtime_aliases_cannot_turn_telegram_into_a_catch_all() {
        let mut settings = DeviceApplicationLayouts::default();
        let telegram = settings.create_for_application(&DetectedApplication {
            executable: "telegram-desktop".to_owned(),
            identities: vec!["org.telegram.desktop".to_owned()],
            display_name: "Telegram".to_owned(),
            window_title: String::new(),
        });
        settings
            .layouts
            .get_mut(&telegram)
            .unwrap()
            .application_identities
            .extend([
                "desktop".to_owned(),
                "electron".to_owned(),
                "snap".to_owned(),
            ]);
        let vscode = settings.create_for_application(&DetectedApplication {
            executable: "code".to_owned(),
            identities: vec!["electron".to_owned(), "desktop".to_owned()],
            display_name: "Visual Studio Code".to_owned(),
            window_title: String::new(),
        });

        assert!(settings.normalize());
        assert!(!settings.layouts[&telegram]
            .application_identities
            .iter()
            .any(|identity| matches!(identity.as_str(), "desktop" | "electron" | "snap")));
        assert_eq!(
            settings.resolve(Some(&DetectedApplication {
                executable: "code".to_owned(),
                identities: vec!["electron".to_owned(), "desktop".to_owned()],
                display_name: "Visual Studio Code".to_owned(),
                window_title: "main.rs".to_owned(),
            })),
            vscode
        );
    }

    #[test]
    fn official_snap_ticktick_rule_matches_gnome_runtime_identity() {
        let mut settings = DeviceApplicationLayouts::default();
        let layout_id = settings.create_for_application(&DetectedApplication {
            executable: "ticktick".to_owned(),
            identities: normalized_identity_values([
                "ticktick",
                "TickTick",
                "ticktick_ticktick.desktop",
            ]),
            display_name: "TickTick".to_owned(),
            window_title: String::new(),
        });
        let focused = DetectedApplication {
            executable: "ticktick".to_owned(),
            identities: normalized_identity_values(["ticktick_ticktick.desktop", "TickTick"]),
            display_name: "TickTick".to_owned(),
            window_title: "TickTick".to_owned(),
        };

        assert_eq!(settings.resolve(Some(&focused)), layout_id);
    }

    #[test]
    fn official_macos_ticktick_bundle_rule_matches_nsworkspace_identity() {
        let mut settings = DeviceApplicationLayouts::default();
        let layout_id = settings.create_for_application(&DetectedApplication {
            executable: "com.TickTick.task.mac".to_owned(),
            identities: normalized_identity_values([
                "com.TickTick.task.mac",
                "/Applications/TickTick.app/Contents/MacOS/TickTick",
                "TickTick",
            ]),
            display_name: "TickTick".to_owned(),
            window_title: String::new(),
        });
        let focused = DetectedApplication {
            executable: "com.TickTick.task.mac".to_owned(),
            identities: normalized_identity_values([
                "com.TickTick.task.mac",
                "/Applications/TickTick.app/Contents/MacOS/TickTick",
                "TickTick",
            ]),
            display_name: "TickTick".to_owned(),
            window_title: String::new(),
        };

        assert_eq!(settings.resolve(Some(&focused)), layout_id);
    }

    #[test]
    fn macos_bundle_suffix_does_not_match_an_unrelated_application() {
        assert!(!executables_match(
            "com.TickTick.task.mac",
            "com.Other.product.mac"
        ));
    }

    #[test]
    fn distinct_macos_bundle_ids_do_not_conflict_through_shared_aliases() {
        let mut settings = DeviceApplicationLayouts::default();
        settings.create_for_application(&DetectedApplication {
            executable: "com.example.FirstApp".to_owned(),
            identities: vec!["Shared Helper".to_owned(), "finder".to_owned()],
            display_name: "First".to_owned(),
            window_title: String::new(),
        });
        let second = DetectedApplication {
            executable: "com.example.SecondApp".to_owned(),
            identities: vec!["Shared Helper".to_owned(), "finder".to_owned()],
            display_name: "Second".to_owned(),
            window_title: String::new(),
        };

        assert!(!settings.application_rule_exists(&second, "", None));
        assert_eq!(
            settings.resolve(Some(&second)),
            DEFAULT_APPLICATION_LAYOUT_ID
        );
    }

    #[test]
    fn exact_macos_bundle_id_is_a_duplicate_with_an_empty_optional_filter() {
        let mut settings = DeviceApplicationLayouts::default();
        let finder = DetectedApplication {
            executable: "com.apple.finder".to_owned(),
            identities: vec![
                "/System/Library/CoreServices/Finder.app/Contents/MacOS/Finder".to_owned(),
                "Finder".to_owned(),
            ],
            display_name: "Finder".to_owned(),
            window_title: String::new(),
        };
        settings.create_for_application(&finder);

        assert!(settings.application_rule_exists(&finder, "", None));
    }

    #[test]
    fn poisoned_macos_telegram_rule_does_not_capture_ticktick() {
        let mut settings = DeviceApplicationLayouts::default();
        let telegram = settings.create_for_application(&DetectedApplication {
            executable: "ru.keepcoder.Telegram".to_owned(),
            identities: normalized_identity_values([
                "ru.keepcoder.Telegram",
                "/Applications/Telegram.app/Contents/MacOS/Telegram",
            ]),
            display_name: "Telegram".to_owned(),
            window_title: String::new(),
        });
        settings
            .layouts
            .get_mut(&telegram)
            .unwrap()
            .application_identities
            .extend([
                "desktop".to_owned(),
                "electron".to_owned(),
                "mac".to_owned(),
            ]);
        let ticktick = settings.create_for_application(&DetectedApplication {
            executable: "com.TickTick.task.mac".to_owned(),
            identities: normalized_identity_values([
                "com.TickTick.task.mac",
                "/Applications/TickTick.app/Contents/MacOS/TickTick",
            ]),
            display_name: "TickTick".to_owned(),
            window_title: String::new(),
        });

        assert!(settings.normalize());
        assert!(!settings.layouts[&telegram]
            .application_identities
            .iter()
            .any(|identity| matches!(identity.as_str(), "desktop" | "electron" | "mac")));
        assert_eq!(
            settings.resolve(Some(&DetectedApplication {
                executable: "com.TickTick.task.mac".to_owned(),
                identities: normalized_identity_values([
                    "com.TickTick.task.mac",
                    "/Applications/TickTick.app/Contents/MacOS/TickTick",
                ]),
                display_name: "TickTick".to_owned(),
                window_title: String::new(),
            })),
            ticktick
        );
    }

    #[test]
    fn older_layouts_are_enriched_from_the_current_application_catalog() {
        let mut settings = DeviceApplicationLayouts::default();
        let layout_id = settings.create_for_application(&app("gnome-terminal", ""));
        settings
            .layouts
            .get_mut(&layout_id)
            .unwrap()
            .application_identities = Vec::new();

        let catalog = [DetectedApplication {
            executable: "org.gnome.Terminal".to_owned(),
            identities: vec![
                "org.gnome.Terminal.desktop".to_owned(),
                "Gnome-terminal".to_owned(),
            ],
            display_name: "Terminal".to_owned(),
            window_title: String::new(),
        }];

        assert!(settings.enrich_application_identities(&catalog));
        assert_eq!(
            settings.resolve(Some(&app("gnome-terminal-server", "Terminal"))),
            layout_id
        );
    }

    #[test]
    fn vscode_to_telegram_switches_to_each_configured_layout() {
        let mut settings = DeviceApplicationLayouts::default();
        let vscode = settings.create_for_application(&app("com.visualstudio.code", "main.rs"));
        let telegram = settings.create_for_application(&app("org.telegram.desktop", "Telegram"));

        assert_eq!(settings.resolve(Some(&app("code", "main.rs"))), vscode);
        settings.active_layout_id = vscode;
        assert_eq!(
            settings.resolve(Some(&app("Telegram", "Ergohaven — Telegram"))),
            telegram
        );
    }

    #[test]
    fn selected_app_matches_any_captured_identity_without_app_specific_code() {
        let mut selected = app("com.vendor.product.desktop", "");
        selected.identities = vec![
            "product-launcher".to_owned(),
            "VendorProductWindow".to_owned(),
        ];
        let mut settings = DeviceApplicationLayouts::default();
        let layout = settings.create_for_application(&selected);

        let mut foreground = app("product-bin", "Project");
        foreground.identities = vec!["VendorProductWindow".to_owned()];
        assert_eq!(settings.resolve(Some(&foreground)), layout);
    }

    #[test]
    fn vscode_desktop_and_url_handler_ids_match_runtime_process() {
        assert!(executables_match("com.visualstudio.code", "code"));
        assert!(executables_match("code-url-handler", "code"));
        assert!(!executables_match("codeblocks", "code"));
        assert!(executables_match("com.acme.paint.desktop", "paint"));
        assert!(executables_match("io.github.zen_browser.Zen", "zen"));
        assert!(!executables_match("com.acme.paint.desktop", "painter"));
    }

    #[test]
    fn default_names_migrate_per_layer_without_overwriting_custom_names() {
        let mut settings = DeviceApplicationLayouts::default();
        let default = settings
            .layouts
            .get_mut(DEFAULT_APPLICATION_LAYOUT_ID)
            .expect("default layout");
        default.layer_names[0] = "Base".to_owned();
        default.layer_names[1] = "My Navigation".to_owned();
        default.layer_names[2] = "Layer 2".to_owned();

        assert!(settings.normalize());
        let names = &settings
            .layouts
            .get(DEFAULT_APPLICATION_LAYOUT_ID)
            .expect("default layout")
            .layer_names;
        assert_eq!(names[0], "Numbers");
        assert_eq!(names[1], "My Navigation");
        assert_eq!(names[2], "Mouse");
    }

    #[test]
    fn title_specific_layout_wins() {
        let mut settings = DeviceApplicationLayouts::default();
        let generic = settings.create_for_application(&app("code", "main.rs"));
        let specific = settings.create_for_application(&app("code", "README"));
        settings.layouts.get_mut(&specific).unwrap().title_contains = "README".to_owned();
        assert_eq!(
            settings.resolve(Some(&app("code", "README — Visual Studio Code"))),
            specific
        );
        assert_eq!(
            settings.resolve(Some(&app("code", "main.rs — Visual Studio Code"))),
            generic
        );
    }

    #[test]
    fn layout_names_are_unique_case_insensitively() {
        let mut settings = DeviceApplicationLayouts::default();
        let telegram = settings.create_for_application_named(
            &app("telegram-desktop", "Telegram"),
            Some("Telegram"),
            "",
        );

        assert!(settings.layout_name_exists(" telegram ", None));
        assert!(settings.layout_name_exists("TELEGRAM", None));
        assert!(!settings.layout_name_exists("Telegram", Some(&telegram)));
        assert!(!settings.layout_name_exists("Messenger", None));
    }

    #[test]
    fn rename_layout_changes_only_the_display_name_and_revision() {
        let mut settings = DeviceApplicationLayouts::default();
        let telegram = settings.create_for_application_named(
            &app("telegram-desktop", "Telegram"),
            Some("Telegram"),
            "",
        );
        let before = settings.layouts.get(&telegram).unwrap().clone();

        assert!(settings.rename_layout(&telegram, " Telegram — работа "));
        let renamed = settings.layouts.get(&telegram).unwrap();
        assert_eq!(renamed.name, "Telegram — работа");
        assert_eq!(renamed.executable, before.executable);
        assert_eq!(
            renamed.application_identities,
            before.application_identities
        );
        assert_eq!(renamed.layers, before.layers);
        assert_eq!(renamed.layer_names, before.layer_names);
        assert_eq!(renamed.automatic_switching, before.automatic_switching);
        assert_ne!(renamed.revision, before.revision);
    }

    #[test]
    fn rename_layout_rejects_default_empty_and_duplicate_names() {
        let mut settings = DeviceApplicationLayouts::default();
        let telegram = settings.create_for_application_named(
            &app("telegram-desktop", "Telegram"),
            Some("Telegram"),
            "",
        );
        let blender =
            settings.create_for_application_named(&app("blender", "Blender"), Some("Blender"), "");

        assert!(!settings.rename_layout(DEFAULT_APPLICATION_LAYOUT_ID, "Primary"));
        assert!(!settings.rename_layout(&telegram, "  "));
        assert!(!settings.rename_layout(&telegram, " blender "));
        assert_eq!(settings.layouts[&telegram].name, "Telegram");
        assert_eq!(settings.layouts[&blender].name, "Blender");
        assert_eq!(
            settings.layouts[DEFAULT_APPLICATION_LAYOUT_ID].name,
            "Default"
        );
    }

    #[test]
    fn duplicate_application_rules_require_distinct_title_filters() {
        let mut settings = DeviceApplicationLayouts::default();
        let vscode = app("com.visualstudio.code", "Entropy");
        let generic = settings.create_for_application_named(&vscode, Some("VS Code"), "");

        assert!(settings.application_rule_exists(&vscode, "", None));
        assert!(!settings.application_rule_exists(&vscode, "Firmware", None));

        let firmware =
            settings.create_for_application_named(&vscode, Some("VS Code Firmware"), "Firmware");
        assert!(settings.application_rule_exists(&vscode, "firmware", None));
        assert!(!settings.application_rule_exists(&vscode, "Entropy", None));
        assert!(!settings.application_rule_exists(&vscode, "", Some(&generic)));
        assert!(!settings.application_rule_exists(&vscode, "Firmware", Some(&firmware),));
    }

    #[test]
    fn legacy_ambiguous_rules_resolve_deterministically() {
        let mut settings = DeviceApplicationLayouts::default();
        let first = settings.create_for_application(&app("telegram-desktop", "Telegram"));
        let second = settings.create_for_application(&app("telegram-desktop", "Telegram"));
        assert!(first < second);

        assert_eq!(settings.resolve(Some(&app("Telegram", "Telegram"))), first);
    }

    #[test]
    fn normalization_repairs_stale_inner_ids_before_deletion() {
        let mut settings = DeviceApplicationLayouts::default();
        let finder = settings.create_for_application(&app("com.apple.finder", "Finder"));
        let telegram = settings.create_for_application(&app("ru.keepcoder.Telegram", "Telegram"));
        settings.layouts.get_mut(&finder).unwrap().layers[0][0] = 0x1111;
        settings.layouts.get_mut(&telegram).unwrap().layers[0][0] = 0x2222;

        settings.layouts.get_mut(&finder).unwrap().id = telegram.clone();
        settings.layouts.get_mut(&telegram).unwrap().id = finder.clone();
        settings.active_layout_id = finder.clone();
        settings.editor_layout_id = telegram.clone();
        assert!(settings.normalize());
        assert_eq!(settings.layouts[&finder].id, finder);
        assert_eq!(settings.layouts[&telegram].id, telegram);

        assert!(settings.remove(&telegram));
        assert_eq!(settings.active_layout_id, finder);
        assert_eq!(settings.editor_layout_id, DEFAULT_APPLICATION_LAYOUT_ID);
        assert_eq!(settings.layouts[&finder].layers[0][0], 0x1111);
        assert_eq!(
            settings.resolve(Some(&app("com.apple.finder", "Finder"))),
            finder
        );
    }

    #[test]
    fn edit_targets_stable_id_even_when_editor_selection_changes() {
        let mut settings = DeviceApplicationLayouts::default();
        let finder = settings.create_for_application(&app("com.apple.finder", "Finder"));
        let telegram = settings.create_for_application(&app("ru.keepcoder.Telegram", "Telegram"));
        let telegram_before = settings.layouts[&telegram].clone();
        settings.editor_layout_id = telegram.clone();

        assert!(settings.update_application_rule(
            &finder,
            &app("com.apple.Safari", "Safari"),
            "Browser",
            "Private"
        ));
        assert_eq!(settings.layouts[&finder].executable, "com.apple.Safari");
        assert_eq!(settings.layouts[&finder].name, "Browser");
        assert_eq!(settings.layouts[&finder].title_contains, "Private");
        assert_eq!(settings.layouts[&telegram], telegram_before);
    }

    #[test]
    fn deleted_profile_id_is_not_reused_after_legacy_settings_normalize() {
        let mut settings = DeviceApplicationLayouts::default();
        let finder = settings.create_for_application(&app("com.apple.finder", "Finder"));
        settings.next_id = 1;
        assert!(settings.normalize());
        assert!(settings.remove(&finder));

        let replacement = settings.create_for_application(&app("com.apple.finder", "Finder"));
        assert_ne!(replacement, finder);
    }

    #[test]
    fn automatic_switching_is_independent_per_layout() {
        let mut settings = DeviceApplicationLayouts::default();
        let manual = settings.create_for_application(&app("calculator", "Calculator"));
        let figma = settings.create_for_application(&app("figma", "Draft"));
        settings.active_layout_id = manual.clone();
        settings
            .layouts
            .get_mut(&figma)
            .unwrap()
            .automatic_switching = false;
        assert_eq!(settings.resolve(Some(&app("figma", "Draft"))), manual);
        settings
            .layouts
            .get_mut(&figma)
            .unwrap()
            .automatic_switching = true;
        assert_eq!(settings.resolve(Some(&app("figma", "Draft"))), figma);
    }

    #[test]
    fn known_applications_always_switch_and_unknown_windows_follow_fallback_setting() {
        let mut settings = DeviceApplicationLayouts::default();
        let vscode = settings.create_for_application(&app("code", "Entropy"));
        let browser = settings.create_for_application(&app("firefox", "Docs"));
        settings.active_layout_id = vscode.clone();

        let resolved_browser = settings.resolve(Some(&app("firefox", "Docs")));
        assert_eq!(resolved_browser, browser);
        settings.active_layout_id = resolved_browser;

        assert_eq!(
            settings.resolve(Some(&app("entropy", "Entropy"))),
            DEFAULT_APPLICATION_LAYOUT_ID
        );

        settings.automatically_return_to_default = false;
        assert_eq!(settings.resolve(Some(&app("code", "Entropy"))), vscode);
        settings.active_layout_id = vscode.clone();
        assert_eq!(settings.resolve(Some(&app("firefox", "Docs"))), browser);
        settings.active_layout_id = browser.clone();
        assert_eq!(settings.resolve(Some(&app("entropy", "Entropy"))), browser);
        assert_eq!(
            settings.resolve(Some(&app("terminal", "Terminal"))),
            browser
        );
        assert_eq!(settings.resolve(None), browser);
    }

    #[test]
    fn legacy_unknown_window_setting_migrates_to_automatic_return_setting() {
        let settings: DeviceApplicationLayouts = serde_json::from_str(
            r#"{
                "unknown_window_uses_default": false
            }"#,
        )
        .unwrap();

        assert!(!settings.automatically_return_to_default);

        let serialized = serde_json::to_value(settings).unwrap();
        assert_eq!(
            serialized["automatically_return_to_default"],
            serde_json::Value::Bool(false)
        );
        assert!(serialized.get("unknown_window_uses_default").is_none());
    }

    #[test]
    fn legacy_global_automatic_switching_is_migrated_once() {
        let mut settings = DeviceApplicationLayouts::default();
        let figma = settings.create_for_application(&app("figma", "Draft"));
        let blender = settings.create_for_application(&app("blender", "Scene"));
        settings.legacy_automatic_switching = Some(false);

        assert!(settings.normalize());
        assert!(!settings.layouts[&figma].automatic_switching);
        assert!(!settings.layouts[&blender].automatic_switching);

        settings
            .layouts
            .get_mut(&figma)
            .unwrap()
            .automatic_switching = true;
        assert!(!settings.normalize());
        assert!(settings.layouts[&figma].automatic_switching);
    }

    #[test]
    fn legacy_single_layer_migrates_without_losing_keycodes() {
        let json = r#"{
            "id":"figma_1",
            "name":"Figma",
            "executable":"figma",
            "keycodes":[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15],
            "revision":1
        }"#;
        let mut layout: ApplicationLayout = serde_json::from_str(json).unwrap();
        assert!(layout.normalize());
        assert_eq!(layout.layers.len(), APPLICATION_LAYOUT_LAYER_COUNT);
        assert_eq!(layout.layers[0][0], 1);
        assert_eq!(layout.layers[0][14], 15);
    }

    #[test]
    fn each_application_layout_keeps_independent_layers_and_names() {
        let mut settings = DeviceApplicationLayouts::default();
        let figma = settings.create_for_application(&app("figma", "Draft"));
        let blender = settings.create_for_application(&app("blender", "Scene"));

        let layout = settings.layouts.get_mut(&figma).unwrap();
        assert!(layout.set_keycode(15, 4, 0x4321));
        assert!(layout.set_layer_name(15, "Components".to_owned()));

        assert_eq!(settings.layouts[&figma].layers[15][4], 0x4321);
        assert_eq!(settings.layouts[&figma].layer_names[15], "Components");
        assert_ne!(settings.layouts[&blender].layers[15][4], 0x4321);
        assert_ne!(settings.layouts[&blender].layer_names[15], "Components");
    }

    #[test]
    fn packet_family_carries_all_controls_and_crc() {
        let mut layout = ApplicationLayout::default_layout();
        layout.name = "Telegram".to_owned();
        for (layer, keycodes) in layout.layers.iter_mut().enumerate() {
            for (index, keycode) in keycodes.iter_mut().enumerate() {
                *keycode = 0x1000 + layer as u16 * 0x100 + index as u16;
            }
        }
        let packets = ApplicationLayoutSnapshot::from_layout(&layout).packets();
        assert_eq!(packets.len(), 50);
        assert_eq!(
            packets.iter().map(|packet| packet[0]).collect::<Vec<_>>(),
            std::iter::once(HID_APPLICATION_LAYOUT_BEGIN)
                .chain(
                    std::iter::repeat(HID_APPLICATION_LAYOUT_KEYCODES)
                        .take(APPLICATION_LAYOUT_LAYER_COUNT * 2),
                )
                .chain(
                    std::iter::repeat(HID_APPLICATION_LAYOUT_LAYER_NAME)
                        .take(APPLICATION_LAYOUT_LAYER_COUNT),
                )
                .chain(std::iter::once(HID_APPLICATION_LAYOUT_COMMIT))
                .collect::<Vec<_>>()
        );
        assert_eq!(packets[0][0], HID_APPLICATION_LAYOUT_BEGIN);
        assert_eq!(packets[0][8], APPLICATION_LAYOUT_LAYER_COUNT as u8);
        assert_eq!(packets[0][9], 8);
        assert_eq!(&packets[0][10..18], b"Telegram");
        assert_eq!(packets[1][2], 0);
        assert_eq!(packets[1][3], 0);
        assert_eq!(packets[1][4], 13);
        assert_eq!(packets[2][3], 13);
        assert_eq!(packets[2][4], 2);
        assert_eq!(packets[31][2], 15);
        assert_eq!(packets[31][3], 0);
        assert_eq!(packets[32][2], 15);
        assert_eq!(packets[32][3], 13);
        assert_eq!(packets[33][0], HID_APPLICATION_LAYOUT_LAYER_NAME);
        assert_eq!(packets[33][2], 0);
        assert_eq!(packets[33][3], 7);
        assert_eq!(&packets[33][4..11], b"Numbers");
        assert_eq!(packets[48][2], 15);
        let commit = packets.last().unwrap();
        assert_eq!(commit[0], HID_APPLICATION_LAYOUT_COMMIT);
        assert_ne!(u16::from_le_bytes([commit[7], commit[8]]), 0);
    }

    #[test]
    fn runtime_layout_name_is_utf8_safe_and_fits_one_packet() {
        let mut layout = ApplicationLayout::default_layout();
        layout.name = "Очень длинная раскладка Telegram".to_owned();

        let packets = ApplicationLayoutSnapshot::from_layout(&layout).packets();
        let length = usize::from(packets[0][9]);
        let encoded = &packets[0][10..10 + length];

        assert!(length <= APPLICATION_LAYOUT_NAME_BYTES);
        assert!(std::str::from_utf8(encoded).is_ok());
        assert!(layout
            .name
            .starts_with(std::str::from_utf8(encoded).unwrap()));
    }

    #[test]
    fn runtime_layer_names_are_utf8_safe_and_keepalive_is_one_packet() {
        let mut layout = ApplicationLayout::default_layout();
        layout.layer_names[3] = "Очень длинное имя слоя Медиа".to_owned();

        let snapshot = ApplicationLayoutSnapshot::from_layout(&layout);
        let packets = snapshot.packets();
        let packet = &packets[33 + 3];
        let length = usize::from(packet[3]);
        let encoded = &packet[4..4 + length];

        assert_eq!(packet[0], HID_APPLICATION_LAYOUT_LAYER_NAME);
        assert!(length <= APPLICATION_LAYOUT_NAME_BYTES);
        assert!(std::str::from_utf8(encoded).is_ok());
        assert!(layout.layer_names[3].starts_with(std::str::from_utf8(encoded).unwrap()));

        let keepalive = snapshot.keepalive_packet();
        assert_eq!(keepalive[0], HID_APPLICATION_LAYOUT_KEEPALIVE);
        assert_eq!(keepalive[1], APPLICATION_LAYOUT_PROTOCOL_VERSION);
        assert_eq!(keepalive[2], 1);
        assert_eq!(&keepalive[3..7], &layout.revision.to_le_bytes());
    }

    #[test]
    fn existing_four_layer_layout_expands_to_sixteen_without_data_loss() {
        let json = r#"{
            "id":"figma_1",
            "name":"Figma",
            "executable":"figma",
            "layers":[
                [1,2,3,4,5,6,7,8,9,10,11,12,13,14,15],
                [16,17,18,19,20,21,22,23,24,25,26,27,28,29,30],
                [31,32,33,34,35,36,37,38,39,40,41,42,43,44,45],
                [46,47,48,49,50,51,52,53,54,55,56,57,58,59,60]
            ],
            "layer_names":["Base","Tools","Components","Review"],
            "revision":1
        }"#;
        let mut layout: ApplicationLayout = serde_json::from_str(json).unwrap();

        assert!(layout.normalize());
        assert_eq!(layout.layers.len(), 16);
        assert_eq!(layout.layer_names.len(), 16);
        assert_eq!(layout.layers[3][14], 60);
        assert_eq!(layout.layer_names[3], "Review");
        assert_eq!(layout.layers[15], default_keycodes());
        assert_eq!(layout.layer_names[15], "Fifteen");
    }
}
