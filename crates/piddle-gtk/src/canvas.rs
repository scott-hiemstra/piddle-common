//! The bubble canvas widget: hands its size to a [`Paint`] view and asks it
//! to render through GtkSnapshot.

use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, pango};

/// What a [`Canvas`] draws. Each app's view (processes, pods) implements it.
pub trait Paint {
    /// The widget's size in pixels; called before every paint and on resize.
    fn resize(&mut self, width: f64, height: f64);
    fn paint(&mut self, snapshot: &gtk::Snapshot, pctx: &pango::Context);
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Canvas {
        pub view: OnceCell<Rc<RefCell<dyn Paint>>>,
        pub on_resize: RefCell<Option<Box<dyn Fn()>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Canvas {
        const NAME: &'static str = "PiddleCanvas";
        type Type = super::Canvas;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Canvas {}

    impl WidgetImpl for Canvas {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();
            if let Some(view) = self.view.get() {
                let mut v = view.borrow_mut();
                v.resize(obj.width() as f64, obj.height() as f64);
                v.paint(snapshot, &obj.pango_context());
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
            if let Some(view) = self.view.get() {
                view.borrow_mut().resize(width as f64, height as f64);
            }
            if let Some(f) = self.on_resize.borrow().as_ref() {
                f();
            }
        }
    }
}

glib::wrapper! {
    pub struct Canvas(ObjectSubclass<imp::Canvas>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Canvas {
    pub fn new<P: Paint + 'static>(view: Rc<RefCell<P>>) -> Self {
        let c: Self = glib::Object::new();
        let view: Rc<RefCell<dyn Paint>> = view;
        let _ = c.imp().view.set(view);
        c
    }

    pub fn connect_resized(&self, f: impl Fn() + 'static) {
        *self.imp().on_resize.borrow_mut() = Some(Box::new(f));
    }
}
