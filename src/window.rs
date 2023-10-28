use std::iter;

use adw::subclass::prelude::*;
use anyhow::Result;
use gtk::{
    gio,
    glib::{self, clone},
    prelude::*,
};

use crate::{
    application::Application,
    config::{APP_ID, PROFILE},
    ngspice::{self, NgSpice},
};

mod imp {
    use glib::once_cell::unsync::OnceCell;

    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Spicy/ui/window.ui")]
    pub struct Window {
        #[template_child]
        pub(super) toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub(super) play_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) circuit_text_view: TemplateChild<gtk::TextView>,
        #[template_child]
        pub(super) output_text_view: TemplateChild<gtk::TextView>,

        pub(super) ngspice: OnceCell<NgSpice>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "SpicyWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            if PROFILE == "Devel" {
                obj.add_css_class("devel");
            }

            self.play_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    if let Err(err) = obj.start_simulator() {
                        tracing::error!("Failed to start simulator: {:?}", err);
                        obj.imp()
                            .toast_overlay
                            .add_toast(adw::Toast::new("Failed to start simulator"));
                    }
                }));

            ngspice::set_output(clone!(@weak obj => move |string| {
                let output_buffer = obj.imp().output_text_view.buffer();
                let text = if string.starts_with("stdout") {
                    let string = string.trim_start_matches("stdout").trim();
                    if string.starts_with('*') {
                        format!(
                            "<span color=\"green\">{}</span>\n",
                            glib::markup_escape_text(string)
                        )
                    } else {
                        format!("{}\n", glib::markup_escape_text(string))
                    }
                } else if string.starts_with("stderr") {
                    format!(
                        "<span color=\"red\">{}</span>\n",
                        glib::markup_escape_text(string.trim_start_matches("stderr").trim())
                    )
                } else {
                    format!("{}\n", glib::markup_escape_text(string.trim()))
                };
                output_buffer.insert_markup(&mut output_buffer.end_iter(), &text);
            }));

            obj.load_window_size();
        }

        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {
        fn close_request(&self) -> glib::Propagation {
            if let Err(err) = self.obj().save_window_size() {
                tracing::warn!("Failed to save window state, {}", &err);
            }

            self.parent_close_request()
        }
    }

    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup, gtk::Root;
}

impl Window {
    pub fn new(app: &Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    fn start_simulator(&self) -> Result<()> {
        let imp = self.imp();

        imp.output_text_view.buffer().set_text("");

        let circuit_buffer = imp.circuit_text_view.buffer();
        let circuit = circuit_buffer.text(
            &circuit_buffer.start_iter(),
            &circuit_buffer.end_iter(),
            true,
        );
        let circuit = circuit.trim();

        let ngspice = imp.ngspice.get_or_try_init(NgSpice::new)?;
        if circuit.trim().is_empty() {
            ngspice.circuit(&[] as &[String])?;
        } else {
            ngspice.circuit(circuit.split('\n').chain(iter::once(".end")))?;
        }

        Ok(())
    }

    fn save_window_size(&self) -> Result<(), glib::BoolError> {
        let settings = gio::Settings::new(APP_ID);

        let (width, height) = self.default_size();
        settings.set_int("window-width", width)?;
        settings.set_int("window-height", height)?;

        settings.set_boolean("is-maximized", self.is_maximized())?;

        Ok(())
    }

    fn load_window_size(&self) {
        let settings = gio::Settings::new(APP_ID);
        let width = settings.int("window-width");
        let height = settings.int("window-height");
        let is_maximized = settings.boolean("is-maximized");

        self.set_default_size(width, height);

        if is_maximized {
            self.maximize();
        }
    }
}
