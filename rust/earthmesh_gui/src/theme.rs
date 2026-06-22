//! EarthMesh GUI design system (MVP): spacing scale, status / map-layer colors,
//! card style, light/dark theme. Additive — `apply` tweaks egui visuals without
//! replacing the existing `configure_style`.
//!
//! Staged design-system API: some helpers (accent / layer_color) are wired
//! incrementally as the map legend / layer list land, so allow dead_code here.
#![allow(dead_code)]

use eframe::egui::{self, Color32};

/// Consistent spacing scale (logical points).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spacing {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
}

impl Spacing {
    pub const SCALE: Spacing = Spacing {
        xs: 2.0,
        sm: 4.0,
        md: 8.0,
        lg: 16.0,
        xl: 24.0,
    };
}

// Status colors (shared with the quality dashboard badges).
pub const PASS: Color32 = Color32::from_rgb(46, 160, 67);
pub const WARN: Color32 = Color32::from_rgb(210, 153, 34);
pub const FAIL: Color32 = Color32::from_rgb(218, 54, 51);
pub const INFO: Color32 = Color32::from_rgb(47, 129, 247);
pub const NEUTRAL: Color32 = Color32::from_rgb(139, 148, 158);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EarthMeshTheme {
    pub dark: bool,
}

impl Default for EarthMeshTheme {
    fn default() -> Self {
        Self::light()
    }
}

impl EarthMeshTheme {
    pub fn light() -> Self {
        Self { dark: false }
    }
    pub fn dark() -> Self {
        Self { dark: true }
    }

    pub fn spacing(&self) -> Spacing {
        Spacing::SCALE
    }

    pub fn accent(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(88, 166, 255)
        } else {
            Color32::from_rgb(31, 111, 235)
        }
    }

    pub fn card_fill(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(33, 38, 45)
        } else {
            Color32::from_rgb(246, 248, 250)
        }
    }

    /// pass / warn / fail / info → color (anything else → neutral).
    pub fn status_color(&self, level: &str) -> Color32 {
        match level.to_ascii_lowercase().as_str() {
            "pass" | "ok" | "success" => PASS,
            "warn" | "warning" => WARN,
            "fail" | "error" => FAIL,
            "info" => INFO,
            _ => NEUTRAL,
        }
    }

    /// Stable color per map data layer (land/ocean/river/coast/...).
    pub fn layer_color(&self, layer: &str) -> Color32 {
        match layer.to_ascii_lowercase().as_str() {
            "land" => Color32::from_rgb(120, 170, 90),
            "ocean" | "sea" => Color32::from_rgb(60, 120, 200),
            "river" | "river_network" => Color32::from_rgb(40, 140, 200),
            "coast" | "coastline" => Color32::from_rgb(220, 150, 60),
            "estuary" | "river_mouth" => Color32::from_rgb(180, 100, 200),
            "wetland" | "wetland_delta" => Color32::from_rgb(90, 150, 130),
            "urban" => Color32::from_rgb(180, 70, 70),
            "mesh" => NEUTRAL,
            _ => NEUTRAL,
        }
    }

    /// Apply visuals + spacing to the egui context (called after `configure_style`).
    pub fn apply(&self, ctx: &egui::Context) {
        let visuals = if self.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        ctx.set_visuals(visuals);
        let s = self.spacing();
        ctx.global_style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(s.sm, s.sm);
            style.spacing.button_padding = egui::vec2(s.md, s.sm);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_color_maps_levels() {
        let t = EarthMeshTheme::light();
        assert_eq!(t.status_color("pass"), PASS);
        assert_eq!(t.status_color("WARN"), WARN);
        assert_eq!(t.status_color("fail"), FAIL);
        assert_eq!(t.status_color("info"), INFO);
        assert_eq!(t.status_color("whatever"), NEUTRAL);
    }

    #[test]
    fn light_and_dark_differ() {
        assert!(!EarthMeshTheme::light().dark);
        assert!(EarthMeshTheme::dark().dark);
        assert_ne!(
            EarthMeshTheme::light().card_fill(),
            EarthMeshTheme::dark().card_fill()
        );
    }

    #[test]
    fn spacing_scale_is_monotonic() {
        let s = Spacing::SCALE;
        assert!(s.xs < s.sm && s.sm < s.md && s.md < s.lg && s.lg < s.xl);
    }

    #[test]
    fn layer_colors_distinct_for_land_ocean() {
        let t = EarthMeshTheme::light();
        assert_ne!(t.layer_color("land"), t.layer_color("ocean"));
        assert_ne!(t.layer_color("river"), t.layer_color("coast"));
    }
}
