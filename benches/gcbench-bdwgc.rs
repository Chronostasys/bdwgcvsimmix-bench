use std::{mem::size_of, ptr::null_mut, thread::{sleep, available_parallelism}};

use bdwgcvsimmix_bench::*;
use criterion::{criterion_group, criterion_main, Criterion};
use immix::{VtableFunc, Collector, VisitFunc, ObjectType};
use instant::Duration;
use libc::malloc;
use rand::random;

#[inline(never)]
fn immix_noop1(_u:usize) {
    
}

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
        // mark_ptr(gc, (&mut (*obj).d) as *mut *mut u64 as *mut u8);
        mark_ptr(gc, (&mut (*obj).e) as *mut *mut GCTestObj as *mut u8);
    }
}
unsafe extern "C" fn fin(_: *mut u8,_: *mut u8){
    println!("finalizer triggered");
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
        // space.add_root(rustptr, ObjectType::Pointer);
        Populate(kLongLivedTreeDepth,  long_lived, space);
        let time_start = std::time::Instant::now();
        // let _tt = space.collect();
        let ep = time_start.elapsed();
        // println!("ep: {:?}", ep);
        let mut d = kMinTreeDepth;
        while d <= kMaxTreeDepth {
            TimeConstruction(d, space);
            // Populate(d,  long_lived, space);
            d += 2;
            let time_start = std::time::Instant::now();
            // let _tt = space.collect();
            let ep = time_start.elapsed();
            // println!("ep: {:?}", ep);
        }
        // let _tt = space.collect();
        // space.remove_root(rustptr);
        let time_start = std::time::Instant::now();
        // let _tt = space.collect();
        let ep = time_start.elapsed();
        // println!("ep: {:?}", ep);
        // sleep(Duration::from_secs(100000));
        keep_on_stack!(&mut long_lived);
        let t = t.elapsed();
        // println!("time: {:?}", t);
        // tt.0 + tt.1
        t
    }

}



fn bench_malloc() -> *mut GCTestObj {
    unsafe { GC_malloc(size_of::<GCTestObj>()) as *mut GCTestObj }
}
pub struct Node {
    left: Option<Gc<Self>>,
    right: Option<Gc<Self>>,
    i: i32,
    j: i32,
}

fn TreeSize(i: i32) -> i32 {
    (1 << (i + 1)) - 1
}

fn NumIters(i: i32) -> i32 {
    2 * TreeSize(kStretchTreeDepth) / TreeSize(i)
}
#[inline(never)]
unsafe fn Populate(idepth: i32, thisnode: * mut GCTestObj, space: &mut Heap) {
    if idepth <= 0 {
        return;
    }
    (*thisnode).b = alloc_test_obj(space);
    (*thisnode).e = alloc_test_obj(space);
    Populate(idepth - 1, (*thisnode).e, space);
    Populate(idepth - 1, (*thisnode).b, space);
}
#[inline(never)]
unsafe fn MakeTree(idepth: i32, space: &mut Heap) -> * mut GCTestObj {
    if idepth <= 0 {
        return alloc_test_obj(space);
    } else {
        let left = MakeTree(idepth - 1, space);
        let right = MakeTree(idepth - 1, space);
        let result = alloc_test_obj(space);
        (*result).b = left;
        (*result).e = right;
        result
    }
}
#[inline(never)]
unsafe fn TimeConstruction(depth: i32, space: &mut Heap) {
    let iNumIters = NumIters(depth);

    let start = instant::Instant::now();
    for _ in 0..iNumIters {
        let tempTree = alloc_test_obj(space);
        Populate(depth,  tempTree, space);
        keep_on_stack!(tempTree)
        // destroy tempTree
    }

    let start = instant::Instant::now();
    for _ in 0..iNumIters {
        let tempTree = MakeTree(depth, space);
        keep_on_stack!(tempTree)
    }
}
const kStretchTreeDepth: i32 = 18;
const kLongLivedTreeDepth: i32 = 16;
const kArraySize: i32 = 500000;
const kMinTreeDepth: i32 = 4;
const kMaxTreeDepth: i32 = 16;
struct Array {
    value: [f64; kArraySize as usize],
}

fn criterion_bench(c: &mut Criterion) {
    let mut heap = Heap::new();

    let mut group = c.benchmark_group("bdwgc");
    group.sample_size(10);
    // group.bench_function("gc malloc", |b|b.iter(bench_malloc));
    // group.bench_function("gcbench", |b| b.iter(|| gcbench(&mut heap)));

    group.bench_function("gcbench", |b| b.iter_custom(|i| {
        // let mut heap = Heap::new();
        let mut sum = Duration::new(0, 0);
        for _ in 0..i {
            sum += gcbench(&mut heap);
        }
        sum
    }));
    group.bench_function("gcb", |b| {
        b.iter(|| {
            let mut threads = Vec::with_capacity(4);
            for _ in 0..get_threads() {
                threads.push(std::thread::spawn(move || {
                    let mut heap = heap;
                    heap.register_current_thread();
                    gcbench(&mut heap);
                    unsafe{ GC_unregister_my_thread();}
                }));
            }

            while let Some(th) = threads.pop() {
                th.join().unwrap();
            }
        });
    });
}
criterion_group!(benches, criterion_bench);
criterion_main!(benches);

fn get_threads() -> usize {
    available_parallelism().unwrap().get()
}
