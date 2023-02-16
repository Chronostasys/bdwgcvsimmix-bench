use std::thread::available_parallelism;

use bdwgcvsimmix_bench::*;
use criterion::{criterion_group, criterion_main, Criterion};
use immix::{Collector, VisitFunc, VtableFunc};
use instant::Duration;

#[inline(never)]
fn immix_noop1(_u: usize) {}

macro_rules! keep_on_stack {
    ($($e: expr),*) => {
        $(
            immix_noop1($e as *const _ as usize)
        )*
    }
}

static mut ROOT: *mut u8 = 0 as *mut _;

struct GCTestObj {
    _vtable: VtableFunc,
    b: *mut GCTestObj,
    d: *mut u64,
    e: *mut GCTestObj,
}
fn gctest_vtable(
    ptr: *mut u8,
    gc: &Collector,
    mark_ptr: VisitFunc,
    _mark_complex: VisitFunc,
    _mark_trait: VisitFunc,
) {
    let obj = ptr as *mut GCTestObj;
    unsafe {
        mark_ptr(gc, (&mut (*obj).b) as *mut *mut GCTestObj as *mut u8);
        mark_ptr(gc, (&mut (*obj).d) as *mut *mut u64 as *mut u8);
        mark_ptr(gc, (&mut (*obj).e) as *mut *mut GCTestObj as *mut u8);
    }
}

unsafe fn alloc_test_obj(space: &mut Heap) -> *mut GCTestObj {
    let a = space.alloc_raw::<GCTestObj>();
    // let a = malloc(size_of::<GCTestObj>()) as *mut GCTestObj;
    a.write(GCTestObj {
        _vtable: gctest_vtable,
        b: std::ptr::null_mut(),
        d: std::ptr::null_mut(),
        e: std::ptr::null_mut(),
    });
    // GC_register_finalizer(a as * mut u8, fin, null_mut(), null_mut(), null_mut());
    a
}
fn gcbench(space: &mut Heap) -> Duration {
    unsafe {
        let t = std::time::Instant::now();
        let mut long_lived = alloc_test_obj(space);
        let rustptr = (&mut long_lived) as *mut *mut GCTestObj as *mut u8;
        ROOT = rustptr;
        space.add_root(rustptr);
        populate(K_LONG_LIVED_TREE_DEPTH, long_lived, space);
        // let _tt = space.collect();
        // println!("ep: {:?}", ep);
        let mut d = K_MIN_TREE_DEPTH;
        while d <= K_MAX_TREE_DEPTH {
            time_construction(d, space);
            // Populate(d,  long_lived, space);
            d += 2;
            // println!("ep: {:?}", ep);
        }
        // let _tt = space.collect();
        // space.remove_root(rustptr);
        // let _tt = space.collect();
        // println!("ep: {:?}", ep);
        // sleep(Duration::from_secs(100000));
        space.remove_root(rustptr);
        keep_on_stack!(&mut long_lived);
        let t = t.elapsed();
        // println!("time: {:?}", t);
        // tt.0 + tt.1
        t
    }
}

fn tree_size(i: i32) -> i32 {
    (1 << (i + 1)) - 1
}

fn num_iters(i: i32) -> i32 {
    2 * tree_size(K_STRETCH_TREE_DEPTH) / tree_size(i)
}
#[inline(never)]
unsafe fn populate(idepth: i32, thisnode: *mut GCTestObj, space: &mut Heap) {
    if idepth <= 0 {
        return;
    }
    (*thisnode).b = alloc_test_obj(space);
    (*thisnode).e = alloc_test_obj(space);
    populate(idepth - 1, (*thisnode).e, space);
    populate(idepth - 1, (*thisnode).b, space);
}
#[inline(never)]
unsafe fn make_tree(idepth: i32, space: &mut Heap) -> *mut GCTestObj {
    if idepth <= 0 {
        return alloc_test_obj(space);
    } else {
        let mut left = make_tree(idepth - 1, space);
        let rustptr1 = (&mut left) as *mut *mut GCTestObj as *mut u8;
        space.add_root(rustptr1);
        let mut right = make_tree(idepth - 1, space);
        let rustptr2 = (&mut right) as *mut *mut GCTestObj as *mut u8;
        space.add_root(rustptr2);
        let mut result = alloc_test_obj(space);
        let rustptr3 = (&mut result) as *mut *mut GCTestObj as *mut u8;
        space.add_root(rustptr3);
        (*result).b = left;
        (*result).e = right;
        space.remove_root(rustptr2);
        space.remove_root(rustptr1);
        space.remove_root(rustptr3);
        result
    }
}
#[inline(never)]
unsafe fn time_construction(depth: i32, space: &mut Heap) {
    let i_num_iters = num_iters(depth);

    for _ in 0..i_num_iters {
        let mut temp_tree = alloc_test_obj(space);
        let rustptr = (&mut temp_tree) as *mut *mut GCTestObj as *mut u8;
        space.add_root(rustptr);
        populate(depth, temp_tree, space);
        space.remove_root(rustptr);
        keep_on_stack!(temp_tree)
        // destroy tempTree
    }

    for _ in 0..i_num_iters {
        let _temp_tree = make_tree(depth, space);
        keep_on_stack!(_temp_tree)
    }
}
const K_STRETCH_TREE_DEPTH: i32 = 18;
const K_LONG_LIVED_TREE_DEPTH: i32 = 16;

const K_MIN_TREE_DEPTH: i32 = 4;
const K_MAX_TREE_DEPTH: i32 = 16;

fn criterion_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("bdwgc");
    group.sample_size(10);
    // group.bench_function("gc malloc", |b|b.iter(bench_malloc));
    // group.bench_function("gcbench", |b| b.iter(|| gcbench(&mut heap)));

    // group.bench_function("gcbench", |b| b.iter_custom(|i| {
    //     let mut heap = Heap::new();
    //     let mut sum = Duration::new(0, 0);
    //     for _ in 0..i {
    //         sum += gcbench(&mut heap);
    //     }
    //     sum
    // }));
    let mut heap = Heap::new();
    group.bench_function(format!("bdwgc {} threads", get_threads()), |b| {
        b.iter(|| {
            let mut threads = Vec::with_capacity(4);
            for _ in 0..get_threads() {
                threads.push(std::thread::spawn(move || {
                    heap.register_current_thread();
                    gcbench(&mut heap);
                    unsafe {
                        GC_unregister_my_thread();
                    }
                }));
            }

            while let Some(th) = threads.pop() {
                th.join().unwrap();
            }
        });
    });
}

fn get_threads() -> usize {
    available_parallelism().unwrap().get()
}

criterion_group!(benches, criterion_bench);
criterion_main!(benches);
