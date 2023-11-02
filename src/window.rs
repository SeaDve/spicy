use std::{
    error,
    fmt::{self, Write},
};

use adw::{prelude::*, subclass::prelude::*};
use anyhow::{bail, Context, Result};
use elektron_ngspice::ComplexSlice;
use gettextrs::gettext;
use gtk::{
    gio,
    glib::{self, clone},
};
use plotters_gtk4::SnapshotBackend;

use crate::{
    application::Application,
    circuit::Circuit,
    config::{APP_ID, PROFILE},
    i18n::gettext_f,
    ngspice::{Callbacks, NgSpice},
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
        pub(super) output_scrolled_window: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub(super) output_view: TemplateChild<gtk::TextView>,
        #[template_child]
        pub(super) command_entry: TemplateChild<gtk::Entry>,

        pub(super) circuit_binding_group: glib::BindingGroup,
        pub(super) circuit_signal_group: OnceCell<glib::SignalGroup>,

        pub(super) ngspice: OnceCell<NgSpice>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "SpicyWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            klass.install_action("win.load-circuit", None, |obj, _, _| {
                if let Err(err) = obj.load_circuit() {
                    tracing::error!("Failed to load circuit: {:?}", err);
                    obj.add_message_toast(&gettext("Failed to load circuit"));
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
                        gettext("Untitled Circuit")
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

            let circuit_signal_group = glib::SignalGroup::new::<Circuit>();
            circuit_signal_group.connect_notify_local(
                Some("busy-progress"),
                clone!(@weak obj => move |_, _| {
                    obj.update_save_actions();
                }),
            );
            self.circuit_signal_group.set(circuit_signal_group).unwrap();

            self.command_entry
                .connect_changed(clone!(@weak obj => move |_| {
                    obj.update_run_command_action();
                }));
            self.command_entry
                .connect_activate(clone!(@weak obj => move |_| {
                    WidgetExt::activate_action(&obj, "win.run-command", None).unwrap();
                }));

            let ngspice_cb = Callbacks::new(
                clone!(@weak obj => move |string| {
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
                }),
                clone!(@weak obj => move |_, _, _| {
                    obj.close();
                }),
            );
            match NgSpice::new(ngspice_cb) {
                Ok(ngspice) => self.ngspice.set(ngspice).unwrap(),
                Err(err) => {
                    tracing::error!("Failed to initialize ngspice: {:?}", err);
                    obj.add_message_toast(&gettext("Can't initialize Ngspice"));
                }
            }

            obj.load_window_size();

            obj.set_circuit(&Circuit::draft());
            obj.update_run_command_action();
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
        @implements gio::ActionMap, gio::ActionGroup, gtk::Root, gtk::Native;
}

impl Window {
    pub fn new(app: &Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    fn set_circuit(&self, circuit: &Circuit) {
        let imp = self.imp();

        imp.circuit_view.set_buffer(Some(circuit));

        imp.circuit_binding_group.set_source(Some(circuit));

        let circuit_signal_group = imp.circuit_signal_group.get().unwrap();
        circuit_signal_group.set_target(Some(circuit));

        self.update_save_actions();
    }

    fn circuit(&self) -> Circuit {
        self.imp().circuit_view.buffer().downcast().unwrap()
    }

    fn add_message_toast(&self, message: &str) {
        let toast = adw::Toast::new(message);
        self.imp().toast_overlay.add_toast(toast);
    }

    fn output_view_scroll_idle(&self, scroll_type: gtk::ScrollType, horizontal: bool) {
        glib::idle_add_local_once(clone!(@weak self as obj => move || {
            obj.imp()
                .output_scrolled_window
                .emit_scroll_child(scroll_type, horizontal);
        }));
    }

    fn output_view_append_command(&self, command: &str) {
        let output_buffer = self.imp().output_view.buffer();
        output_buffer.insert_markup(
            &mut output_buffer.end_iter(),
            &format!("<span style=\"italic\">$ {}</span>\n", command),
        );
    }

    fn output_view_show_plot(&self, plot_name: &str) -> Result<()> {
        let imp = self.imp();

        let ngspice = imp.ngspice.get().context("Ngspice was not initialized")?;
        let vec_names = ngspice.all_vecs(plot_name)?;

        let output_buffer = imp.output_view.buffer();
        let mut end_iter = output_buffer.end_iter();

        if vec_names.iter().any(|name| name == "time") {
            let mut time_vec = Vec::new();
            let mut other_vecs = Vec::new();
            for vec_name in vec_names {
                let vec_info = ngspice.vector_info(&vec_name)?;
                let real = match &vec_info.data {
                    ComplexSlice::Real(real) => real,
                    ComplexSlice::Complex(_) => bail!("Data contains complex"),
                };
                if vec_name == "time" {
                    time_vec.extend_from_slice(real);
                } else {
                    other_vecs.push((vec_name, real.to_vec()));
                }
            }

            let width = imp.output_scrolled_window.width();
            let height = imp.output_scrolled_window.height();
            let snapshot =
                current_plot_to_snapshot(plot_name, &time_vec, &other_vecs, width, height)?;
            let paintable = snapshot
                .to_paintable(None)
                .context("No paintable from snapshot")?;

            end_iter.forward_line();
            output_buffer.insert_paintable(&mut end_iter, &paintable);

            end_iter.forward_to_line_end();
            output_buffer.insert(&mut end_iter, "\n");
        } else {
            let mut text = String::new();
            for vec_name in vec_names {
                let vec_info = ngspice.vector_info(&vec_name)?;
                match vec_info.data {
                    ComplexSlice::Real(real) => {
                        writeln!(text, "{}: {}", vec_name, real[0]).unwrap();
                    }
                    ComplexSlice::Complex(complex) => {
                        writeln!(
                            text,
                            "{}: {} + {}i",
                            vec_name, complex[0].cx_real, complex[0].cx_imag
                        )
                        .unwrap();
                    }
                }
            }

            output_buffer.insert(&mut end_iter, &text);
        }

        Ok(())
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

    fn load_circuit(&self) -> Result<()> {
        let imp = self.imp();

        let circuit = self.circuit();
        let circuit_text = circuit.text(&circuit.start_iter(), &circuit.end_iter(), true);

        self.output_view_append_command("source");

        let ngspice = imp.ngspice.get().context("Ngspice was not initialized")?;
        ngspice.circuit(circuit_text.lines())?;

        self.output_view_scroll_idle(gtk::ScrollType::End, false);

        Ok(())
    }

    fn run_command(&self) -> Result<()> {
        let imp = self.imp();

        let command = imp.command_entry.text();
        imp.command_entry.set_text("");

        self.output_view_append_command(&command);

        let ngspice = imp.ngspice.get().context("Ngspice was not initialized")?;

        match command.split_whitespace().collect::<Vec<_>>().as_slice() {
            ["source"] => {
                let circuit = self.circuit();
                let circuit_text = circuit.text(&circuit.start_iter(), &circuit.end_iter(), true);
                ngspice.circuit(circuit_text.lines())?;
            }
            ["showplot"] => {
                let current_plot_name = ngspice.current_plot()?;
                self.output_view_show_plot(&current_plot_name)?;
            }
            ["showplot", plot_name] => {
                self.output_view_show_plot(plot_name)?;
            }
            ["clear", ..] => {
                imp.output_view.buffer().set_text("");
            }
            _ => {
                ngspice.command(&command)?;
            }
        }

        self.output_view_scroll_idle(gtk::ScrollType::End, false);

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

    fn update_run_command_action(&self) {
        let is_command_empty = self.imp().command_entry.text().is_empty();
        self.action_set_enabled("win.run-command", !is_command_empty);
    }

    fn update_save_actions(&self) {
        let is_circuit_busy = self.circuit().is_busy();
        self.action_set_enabled("win.save-circuit", !is_circuit_busy);
        self.action_set_enabled("win.save-circuit-as", !is_circuit_busy);
    }
}

fn current_plot_to_snapshot(
    plot_name: &str,
    time_vec: &[f64],
    other_vecs: &[(String, Vec<f64>)],
    width: i32,
    height: i32,
) -> Result<gtk::Snapshot> {
    use plotters::prelude::*;

    // TODO Write paintable backend supporting Adwaita dark theme and colors
    let snapshot = gtk::Snapshot::new();
    let root_area =
        SnapshotBackend::new(&snapshot, (width as u32, height as u32)).into_drawing_area();
    root_area.fill(&WHITE)?;

    let x_min = *time_vec
        .iter()
        .min_by(|a, b| a.total_cmp(b))
        .context("Empty time data")?;
    let x_max = *time_vec
        .iter()
        .max_by(|a, b| a.total_cmp(b))
        .context("Empty time data")?;

    let y_min = *other_vecs
        .iter()
        .flat_map(|(_, vec)| vec.iter())
        .min_by(|a, b| a.total_cmp(b))
        .context("Empty other data")?;
    let y_max = *other_vecs
        .iter()
        .flat_map(|(_, vec)| vec.iter())
        .max_by(|a, b| a.total_cmp(b))
        .context("Empty other data")?;

    let mut cc = ChartBuilder::on(&root_area)
        .margin_left(10)
        .margin_right(20)
        .margin_top(20)
        .margin_bottom(10)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .caption(plot_name, ("sans-serif", 20))
        .build_cartesian_2d(x_min..x_max, y_min..y_max)?;

    cc.configure_mesh()
        .x_desc("Time (ms)")
        .x_label_formatter(&|v| format!("{:.0}", v * 1e3))
        .y_label_formatter(&|v| format!("{:.1}", v))
        .draw()?;

    let colors = [RED, GREEN, BLUE, CYAN, MAGENTA, YELLOW];
    for ((name, vec), color) in other_vecs.iter().zip(colors.into_iter().cycle()) {
        let style = ShapeStyle {
            color: color.into(),
            filled: true,
            stroke_width: 1,
        };
        cc.draw_series(LineSeries::new(
            time_vec.iter().copied().zip(vec.iter().copied()),
            style,
        ))?
        .label(name)
        .legend(move |(x, y)| PathElement::new([(x, y), (x + 20, y)], style));
    }

    cc.configure_series_labels()
        .position(SeriesLabelPosition::UpperRight)
        .background_style(WHITE.mix(0.8))
        .border_style(BLACK)
        .draw()?;

    drop(cc);
    drop(root_area);

    Ok(snapshot)
}
