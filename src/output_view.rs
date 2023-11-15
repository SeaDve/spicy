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
        pub(super) buffer: TemplateChild<gtk::TextBuffer>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for OutputView {
        const NAME: &'static str = "SpicyOutputView";
        type Type = super::OutputView;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action("output-view.clear", None, |obj, _, _| {
                obj.clear();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for OutputView {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.buffer.connect_changed(clone!(@weak obj => move |_| {
                obj.update_clear_action();
            }));

            obj.update_clear_action();
        }

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
        let imp = self.imp();
        imp.buffer.insert(&mut imp.buffer.end_iter(), text);
        self.scroll_down_idle();
    }

    pub fn appendln(&self, text: &str) {
        self.append(&format!("{}\n", text));
    }

    pub fn append_markup(&self, markup: &str) {
        let imp = self.imp();
        imp.buffer.insert_markup(&mut imp.buffer.end_iter(), markup);
        self.scroll_down_idle();
    }

    pub fn appendln_colored(&self, text: &str, color: &str) {
        self.append_markup(&format!(
            "<span color=\"{}\">{}</span>\n",
            color,
            glib::markup_escape_text(text)
        ));
    }

    pub fn appendln_command(&self, command: &str) {
        self.append_markup(&format!(
            "<span style=\"italic\">$ {}</span>\n",
            glib::markup_escape_text(command)
        ));
    }

    pub fn clear(&self) {
        self.imp().buffer.set_text("");
    }

    fn scroll_down_idle(&self) {
        glib::idle_add_local_once(clone!(@weak self as obj => move || {
            obj.imp()
                .scrolled_window
                .emit_scroll_child(gtk::ScrollType::End, false);
        }));
    }

    fn update_clear_action(&self) {
        self.action_set_enabled("output-view.clear", self.imp().buffer.char_count() != 0);
    }
}
