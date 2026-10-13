// Regression for RUSTSEC-2024-0429. Run optimized: the original immutable out-pointer
// may appear to work in debug mode and crash only once the compiler optimizes it.
#![cfg(target_os = "linux")]
use glib::prelude::*;

#[test]
fn every_string_iterator_operation_survives_optimization() {
    let v = ["zero", "one", "two", "three", "żółw", "five"].to_variant();
    for _ in 0..1_000 {
        let mut i = v.array_iter_str().unwrap();
        assert_eq!(i.next(), Some("zero"));
        assert_eq!(i.next_back(), Some("five"));
        assert_eq!(i.nth(1), Some("two"));
        assert_eq!(i.nth_back(0), Some("żółw"));
        assert_eq!(i.last(), Some("three"));
        assert_eq!(v.array_iter_str().unwrap().last(), Some("five"));
        assert_eq!(
            v.array_iter_str().unwrap().collect::<Vec<_>>(),
            ["zero", "one", "two", "three", "żółw", "five"]
        );
        let mut i = v.array_iter_str().unwrap();
        assert_eq!(i.nth(usize::MAX), None);
        assert_eq!(i.next_back(), None);
        let mut i = v.array_iter_str().unwrap();
        assert_eq!(i.nth_back(usize::MAX), None);
        assert_eq!(i.next(), None);
    }
}
