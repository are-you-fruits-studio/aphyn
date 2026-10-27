pub struct ExecOnDrop<F: FnOnce()> {
    f: Option<F>,
}

impl<F: FnOnce()> ExecOnDrop<F> {
    pub fn new(f: F) -> Self {
        Self { f: Some(f) }
    }
}
impl<F: FnOnce()> Drop for ExecOnDrop<F> {
    fn drop(&mut self) {
        self.f.take().unwrap()()
    }
}
