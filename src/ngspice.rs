use std::{
    cell::{OnceCell, RefCell},
    fmt,
    rc::Rc,
    sync::Arc,
};

use anyhow::Result;
use gtk::glib;

static mut OUTPUT_INSTANCE: Output = Output::new();

type Callback = Rc<RefCell<Option<Box<dyn Fn(String)>>>>;

struct Output {
    cb: OnceCell<Callback>,
    tx: OnceCell<glib::Sender<String>>,
}

impl Output {
    const fn new() -> Self {
        Self {
            cb: OnceCell::new(),
            tx: OnceCell::new(),
        }
    }

    fn set_cb(&mut self, func: impl Fn(String) + 'static) {
        self.cb().replace(Some(Box::new(func)));
    }

    fn cb(&self) -> &Callback {
        self.cb.get_or_init(|| Rc::new(RefCell::new(None)))
    }
}

impl elektron_ngspice::Callbacks for Output {
    fn send_char(&mut self, string: &str) {
        if let Some(ref mut tx) = self.tx.get_mut() {
            tx.send(string.to_string()).unwrap();
        } else {
            let (tx, rx) = glib::MainContext::channel(glib::Priority::default());
            self.tx.set(tx).unwrap();

            let cb = self.cb().clone();
            rx.attach(None, move |s| {
                if let Some(ref cb) = *cb.borrow() {
                    cb(s);
                }
                glib::ControlFlow::Continue
            });

            self.tx.get_mut().unwrap().send(string.to_string()).unwrap();
        }
    }
}

pub fn set_output(func: impl Fn(String) + 'static) {
    unsafe { OUTPUT_INSTANCE.set_cb(func) };
}

pub struct NgSpice {
    inner: Arc<elektron_ngspice::NgSpice<'static, Output>>,
}

impl fmt::Debug for NgSpice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NgSpice").finish()
    }
}

impl NgSpice {
    pub fn new() -> Result<Self> {
        Ok(Self {
            inner: unsafe { elektron_ngspice::NgSpice::new(&mut OUTPUT_INSTANCE)? },
        })
    }

    pub fn circuit(&self, circuit: impl IntoIterator<Item = impl Into<String>>) -> Result<()> {
        self.inner
            .circuit(circuit.into_iter().map(|s| s.into()).collect::<Vec<_>>())?;
        Ok(())
    }
}
