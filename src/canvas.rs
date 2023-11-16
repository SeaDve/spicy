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
pub struct Component(Rc<RefCell<ComponentInner>>);

struct ComponentInner {
    bounds: Rect,
}

impl Component {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self(Rc::new(RefCell::new(ComponentInner {
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
        pub(super) components: RefCell<Vec<Component>>,

        pub(super) drag_component: RefCell<Option<Component>>,
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

            self.components
                .borrow_mut()
                .push(Component::new(40.0, 40.0, 30.0, 30.0));
            self.components
                .borrow_mut()
                .push(Component::new(10.0, 25.0, 15.0, 15.0));
        }
    }

    impl WidgetImpl for Canvas {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            for c in self.components.borrow().iter() {
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

        for component in imp.components.borrow().iter() {
            if component.contains(x as f32, y as f32) {
                imp.drag_component.replace(Some(component.clone()));
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

        if let Some(component) = imp.drag_component.borrow().as_ref() {
            let mut new_x = component.x() + dx;
            let mut new_y = component.y() + dy;

            let mut overshoot_x = 0.0;
            let mut overshoot_y = 0.0;

            let width = self.width() as f32;
            let height = self.height() as f32;

            // Keep the component within the canvas bounds
            if new_x < 0.0 {
                overshoot_x = -new_x;
                new_x = 0.0;
            } else if new_x + component.width() > width {
                overshoot_x = width - (new_x + component.width());
                new_x = width - component.width();
            }
            if new_y < 0.0 {
                overshoot_y = -new_y;
                new_y = 0.0;
            } else if new_y + component.height() > height {
                overshoot_y = height - (new_y + component.height());
                new_y = height - component.height();
            }

            dx += overshoot_x;
            dy += overshoot_y;

            component.move_to(new_x, new_y);
            self.queue_draw();
        };

        imp.drag_start
            .set(Some(Point::new(drag_start.x() + dx, drag_start.y() + dy)));
    }

    #[template_callback]
    fn drag_end(&self, _: f64, _: f64) {
        let imp = self.imp();

        imp.drag_component.replace(None);
    }
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}
