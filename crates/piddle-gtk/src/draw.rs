//! Small GtkSnapshot and Pango helpers used by the canvases, cards and legends.

use gtk::prelude::*;
use gtk::{gdk, graphene, gsk, pango};

pub fn layout(pctx: &pango::Context, s: &str, px: f64, bold: bool) -> pango::Layout {
    let l = pango::Layout::new(pctx);
    l.set_font_description(Some(&font(px, bold)));
    l.set_text(s);
    l
}

pub fn text(snap: &gtk::Snapshot, l: &pango::Layout, x: f64, y: f64, color: gdk::RGBA) {
    snap.save();
    snap.translate(&point(x.round(), y.round()));
    snap.append_layout(l, &color);
    snap.restore();
}

pub fn ring(snap: &gtk::Snapshot, cx: f64, cy: f64, r: f64, width: f64, color: gdk::RGBA) {
    let rr = gsk::RoundedRect::from_rect(rect(cx - r, cy - r, r * 2.0, r * 2.0), r as f32);
    snap.append_border(&rr, &[width as f32; 4], &[color; 4]);
}

#[allow(clippy::too_many_arguments)]
pub fn card(snap: &gtk::Snapshot, x: f64, y: f64, w: f64, h: f64, radius: f64, fill: gdk::RGBA, border: gdk::RGBA) {
    let b = rect(x, y, w, h);
    let rr = gsk::RoundedRect::from_rect(b, radius as f32);
    snap.push_rounded_clip(&rr);
    snap.append_color(&fill, &b);
    snap.pop();
    snap.append_border(&rr, &[1.0; 4], &[border; 4]);
}

pub fn rect(x: f64, y: f64, w: f64, h: f64) -> graphene::Rect {
    graphene::Rect::new(x as f32, y as f32, w as f32, h as f32)
}

pub fn point(x: f64, y: f64) -> graphene::Point {
    graphene::Point::new(x as f32, y as f32)
}

pub fn rgba(r: f64, g: f64, b: f64, a: f64) -> gdk::RGBA {
    gdk::RGBA::new(r as f32, g as f32, b as f32, a.clamp(0.0, 1.0) as f32)
}

pub fn rgb(r: f64, g: f64, b: f64) -> gdk::RGBA {
    rgba(r, g, b, 1.0)
}

pub fn font(px: f64, bold: bool) -> pango::FontDescription {
    let mut fd = pango::FontDescription::from_string(if bold { "Sans Bold" } else { "Sans" });
    fd.set_absolute_size(px * pango::SCALE as f64);
    fd
}

/// Escape text for Pango markup. Run `piddle_encode::sanitize` first on
/// anything untrusted; this only makes it safe inside markup.
pub fn glib_escape(s: &str) -> String {
    gtk::glib::markup_escape_text(s).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_keeps_markup_inert() {
        assert_eq!(glib_escape("<b>x</b> & 'y'"), "&lt;b&gt;x&lt;/b&gt; &amp; &apos;y&apos;");
    }

    #[test]
    fn font_size_is_absolute_pixels() {
        let fd = font(12.0, true);
        assert_eq!(fd.size(), 12 * pango::SCALE);
        assert!(fd.is_size_absolute());
        assert_eq!(fd.weight(), pango::Weight::Bold);
    }

    #[test]
    fn alpha_is_clamped() {
        assert_eq!(rgba(0.1, 0.2, 0.3, 1.7).alpha(), 1.0);
        assert_eq!(rgba(0.1, 0.2, 0.3, -1.0).alpha(), 0.0);
    }
}
