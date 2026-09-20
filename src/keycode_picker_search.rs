use super::*;

// Picker search. Pipeline: query -> normalize -> memo check -> corpus
// (5 sources below) -> substring match -> cached Vec<SearchEntry> ->
// results grid. Everything above the final impl block is egui-free.

/// One searchable pick: the binding to assign plus the strings it matches on.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct SearchEntry {
    /// Assigned as the picker result when the entry is clicked.
    pub(super) binding: crate::keyboard::KeyBinding,
    /// Visible caption; drawn on the result keycap and matched against.
    pub(super) label: String,
    /// Technical alias haystack: QMK name, slot id ("Tap Dance TD3"), user names.
    pub(super) name: String,
    /// Localized description; shown as the tooltip and matched against.
    pub(super) tooltip: String,
}

impl SearchEntry {
    fn matches(&self, needle_lower: &str) -> bool {
        search_text_matches(needle_lower, &self.label, &self.name, &self.tooltip)
    }
}

/// Search state of the picker: the live query plus results cached for it.
#[derive(Default)]
pub(super) struct PickerSearch {
    /// Text bound to the search field.
    pub(super) query: String,
    /// Normalized query the cached results were computed for.
    resolved_query: String,
    results: Vec<SearchEntry>,
}

impl PickerSearch {
    pub(super) fn is_active(&self) -> bool {
        !self.resolved_query.is_empty()
    }

    pub(super) fn results(&self) -> &[SearchEntry] {
        &self.results
    }

    pub(super) fn reset(&mut self) {
        self.query.clear();
        self.resolved_query.clear();
        self.results.clear();
    }
}

fn normalized_query(query: &str) -> String {
    query.trim().replace('\n', " ").to_lowercase()
}

/// Case-insensitive substring match against a key's visible label, technical
/// name, and localized tooltip. `needle_lower` must already be normalized.
pub(super) fn search_text_matches(
    needle_lower: &str,
    label: &str,
    name: &str,
    tooltip: &str,
) -> bool {
    let normalize = |text: &str| text.replace('\n', " ").to_lowercase();
    normalize(label).contains(needle_lower)
        || name.to_lowercase().contains(needle_lower)
        || normalize(tooltip).contains(needle_lower)
}

impl KeycodePicker {
    /// Clear the search field and cached results (e.g. when the host page
    /// hands the picker a new target).
    pub(crate) fn reset_search(&mut self) {
        self.search.reset();
    }

    /// Recompute cached search results when the query changed.
    pub(super) fn refresh_vial_search_results(&mut self) {
        let query = normalized_query(&self.search.query);
        if query == self.search.resolved_query {
            return;
        }
        self.search.resolved_query = query.clone();
        self.search.results.clear();
        if query.is_empty() {
            return;
        }

        let mut corpus: Vec<SearchEntry> = Vec::new();
        self.collect_keycode_entries(&mut corpus);
        self.collect_custom_keycode_entries(&mut corpus);
        self.collect_special_entries(&mut corpus);
        self.collect_macro_slot_entries(&mut corpus);
        self.collect_tap_dance_slot_entries(&mut corpus);
        self.collect_universal_symbol_entries(&mut corpus);

        let mut results: Vec<SearchEntry> = Vec::new();
        for entry in corpus {
            if entry.matches(&query) && !results.iter().any(|hit| hit.binding == entry.binding) {
                results.push(entry);
            }
        }
        self.search.results = results;
    }

    // ── Corpus: every source of searchable picks, one function each ──

    /// The full QMK/Vial keycode table, minus what the firmware does not support.
    fn collect_keycode_entries(&self, out: &mut Vec<SearchEntry>) {
        let custom_pairs = self.custom_keycode_pairs();
        for kc in KEYCODES.iter() {
            if !self.vial_keycode_supported(kc) {
                continue;
            }
            out.push(SearchEntry {
                binding: crate::keyboard::KeyBinding::Vial(kc.value),
                label: keycode_label_with_names_and_layout(
                    kc.value,
                    &custom_pairs,
                    &self.layer_names,
                    self.key_legend_layout,
                ),
                name: kc.name.to_string(),
                tooltip: crate::i18n::tr_text(
                    self.language,
                    &self.picker_keycode_tooltip(kc.value, &custom_pairs),
                ),
            });
        }
    }

    /// Device-defined custom keycodes; name, label, and title come from firmware.
    fn collect_custom_keycode_entries(&self, out: &mut Vec<SearchEntry>) {
        for (name, label, title, value) in &self.custom_keycodes {
            if label.trim().is_empty() {
                continue;
            }
            out.push(SearchEntry {
                binding: crate::keyboard::KeyBinding::Vial(*value),
                label: label.clone(),
                name: name.clone(),
                tooltip: crate::i18n::tr_text(self.language, title),
            });
        }
    }

