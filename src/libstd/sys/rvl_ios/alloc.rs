use crate::ptr;
use crate::sys_common::alloc::{MIN_ALIGN, realloc_fallback};
use crate::alloc::{GlobalAlloc, Layout, System};

#[stable(feature = "alloc_system_type", since = "1.28.0")]
unsafe impl GlobalAlloc for System {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.align() <= MIN_ALIGN && layout.align() <= layout.size() {
            ogc_sys::malloc(layout.size() as u32) as *mut u8
        } else {
            aligned_malloc(&layout)
        }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if layout.align() <= MIN_ALIGN && layout.align() <= layout.size() {
            ogc_sys::calloc(layout.size() as u32, 1) as *mut u8
        } else {
            let ptr = self.alloc(layout.clone());
            if !ptr.is_null() {
                ptr::write_bytes(ptr, 0, layout.size());
            }
            ptr
        }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        ogc_sys::free(ptr as *mut libc::c_void)
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if layout.align() <= MIN_ALIGN && layout.align() <= new_size {
            ogc_sys::realloc(ptr as *mut libc::c_void, new_size as u32) as *mut u8
        } else {
            realloc_fallback(self, ptr, layout, new_size)
        }
    }
}

#[cfg(any(target_os = "android",
          target_os = "hermit",
          target_os = "redox",
          target_os = "solaris",
          target_os = "rvl-ios"))]
#[inline]
unsafe fn aligned_malloc(layout: &Layout) -> *mut u8 {
    ogc_sys::memalign(layout.align(), layout.size()) as *mut u8
}
