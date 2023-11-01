use std::{fmt, sync::Arc};

use anyhow::Result;

pub struct Callbacks {
    send_char: Box<dyn Fn(&str)>,
    controlled_exit: Box<dyn Fn(i32, bool, bool)>,
}

impl Callbacks {
    pub fn new(
        send_char: impl Fn(&str) + 'static,
        controlled_exit: impl Fn(i32, bool, bool) + 'static,
    ) -> Self {
        Self {
            send_char: Box::new(send_char),
            controlled_exit: Box::new(controlled_exit),
        }
    }
}

impl elektron_ngspice::Callbacks for Callbacks {
    fn send_char(&mut self, s: &str) {
        (self.send_char)(s);
    }

    fn controlled_exit(&mut self, status: i32, unload: bool, quit: bool) {
        (self.controlled_exit)(status, unload, quit);
    }
}

pub struct NgSpice {
    inner: Arc<elektron_ngspice::NgSpice<'static, Callbacks>>,
}

impl fmt::Debug for NgSpice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NgSpice").finish()
    }
}

impl NgSpice {
    pub fn new(callbacks: Callbacks) -> Result<Self> {
        static mut CALLBACKS_INSTANCE: Option<Callbacks> = None;

        let inner = unsafe {
            assert!(
                CALLBACKS_INSTANCE.is_none(),
                "Multiple Ngspice instance is not supported"
            );

            CALLBACKS_INSTANCE.replace(callbacks);

            elektron_ngspice::NgSpice::new(CALLBACKS_INSTANCE.as_mut().unwrap())?
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
