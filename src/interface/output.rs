pub trait AppOutput: Send + Sync {
    fn info(&self, message: &str);
    fn error(&self, message: &str);
}

#[derive(Default, Clone, Copy)]
pub struct StdAppOutput;

impl AppOutput for StdAppOutput {
    fn info(&self, message: &str) {
        crate::infrastructure::logging::info(message);
    }

    fn error(&self, message: &str) {
        crate::infrastructure::logging::error(message);
    }
}

#[cfg(test)]
#[derive(Default, Clone)]
pub struct BufferAppOutput {
    inner: std::sync::Arc<std::sync::Mutex<BufferAppOutputState>>,
}

#[cfg(test)]
#[derive(Default)]
struct BufferAppOutputState {
    infos: Vec<String>,
    errors: Vec<String>,
}

#[cfg(test)]
impl BufferAppOutput {
    pub fn infos(&self) -> Vec<String> {
        self.inner.lock().expect("buffer output lock").infos.clone()
    }

    pub fn errors(&self) -> Vec<String> {
        self.inner
            .lock()
            .expect("buffer output lock")
            .errors
            .clone()
    }
}

#[cfg(test)]
impl AppOutput for BufferAppOutput {
    fn info(&self, message: &str) {
        self.inner
            .lock()
            .expect("buffer output lock")
            .infos
            .push(message.to_owned());
    }

    fn error(&self, message: &str) {
        self.inner
            .lock()
            .expect("buffer output lock")
            .errors
            .push(message.to_owned());
    }
}
