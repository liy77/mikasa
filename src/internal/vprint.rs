#[macro_export]
macro_rules! vprint {
    ($($arg:tt)*) => {
        if std::env::var_os("MIKASA_VERBOSE")
            .or_else(|| std::env::var_os("MIKASA_VERBOSE"))
            .map(|s| s.to_string_lossy() == "1")
            .unwrap_or(false)
        {
            println!($($arg)*);
        }
    };
}
