use anyhow::Result;
use gtk::{gio, glib, prelude::*, subclass::prelude::*};

use crate::{ngspice::NgSpice, plot::Plot};

mod imp {
    use std::cell::RefCell;

    use super::*;

    #[derive(Default)]
    pub struct Plots {
        pub(super) inner: RefCell<Vec<Plot>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Plots {
        const NAME: &'static str = "SpicyPlots";
        type Type = super::Plots;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for Plots {}

    impl ListModelImpl for Plots {
        fn item_type(&self) -> glib::Type {
            Plot::static_type()
        }

        fn n_items(&self) -> u32 {
            self.inner.borrow().len() as u32
        }

        fn item(&self, position: u32) -> Option<glib::Object> {
            self.inner
                .borrow()
                .get(position as usize)
                .map(|plot| plot.clone().upcast())
        }
    }
}

glib::wrapper! {
    pub struct Plots(ObjectSubclass<imp::Plots>)
        @implements gio::ListModel;
}

impl Plots {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub async fn update(&self, ngspice: &NgSpice) -> Result<()> {
        let imp = self.imp();

        let prev_len = imp.inner.borrow().len() as u32;

        imp.inner.borrow_mut().clear();

        let current_plot_name = ngspice.current_plot_name().await?;
        let new_plots = ngspice
            .all_plot_names()
            .await?
            .into_iter()
            .map(|plot_name| Plot::new(&plot_name, plot_name == current_plot_name));
        imp.inner.borrow_mut().extend(new_plots);

        debug_assert_eq!(
            imp.inner
                .borrow()
                .iter()
                .filter(|plot| plot.is_current())
                .count(),
            1
        );

        let new_len = imp.inner.borrow().len() as u32;

        self.items_changed(0, prev_len, new_len);

        Ok(())
    }
}

impl Default for Plots {
    fn default() -> Self {
        Self::new()
    }
}
