pub struct Buffer<'buf> {
    inner: &'buf [u8],
    position: usize,
}

impl<'buf> Clone for Buffer<'buf> {
    fn clone(&self) -> Self {
        Buffer {
            inner: self.inner,
            position: self.position,
        }
    }
}

impl<'buf> Buffer<'buf> {
    pub fn new(buf: &'buf [u8]) -> Self {
        Buffer {
            inner: buf,
            position: 0,
        }
    }

    pub fn with_offset(buf: &'buf [u8], offset: usize) -> Self {
        Buffer {
            inner: buf,
            position: offset,
        }
    }

    pub fn remaining(&self) -> &[u8] {
        &self.inner[self.position..]
    }

    pub fn set_position(&mut self, position: usize) {
        self.position = position;
    }

    pub fn incr_position(&mut self, offset: usize) {
        self.position += offset;
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn into_inner(self) -> &'buf[u8] {
        self.inner
    }
}
