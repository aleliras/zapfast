//! Privacy blur (anti-spy) overlay: paints a clean visual mask over text, images
//! and avatars without changing layout. Reveal on hover or while holding Alt.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Color32, Rect, vec2};

use crate::settings::{BlurScope, PrivacyBlurSettings};
use crate::theme::{Icon, Palette};
use crate::ui::widgets;

/// Global per-window state for the privacy blur overlay.
pub struct PrivacyBlurState {
    /// Whether the Alt key is currently held (global reveal).
    pub alt_held: bool,
    /// Timestamp of last user input (mouse/keyboard), for auto-blur timer.
    pub last_input: Instant,
    /// Cached blurred avatar textures, keyed by image URI.
    blurred_avatars: HashSet<String>,
}

impl Default for PrivacyBlurState {
    fn default() -> Self {
        Self {
            alt_held: false,
            last_input: Instant::now(),
            blurred_avatars: HashSet::new(),
        }
    }
}

impl PrivacyBlurState {
    /// Updates the Alt-held state from egui input.
    pub fn update_alt_held(&mut self, ctx: &egui::Context) {
        self.alt_held = ctx.input(|input| {
            input.modifiers.alt
                && !input.modifiers.shift
                && !input.modifiers.ctrl
                && !input.modifiers.mac_cmd
        });
    }

    /// Notes user activity (mouse move, click, key press) for the auto-blur timer.
    pub fn note_input(&mut self) {
        self.last_input = Instant::now();
    }

    /// Whether the auto-blur timer has elapsed and should force-enable blur.
    pub fn auto_blur_elapsed(&self, settings: &PrivacyBlurSettings) -> bool {
        if let Some(secs) = settings.auto_blur_after_secs {
            self.last_input.elapsed() >= Duration::from_secs(secs)
        } else {
            false
        }
    }

    /// Checks if a specific chat should be blurred based on scope.
    pub fn should_blur_chat(&self, settings: &PrivacyBlurSettings, chat_id: &str) -> bool {
        if !settings.enabled {
            return false;
        }
        match &settings.scope {
            BlurScope::AllChats => true,
            BlurScope::SpecificChats(jids) => jids.contains(chat_id),
        }
    }

    /// Checks if an element type should be blurred for the current chat.
    pub fn should_blur_element(
        &self,
        settings: &PrivacyBlurSettings,
        element: BlurElement,
    ) -> bool {
        if !settings.enabled {
            return false;
        }
        match element {
            BlurElement::ContactNames => settings.blur_contact_names,
            BlurElement::Avatars => settings.blur_avatars,
            BlurElement::ChatPreviews => settings.blur_chat_previews,
            BlurElement::Messages => settings.blur_messages,
            BlurElement::Composer => settings.blur_composer,
        }
    }

    /// Whether the element is currently revealed (hover or Alt held).
    pub fn is_revealed(&self, rect: Rect, ui: &egui::Ui) -> bool {
        if self.alt_held {
            return true;
        }
        ui.rect_contains_pointer(rect)
    }

    /// Registers a blurred avatar URI so we can cache its downscaled texture.
    pub fn register_blurred_avatar(&mut self, uri: String) {
        self.blurred_avatars.insert(uri);
    }
}

/// Which UI element is being considered for blurring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlurElement {
    ContactNames,
    Avatars,
    ChatPreviews,
    Messages,
    Composer,
}

/// Returns a semi-transparent version of the palette's surface color for the blur mask.
fn blur_mask_color(palette: &Palette, strength: f32) -> Color32 {
    // Use a frosted-glass effect: semi-transparent surface color.
    let alpha = (strength * 255.0) as u8;
    Color32::from_rgba_unmultiplied(
        palette.surface.r(),
        palette.surface.g(),
        palette.surface.b(),
        alpha,
    )
}

/// Paints a privacy blur mask over `rect` using the palette's surface colour.
/// The mask preserves layout (same rect) and uses a subtle frosted-glass look.
pub fn paint_blur_mask(ui: &egui::Ui, palette: &Palette, rect: Rect, strength: f32) {
    let color = blur_mask_color(palette, strength);
    ui.painter().rect_filled(rect, 0.0, color);
}

/// Paints a contact name with optional blur. Returns the text galley for layout.
#[allow(clippy::too_many_arguments)]
pub fn paint_contact_name(
    ui: &mut egui::Ui,
    palette: &Palette,
    text: &str,
    pos: egui::Pos2,
    font: egui::FontId,
    color: Color32,
    _width: f32,
    _max_lines: usize,
    blur: &PrivacyBlurState,
    settings: &PrivacyBlurSettings,
    chat_id: &str,
    element_rect: Rect,
) -> Arc<egui::Galley> {
    let should_blur = blur.should_blur_chat(settings, chat_id)
        && blur.should_blur_element(settings, BlurElement::ContactNames);
    let revealed = blur.is_revealed(element_rect, ui);

    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font.clone(), color);

    if should_blur && !revealed {
        // Draw the blur mask over the text area.
        let text_rect = Rect::from_min_size(pos, galley.size());
        paint_blur_mask(ui, palette, text_rect, 0.7);
        // Draw placeholder text (●●●) to keep layout stable.
        let dots = "●".repeat(text.chars().count().min(12));
        let dots_galley = ui.painter().layout_no_wrap(dots, font.clone(), palette.dim);
        ui.painter().galley(pos, dots_galley.clone(), palette.dim);
    } else {
        ui.painter().galley(pos, galley.clone(), color);
    }
    galley
}

