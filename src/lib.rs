use std::ops::{Deref, DerefMut};

use immix::{ObjectType, SPACE};

#[link(name = "gc")]
extern "C" {
    pub fn GC_malloc(size: usize) -> *mut u8;
    pub fn GC_init();
    pub fn GC_enable_incremental();
    pub fn GC_set_disable_automatic_collection(disable: i32);
    pub fn GC_gcollect();
    pub fn GC_gcollect_and_unmap();
    pub fn GC_get_heap_size() -> usize;
    pub fn GC_set_markers_count(count: u32);
    pub fn GC_dump();
    pub fn GC_register_finalizer(
        obj: *mut u8,
        fn_ptr: unsafe extern "C" fn(*mut u8, *mut u8),
        client_data: *mut u8,
        old_fn_ptr: *mut *mut u8,
        old_client_data: *mut *mut u8,
    );
    pub fn GC_add_roots(start: *mut *mut u8, end: *mut *mut u8);
    pub fn GC_remove_roots(start: *mut *mut u8, end: *mut *mut u8);
    // GC_API size_t GC_CALL GC_get_prof_stats(struct GC_prof_stats_s *,
    //  size_t /* stats_sz */);
    pub fn GC_get_prof_stats(state: *mut GCState, size: usize) -> usize;
    pub fn GC_allow_register_threads();
    pub fn GC_register_my_thread(stack: *mut StackBase) -> i32;
    pub fn GC_unregister_my_thread() -> i32;
    pub fn GC_get_stack_base(stack: *mut StackBase) -> i32;
    pub fn GC_disable();
    pub fn GC_enable();
    pub fn GC_set_on_collection_event(fn_ptr: unsafe extern "C" fn(tp: i8));
}

// unsafe extern "C" fn callback(tp:i8) {
//     if tp == 6 {
//         println!("GC stw start");
//     }
// }

#[derive(Debug)]
#[repr(C)]
pub struct StackBase {
    pub mem_base: *mut u8,
    pub reg_base: *mut u8,
}

pub struct Gc<T> {
    ptr: *mut T,
}

impl<T> Deref for Gc<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<T> DerefMut for Gc<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr }
    }
}

#[derive(Copy, Clone)]
pub struct Heap;

impl Heap {
    pub fn new() -> Self {
        unsafe {
            // GC_set_markers_count(1);
            GC_init();
            // GC_set_on_collection_event(callback);
            // GC_set_disable_automatic_collection(1);
            GC_allow_register_threads();
            // GC_disable();
            // GC_enable_incremental();
        }
        Self
    }

    pub fn allocate<T>(&mut self, value: T) -> Gc<T> {
        Gc {
            ptr: unsafe {
                let p = GC_malloc(std::mem::size_of::<T>()).cast::<T>();
                p.write(value);
                p
            },
        }
    }

    pub fn register_current_thread(&mut self) {
        let mut stack = StackBase {
            mem_base: std::ptr::null_mut(),
            reg_base: std::ptr::null_mut(),
        };
        unsafe {
            GC_get_stack_base(&mut stack);
            GC_register_my_thread(&mut stack);
        }
    }
    pub fn alloc_raw<T>(&mut self) -> *mut T {
        unsafe { GC_malloc(std::mem::size_of::<T>()).cast::<T>() }
    }
    pub fn collect(&mut self) {
        unsafe {
            GC_enable();
            GC_gcollect();
            GC_disable();
            // GC_gcollect_and_unmap();
            // GC_dump();
        }
    }
    pub fn print_state(&mut self) {
        // let mut state = GCState::default();
        // unsafe {
        //     GC_get_prof_stats(&mut state,std::mem::size_of::<GCState>());
        // }
        // println!("{:?}",state);
    }

    /// 虽然bdw不需要shadow stack，但是生产中immix也不需要（使用stackmap
    /// 所以为了公平起见，这里搞个假的shadow stack来平衡开销
    pub fn add_root(&self, root: *mut u8) {
        SPACE.with(|space| {
            space.borrow_mut().add_root(root, ObjectType::Pointer);
        });
    }
    pub fn remove_root(&self, root: *mut u8) {
        SPACE.with(|space| {
            space.borrow_mut().remove_root(root);
        });
    }
}

impl<T> Clone for Gc<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Gc<T> {}

// struct GC_prof_stats_s {
//     GC_word heapsize_full;
//               /* Heap size in bytes (including the area unmapped to OS).  */
//               /* Same as GC_get_heap_size() + GC_get_unmapped_bytes().    */
//     GC_word free_bytes_full;
//               /* Total bytes contained in free and unmapped blocks.       */
//               /* Same as GC_get_free_bytes() + GC_get_unmapped_bytes().   */
//     GC_word unmapped_bytes;
//               /* Amount of memory unmapped to OS.  Same as the value      */
//               /* returned by GC_get_unmapped_bytes().                     */
//     GC_word bytes_allocd_since_gc;
//               /* Number of bytes allocated since the recent collection.   */
//               /* Same as returned by GC_get_bytes_since_gc().             */
//     GC_word allocd_bytes_before_gc;
//               /* Number of bytes allocated before the recent garbage      */
//               /* collection.  The value may wrap.  Same as the result of  */
//               /* GC_get_total_bytes() - GC_get_bytes_since_gc().          */
//     GC_word non_gc_bytes;
//               /* Number of bytes not considered candidates for garbage    */
//               /* collection.  Same as returned by GC_get_non_gc_bytes().  */
//     GC_word gc_no;
//               /* Garbage collection cycle number.  The value may wrap     */
//               /* (and could be -1).  Same as returned by GC_get_gc_no().  */
//     GC_word markers_m1;
//               /* Number of marker threads (excluding the initiating one). */
//               /* Same as returned by GC_get_parallel (or 0 if the         */
//               /* collector is single-threaded).                           */
//     GC_word bytes_reclaimed_since_gc;
//               /* Approximate number of reclaimed bytes after recent GC.   */
//     GC_word reclaimed_bytes_before_gc;
//               /* Approximate number of bytes reclaimed before the recent  */
//               /* garbage collection.  The value may wrap.                 */
//     GC_word expl_freed_bytes_since_gc;
//               /* Number of bytes freed explicitly since the recent GC.    */
//               /* Same as returned by GC_get_expl_freed_bytes_since_gc().  */
//     GC_word obtained_from_os_bytes;
//               /* Total amount of memory obtained from OS, in bytes.       */
//   };

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct GCState {
    heapsize_full: usize,
    free_bytes_full: usize,
    unmapped_bytes: usize,
    bytes_allocd_since_gc: usize,
    allocd_bytes_before_gc: usize,
    non_gc_bytes: usize,
    gc_no: usize,
    markers_m1: usize,
    bytes_reclaimed_since_gc: usize,
    reclaimed_bytes_before_gc: usize,
    expl_freed_bytes_since_gc: usize,
    obtained_from_os_bytes: usize,
}
