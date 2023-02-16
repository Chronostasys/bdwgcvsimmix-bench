use std::{mem::size_of, thread::available_parallelism, time::Duration};

use immix::*;
use libc::malloc;
use rand::random;



#[repr(C)]
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

static mut ALLOC_NUM: usize = 0;

unsafe fn alloc_test_obj(gc: &mut Collector) -> *mut GCTestObj {
    let a = gc.alloc(size_of::<GCTestObj>(), ObjectType::Complex) as *mut GCTestObj;
    // let a = malloc(size_of::<GCTestObj>()) as *mut GCTestObj;
    a.write(GCTestObj {
        _vtable: gctest_vtable,
        b: std::ptr::null_mut(),
        d: std::ptr::null_mut(),
        e: std::ptr::null_mut(),
    });
    a
}

fn gcbench(space: & mut Collector) -> Duration {
    unsafe {
        // sleep(Duration::from_secs(1000));
        let t = std::time::Instant::now();
        let mut long_lived = alloc_test_obj(space);
        let rustptr = (&mut long_lived) as *mut *mut GCTestObj as *mut u8;
        space.add_root(rustptr, ObjectType::Pointer);
        populate(K_LONG_LIVED_TREE_DEPTH,  long_lived, space);
        // let _tt = space.collect();
        let mut d = K_MIN_TREE_DEPTH;
        while d <= K_MAX_TREE_DEPTH {
            time_construction(d, space);
            // populate(d,  long_lived, space);
            d += 2;
            // let _tt = space.collect();
        }
        let time_start = std::time::Instant::now();
        // let _tt = space.collect();
        let ep = time_start.elapsed();
        // println!("ep: {:?} tt: {:?}", ep,_tt);
        space.remove_root(rustptr);
        
        // let _tt = space.collect();
        let t = t.elapsed();
        // space.collect();
        // println!("done collect");
        // loop {
        //     space.collect();
        //     GLOBAL_ALLOCATOR.unmap_all();
        //     println!("done unmap");
        //     sleep(Duration::from_millis(10));
        //     println!("gc: {}, size: {}", space.get_id(), space.get_size());
        // }
        // println!("alloc num: {}",ALLOC_NUM);
        // sleep(Duration::from_secs(1000));

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
unsafe fn populate(idepth: i32, thisnode: * mut GCTestObj, space: &mut Collector) {
    if idepth <= 0 {
        return;
    }
    // space.collect();
    (*thisnode).b = alloc_test_obj(space);
    (*thisnode).e = alloc_test_obj(space);
    populate(idepth - 1, (*thisnode).e, space);
    populate(idepth - 1, (*thisnode).b, space);
}
 
unsafe fn make_tree(idepth: i32, space: &mut Collector) -> * mut GCTestObj {
    if idepth <= 0 {
        // space.collect();
        return alloc_test_obj(space);
    } else {
        let mut left = make_tree(idepth - 1, space);
        let rustptr1 = (&mut left) as *mut *mut GCTestObj as *mut u8;
        space.add_root(rustptr1, ObjectType::Pointer);
        let mut right = make_tree(idepth - 1, space);
        let rustptr2 = (&mut right) as *mut *mut GCTestObj as *mut u8;
        space.add_root(rustptr2, ObjectType::Pointer);
        // space.collect();
        let mut result = alloc_test_obj(space);
        let rustptr3 = (&mut result) as *mut *mut GCTestObj as *mut u8;
        space.add_root(rustptr3, ObjectType::Pointer);
        (*result).b = left;
        (*result).e = right;
        space.remove_root(rustptr2);
        space.remove_root(rustptr1);
        space.remove_root(rustptr3);
        result
    }
}
#[inline(never)]
unsafe fn time_construction(depth: i32, space: &mut Collector) {
    let i_num_iters = num_iters(depth);

    for _ in 0..i_num_iters {
        let mut temp_tree = alloc_test_obj(space);
        let rustptr = (&mut temp_tree) as *mut *mut GCTestObj as *mut u8;
        space.add_root(rustptr, ObjectType::Pointer);
        populate(depth,  temp_tree, space);
        space.remove_root(rustptr);

        // destroy tempTree
    }

    for _ in 0..i_num_iters {
        let _temp_tree = make_tree(depth, space);
    }
}
const K_STRETCH_TREE_DEPTH: i32 = 18;
const K_LONG_LIVED_TREE_DEPTH: i32 = 16;
// const K_ARRAY_SIZE: i32 = 500000;
const K_MIN_TREE_DEPTH: i32 = 4;
const K_MAX_TREE_DEPTH: i32 = 16;



fn main() {
    // let mut heap = Heap::new();
    let mut threads = Vec::with_capacity(4);
    for _ in 0..1 {
        threads.push(std::thread::spawn(move || {
            SPACE.with(|space| {
                let mut space = space.borrow_mut();
                let tt = gcbench(&mut space);
                // t.elapsed()
                tt
            });
        }));
    }

    while let Some(th) = threads.pop() {
        th.join().unwrap();
    }
}