/// Paints a chat preview (last message) with optional blur.
#[allow(clippy::too_many_arguments)]
pub fn paint_chat_preview(
    ui: &mut egui::Ui,
    palette: &Palette,
    text: &str,
    pos: egui::Pos2,
    font: egui::FontId,
    color: Color32,
    width: f32,
    blur: &PrivacyBlurState,
    settings: &PrivacyBlurSettings,
    chat_id: &str,
    element_rect: Rect,
) {
    let should_blur = blur.should_blur_chat(settings, chat_id)
        && blur.should_blur_element(settings, BlurElement::ChatPreviews);
    let revealed = blur.is_revealed(element_rect, ui);

    let galley = ui
        .painter()
        .layout(text.to_owned(), font.clone(), color, width);

    if should_blur && !revealed {
        let text_rect = Rect::from_min_size(pos, galley.size());
        paint_blur_mask(ui, palette, text_rect, 0.7);
        let dots = "●●●";
        let dots_galley = ui
            .painter()
            .layout_no_wrap(dots.to_owned(), font, palette.dim);
        ui.painter().galley(pos, dots_galley, palette.dim);
    } else {
        ui.painter().galley(pos, galley, color);
    }
}

/// Paints an avatar with optional blur using a downscaled texture.
#[allow(clippy::too_many_arguments)]
pub fn paint_avatar(
    ui: &mut egui::Ui,
    palette: &Palette,
    rect: Rect,
    name: &str,
    id: &str,
    picture: Option<&std::path::Path>,
    blur: &mut PrivacyBlurState,
    settings: &PrivacyBlurSettings,
    chat_id: &str,
) {
    let should_blur = blur.should_blur_chat(settings, chat_id)
        && blur.should_blur_element(settings, BlurElement::Avatars);
    let revealed = blur.is_revealed(rect, ui);

    if should_blur && !revealed && picture.is_some() {
        // Use the cached blurred texture path.
        if let Some(path) = picture {
            let uri = crate::util::image_uri(path);
            blur.register_blurred_avatar(uri.clone());
            // We need to load the original texture first to downscale it.
            // The caller should ensure the texture is already in egui's cache.
            // For now, paint a blurred placeholder.
            let color = Color32::from_rgba_unmultiplied(
                palette.surface.r(),
                palette.surface.g(),
                palette.surface.b(),
                153, // 0.6 * 255
            );
            ui.painter()
                .circle_filled(rect.center(), rect.width() / 2.0, color);
            // Draw initials as a fallback.
            let initials = crate::util::initials(name);
            let font = crate::theme::semibold(rect.width() * 0.35);
            let galley = ui.painter().layout_no_wrap(initials, font, palette.text);
            ui.painter()
                .galley(rect.center() - galley.size() / 2.0, galley, palette.text);
            return;
        }
    }

    // Normal avatar painting (delegates to widgets::paint_avatar).
    widgets::paint_avatar(ui, palette, rect, name, id, picture);
}

/// Paints a message bubble content with optional blur overlay.
/// This is called after the normal bubble content is painted.
pub fn paint_message_blur_overlay(
    ui: &egui::Ui,
    palette: &Palette,
    bubble_rect: Rect,
    blur: &PrivacyBlurState,
    settings: &PrivacyBlurSettings,
    chat_id: &str,
) {
    let should_blur = blur.should_blur_chat(settings, chat_id)
        && blur.should_blur_element(settings, BlurElement::Messages);
    let revealed = blur.is_revealed(bubble_rect, ui);

    if should_blur && !revealed {
        // Frosted glass overlay over the entire bubble.
        paint_blur_mask(ui, palette, bubble_rect, 0.8);
        // Subtle "eye" icon hint at top-right.
        let icon_rect =
            Rect::from_min_size(bubble_rect.right_top() + vec2(-24.0, 4.0), vec2(20.0, 20.0));
        crate::theme::paint_icon(ui, Icon::EyeOff, icon_rect, 16.0, palette.dim);
    }
}

/// Paints the composer text with optional blur.
#[allow(clippy::too_many_arguments)]
pub fn paint_composer_blur(
    ui: &mut egui::Ui,
    palette: &Palette,
    text: &str,
    pos: egui::Pos2,
    font: egui::FontId,
    color: Color32,
    width: f32,
    blur: &PrivacyBlurState,
    settings: &PrivacyBlurSettings,
    element_rect: Rect,
) -> Arc<egui::Galley> {
    let should_blur = blur.should_blur_element(settings, BlurElement::Composer);
    let revealed = blur.is_revealed(element_rect, ui);

    let galley = ui
        .painter()
        .layout(text.to_owned(), font.clone(), color, width);

    if should_blur && !revealed {
        let text_rect = Rect::from_min_size(pos, galley.size());
        paint_blur_mask(ui, palette, text_rect, 0.7);
        let dots = "●".repeat(text.chars().count().min(20));
        let dots_galley = ui.painter().layout_no_wrap(dots, font, palette.dim);
        ui.painter().galley(pos, dots_galley, palette.dim);
    } else {
        ui.painter().galley(pos, galley.clone(), color);
    }
    galley
}

/// Draws the quick-toggle button (eye/shield) in the rail or header.
pub fn draw_toggle_button(
    ui: &mut egui::Ui,
    palette: &Palette,
    enabled: bool,
    tooltip: &str,
) -> egui::Response {
    let (icon, color) = if enabled {
        (Icon::Shield, palette.accent)
    } else {
        (Icon::EyeOff, palette.secondary)
    };
    crate::theme::icon_button(ui, icon, 18.0, color, palette.text, tooltip)
}

/// Draws the "Presentation mode" preset button.
pub fn draw_presentation_mode_button(
    ui: &mut egui::Ui,
    palette: &Palette,
    tooltip: &str,
) -> egui::Response {
    crate::theme::icon_button(
        ui,
        Icon::Monitor,
        18.0,
        palette.accent,
        palette.text,
        tooltip,
    )
}
