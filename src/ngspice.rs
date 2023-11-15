use std::{fmt, sync::Arc};

use anyhow::Result;
use elektron_ngspice::VectorInfo;
use futures_channel::mpsc;
use futures_util::StreamExt;
use gtk::{gio, glib};

pub struct Callbacks {
    send_char_tx: mpsc::UnboundedSender<String>,
    controlled_exit_tx: mpsc::UnboundedSender<(i32, bool, bool)>,
}

impl Callbacks {
    pub fn new(
        send_char: impl Fn(String) + 'static,
        controlled_exit: impl Fn(i32, bool, bool) + 'static,
    ) -> Self {
        let (send_char_tx, mut send_char_rx) = mpsc::unbounded();
        glib::spawn_future_local(async move {
            while let Some(string) = send_char_rx.next().await {
                send_char(string);
            }
        });

        let (controlled_exit_tx, mut controlled_exit_rx) = mpsc::unbounded();
        glib::spawn_future_local(async move {
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

    pub async fn circuit(
        &self,
        circuit: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<()> {
        let circuit = circuit.into_iter().map(|s| s.into()).collect::<Vec<_>>();
        self.unblock(|inner| inner.circuit(circuit)).await?;
        Ok(())
    }

    pub async fn current_plot_name(&self) -> Result<String> {
        let current_plot_name = self.unblock(|inner| inner.current_plot()).await?;
        Ok(current_plot_name)
    }

    pub async fn all_plot_names(&self) -> Result<Vec<String>> {
        let all_plot_names = self.unblock(|inner| inner.all_plots()).await?;
        Ok(all_plot_names)
    }

    pub async fn all_vector_names(&self, plot_name: impl Into<String>) -> Result<Vec<String>> {
        let plot_name = plot_name.into();
        let all_vector_names = self
            .unblock(move |inner| inner.all_vecs(&plot_name))
            .await?;
        Ok(all_vector_names)
    }

    pub async fn vector_info(&self, vector_name: impl Into<String>) -> Result<VectorInfo<'_>> {
        let vec_name = vector_name.into();
        let vector_info = self
            .unblock(move |inner| inner.vector_info(&vec_name))
            .await?;
        Ok(vector_info)
    }

    pub async fn command(&self, command: impl Into<String>) -> Result<()> {
        let command = command.into();
        self.unblock(move |inner| inner.command(&command)).await?;
        Ok(())
    }

    /// Spawns a task on the thread pool and returns a future that resolves to
    /// the return value of the task.
    #[must_use]
    async fn unblock<F, R>(&self, func: F) -> R
    where
        F: FnOnce(&elektron_ngspice::NgSpice<'static, Callbacks>) -> R + Send + 'static,
        R: Send + 'static,
    {
        let inner = self.inner.clone();
        gio::spawn_blocking(move || func(&inner))
            .await
            .expect("Failed to spawn blocking task")
    }
}
