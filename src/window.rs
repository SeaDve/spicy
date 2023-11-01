use std::{error, fmt};

use adw::{prelude::*, subclass::prelude::*};
use anyhow::{Context, Result};
use gettextrs::gettext;
use gtk::{
    gio,
    glib::{self, clone},
};

use crate::{
    application::Application,
    circuit::Circuit,
    config::{APP_ID, PROFILE},
    i18n::gettext_f,
    ngspice::NgSpice,
};

/// Indicates that a task was cancelled.
#[derive(Debug)]
struct Cancelled;

impl fmt::Display for Cancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Task cancelled")
    }
}

impl error::Error for Cancelled {}

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
        pub(super) progress_bar: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub(super) circuit_view: TemplateChild<gtk_source::View>,
        #[template_child]
        pub(super) command_entry: TemplateChild<gtk::Entry>,
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

            klass.install_action("win.run-simulator", None, |obj, _, _| {
                if let Err(err) = obj.run_simulator() {
                    tracing::error!("Failed to run simulator: {:?}", err);
                    obj.add_message_toast(&gettext("Failed to run simulator"));
                }
            });

            klass.install_action("win.run-command", None, |obj, _, _| {
                if let Err(err) = obj.run_command() {
                    tracing::error!("Failed to run command: {:?}", err);
                    obj.add_message_toast(&gettext("Failed to run command"));
                }
            });

            klass.install_action_async("win.new-circuit", None, |obj, _, _| async move {
                if obj.handle_unsaved_changes(&obj.circuit()).await.is_err() {
                    return;
                }

                obj.set_circuit(&Circuit::draft());
            });

            klass.install_action_async("win.open-circuit", None, |obj, _, _| async move {
                if obj.handle_unsaved_changes(&obj.circuit()).await.is_err() {
                    return;
                }

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
                if let Err(err) = obj.save_circuit(&obj.circuit()).await {
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
                if let Err(err) = obj.save_circuit_as(&obj.circuit()).await {
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
                    let label = if title.is_empty() {
                        gettext("New Circuit")
                    } else {
                        title
                    };
                    Some(label.into())
                })
                .sync_create()
                .build();
            self.circuit_binding_group
                .bind("busy-progress", &*self.progress_bar, "fraction")
                .sync_create()
                .build();
            self.circuit_binding_group
                .bind("busy-progress", &*self.progress_bar, "visible")
                .transform_to(|_, value| {
                    let busy_progress = value.get::<f64>().unwrap();
                    let visible = busy_progress != 1.0;
                    Some(visible.into())
                })
                .sync_create()
                .build();

            self.command_entry
                .connect_changed(clone!(@weak obj => move |_| {
                    obj.update_run_command_action_state();
                }));
            self.command_entry
                .connect_activate(clone!(@weak obj => move |_| {
                    WidgetExt::activate_action(&obj, "win.run-command", None).unwrap();
                }));

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

            obj.set_circuit(&Circuit::draft());
            obj.update_run_command_action_state();
        }

        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {
        fn close_request(&self) -> glib::Propagation {
            let obj = self.obj();

            if let Err(err) = obj.save_window_size() {
                tracing::warn!("Failed to save window state, {}", &err);
            }

            let curr_circuit = obj.circuit();
            if curr_circuit.is_modified() {
                let ctx = glib::MainContext::default();
                ctx.spawn_local(clone!(@weak obj => async move {
                    if obj.handle_unsaved_changes(&curr_circuit).await.is_err() {
                        return;
                    }
                    obj.destroy();
                }));
                return glib::Propagation::Stop;
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

    fn set_circuit(&self, circuit: &Circuit) {
        let imp = self.imp();

        imp.circuit_view.set_buffer(Some(circuit));
        imp.circuit_binding_group.set_source(Some(circuit));
    }

    fn circuit(&self) -> Circuit {
        self.imp().circuit_view.buffer().downcast().unwrap()
    }

    fn add_message_toast(&self, message: &str) {
        let toast = adw::Toast::new(message);
        self.imp().toast_overlay.add_toast(toast);
    }

    /// Returns `Ok` if unsaved changes are handled and can proceed, `Err` if
    /// the next operation should be aborted.
    async fn handle_unsaved_changes(&self, circuit: &Circuit) -> Result<()> {
        if !circuit.is_modified() {
            return Ok(());
        }

        match self.present_save_changes_dialog(circuit).await {
            Ok(_) => Ok(()),
            Err(err) => {
                if !err.is::<Cancelled>()
                    && !err
                        .downcast_ref::<glib::Error>()
                        .is_some_and(|error| error.matches(gtk::DialogError::Dismissed))
                {
                    tracing::error!("Failed to save changes to circuit: {:?}", err);
                    self.add_message_toast(&gettext("Failed to save changes to circuit"));
                }
                Err(err)
            }
        }
    }

    /// Returns `Ok` if unsaved changes are handled and can proceed, `Err` if
    /// the next operation should be aborted.
    async fn present_save_changes_dialog(&self, circuit: &Circuit) -> Result<()> {
        const CANCEL_RESPONSE_ID: &str = "cancel";
        const DISCARD_RESPONSE_ID: &str = "discard";
        const SAVE_RESPONSE_ID: &str = "save";

        let file_name = circuit
            .file()
            .and_then(|file| {
                file.path()
                    .unwrap()
                    .file_name()
                    .map(|file_name| file_name.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| gettext("Untitled Circuit"));
        let dialog = adw::MessageDialog::builder()
            .modal(true)
            .transient_for(self)
            .heading(gettext("Save Changes?"))
            .body(gettext_f(
                // Translators: Do NOT translate the contents between '{' and '}', this is a variable name.
                "“{file_name}” contains unsaved changes. Changes which are not saved will be permanently lost.",
                &[("file_name", &file_name)],
            ))
            .close_response(CANCEL_RESPONSE_ID)
            .default_response(SAVE_RESPONSE_ID)
            .build();

        dialog.add_response(CANCEL_RESPONSE_ID, &gettext("Cancel"));

        dialog.add_response(DISCARD_RESPONSE_ID, &gettext("Discard"));
        dialog.set_response_appearance(DISCARD_RESPONSE_ID, adw::ResponseAppearance::Destructive);

        let save_response_text = if circuit.file().is_some() {
            gettext("Save")
        } else {
            gettext("Save As…")
        };
        dialog.add_response(SAVE_RESPONSE_ID, &save_response_text);
        dialog.set_response_appearance(SAVE_RESPONSE_ID, adw::ResponseAppearance::Suggested);

        match dialog.choose_future().await.as_str() {
            CANCEL_RESPONSE_ID => Err(Cancelled.into()),
            DISCARD_RESPONSE_ID => Ok(()),
            SAVE_RESPONSE_ID => self.save_circuit(circuit).await,
            _ => unreachable!(),
        }
    }

    fn run_simulator(&self) -> Result<()> {
        let imp = self.imp();

        imp.output_view.buffer().set_text("");

        let circuit = self.circuit();
        let circuit_text = circuit.text(&circuit.start_iter(), &circuit.end_iter(), true);

        let ngspice = imp.ngspice.get().context("Ngspice was not initialized")?;
        ngspice.circuit(circuit_text.lines())?;

        Ok(())
    }

    fn run_command(&self) -> Result<()> {
        let imp = self.imp();

        let command = imp.command_entry.text();
        imp.command_entry.set_text("");

        let output_buffer = imp.output_view.buffer();
        output_buffer.insert_markup(
            &mut output_buffer.end_iter(),
            &format!("<span style=\"italic\">$ {}</span>\n", command),
        );

        let ngspice = imp.ngspice.get().context("Ngspice was not initialized")?;
        ngspice.command(&command)?;

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

        let circuit = Circuit::for_file(&file);
        let prev_circuit = self.circuit();
        self.set_circuit(&circuit);

        if let Err(err) = circuit.load().await {
            self.set_circuit(&prev_circuit);
            return Err(err);
        }

        Ok(())
    }

    async fn save_circuit(&self, circuit: &Circuit) -> Result<()> {
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

            circuit.save_draft_to(&file).await?;
        }

        Ok(())
    }

    async fn save_circuit_as(&self, circuit: &Circuit) -> Result<()> {
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

    fn update_run_command_action_state(&self) {
        let is_command_empty = self.imp().command_entry.text().is_empty();
        self.action_set_enabled("win.run-command", !is_command_empty);
    }
}
