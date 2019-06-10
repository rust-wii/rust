use crate::cell::UnsafeCell;
use crate::mem;

pub struct Mutex {
    inner: UnsafeCell<libc::pthread_mutex_t>,
}

#[inline]
pub unsafe fn raw(m: &Mutex) -> *mut libc::pthread_mutex_t {
    m.inner.get()
}

unsafe impl Send for Mutex {}
unsafe impl Sync for Mutex {}

#[allow(dead_code)] // sys isn't exported yet
impl Mutex {
    pub const fn new() -> Mutex {
        // Might be moved to a different address, so it is better to avoid
        // initialization of potentially opaque OS data before it landed.
        // Be very careful using this newly constructed `Mutex`, reentrant
        // locking is undefined behavior until `init` is called!
        Mutex { inner: UnsafeCell::new(libc::PTHREAD_MUTEX_INITIALIZER) }
    }

    #[inline]
    pub unsafe fn init(&mut self) {
        // Issue #33770
        //
        // A pthread mutex initialized with PTHREAD_MUTEX_INITIALIZER will have
        // a type of PTHREAD_MUTEX_DEFAULT, which has undefined behavior if you
        // try to re-lock it from the same thread when you already hold a lock.
        //
        // In practice, glibc takes advantage of this undefined behavior to
        // implement hardware lock elision, which uses hardware transactional
        // memory to avoid acquiring the lock. While a transaction is in
        // progress, the lock appears to be unlocked. This isn't a problem for
        // other threads since the transactional memory will abort if a conflict
        // is detected, however no abort is generated if re-locking from the
        // same thread.
        //
        // Since locking the same mutex twice will result in two aliasing &mut
        // references, we instead create the mutex with type
        // PTHREAD_MUTEX_NORMAL which is guaranteed to deadlock if we try to
        // re-lock it from the same thread, thus avoiding undefined behavior.

        //let mut attr: libc::pthread_mutexattr_t = mem::uninitialized();
        // let r = libc::pthread_mutexattr_init(&mut attr);
        // debug_assert_eq!(r, 0);
        // let r = libc::pthread_mutexattr_settype(&mut attr, libc::PTHREAD_MUTEX_NORMAL);
        // debug_assert_eq!(r, 0);
        //let r = libc::pthread_mutexattr_destroy(&mut attr);
        //debug_assert_eq!(r, 0);

        // Note: libogc doesnt require special init functions as seen above.
        // The LWP_MutexInit function uses a boolean to determine if it will be recursive or not.
        let r = ogc_sys::LWP_MutexInit(self.inner.get() as *mut u32, false);
        debug_assert_eq!(r, 0);
    }

    #[inline]
    pub unsafe fn lock(&self) {
        //let r = libc::pthread_mutex_lock(self.inner.get());
        let r = ogc_sys::LWP_MutexLock(self.inner.get() as u32);
        debug_assert_eq!(r, 0);
    }

    #[inline]
    pub unsafe fn unlock(&self) {
        //let r = libc::pthread_mutex_unlock(self.inner.get());
        let r = ogc_sys::LWP_MutexUnlock(self.inner.get() as u32);
        debug_assert_eq!(r, 0);
    }

    #[inline]
    pub unsafe fn try_lock(&self) -> bool {
        //libc::pthread_mutex_trylock(self.inner.get()) == 0
        ogc_sys::LWP_MutexTryLock(self.inner.get() as u32) == 0
    }

    #[inline]
    pub unsafe fn destroy(&self) {
        //let r = libc::pthread_mutex_destroy(self.inner.get());
        let r = ogc_sys::LWP_MutexDestroy(self.inner.get() as u32);
        debug_assert_eq!(r, 0);
    }
}

pub struct ReentrantMutex {
    inner: UnsafeCell<libc::pthread_mutex_t>,
}

unsafe impl Send for ReentrantMutex {}
unsafe impl Sync for ReentrantMutex {}

impl ReentrantMutex {
    pub unsafe fn uninitialized() -> ReentrantMutex {
        ReentrantMutex { inner: mem::uninitialized() }
    }

    pub unsafe fn init(&mut self) {
        // let mut attr: libc::pthread_mutexattr_t = mem::uninitialized();
        // let result = libc::pthread_mutexattr_init(&mut attr as *mut _);
        // debug_assert_eq!(result, 0);
        // let result = libc::pthread_mutexattr_settype(&mut attr as *mut _,
        //                                             libc::PTHREAD_MUTEX_RECURSIVE);
        // debug_assert_eq!(result, 0);
        // let result = libc::pthread_mutexattr_destroy(&mut attr as *mut _);
        // debug_assert_eq!(result, 0);
        
        // Note: libogc doesnt require special init functions as seen above.
        // The LWP_MutexInit function uses a boolean to determine if it will be recursive or not.
        let result = ogc_sys::LWP_MutexInit(self.inner.get() as *mut u32, true);
        debug_assert_eq!(result, 0);
    }

    pub unsafe fn lock(&self) {
        //let result = libc::pthread_mutex_lock(self.inner.get());
        let result = ogc_sys::LWP_MutexLock(self.inner.get() as u32);
        debug_assert_eq!(result, 0);
    }

    #[inline]
    pub unsafe fn try_lock(&self) -> bool {
        //libc::pthread_mutex_trylock(self.inner.get()) == 0
        ogc_sys::LWP_MutexTryLock(self.inner.get() as u32) == 0
    }

    pub unsafe fn unlock(&self) {
        //let result = libc::pthread_mutex_unlock(self.inner.get());
        let result = ogc_sys::LWP_MutexUnlock(self.inner.get() as u32);
        debug_assert_eq!(result, 0);
    }

    pub unsafe fn destroy(&self) {
        //let result = libc::pthread_mutex_destroy(self.inner.get());
        let result = ogc_sys::LWP_MutexDestroy(self.inner.get() as u32);
        debug_assert_eq!(result, 0);
    }
}
