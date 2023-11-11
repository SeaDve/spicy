use std::{fmt, sync::Arc};

use anyhow::Result;
use elektron_ngspice::VectorInfo;
use futures_channel::mpsc;
use futures_util::StreamExt;
use gtk::glib;

pub struct Callbacks {
    send_char_tx: mpsc::UnboundedSender<String>,
    controlled_exit_tx: mpsc::UnboundedSender<(i32, bool, bool)>,
}

impl Callbacks {
    pub fn new(
        send_char: impl Fn(String) + 'static,
        controlled_exit: impl Fn(i32, bool, bool) + 'static,
    ) -> Self {
        let ctx = glib::MainContext::default();

        let (send_char_tx, mut send_char_rx) = mpsc::unbounded();
        ctx.spawn_local(async move {
            while let Some(string) = send_char_rx.next().await {
                send_char(string);
            }
        });

        let (controlled_exit_tx, mut controlled_exit_rx) = mpsc::unbounded();
        ctx.spawn_local(async move {
            while let Some((status, unload, quit)) = controlled_exit_rx.next().await {
                controlled_exit(status, unload, quit);
            }
        });

        Self {
            send_char_tx,
            controlled_exit_tx,
        }
    }
}

impl elektron_ngspice::Callbacks for Callbacks {
    fn send_char(&mut self, string: &str) {
        self.send_char_tx
            .unbounded_send(string.to_string())
            .unwrap();
    }

    fn controlled_exit(&mut self, status: i32, unload: bool, quit: bool) {
        self.controlled_exit_tx
            .unbounded_send((status, unload, quit))
            .unwrap();
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

    pub fn current_plot(&self) -> Result<String> {
        Ok(self.inner.current_plot()?)
    }

    pub fn all_plots(&self) -> Result<Vec<String>> {
        Ok(self.inner.all_plots()?)
    }

    pub fn all_vecs(&self, plot_name: &str) -> Result<Vec<String>> {
        Ok(self.inner.all_vecs(plot_name)?)
    }

    pub fn vector_info(&self, vec_name: &str) -> Result<VectorInfo<'_>> {
        Ok(self.inner.vector_info(vec_name)?)
    }

    pub fn command(&self, command: &str) -> Result<()> {
        self.inner.command(command)?;
        Ok(())
    }
}
