//! Reusable egui UI components built on [`crate::theme::EarthMeshTheme`]: status
//! badges, cards, status messages, section headers. Thin rendering helpers — the
//! pure mappings (icon/level) are unit-tested; rendering is compile-checked.
//!
//! Staged component library: some helpers (card / section_header) are wired
//! incrementally, so allow dead_code here.
#![allow(dead_code)]

use eframe::egui::{self, Color32, RichText};

use crate::theme::EarthMeshTheme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageKind {
    Info,
    Success,
    Warning,
    Error,
}

impl MessageKind {
    pub fn level(&self) -> &'static str {
        match self {
            MessageKind::Info => "info",
            MessageKind::Success => "pass",
            MessageKind::Warning => "warn",
            MessageKind::Error => "fail",
        }
    }
    pub fn icon(&self) -> &'static str {
        match self {
            MessageKind::Info => "ℹ",
            MessageKind::Success => "✔",
            MessageKind::Warning => "⚠",
            MessageKind::Error => "✖",
        }
    }
}

/// A small colored status pill (e.g. PASS / WARN / FAIL).
pub fn status_badge(ui: &mut egui::Ui, theme: &EarthMeshTheme, level: &str, text: &str) {
    let color = theme.status_color(level);
    ui.label(
        RichText::new(format!(" {} ", text))
            .color(Color32::WHITE)
            .background_color(color)
            .strong(),
    );
}

/// A card (grouped, filled frame) with an optional bold title.
pub fn card<R>(
    ui: &mut egui::Ui,
    theme: &EarthMeshTheme,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::Frame::group(ui.style())
        .fill(theme.card_fill())
        .inner_margin(theme.spacing().md)
        .show(ui, |ui| {
            if !title.is_empty() {
                ui.label(RichText::new(title).strong());
                ui.add_space(theme.spacing().sm);
            }
            add_contents(ui)
        })
        .inner
}

/// An info/success/warning/error line with an icon and status color.
pub fn status_message(ui: &mut egui::Ui, theme: &EarthMeshTheme, kind: MessageKind, text: &str) {
    let color = theme.status_color(kind.level());
    ui.label(RichText::new(format!("{} {}", kind.icon(), text)).color(color));
}

/// A bold section header.
pub fn section_header(ui: &mut egui::Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(RichText::new(text).heading());
}

/// An empty-state hint (dimmed, centered-ish).
pub fn empty_state(ui: &mut egui::Ui, text: &str) {
    ui.add_space(8.0);
    ui.label(RichText::new(text).weak().italics());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_kind_levels_and_icons() {
        assert_eq!(MessageKind::Success.level(), "pass");
        assert_eq!(MessageKind::Warning.level(), "warn");
        assert_eq!(MessageKind::Error.level(), "fail");
        assert_eq!(MessageKind::Info.level(), "info");
        assert!(!MessageKind::Warning.icon().is_empty());
    }
}
