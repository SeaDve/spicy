use gtk::{
    glib::{self, clone, closure_local},
    prelude::*,
    subclass::prelude::*,
};

use crate::{plot::Plot, plots::Plots};

mod imp {
    use std::marker::PhantomData;

    use gtk::glib::{once_cell::sync::Lazy, subclass::Signal};

    use super::*;

    #[derive(Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::PlotsDropdown)]
    #[template(resource = "/io/github/seadve/Spicy/ui/plots_dropdown.ui")]
    pub struct PlotsDropdown {
        #[property(get = Self::icon_name, set = Self::set_icon_name)]
        pub(super) icon_name: PhantomData<Option<String>>,

        #[template_child]
        pub(super) inner: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub(super) list_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) selection_model: TemplateChild<gtk::NoSelection>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlotsDropdown {
        const NAME: &'static str = "SpicyPlotsDropdown";
        type Type = super::PlotsDropdown;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PlotsDropdown {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.list_view
                .connect_activate(clone!(@weak obj => move |_, position| {
                    let imp = obj.imp();
                    let plot = imp
                        .selection_model
                        .item(position)
                        .unwrap()
                        .downcast::<Plot>()
                        .unwrap();
                    obj.emit_by_name::<()>("plot-activated", &[&plot]);
                    imp.inner.popdown();
                }));
        }

        fn dispose(&self) {
            self.dispose_template();
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder("plot-activated")
                    .param_types([Plot::static_type()])
                    .build()]
            });

            SIGNALS.as_ref()
        }
    }

    impl WidgetImpl for PlotsDropdown {}

    impl PlotsDropdown {
        fn set_icon_name(&self, icon_name: Option<&str>) {
            self.inner.set_icon_name(icon_name.unwrap_or_default());
        }

        fn icon_name(&self) -> Option<String> {
            self.inner.icon_name().map(|icon_name| icon_name.into())
        }
    }
}

glib::wrapper! {
    pub struct PlotsDropdown(ObjectSubclass<imp::PlotsDropdown>)
        @extends gtk::Widget;
}

impl PlotsDropdown {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn connect_plot_activated<F>(&self, cb: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, &Plot) + 'static,
    {
        self.connect_closure(
            "plot-activated",
            true,
            closure_local!(|obj: &Self, plot: &Plot| {
                cb(obj, plot);
            }),
        )
    }

    pub fn bind_plots(&self, plots: &Plots) {
        let imp = self.imp();

        imp.selection_model.set_model(Some(plots));
    }
}

#[gtk::template_callbacks]
impl PlotsDropdown {
    #[template_callback]
    fn row_star_opacity(_: &glib::Object, is_current: bool) -> f64 {
        if is_current {
            1.0
        } else {
            0.0
        }
    }
}
