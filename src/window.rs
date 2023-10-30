use adw::subclass::prelude::*;
use anyhow::{Context, Result};
use gettextrs::gettext;
use gtk::{
    gio,
    glib::{self, clone},
    prelude::*,
};

use crate::{
    application::Application,
    circuit::Circuit,
    config::{APP_ID, PROFILE},
    ngspice::NgSpice,
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
        pub(super) circuit_modified_status: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) circuit_title_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) circuit_view: TemplateChild<gtk_source::View>,
        #[template_child]
        pub(super) output_view: TemplateChild<gtk::TextView>,

        pub(super) ngspice: OnceCell<NgSpice>,
        pub(super) circuit_binding_group: glib::BindingGroup,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "SpicyWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action_async("win.run-simulator", None, |obj, _, _| async move {
                if let Err(err) = obj.run_simulator() {
                    tracing::error!("Failed to run simulator: {:?}", err);
                    obj.add_message_toast(&gettext("Failed to run simulator"));
                }
            });

            klass.install_action_async("win.new-circuit", None, |obj, _, _| async move {
                obj.set_circuit(Circuit::draft());
            });

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

            klass.install_action_async("win.save-circuit-as", None, |obj, _, _| async move {
                if let Err(err) = obj.save_circuit_as().await {
                    if !err
                        .downcast_ref::<glib::Error>()
                        .is_some_and(|error| error.matches(gtk::DialogError::Dismissed))
                    {
                        tracing::error!("Failed to save circuit as: {:?}", err);
                        obj.add_message_toast(&gettext("Failed to save circuit as"));
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

            self.circuit_binding_group
                .bind("is-modified", &*self.circuit_modified_status, "visible")
                .sync_create()
                .build();
            self.circuit_binding_group
                .bind("title", &*self.circuit_title_label, "label")
                .transform_to(|_, value| {
                    let title = value.get::<String>().unwrap();

                    let transformed = if title.is_empty() {
                        gettext("New Circuit")
                    } else {
                        title
                    };

                    Some(transformed.into())
                })
                .sync_create()
                .build();

            let ngspice_ret = NgSpice::new(clone!(@weak obj => move |string| {
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
            match ngspice_ret {
                Ok(ngspice) => self.ngspice.set(ngspice).unwrap(),
                Err(err) => {
                    tracing::error!("Failed to initialize ngspice: {:?}", err);
                    obj.add_message_toast(&gettext("Can't initialize Ngspice"));
                }
            }

            obj.load_window_size();

            obj.set_circuit(Circuit::draft());
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

    fn set_circuit(&self, circuit: Circuit) {
        let imp = self.imp();

        imp.circuit_view.set_buffer(Some(&circuit));
        imp.circuit_binding_group.set_source(Some(&circuit));
    }

    fn circuit(&self) -> Circuit {
        self.imp().circuit_view.buffer().downcast().unwrap()
    }

    fn run_simulator(&self) -> Result<()> {
        let imp = self.imp();

        imp.output_view.buffer().set_text("");

        let circuit = self.circuit();
        let circuit_text = circuit.text(&circuit.start_iter(), &circuit.end_iter(), true);
        let circuit = circuit_text.trim();
        imp.ngspice
            .get()
            .context("Ngspice was not initialized")?
            .circuit(circuit.split('\n'))?;

        Ok(())
    }

    async fn open_circuit(&self) -> Result<()> {
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

        let circuit = Circuit::open(&file).await?;
        self.set_circuit(circuit);

        Ok(())
    }

    async fn save_circuit(&self) -> Result<()> {
        let circuit = self.circuit();

        if circuit.file().is_some() {
            circuit.save().await?;
        } else {
            let filter = gtk::FileFilter::new();
            filter.set_property("name", gettext("Plain Text Files"));
            filter.add_mime_type("text/plain");

            let filters = gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&filter);

            let dialog = gtk::FileDialog::builder()
                .title(gettext("Save Circuit"))
                .filters(&filters)
                .modal(true)
                .initial_name(format!("{}.cir", circuit.title()))
                .build();
            let file = dialog.save_future(Some(self)).await?;

            circuit.save_draft_as(&file).await?;
        }

        Ok(())
    }

    async fn save_circuit_as(&self) -> Result<()> {
        let circuit = self.circuit();

        let filter = gtk::FileFilter::new();
        filter.set_property("name", gettext("Plain Text Files"));
        filter.add_mime_type("text/plain");

        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);

        let dialog = gtk::FileDialog::builder()
            .title(gettext("Save Circuit As"))
            .filters(&filters)
            .modal(true)
            .initial_name(format!("{}.cir", circuit.title()))
            .build();
        let file = dialog.save_future(Some(self)).await?;

        circuit.save_as(&file).await?;

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
