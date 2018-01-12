use errors::{Error, ErrorKind};

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

    /// Read the buffer until the given byte is read
    pub fn read_until(&mut self, c: u8) -> Result<&[u8], Error> {
        let start = self.position;
        while self.position < self.inner.len() {
            if self.inner[self.position] == c {
                return Ok(&self.inner[start..self.position]);
            }
            self.position += 1;
        }
        Err(ErrorKind::Eof.into())
    }

    /// Read the buffer byte by byte, passing each byte to the provided function, until it returns
    /// `false`.
    pub fn read_while<F>(&mut self, f: F) -> Result<&[u8], Error>
    where
        F: Fn(u8) -> bool,
    {
        let start = self.position;
        while self.position < self.inner.len() {
            if !f(self.inner[self.position]) {
                return Ok(&self.inner[start..self.position]);
            }
            self.position += 1;
        }
        Err(ErrorKind::Eof.into())
    }

    pub fn read(&mut self) -> Result<u8, Error> {
        if self.position < self.inner.len() {
            let c = self.inner[self.position];
            self.position += 1;
            Ok(c)
        } else {
            Err(ErrorKind::Eof.into())
        }
    }

    pub fn read_n(&mut self, n: usize) -> Result<&[u8], Error> {
        if self.position + n < self.inner.len() {
            let start = self.position;
            self.position += n;
            Ok(&self.inner[start..self.position])
        } else {
            Err(ErrorKind::Eof.into())
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
