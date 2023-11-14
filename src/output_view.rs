use gtk::{
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Spicy/ui/output_view.ui")]
    pub struct OutputView {
        #[template_child]
        pub(super) scrolled_window: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub(super) text_view: TemplateChild<gtk::TextView>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for OutputView {
        const NAME: &'static str = "SpicyOutputView";
        type Type = super::OutputView;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for OutputView {
        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for OutputView {}
}

glib::wrapper! {
    pub struct OutputView(ObjectSubclass<imp::OutputView>)
        @extends gtk::Widget;
}

impl OutputView {
    pub fn append(&self, text: &str) {
        let buffer = self.imp().text_view.buffer();
        buffer.insert(&mut buffer.end_iter(), text);
        self.scroll_down_idle();
    }

    pub fn appendln(&self, text: &str) {
        self.append(&format!("{}\n", text));
    }

    pub fn append_markup(&self, markup: &str) {
        let buffer = self.imp().text_view.buffer();
        buffer.insert_markup(&mut buffer.end_iter(), markup);
        self.scroll_down_idle();
    }

    pub fn appendln_colored(&self, text: &str, color: &str) {
        self.append_markup(&format!(
            "<span color=\"{}\">{}</span>\n",
            color,
            glib::markup_escape_text(text)
        ));
        self.scroll_down_idle();
    }

    pub fn appendln_command(&self, command: &str) {
        self.append_markup(&format!(
            "<span style=\"italic\">$ {}</span>\n",
            glib::markup_escape_text(command)
        ));
        self.scroll_down_idle();
    }

    pub fn clear(&self) {
        self.imp().text_view.buffer().set_text("");
    }

    fn scroll_down_idle(&self) {
        glib::idle_add_local_once(clone!(@weak self as obj => move || {
            obj.imp()
                .scrolled_window
                .emit_scroll_child(gtk::ScrollType::End, false);
        }));
    }
}
