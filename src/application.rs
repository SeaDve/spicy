use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib, prelude::*};

use crate::{
    config::{APP_ID, PKGDATADIR, PROFILE, VERSION},
    window::Window,
};

mod imp {
    use super::*;
    use glib::WeakRef;
    use std::cell::OnceCell;

    #[derive(Debug, Default)]
    pub struct Application {
        pub(super) window: OnceCell<WeakRef<Window>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Application {
        const NAME: &'static str = "SpicyApplication";
        type Type = super::Application;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for Application {}

    impl ApplicationImpl for Application {
        fn activate(&self) {
            self.parent_activate();

            let app = self.obj();

            if let Some(window) = self.window.get() {
                let window = window.upgrade().unwrap();
                window.present();
                return;
            }

            let window = Window::new(&app);
            self.window
                .set(window.downgrade())
                .expect("Window already set.");

            window.present();
        }

        fn startup(&self) {
            self.parent_startup();
            let app = self.obj();

            gtk::Window::set_default_icon_name(APP_ID);

            app.setup_gactions();
            app.setup_accels();
        }
    }

    impl GtkApplicationImpl for Application {}
    impl AdwApplicationImpl for Application {}
}

glib::wrapper! {
    pub struct Application(ObjectSubclass<imp::Application>)
        @extends gio::Application, gtk::Application,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl Application {
    fn window(&self) -> Window {
        self.imp().window.get().unwrap().upgrade().unwrap()
    }

    fn setup_gactions(&self) {
        let action_quit = gio::ActionEntry::builder("quit")
            .activate(move |app: &Self, _, _| {
                // This is needed to trigger the delete event and saving the window state
                app.window().close();
                app.quit();
            })
            .build();

        let action_about = gio::ActionEntry::builder("about")
            .activate(|app: &Self, _, _| {
                app.show_about_dialog();
            })
            .build();
        self.add_action_entries([action_quit, action_about]);
    }

    fn setup_accels(&self) {
        self.set_accels_for_action("app.quit", &["<Control>q"]);
        self.set_accels_for_action("window.close", &["<Control>w"]);

        self.set_accels_for_action("win.run-simulator", &["F5"]);
        self.set_accels_for_action("win.new-circuit", &["<Control>n"]);
        self.set_accels_for_action("win.open-circuit", &["<Control>o"]);
        self.set_accels_for_action("win.save-circuit", &["<Control>s"]);
        self.set_accels_for_action("win.save-circuit-as", &["<Control><Shift>s"]);
    }

    fn show_about_dialog(&self) {
        let win = adw::AboutWindow::builder()
            .modal(true)
            .transient_for(&self.window())
            .application_icon(APP_ID)
            .application_name(gettext("Spicy"))
            .developer_name(gettext("Dave Patrick Caberto"))
            .version(VERSION)
            .copyright(gettext("© 2023 Dave Patrick Caberto"))
            .license_type(gtk::License::Gpl30)
            // Translators: Replace "translator-credits" with your names. Put a comma between.
            .translator_credits(gettext("translator-credits"))
            .issue_url("https://github.com/SeaDve/spicy/issues")
            .support_url("https://github.com/SeaDve/spicy/discussions")
            .build();

        win.present();
    }

    pub fn run(&self) -> glib::ExitCode {
        tracing::info!("Spicy ({})", APP_ID);
        tracing::info!("Version: {} ({})", VERSION, PROFILE);
        tracing::info!("Datadir: {}", PKGDATADIR);

        ApplicationExtManual::run(self)
    }
}

impl Default for Application {
    fn default() -> Self {
        glib::Object::builder()
            .property("application-id", APP_ID)
            .property("resource-base-path", "/io/github/seadve/Spicy/")
            .build()
    }
}
