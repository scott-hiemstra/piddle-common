//! Details windows and confirmations shared by the PIDdle family's GUIs:
//! the window frame, stat cards, the type-the-name confirmation used for
//! risky actions, and the CSS they all use.
//!
//! Each app supplies its own wording (what is at risk, what to type); the
//! look and the confirmation behavior are the same everywhere: the action
//! button stays disabled until the name is typed exactly, Enter only
//! confirms once it is, and Escape cancels.

use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::draw::glib_escape;

/// Styles for details windows, cards, action buttons, the risk banner,
/// status pills and the confirmation dialog. Install it with [`add_css`]
/// before an app's own stylesheet, so the app can override it.
pub const CSS: &str = r#"
window.details, .details { background: #0f1218; color: #e6e9ef; }
.details headerbar { background: #0f1218; border-bottom: 1px solid rgba(255,255,255,0.05); }
.details headerbar .title { font-weight: 800; letter-spacing: 0.5px; }
.details-title { font-size: 21px; font-weight: 800; }
.details-subtitle { color: rgba(200,210,230,0.65); font-feature-settings: "tnum"; }
.pill { background: rgba(255,255,255,0.07); border: 1px solid rgba(255,255,255,0.10); border-radius: 999px;
  padding: 3px 10px; font-size: 11px; font-weight: 600; color: rgba(220,226,238,0.9); }
.pill.pill-bad { background: rgba(240,90,90,0.14); border-color: rgba(240,90,90,0.45); color: #f6a3a3; }
.details-card { background: #161a22; border: 1px solid rgba(255,255,255,0.06); border-radius: 10px; padding: 12px 14px; }
.card-label { font-size: 10px; font-weight: 700; letter-spacing: 1px; color: rgba(158,173,204,0.9); }
.card-sub { font-size: 11px; color: rgba(200,210,230,0.55); }
.big-num { font-size: 22px; font-weight: 800; font-feature-settings: "tnum"; margin-top: 2px; }
.info-key { color: rgba(200,210,230,0.55); }
.info-val { color: #e6e9ef; }
.cmdline-box { background: #0a0c11; border: 1px solid rgba(255,255,255,0.08); border-radius: 8px; }
.details-status { color: rgba(200,210,230,0.7); font-size: 12px; }
.details-status.status-bad { color: #f6a3a3; }
button.action { background: #1d222c; color: #e6e9ef; border: 1px solid rgba(255,255,255,0.12); border-radius: 8px;
  padding: 6px 16px; box-shadow: none; margin: 0; }
button.action:hover { background: #272d3a; }
button.action.destructive-action { background: #b3261e; border-color: #c9372d; color: #ffffff; }
button.action.destructive-action:hover { background: #c9372d; }
button.action:disabled { opacity: 0.4; }
.risk-banner { background: rgba(245,158,11,0.10); border: 1px solid rgba(245,158,11,0.35); border-radius: 8px;
  padding: 9px 12px; color: #f5c56b; }
.confirm-heading { font-size: 16px; font-weight: 800; }
.confirm-entry { background: #0a0c11; color: #e6e9ef; border: 1px solid rgba(255,255,255,0.15); border-radius: 7px; min-height: 30px; }
.confirm-entry:focus-within { border-color: #c9372d; }
.confirm-entry text placeholder { color: rgba(230,233,239,0.3); font-style: italic; }
"#;

/// Install [`CSS`] for `display`. Call it before adding the app's own
/// stylesheet at the same priority, so the app's rules win.
pub fn add_css(display: &gdk::Display) {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    gtk::style_context_add_provider_for_display(display, &provider, gtk::STYLE_PROVIDER_PRIORITY_USER + 1);
}

/// A details window: a header bar titled `heading`, transient for
/// `parent`, closing on Escape. Returns the window and its padded vertical
/// content box; the caller fills the box and presents the window.
pub fn window(parent: &impl IsA<gtk::Window>, heading: &str, title: &str, width: i32) -> (gtk::Window, gtk::Box) {
    let header = gtk::HeaderBar::new();
    let t = gtk::Label::new(Some(heading));
    t.add_css_class("title");
    header.set_title_widget(Some(&t));
    let win = gtk::Window::builder().title(title).transient_for(parent).default_width(width).titlebar(&header).build();
    win.add_css_class("details");
    let root = gtk::Box::new(gtk::Orientation::Vertical, 16);
    root.set_margin_top(18);
    root.set_margin_bottom(18);
    root.set_margin_start(20);
    root.set_margin_end(20);
    win.set_child(Some(&root));
    close_on_escape(&win);
    (win, root)
}

/// Escape closes `win`, even when a focused child would see the key first.
pub fn close_on_escape(win: &gtk::Window) {
    let esc = gtk::ShortcutController::new();
    esc.set_propagation_phase(gtk::PropagationPhase::Capture);
    esc.add_shortcut(gtk::Shortcut::new(
        Some(gtk::KeyvalTrigger::new(gdk::Key::Escape, gdk::ModifierType::empty())),
        Some(gtk::CallbackAction::new(|w, _| {
            if let Some(w) = w.downcast_ref::<gtk::Window>() {
                w.close();
            }
            glib::Propagation::Stop
        })),
    ));
    win.add_controller(esc);
}

/// Adds a stat card (a small title, a big value, a caption) to `row` and
/// returns its value and caption labels.
pub fn stat_card(row: &gtk::Box, title: &str) -> (gtk::Label, gtk::Label) {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 2);
    card.add_css_class("details-card");
    let t = gtk::Label::builder().label(title).xalign(0.0).build();
    t.add_css_class("card-label");
    let v = gtk::Label::builder().xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).build();
    v.add_css_class("big-num");
    let s = gtk::Label::builder().xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).build();
    s.add_css_class("card-sub");
    card.append(&t);
    card.append(&v);
    card.append(&s);
    row.append(&card);
    (v, s)
}

/// Set a label's text only when it changed, so a once-a-second refresh
/// doesn't reset a selection the user is making.
pub fn set_text(label: &gtk::Label, text: &str) {
    if label.text() != text {
        label.set_text(text);
    }
}

/// What a [`confirm_typed`] dialog says.
pub struct TypedConfirm<'a> {
    /// Header bar title, e.g. "Critical process" or "Careful".
    pub title: &'a str,
    /// The question, e.g. "Delete shop/web-0?".
    pub heading: &'a str,
    /// Why this is risky, shown in the risk banner.
    pub warning: &'a str,
    /// What will happen, shown dimmed below the banner.
    pub detail: &'a str,
    /// What must be typed to enable the action.
    pub name: &'a str,
    /// The action button's label.
    pub verb: &'a str,
    /// The entry's placeholder, e.g. "process name".
    pub placeholder: &'a str,
}

/// The stronger confirmation for risky actions: a modal dialog whose action
/// button stays disabled until `c.name` is typed exactly (surrounding
/// spaces aside). Cancel, Escape and closing the dialog do nothing.
pub fn confirm_typed(parent: &impl IsA<gtk::Window>, c: TypedConfirm, on_confirm: impl Fn() + 'static) {
    let dlg = gtk::Window::builder().transient_for(parent).modal(true).resizable(false).title(c.verb).build();
    dlg.add_css_class("details");
    dlg.add_css_class("confirm");
    let header = gtk::HeaderBar::new();
    header.set_show_title_buttons(false);
    let t = gtk::Label::new(Some(c.title));
    t.add_css_class("title");
    header.set_title_widget(Some(&t));
    dlg.set_titlebar(Some(&header));

    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.set_margin_top(18);
    root.set_margin_bottom(18);
    root.set_margin_start(20);
    root.set_margin_end(20);
    let wrapped = |text: &str, class: &str| {
        let l =
            gtk::Label::builder().label(text.trim()).xalign(0.0).wrap(true).width_chars(44).max_width_chars(60).build();
        l.add_css_class(class);
        root.append(&l);
    };
    wrapped(c.heading, "confirm-heading");
    wrapped(c.warning, "risk-banner");
    wrapped(c.detail, "card-sub");
    let prompt = gtk::Label::builder().xalign(0.0).build();
    prompt.set_markup(&format!("Type <b>{}</b> to confirm:", glib_escape(c.name)));
    root.append(&prompt);
    let entry = gtk::Entry::builder().placeholder_text(c.placeholder).build();
    entry.add_css_class("confirm-entry");
    root.append(&entry);

    let cancel = gtk::Button::with_label("Cancel");
    cancel.add_css_class("action");
    let go = gtk::Button::with_label(c.verb);
    go.add_css_class("action");
    go.add_css_class("destructive-action");
    go.set_sensitive(false);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    buttons.set_margin_top(6);
    buttons.append(&cancel);
    buttons.append(&go);
    root.append(&buttons);
    dlg.set_child(Some(&root));

    let expected = c.name.to_string();
    entry.connect_changed(glib::clone!(
        #[weak]
        go,
        move |e| go.set_sensitive(name_matches(&e.text(), &expected))
    ));
    entry.connect_activate(glib::clone!(
        #[weak]
        go,
        move |_| {
            if go.is_sensitive() {
                go.emit_clicked();
            }
        }
    ));
    cancel.connect_clicked(glib::clone!(
        #[weak]
        dlg,
        move |_| dlg.close()
    ));
    go.connect_clicked(glib::clone!(
        #[weak]
        dlg,
        move |_| {
            dlg.close();
            on_confirm();
        }
    ));
    close_on_escape(&dlg);
    dlg.present();
    entry.grab_focus();
}

/// Whether what was typed confirms `name`: exactly, apart from surrounding
/// spaces. No case folding: "Web-0" is not "web-0".
pub fn name_matches(typed: &str, name: &str) -> bool {
    !name.is_empty() && typed.trim() == name
}

#[cfg(test)]
mod tests {
    use super::name_matches;

    #[test]
    fn typed_name_must_match_exactly() {
        assert!(name_matches("web-0", "web-0"));
        assert!(name_matches("  web-0 ", "web-0"));
        assert!(!name_matches("Web-0", "web-0"));
        assert!(!name_matches("web-", "web-0"));
        assert!(!name_matches("web-00", "web-0"));
        // An empty name never confirms, even with an empty entry.
        assert!(!name_matches("", ""));
    }
}
