use gtk::{
    gdk,
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};

use crate::color_widget::ColorWidget;

mod imp {
    use std::{cell::OnceCell, marker::PhantomData};

    use super::*;

    #[derive(Default, glib::Properties, gtk::CompositeTemplate)]
    #[properties(wrapper_type = super::PlotViewFilterRow)]
    #[template(resource = "/io/github/seadve/Spicy/ui/plot_view_filter_row.ui")]
    pub struct PlotViewFilterRow {
        #[property(get, set, construct_only)]
        pub(super) name: OnceCell<String>,
        #[property(get, set, construct_only)]
        pub(super) color: OnceCell<gdk::RGBA>,
        #[property(get = Self::is_active)]
        pub(super) is_active: PhantomData<bool>,

        #[template_child]
        pub(super) color_widget: TemplateChild<ColorWidget>,
        #[template_child]
        pub(super) title_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) check_button: TemplateChild<gtk::CheckButton>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlotViewFilterRow {
        const NAME: &'static str = "SpicyPlotViewFilterRow";
        type Type = super::PlotViewFilterRow;
        type ParentType = gtk::ListBoxRow;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("plotviewfilterrow");
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PlotViewFilterRow {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.title_label.set_label(&obj.name());
            self.color_widget.set_color(obj.color());

            self.check_button
                .connect_active_notify(clone!(@weak obj => move |_| {
                    obj.notify_is_active();
                }));
        }
    }

    impl WidgetImpl for PlotViewFilterRow {}
    impl ListBoxRowImpl for PlotViewFilterRow {}

    impl PlotViewFilterRow {
        fn is_active(&self) -> bool {
            self.check_button.is_active()
        }
    }
}

glib::wrapper! {
    pub struct PlotViewFilterRow(ObjectSubclass<imp::PlotViewFilterRow>)
        @extends gtk::Widget, gtk::ListBoxRow;
}

impl PlotViewFilterRow {
    pub fn new(name: &str, color: gdk::RGBA) -> Self {
        glib::Object::builder()
            .property("name", name)
            .property("color", color)
            .build()
    }

    pub fn handle_activation(&self) {
        let was_activated = self.imp().check_button.activate();
        debug_assert!(was_activated);
    }
}
