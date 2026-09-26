// Test helpers outside #[test] functions are not covered by the clippy.toml
// test allowances.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! The encoder is meant for small systems: after construction, encoding and
//! synthesizing a full transmission must not touch the allocator. This test
//! pins that guarantee with a counting global allocator.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use sstv::{Encoder, RgbPixel, Synthesizer, modes};

/// Wraps the system allocator, counting every allocation and reallocation
/// made by the current thread. The count is thread-local so that harness
/// threads (libtest bookkeeping, output capture) cannot pollute it.
struct CountingAllocator;

thread_local! {
    // const-initialized TLS: accessing it never allocates, so the allocator
    // can touch it without recursing.
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

/// Count an allocation, tolerating threads that allocate while their TLS is
/// already torn down (panicking inside the allocator would abort).
fn count_allocation() {
    let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        unsafe { System.alloc(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count_allocation();
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn encoding_does_not_allocate_after_construction() {
    let mode = modes::ROBOT_36;
    let (width, height) = mode.resolution();
    let image =
        (0..width * height).map(|i| RgbPixel::new(i as u8, (i >> 8) as u8, (i >> 16) as u8));

    let encoder = Encoder::new(mode, image).expect("construct encoder");
    let synthesizer = Synthesizer::new(encoder, 48_000);

    let before = ALLOCATIONS.with(Cell::get);
    // Construction allocates on this thread; a zero count means the
    // instrumentation itself is broken.
    assert!(before > 0, "the counting allocator is not counting");
    // Consume the entire transmission without allocating on our side. The
    // samples pass through `black_box` so the loop cannot be optimized away.
    let mut samples = 0usize;
    for sample in synthesizer {
        samples += 1;
        std::hint::black_box(sample);
    }
    let after = ALLOCATIONS.with(Cell::get);

    assert!(samples > 0, "expected a non-empty transmission");
    assert_eq!(
        after - before,
        0,
        "encoding allocated {} times after construction",
        after - before
    );
}
