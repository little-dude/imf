// create a new buffer from an ascii string
macro_rules! b {
    ($e:expr) => (&Buffer::new($e.as_ref()));
}

// the ok! macro makes sure parsing succeeds and the expect number of bytes is read
macro_rules! ok {
    ($function:ident, $bytes:expr, $expected_len:expr) => (assert_eq!($function(b!($bytes)).unwrap(), $expected_len));
}

// the eof! macro makes sure parsing fails with ErrorKind::Eof
macro_rules! eof {
    ($function:ident, $bytes:expr) => (assert!($function(b!($bytes)).unwrap_err().is_eof()));
}

// the tok! macro makes sure parsing fails with ErrorKind::Token
macro_rules! tok {
    ($function:ident, $bytes:expr, $token:expr, $byte:expr, $position:expr) => (
        let e = $function(b!($bytes)).unwrap_err();
        if let ErrorKind::Token { token, byte, position } = *e.kind() {
            assert_eq!(token, $token);
            assert_eq!(byte, $byte);
            assert_eq!(position, $position);
        }
        );
    ($function:ident, $bytes:expr) => (
        let e = $function(b!($bytes)).unwrap_err();
        assert!(e.is_token());
        )
}
