use anyhow::Result;
use gtk::{
    gdk,
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};
use plotters::style::RGBAColor;
use plotters_gtk4::{Paintable, PaintableBackend};

use crate::{colors, plot_view_filter_row::PlotViewFilterRow};

const COLORS: [gdk::RGBA; 7] = [
    colors::BLUE_3,
    colors::GREEN_3,
    colors::YELLOW_3,
    colors::ORANGE_3,
    colors::RED_3,
    colors::PURPLE_3,
    colors::BROWN_3,
];

struct Vector {
    name: String,
    data: Vec<f64>,
    color: gdk::RGBA,
    is_visible: bool,
}

mod imp {
    use std::cell::RefCell;

    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Spicy/ui/plot_view.ui")]
    pub struct PlotView {
        #[template_child]
        pub(super) picture: TemplateChild<gtk::Picture>, // Unused
        #[template_child]
        pub(super) paintable: TemplateChild<Paintable>,
        #[template_child]
        pub(super) separator: TemplateChild<gtk::Separator>, // Unused
        #[template_child]
        pub(super) scrolled_window: TemplateChild<gtk::ScrolledWindow>, // Unused
        #[template_child]
        pub(super) filter_list_box: TemplateChild<gtk::ListBox>,

        pub(super) time_vector: RefCell<Vec<f64>>,
        pub(super) other_vectors: RefCell<Vec<Vector>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlotView {
        const NAME: &'static str = "SpicyPlotView";
        type Type = super::PlotView;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PlotView {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            self.filter_list_box
                .connect_row_activated(clone!(@weak obj => move |_, row| {
                    let filter_row = row.downcast_ref::<PlotViewFilterRow>().unwrap();
                    filter_row.handle_activation();
                }));
        }

        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for PlotView {}
}

glib::wrapper! {
    pub struct PlotView(ObjectSubclass<imp::PlotView>)
        @extends gtk::Widget;
}

impl PlotView {
    pub fn clear(&self) {
        let imp = self.imp();
        imp.time_vector.borrow_mut().clear();
        imp.other_vectors.borrow_mut().clear();
        imp.paintable.clear();
        self.update_filter_list_box();
    }

    pub fn set_vectors(
        &self,
        time_vector: Vec<f64>,
        other_vectors: Vec<(String, Vec<f64>)>,
    ) -> Result<()> {
        let imp = self.imp();

        imp.time_vector.replace(time_vector);

        imp.other_vectors.replace(
            other_vectors
                .into_iter()
                .zip(COLORS)
                .map(|((name, data), color)| Vector {
                    name,
                    data,
                    color,
                    is_visible: true,
                })
                .collect(),
        );

        self.update_filter_list_box();
        self.update_paintable()?;

        Ok(())
    }

    fn update_paintable(&self) -> Result<()> {
        use plotters::prelude::*;

        let imp = self.imp();

        // TODO Support Adwaita dark theme and colors
        let root_area = PaintableBackend::new(&imp.paintable).into_drawing_area();
        root_area.fill(&WHITE)?;

        let time_vector = imp.time_vector.borrow();
        let other_vectors = imp.other_vectors.borrow();
        let other_vectors_iter = other_vectors.iter().filter(|v| v.is_visible);

        let x_min = time_vector
            .iter()
            .min_by(|a, b| a.total_cmp(b))
            .copied()
            .unwrap_or(0.0);
        let x_max = time_vector
            .iter()
            .max_by(|a, b| a.total_cmp(b))
            .copied()
            .unwrap_or(0.0);

        let y_min = other_vectors_iter
            .clone()
            .flat_map(|vector| vector.data.iter())
            .min_by(|a, b| a.total_cmp(b))
            .copied()
            .unwrap_or(0.0);
        let y_max = other_vectors_iter
            .clone()
            .flat_map(|vector| vector.data.iter())
            .max_by(|a, b| a.total_cmp(b))
            .copied()
            .unwrap_or(0.0);

        let mut cc = ChartBuilder::on(&root_area)
            .margin_left(10)
            .margin_right(20)
            .margin_top(20)
            .margin_bottom(10)
            .x_label_area_size(40)
            .y_label_area_size(40)
            .build_cartesian_2d(x_min..x_max, y_min..y_max)?;

        cc.configure_mesh()
            .x_desc("Time (ms)")
            .x_label_formatter(&|v| format!("{:.0}", v * 1e3))
            .y_label_formatter(&|v| format!("{:.1}", v))
            .draw()?;

        for vector in other_vectors_iter.clone() {
            let style = ShapeStyle {
                color: to_plotters_color(vector.color),
                filled: true,
                stroke_width: 1,
            };
            cc.draw_series(LineSeries::new(
                time_vector.iter().copied().zip(vector.data.iter().copied()),
                style,
            ))?
            .label(&vector.name)
            .legend(move |(x, y)| PathElement::new([(x, y), (x + 20, y)], style));
        }

        root_area.present()?;

        Ok(())
    }

    fn update_filter_list_box(&self) {
        let imp = self.imp();

        imp.filter_list_box.remove_all();

        for vector in imp.other_vectors.borrow().iter() {
            let row = PlotViewFilterRow::new(&vector.name, vector.color);
            row.connect_is_active_notify(clone!(@weak self as obj => move |row| {
                obj.imp()
                    .other_vectors
                    .borrow_mut()
                    .iter_mut()
                    .find(|v| v.name == row.name())
                    .expect("vector must exist")
                    .is_visible = row.is_active();
                if let Err(err) = obj.update_paintable() {
                    tracing::error!("Failed to update paintable: {:?}", err);
                }
            }));
            imp.filter_list_box.append(&row);
        }
    }
}

fn to_plotters_color(rgba: gdk::RGBA) -> RGBAColor {
    RGBAColor(
        (rgba.red() * 255.0) as u8,
        (rgba.green() * 255.0) as u8,
        (rgba.blue() * 255.0) as u8,
        rgba.alpha() as f64,
    )
}
