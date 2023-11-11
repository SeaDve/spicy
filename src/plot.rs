use gtk::{glib, prelude::*, subclass::prelude::*};

mod imp {
    use std::cell::{Cell, OnceCell};

    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::Plot)]
    pub struct Plot {
        #[property(get, set, construct_only)]
        pub(super) name: OnceCell<String>,
        #[property(get, set, construct_only)]
        pub(super) is_current: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Plot {
        const NAME: &'static str = "SpicyPlot";
        type Type = super::Plot;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Plot {}
}

glib::wrapper! {
    pub struct Plot(ObjectSubclass<imp::Plot>);
}

impl Plot {
    pub fn new(name: &str, is_current: bool) -> Self {
        glib::Object::builder()
            .property("name", name)
            .property("is-current", is_current)
            .build()
    }
}
