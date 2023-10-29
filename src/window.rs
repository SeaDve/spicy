use adw::subclass::prelude::*;
use anyhow::{Context, Result};
use gettextrs::gettext;
use gtk::{
    gio,
    glib::{self, clone},
    prelude::*,
};
use gtk_source::prelude::*;

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
        pub(super) run_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) circuit_view: TemplateChild<gtk_source::View>,
        #[template_child]
        pub(super) output_view: TemplateChild<gtk::TextView>,

        pub(super) ngspice: OnceCell<NgSpice>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "SpicyWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action_async("win.open-circuit", None, |obj, _, _| async move {
                if let Err(err) = obj.open_circuit().await {
                    if !err
                        .downcast_ref::<glib::Error>()
                        .is_some_and(|error| error.matches(gtk::DialogError::Dismissed))
                    {
                        tracing::error!("Failed to open circuit: {:?}", err);
                        obj.add_message_toast(&gettext("Failed to open circuit"));
                    }
                }
            });

            klass.install_action_async("win.save-circuit", None, |obj, _, _| async move {
                if let Err(err) = obj.save_circuit().await {
                    if !err
                        .downcast_ref::<glib::Error>()
                        .is_some_and(|error| error.matches(gtk::DialogError::Dismissed))
                    {
                        tracing::error!("Failed to save circuit: {:?}", err);
                        obj.add_message_toast(&gettext("Failed to save circuit"));
                    }
                }
            });
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

            self.run_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    if let Err(err) = obj.start_simulator() {
                        tracing::error!("Failed to start simulator: {:?}", err);
                        obj.add_message_toast(&gettext("Failed to start simulator"));
                    }
                }));

            if let Some(language) = gtk_source::LanguageManager::default().language("spice") {
                let circuit_buffer = self
                    .circuit_view
                    .buffer()
                    .downcast::<gtk_source::Buffer>()
                    .unwrap();
                circuit_buffer.set_language(Some(&language));
                circuit_buffer.set_highlight_syntax(true);
            }

            ngspice::set_output(clone!(@weak obj => move |string| {
                let output_buffer = obj.imp().output_view.buffer();
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

            match NgSpice::new() {
                Ok(ngspice) => self.ngspice.set(ngspice).unwrap(),
                Err(err) => {
                    tracing::error!("Failed to initialize ngspice: {:?}", err);
                    obj.add_message_toast(&gettext("Can't initialize Ngspice"));
                }
            }

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

    fn add_message_toast(&self, message: &str) {
        let toast = adw::Toast::new(message);
        self.imp().toast_overlay.add_toast(toast);
    }

    fn start_simulator(&self) -> Result<()> {
        let imp = self.imp();

        imp.output_view.buffer().set_text("");

        let circuit_buffer = imp.circuit_view.buffer();
        let circuit = circuit_buffer.text(
            &circuit_buffer.start_iter(),
            &circuit_buffer.end_iter(),
            true,
        );
        let circuit = circuit.trim();
        imp.ngspice
            .get()
            .context("Ngspice was not initialized")?
            .circuit(circuit.split('\n'))?;

        Ok(())
    }

    async fn open_circuit(&self) -> Result<()> {
        let imp = self.imp();

        let filter = gtk::FileFilter::new();
        filter.set_property("name", gettext("Plain Text Files"));
        filter.add_mime_type("text/plain");

        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);

        let dialog = gtk::FileDialog::builder()
            .title(gettext("Open Circuit"))
            .filters(&filters)
            .modal(true)
            .build();

        let file = dialog.open_future(Some(self)).await?;
        let source_file = gtk_source::File::builder().location(&file).build();

        let loader = gtk_source::FileLoader::new(
            &imp.circuit_view
                .buffer()
                .downcast::<gtk_source::Buffer>()
                .unwrap(),
            &source_file,
        );
        loader.load_future(glib::Priority::default()).0.await?;

        Ok(())
    }

    async fn save_circuit(&self) -> Result<()> {
        let imp = self.imp();

        let filter = gtk::FileFilter::new();
        filter.set_property("name", gettext("Plain Text Files"));
        filter.add_mime_type("text/plain");

        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);

        let dialog = gtk::FileDialog::builder()
            .title(gettext("Save Circuit"))
            .filters(&filters)
            .modal(true)
            .initial_name(".cir")
            .build();

        let file = dialog.save_future(Some(self)).await?;
        let source_file = gtk_source::File::builder().location(&file).build();

        let saver = gtk_source::FileSaver::new(
            &imp.circuit_view
                .buffer()
                .downcast::<gtk_source::Buffer>()
                .unwrap(),
            &source_file,
        );
        saver.save_future(glib::Priority::default()).0.await?;

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