    /// Curated Special-tab keys: searchable by the same captions the tab shows.
    fn collect_special_entries(&self, out: &mut Vec<SearchEntry>) {
        for (label, value, tip) in self.vial_special_key_entries() {
            if !self.picker_value_supported(value) {
                continue;
            }
            if let Some(kc) = KEYCODES.iter().find(|kc| kc.value == value) {
                if !self.vial_keycode_supported(kc) {
                    continue;
                }
            }
            out.push(SearchEntry {
                binding: crate::keyboard::KeyBinding::Vial(value),
                label,
                name: String::new(),
                tooltip: crate::i18n::tr_text(self.language, &tip),
            });
        }
    }

    /// Macro slots, searchable by "Macro", "M{n}", and the user-given name.
    fn collect_macro_slot_entries(&self, out: &mut Vec<SearchEntry>) {
        if !self.supports_macro {
            return;
        }
        let caption = tr_picker(self.language, "macro_editor.picker_item");
        let custom_pairs = self.custom_keycode_pairs();
        for idx in 0..self.macro_count {
            let value = 0x7700 + idx as u16;
            let user_name = self.macro_names.get(idx).map(String::as_str).unwrap_or("");
            out.push(SearchEntry {
                binding: crate::keyboard::KeyBinding::Vial(value),
                label: keycode_label_with_names_and_layout(
                    value,
                    &custom_pairs,
                    &self.layer_names,
                    self.key_legend_layout,
                ),
                name: format!("{caption} M{idx} {user_name}"),
                tooltip: crate::i18n::tr_text(
                    self.language,
                    &self.picker_keycode_tooltip(value, &custom_pairs),
                ),
            });
        }
    }

    /// Tap Dance slots, searchable by "Tap Dance", "TD{n}", and the slot name.
    fn collect_tap_dance_slot_entries(&self, out: &mut Vec<SearchEntry>) {
        if !self.supports_tap_dance {
            return;
        }
        let caption = tr_picker(self.language, "tap_dance_editor.picker_item");
        let custom_pairs = self.custom_keycode_pairs();
        for idx in 0..self.tap_dance_entries.len() {
            let value = 0x5700 + idx as u16;
            let slot_name = self
                .tap_dance_names
                .get(idx)
                .map(String::as_str)
                .unwrap_or("");
            out.push(SearchEntry {
                binding: crate::keyboard::KeyBinding::Vial(value),
                label: keycode_label_with_names_and_layout(
                    value,
                    &custom_pairs,
                    &self.layer_names,
                    self.key_legend_layout,
                ),
                name: format!("{caption} TD{idx} {slot_name}"),
                tooltip: crate::i18n::tr_text(
                    self.language,
                    &self.picker_keycode_tooltip(value, &custom_pairs),
                ),
            });
        }
    }

    /// Universal Symbols (native RMK actions): layout controls, punctuation,
    /// and Russian letters when the firmware supports them.
    fn collect_universal_symbol_entries(&self, out: &mut Vec<SearchEntry>) {
        if !self.universal_symbols_available() {
            return;
        }
        let push = |out: &mut Vec<SearchEntry>, user_id: u8, name: String| {
            let binding = crate::universal_symbols::binding(user_id);
            let Some(label) = crate::universal_symbols::label_for_user_id(user_id) else {
                return;
            };
            let crate::keyboard::KeyBinding::Rmk(action) = binding else {
                return;
            };
            let tooltip = crate::universal_symbols::tooltip(action).unwrap_or_default();
            out.push(SearchEntry {
                binding,
                label,
                name,
                tooltip: crate::i18n::tr_text(self.language, &tooltip),
            });
        };
        for control in crate::universal_symbols::CONTROLS {
            push(out, control.user_id, control.name.to_string());
        }
        for symbol in crate::universal_symbols::SYMBOLS {
            push(out, symbol.user_id, symbol.symbol.to_string());
        }
        if self.universal_russian_letters_available() {
            for letter in crate::universal_symbols::RUSSIAN_LETTERS {
                push(out, letter.user_id, letter.letter.to_string());
            }
        }
    }

    // ── Results UI ──

