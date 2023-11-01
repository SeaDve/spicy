use std::pin::Pin;

use anyhow::{ensure, Result};
use futures_util::{join, Future, Stream, StreamExt};
use gtk::{
    gio,
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};
use gtk_source::prelude::*;

mod imp {
    use std::{cell::Cell, marker::PhantomData};

    use gtk_source::subclass::prelude::BufferImpl;

    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::Circuit)]
    pub struct Circuit {
        #[property(get = Self::file, set = Self::set_file, construct_only)]
        pub(super) file: PhantomData<Option<gio::File>>,
        #[property(get = Self::title)]
        pub(super) title: PhantomData<String>,
        #[property(get = Self::is_modified)]
        pub(super) is_modified: PhantomData<bool>,
        #[property(get)]
        pub(super) busy_progress: Cell<f64>,

        pub(super) source_file: gtk_source::File,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Circuit {
        const NAME: &'static str = "SpicyCircuit";
        type Type = super::Circuit;
        type ParentType = gtk_source::Buffer;

        fn new() -> Self {
            Self {
                busy_progress: Cell::new(1.0),
                ..Default::default()
            }
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for Circuit {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            let language_manager = gtk_source::LanguageManager::default();
            if let Some(language) = language_manager.language("spice") {
                obj.set_language(Some(&language));
                obj.set_highlight_syntax(true);
            }

            let style_manager = adw::StyleManager::default();
            style_manager.connect_dark_notify(clone!(@weak obj => move |_| {
                obj.update_style_scheme();
            }));

            obj.update_style_scheme();
        }
    }

    impl TextBufferImpl for Circuit {
        fn modified_changed(&self) {
            self.parent_modified_changed();

            self.obj().notify_is_modified();
        }

        fn insert_text(&self, iter: &mut gtk::TextIter, new_text: &str) {
            self.parent_insert_text(iter, new_text);

            let obj = self.obj();

            if obj.file().is_none() {
                obj.notify_title();
            }
        }

        fn delete_range(&self, start: &mut gtk::TextIter, end: &mut gtk::TextIter) {
            self.parent_delete_range(start, end);

            let obj = self.obj();

            if obj.file().is_none() {
                obj.notify_title();
            }
        }
    }

    impl BufferImpl for Circuit {}

    impl Circuit {
        fn file(&self) -> Option<gio::File> {
            use glib::translate::{from_glib_none, ToGlibPtr};

            unsafe {
                // FIXME mark as nullable upstream
                from_glib_none(gtk_source::ffi::gtk_source_file_get_location(
                    self.source_file.to_glib_none().0,
                ))
            }
        }

        fn set_file(&self, file: Option<&gio::File>) {
            self.source_file.set_location(file);
        }

        fn title(&self) -> String {
            let obj = self.obj();

            if let Some(file) = obj.file() {
                file.path()
                    .unwrap()
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            } else {
                obj.parse_title()
            }
        }

        fn is_modified(&self) -> bool {
            gtk::TextBuffer::is_modified(self.obj().upcast_ref())
        }
    }
}

glib::wrapper! {
    pub struct Circuit(ObjectSubclass<imp::Circuit>)
        @extends gtk::TextBuffer, gtk_source::Buffer;
}

impl Circuit {
    pub fn draft() -> Self {
        glib::Object::new()
    }

    pub fn for_file(file: &gio::File) -> Self {
        glib::Object::builder().property("file", file).build()
    }

    pub async fn load(&self) -> Result<()> {
        ensure!(self.file().is_some(), "Circuit must not be a draft");

        let imp = self.imp();

        let loader = gtk_source::FileLoader::new(self, &imp.source_file);
        self.handle_file_io(loader.load_future(glib::Priority::default()))
            .await?;

        Ok(())
    }

    pub async fn save(&self) -> Result<()> {
        ensure!(self.file().is_some(), "Circuit must not be a draft");

        let imp = self.imp();

        let saver = gtk_source::FileSaver::new(self, &imp.source_file);
        self.handle_file_io(saver.save_future(glib::Priority::default()))
            .await?;

        self.set_modified(false);

        Ok(())
    }

    pub async fn save_draft_to(&self, file: &gio::File) -> Result<()> {
        ensure!(self.file().is_none(), "Circuit must be a draft");

        let imp = self.imp();

        imp.source_file.set_location(Some(file));

        let saver = gtk_source::FileSaver::new(self, &imp.source_file);
        self.handle_file_io(saver.save_future(glib::Priority::default()))
            .await?;

        self.notify_title();

        self.set_modified(false);

        Ok(())
    }

    pub async fn save_as(&self, file: &gio::File) -> Result<()> {
        let source_file = gtk_source::File::builder().location(file).build();
        let saver = gtk_source::FileSaver::new(self, &source_file);
        self.handle_file_io(saver.save_future(glib::Priority::default()))
            .await?;

        Ok(())
    }

    fn parse_title(&self) -> String {
        let end = self.end_iter();
        let end_lookup = end
            .backward_search(".end", gtk::TextSearchFlags::CASE_INSENSITIVE, None)
            .map_or(end, |(end_text_start, _)| end_text_start);

        let ret = match end_lookup.backward_search(
            ".title",
            gtk::TextSearchFlags::CASE_INSENSITIVE,
            None,
        ) {
            Some((_, mut text_start)) => {
                if !text_start.ends_word() {
                    text_start.forward_word_end();
                }
                if !text_start.ends_word() {
                    text_start.forward_word_end();
                    text_start.backward_word_start();
                }

                let mut text_end = text_start;
                text_end.forward_to_line_end();

                text_start.visible_text(&text_end)
            }
            _ => {
                let mut text_end = self.start_iter();
                while text_end.char().is_whitespace() && text_end < end_lookup {
                    text_end.forward_char();
                }
                if !text_end.ends_line() {
                    text_end.forward_to_line_end();
                }

                let mut text_start = text_end;
                text_start.backward_line();

                text_start.visible_text(&text_end)
            }
        };

        ret.trim().to_lowercase().to_string()
    }

    #[allow(clippy::type_complexity)]
    async fn handle_file_io(
        &self,
        (io_fut, mut progress_stream): (
            Pin<Box<dyn Future<Output = Result<(), glib::Error>>>>,
            Pin<Box<dyn Stream<Item = (i64, i64)>>>,
        ),
    ) -> Result<()> {
        let progress_fut = async {
            while let Some((current_n_bytes, total_n_bytes)) = progress_stream.next().await {
                let progress = if total_n_bytes == 0 || current_n_bytes > total_n_bytes {
                    1.0
                } else {
                    current_n_bytes as f64 / total_n_bytes as f64
                };
                self.imp().busy_progress.set(progress);
                self.notify_busy_progress();
            }
        };

        let (io_ret, _) = join!(io_fut, progress_fut);
        io_ret?;

        Ok(())
    }

    fn update_style_scheme(&self) {
        let style_manager = adw::StyleManager::default();
        let style_scheme_manager = gtk_source::StyleSchemeManager::default();

        let style_scheme = if style_manager.is_dark() {
            style_scheme_manager
                .scheme("Adwaita-dark")
                .or_else(|| style_scheme_manager.scheme("classic-dark"))
        } else {
            style_scheme_manager
                .scheme("Adwaita")
                .or_else(|| style_scheme_manager.scheme("classic"))
        };

        self.set_style_scheme(style_scheme.as_ref());
    }
}
