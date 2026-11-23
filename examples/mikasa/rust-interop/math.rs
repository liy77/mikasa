// Plain Rust next to the `.mika`. Astra compiles this to wasm (auto-wraps the
// `pub fn` as `extern "C"`); `mikasa run` (transpile) also links it natively.
pub fn fib(n: i64) -> i64 {
    let (mut a, mut b) = (0i64, 1i64);
    for _ in 0..n {
        let t = a + b;
        a = b;
        b = t;
    }
    a
}
