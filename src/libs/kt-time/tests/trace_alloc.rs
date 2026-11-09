//! Test t2143 (part): recording costs no allocation after a thread's first record (the render thread
//! records; invariant 6 forbids its loop to allocate). A counting allocator of this binary alone.

use kt_time::trace::{self, Ev};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}

#[global_allocator]
static A: Counting = Counting;

#[test]
fn t2143_recording_does_not_allocate() {
    trace::install(trace::Config::new(None, "t2143"));
    let n = trace::name("t2143:kernel");
    // the thread's first record names it (one allocation, once)
    trace::ev(Ev::Job).emit();
    let before = ALLOCS.with(|c| c.get());
    for k in 0..10_000u64 {
        let _c = trace::with_cause(k);
        trace::ev(Ev::DecReq).id(trace::key_id(1, 64, 36, k as i64)).a(1).b(k).emit();
        let mut s = trace::span(Ev::ComputeJob);
        s.aux(n).a(k);
        let _ = trace::name("t2143:kernel");
    }
    let after = ALLOCS.with(|c| c.get());
    assert_eq!(before, after, "recording allocated");
}
