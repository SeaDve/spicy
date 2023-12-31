#![warn(
    rust_2018_idioms,
    clippy::items_after_statements,
    clippy::needless_pass_by_value,
    clippy::semicolon_if_nothing_returned,
    clippy::match_wildcard_for_single_variants,
    clippy::inefficient_to_string,
    clippy::map_unwrap_or,
    clippy::implicit_clone,
    clippy::struct_excessive_bools,
    clippy::trivially_copy_pass_by_ref,
    clippy::unreadable_literal,
    clippy::if_not_else,
    clippy::doc_markdown,
    clippy::unused_async,
    clippy::default_trait_access,
    clippy::unnecessary_wraps,
    clippy::unused_self,
    clippy::dbg_macro,
    clippy::todo,
    clippy::map_unwrap_or,
    clippy::or_fun_call,
    clippy::print_stdout
)]

mod application;
mod circuit;
mod color_widget;
mod colors;
mod config;
mod i18n;
mod ngspice;
mod output_view;
mod plot;
mod plot_view;
mod plot_view_filter_row;
mod plots;
mod plots_dropdown;
mod window;

use gettextrs::{gettext, LocaleCategory};
use gtk::{gio, glib};

use self::application::Application;
use self::config::{GETTEXT_PACKAGE, LOCALEDIR, RESOURCES_FILE};

fn main() -> glib::ExitCode {
    tracing_subscriber::fmt::init();

    gettextrs::setlocale(LocaleCategory::LcAll, "");
    gettextrs::bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR).expect("Unable to bind the text domain");
    gettextrs::textdomain(GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    glib::set_application_name(&gettext("Spicy"));

    let res = gio::Resource::load(RESOURCES_FILE).expect("Could not load gresource file");
    gio::resources_register(&res);

    let app = Application::default();
    app.run()
}
