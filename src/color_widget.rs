use gtk::{gdk, glib, graphene::Rect, prelude::*, subclass::prelude::*};

const SIZE: i32 = 16;

mod imp {
    use std::cell::Cell;

    use super::*;

    #[derive(Debug, glib::Properties)]
    #[properties(wrapper_type = super::ColorWidget)]
    pub struct ColorWidget {
        #[property(get, set = Self::set_rgba, explicit_notify)]
        pub(super) color: Cell<gdk::RGBA>,
    }

    impl Default for ColorWidget {
        fn default() -> Self {
            Self {
                color: Cell::new(gdk::RGBA::TRANSPARENT),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ColorWidget {
        const NAME: &'static str = "SpicyColorWidget";
        type Type = super::ColorWidget;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("colorwidget");
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for ColorWidget {
        fn constructed(&self) {
            self.parent_constructed();

            self.obj().set_overflow(gtk::Overflow::Hidden);
        }
    }

    impl WidgetImpl for ColorWidget {
        fn measure(&self, _orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            (SIZE, SIZE, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();
            let bounds = Rect::new(0.0, 0.0, obj.width() as f32, obj.height() as f32);
            snapshot.append_color(&self.color.get(), &bounds);
        }
    }

    impl ColorWidget {
        fn set_rgba(&self, rgba: gdk::RGBA) {
            if self.color.get() == rgba {
                return;
            }

            let obj = self.obj();

            self.color.set(rgba);
            obj.queue_draw();
            obj.notify_color();
        }
    }
}

glib::wrapper! {
    pub struct ColorWidget(ObjectSubclass<imp::ColorWidget>)
        @extends gtk::Widget;
}