    pub(super) fn show_vial_search_results(&mut self, ui: &mut egui::Ui) {
        if self.search.results().is_empty() {
            ui.add_space(52.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(tr_picker(self.language, "key_picker.search_empty"))
                        .size(13.0)
                        .color(ui.visuals().weak_text_color()),
                );
            });
            return;
        }

        ui.label(
            RichText::new(crate::i18n::tr_catalog_format(
                self.language,
                "key_picker.search_matches",
                &[("count", self.search.results().len().to_string().as_str())],
            ))
            .size(11.0)
            .color(Color32::from_gray(150)),
        );
        ui.add_space(4.0);

        let results = self.search.results().to_vec();
        ui.horizontal_wrapped(|ui| {
            for entry in results {
                let resp = ui
                    .add_sized(Self::picker_key_size(ui.ctx()), egui::Button::new(""))
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                Self::paint_compact_picker_label(ui, &resp, &entry.label);
                if resp.clicked() {
                    self.result = Some(entry.binding);
                    self.open = false;
                }
                resp.on_hover_text(entry.tooltip);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard::KeyBinding;

    fn results_contain(picker: &KeycodePicker, value: u16) -> bool {
        picker
            .search
            .results()
            .iter()
            .any(|hit| hit.binding == KeyBinding::Vial(value))
    }

    #[test]
    fn matches_by_label_name_and_tooltip_case_insensitively() {
        assert!(search_text_matches("vol", "Vol+\nUp", "KC_VOLU", "Volume up"));
        assert!(search_text_matches("vol+ up", "Vol+\nUp", "KC_VOLU", ""));
        assert!(search_text_matches("kc_volu", "Vol+", "KC_VOLU", ""));
        assert!(search_text_matches("громкость", "Vol+", "KC_VOLU", "Громкость +"));
        assert!(!search_text_matches("bluetooth", "Vol+", "KC_VOLU", "Volume up"));
    }

    #[test]
    fn collects_matches_across_keycodes_and_macro_names() {
        let mut picker = KeycodePicker {
            macro_count: 2,
            macro_names: vec!["Email signature".into(), String::new()],
            ..Default::default()
        };

        picker.search.query = "email".into();
        picker.refresh_vial_search_results();
        assert!(results_contain(&picker, 0x7700));

        let sample = KEYCODES
            .iter()
            .find(|kc| matches!(kc.category, crate::keycode::KeycodeCategory::Basic))
            .expect("KEYCODES should contain a basic key");
        picker.search.query = sample.name.to_string();
        picker.refresh_vial_search_results();
        assert!(results_contain(&picker, sample.value));

        picker.search.query.clear();
        picker.refresh_vial_search_results();
        assert!(picker.search.results().is_empty());
    }

    #[test]
    fn finds_universal_symbols_when_firmware_supports_them() {
        let mut picker = KeycodePicker {
            supports_universal_symbols: true,
            supports_rmk_native_key_actions: true,
            rmk_native_key_actions_allowed_for_target: true,
            supports_universal_russian_letters: true,
            ..Default::default()
        };
        picker.search.query = "universal".into();
        picker.refresh_vial_search_results();
        let sync = crate::universal_symbols::binding(crate::universal_symbols::USER_SYNC);
        let ru_letter = crate::universal_symbols::binding(
            crate::universal_symbols::USER_RUSSIAN_LETTER_START,
        );
        assert!(picker.search.results().iter().any(|hit| hit.binding == sync));
        assert!(picker.search.results().iter().any(|hit| hit.binding == ru_letter));

        // Without RMK actions allowed for the target, universal entries vanish.
        let mut gated = KeycodePicker {
            supports_universal_symbols: true,
            supports_rmk_native_key_actions: true,
            rmk_native_key_actions_allowed_for_target: false,
            ..Default::default()
        };
        gated.search.query = "universal".into();
        gated.refresh_vial_search_results();
        assert!(!gated.search.results().iter().any(|hit| hit.binding == sync));
    }

    #[test]
    fn finds_special_captions_and_slot_entries() {
        let mut picker = KeycodePicker {
            macro_count: 1,
            macro_names: vec![String::new()],
            tap_dance_entries: vec![TapDanceEntry::default(); 2],
            tap_dance_names: vec![String::new(), "Волна".into()],
            ..Default::default()
        };

        picker.search.query = "tap dan".into();
        picker.refresh_vial_search_results();
        assert!(results_contain(&picker, 0x5700));
        assert!(results_contain(&picker, 0x5701));

        picker.search.query = "волна".into();
        picker.refresh_vial_search_results();
        assert_eq!(picker.search.results().len(), 1);
        assert!(results_contain(&picker, 0x5701));

        picker.search.query = "none".into();
        picker.refresh_vial_search_results();
        assert!(results_contain(&picker, 0x0000));

        picker.search.query = "inherit".into();
        picker.refresh_vial_search_results();
        assert!(results_contain(&picker, 0x0001));

        picker.search.query = "macro".into();
        picker.refresh_vial_search_results();
        assert!(results_contain(&picker, 0x7700));
    }
}
