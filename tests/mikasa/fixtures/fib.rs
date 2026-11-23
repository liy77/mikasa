// Used by tests/mikasa/12_rust_interop.mika via Astra wasm.
pub fn fib(n: i64) -> i64 {
    let (mut a, mut b) = (0i64, 1i64);
    for _ in 0..n {
        let t = a + b;
        a = b;
        b = t;
    }
    a
}
