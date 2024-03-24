use std::{cell::RefCell, rc::Rc};

use gtk::{
    gdk, glib,
    graphene::{Point, Rect},
    prelude::*,
    subclass::prelude::*,
};

const WIDTH: i32 = 800;
const HEIGHT: i32 = 600;

#[derive(Clone)]
pub struct Item(Rc<RefCell<ItemInner>>);

struct ItemInner {
    bounds: Rect,
}

impl Item {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self(Rc::new(RefCell::new(ItemInner {
            bounds: Rect::new(x, y, width, height),
        })))
    }

    pub fn x(&self) -> f32 {
        self.0.borrow().bounds.x()
    }

    pub fn y(&self) -> f32 {
        self.0.borrow().bounds.y()
    }

    pub fn move_to(&self, x: f32, y: f32) {
        self.0.borrow_mut().bounds = Rect::new(x, y, self.width(), self.height());
    }

    pub fn width(&self) -> f32 {
        self.0.borrow().bounds.width()
    }

    pub fn height(&self) -> f32 {
        self.0.borrow().bounds.height()
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        self.0.borrow().bounds.contains_point(&Point::new(x, y))
    }
}

mod imp {
    use std::cell::{Cell, RefCell};

    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Spicy/ui/canvas.ui")]
    pub struct Canvas {
        pub(super) items: RefCell<Vec<Item>>,

        pub(super) drag_item: RefCell<Option<Item>>,
        pub(super) drag_start: Cell<Option<Point>>,
        pub(super) pointer_position: Cell<Option<Point>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Canvas {
        const NAME: &'static str = "SpicyCanvas";
        type Type = super::Canvas;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Canvas {
        fn constructed(&self) {
            self.parent_constructed();

            self.items
                .borrow_mut()
                .push(Item::new(40.0, 40.0, 30.0, 30.0));
            self.items
                .borrow_mut()
                .push(Item::new(10.0, 25.0, 15.0, 15.0));
        }
    }

    impl WidgetImpl for Canvas {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            for c in self.items.borrow().iter() {
                snapshot.append_color(
                    &gdk::RGBA::BLACK,
                    &Rect::new(c.x(), c.y(), c.width(), c.height()),
                );
            }
        }

        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            match orientation {
                gtk::Orientation::Horizontal => (WIDTH, WIDTH, -1, -1),
                gtk::Orientation::Vertical => (HEIGHT, HEIGHT, -1, -1),
                _ => unreachable!(),
            }
        }
    }
}

glib::wrapper! {
    pub struct Canvas(ObjectSubclass<imp::Canvas>)
        @extends gtk::Widget;
}

impl Canvas {
    pub fn new() -> Self {
        glib::Object::new()
    }
}

#[gtk::template_callbacks]
impl Canvas {
    #[template_callback]
    fn enter(&self, x: f64, y: f64) {
        let imp = self.imp();

        imp.pointer_position
            .replace(Some(Point::new(x as f32, y as f32)));
    }

    #[template_callback]
    fn motion(&self, x: f64, y: f64) {
        let imp = self.imp();

        imp.pointer_position
            .replace(Some(Point::new(x as f32, y as f32)));
    }

    #[template_callback]
    fn leave(&self) {
        let imp = self.imp();

        imp.pointer_position.replace(None);
    }

    #[template_callback]
    fn drag_begin(&self, x: f64, y: f64) {
        let imp = self.imp();

        imp.drag_start.replace(Some(Point::new(x as f32, y as f32)));

        for item in imp.items.borrow().iter() {
            if item.contains(x as f32, y as f32) {
                imp.drag_item.replace(Some(item.clone()));
                break;
            }
        }
    }

    #[template_callback]
    fn drag_update(&self, _: f64, _: f64) {
        let imp = self.imp();

        let pointer_position = imp.pointer_position.get().unwrap();
        let drag_start = imp.drag_start.get().unwrap();
        let mut dx = pointer_position.x() - drag_start.x();
        let mut dy = pointer_position.y() - drag_start.y();

        if let Some(item) = imp.drag_item.borrow().as_ref() {
            let mut new_x = item.x() + dx;
            let mut new_y = item.y() + dy;

            let mut overshoot_x = 0.0;
            let mut overshoot_y = 0.0;

            let width = self.width() as f32;
            let height = self.height() as f32;

            // Keep the item within the canvas bounds
            if new_x < 0.0 {
                overshoot_x = -new_x;
                new_x = 0.0;
            } else if new_x + item.width() > width {
                overshoot_x = width - (new_x + item.width());
                new_x = width - item.width();
            }
            if new_y < 0.0 {
                overshoot_y = -new_y;
                new_y = 0.0;
            } else if new_y + item.height() > height {
                overshoot_y = height - (new_y + item.height());
                new_y = height - item.height();
            }

            dx += overshoot_x;
            dy += overshoot_y;

            item.move_to(new_x, new_y);
            self.queue_draw();
        };

        imp.drag_start
            .set(Some(Point::new(drag_start.x() + dx, drag_start.y() + dy)));
    }

    #[template_callback]
    fn drag_end(&self, _: f64, _: f64) {
        let imp = self.imp();

        imp.drag_item.replace(None);
    }
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}
