use anyhow::{ensure, Result};
use gtk::{gio, glib, prelude::*, subclass::prelude::*};
use gtk_source::prelude::*;

mod imp {
    use std::marker::PhantomData;

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

        pub(super) source_file: gtk_source::File,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Circuit {
        const NAME: &'static str = "SpicyCircuit";
        type Type = super::Circuit;
        type ParentType = gtk_source::Buffer;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Circuit {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            if let Some(language) = gtk_source::LanguageManager::default().language("spice") {
                obj.set_language(Some(&language));
                obj.set_highlight_syntax(true);
            }
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

    pub async fn open(file: &gio::File) -> Result<Self> {
        let this: Self = glib::Object::builder().property("file", file).build();
        let imp = this.imp();

        let loader = gtk_source::FileLoader::new(&this, &imp.source_file);
        loader.load_future(glib::Priority::default()).0.await?;

        Ok(this)
    }

    pub async fn save(&self) -> Result<()> {
        let imp = self.imp();

        let saver = gtk_source::FileSaver::new(self, &imp.source_file);
        saver.save_future(glib::Priority::default()).0.await?;

        self.set_modified(false);

        Ok(())
    }

    pub async fn save_draft_as(&self, file: &gio::File) -> Result<()> {
        ensure!(self.file().is_none(), "Circuit must be a draft");

        let imp = self.imp();

        imp.source_file.set_location(Some(file));

        let saver = gtk_source::FileSaver::new(self, &imp.source_file);
        saver.save_future(glib::Priority::default()).0.await?;

        self.notify_title();

        self.set_modified(false);

        Ok(())
    }

    pub async fn save_as(&self, file: &gio::File) -> Result<()> {
        let source_file = gtk_source::File::builder().location(file).build();
        let saver = gtk_source::FileSaver::new(self, &source_file);
        saver.save_future(glib::Priority::default()).0.await?;

        Ok(())
    }

    fn parse_title(&self) -> String {
        let end = self.end_iter();
        let end_lookup = end
            .backward_search(".end", gtk::TextSearchFlags::CASE_INSENSITIVE, None)
            .map(|(start, _)| start)
            .unwrap_or(end);

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

                let mut text_end = copy_text_iter(&text_start);
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

                let mut text_start = copy_text_iter(&text_end);
                text_start.backward_line();

                text_start.visible_text(&text_end)
            }
        };

        ret.trim().to_lowercase().to_string()
    }
}

// FIXME upstream TextIter copy
fn copy_text_iter(text_iter: &gtk::TextIter) -> gtk::TextIter {
    use glib::translate::{FromGlibPtrFull, ToGlibPtr};

    unsafe {
        gtk::TextIter::from_glib_full(gtk::ffi::gtk_text_iter_copy(text_iter.to_glib_none().0))
    }
}
