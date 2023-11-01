use std::{fmt, sync::Arc};

use anyhow::Result;

struct Callback(Box<dyn Fn(&str)>);

impl elektron_ngspice::Callbacks for Callback {
    fn send_char(&mut self, s: &str) {
        (self.0)(s);
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
    pub fn new(cb: impl Fn(&str) + 'static) -> Result<Self> {
        static mut CALLBACK_INSTANCE: Option<Callback> = None;

        let inner = unsafe {
            assert!(
                CALLBACK_INSTANCE.is_none(),
                "Multiple Ngspice instance is not supported"
            );

            CALLBACK_INSTANCE.replace(Callback(Box::new(cb)));

            elektron_ngspice::NgSpice::new(CALLBACK_INSTANCE.as_mut().unwrap())?
        };

        Ok(Self { inner })
    }

    pub fn circuit(&self, circuit: impl IntoIterator<Item = impl Into<String>>) -> Result<()> {
        self.inner
            .circuit(circuit.into_iter().map(|s| s.into()).collect::<Vec<_>>())?;
        Ok(())
    }

    pub fn command(&self, command: &str) -> Result<()> {
        self.inner.command(command)?;
        Ok(())
    }
}
