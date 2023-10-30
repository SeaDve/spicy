use std::{fmt, sync::Arc};

use anyhow::Result;

static mut CALLBACK_INSTANCE: Callback = Callback(None);

#[allow(clippy::type_complexity)]
struct Callback(Option<Box<dyn Fn(&str)>>);

impl elektron_ngspice::Callbacks for Callback {
    fn send_char(&mut self, s: &str) {
        if let Some(ref cb) = self.0 {
            cb(s);
        }
    }
}

pub fn set_output(cb: impl Fn(&str) + 'static) {
    unsafe {
        CALLBACK_INSTANCE.0.replace(Box::new(cb));
    }
}

pub struct NgSpice {
    inner: Arc<elektron_ngspice::NgSpice<'static, Callback>>,
}

impl fmt::Debug for NgSpice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NgSpice").finish()
    }
}

impl NgSpice {
    pub fn new() -> Result<Self> {
        Ok(Self {
            inner: unsafe { elektron_ngspice::NgSpice::new(&mut CALLBACK_INSTANCE)? },
        })
    }

    pub fn circuit(&self, circuit: impl IntoIterator<Item = impl Into<String>>) -> Result<()> {
        self.inner
            .circuit(circuit.into_iter().map(|s| s.into()).collect::<Vec<_>>())?;
        Ok(())
    }
}
