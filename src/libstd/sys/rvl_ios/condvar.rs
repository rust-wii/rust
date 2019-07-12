use crate::cell::UnsafeCell;
use crate::sys::mutex::{self, Mutex};
use crate::time::Duration;

pub struct Condvar {
    inner: UnsafeCell<ogc_sys::pthread_cond_t>,
}

unsafe impl Send for Condvar {}
unsafe impl Sync for Condvar {}

const TIMESPEC_MAX: ogc_sys::timespec =
    ogc_sys::timespec { tv_sec: <ogc_sys::time_t>::max_value(), tv_nsec: 1_000_000_000 - 1, __bindgen_padding_0: 0 };

const CLOCK_MONOTONIC: i32 = 4;

fn saturating_cast_to_time_t(value: u64) -> ogc_sys::time_t {
    if value > <ogc_sys::time_t>::max_value() as u64 {
        <ogc_sys::time_t>::max_value()
    } else {
        value as ogc_sys::time_t
    }
}

impl Condvar {
    pub const fn new() -> Condvar {
        // Might be moved and address is changing it is better to avoid
        // initialization of potentially opaque OS data before it landed
        Condvar { inner: UnsafeCell::new(0) }
    }

    pub unsafe fn init(&mut self) {
        // use crate::mem;
        // let mut attr: libc::pthread_condattr_t = mem::uninitialized();
        // let r = libc::pthread_condattr_init(&mut attr);
        // assert_eq!(r, 0);
        // let r = libc::pthread_condattr_setclock(&mut attr, libc::CLOCK_MONOTONIC);
        // assert_eq!(r, 0);
        // let r = libc::pthread_condattr_destroy(&mut attr);
        // assert_eq!(r, 0);
        let r = ogc_sys::LWP_CondInit(self.inner.get() as *mut u32);
        assert_eq!(r, 0);
    }

    #[inline]
    pub unsafe fn notify_one(&self) {
        // let r = libc::pthread_cond_signal(self.inner.get());
        let r = ogc_sys::LWP_CondSignal(self.inner.get() as u32);
        debug_assert_eq!(r, 0);
    }

    #[inline]
    pub unsafe fn notify_all(&self) {
        // let r = libc::pthread_cond_broadcast(self.inner.get());
        let r = ogc_sys::LWP_CondBroadcast(self.inner.get() as u32);
        debug_assert_eq!(r, 0);
    }

    #[inline]
    pub unsafe fn wait(&self, mutex: &Mutex) {
        // let r = libc::pthread_cond_wait(self.inner.get(), mutex::raw(mutex));
        let r = ogc_sys::LWP_CondWait(self.inner.get() as u32, mutex::raw(mutex) as u32);
        debug_assert_eq!(r, 0);
    }

    // This implementation is used on systems that support pthread_condattr_setclock
    // where we configure condition variable to use monotonic clock (instead of
    // default system clock). This approach avoids all problems that result
    // from changes made to the system time.
    pub unsafe fn wait_timeout(&self, mutex: &Mutex, dur: Duration) -> bool {
        use crate::mem;

        let mut now: ogc_sys::timespec = mem::zeroed();
        let r = ogc_sys::clock_gettime(CLOCK_MONOTONIC as u32, &mut now);
        assert_eq!(r, 0);

        // Nanosecond calculations can't overflow because both values are below 1e9.
        let nsec = dur.subsec_nanos() + now.tv_nsec as u32;

        let sec = saturating_cast_to_time_t(dur.as_secs())
            .checked_add((nsec / 1_000_000_000) as ogc_sys::time_t)
            .and_then(|s| s.checked_add(now.tv_sec));
        let nsec = nsec % 1_000_000_000;

        // TODO: FIX TIMED WAIT
        // let timeout =
        //     sec.map(|s| ogc_sys::timespec { tv_sec: s, tv_nsec: nsec as _ }).unwrap_or(TIMESPEC_MAX);

        // let r = ogc_sys::LWP_CondTimedWait(self.inner.get() as u32, mutex::raw(mutex) as u32, &timeout);
        // assert!(r == libc::ETIMEDOUT || r == 0);
        // r == 0
        true
    }

    #[inline]
    pub unsafe fn destroy(&self) {
        // let r = libc::pthread_cond_destroy(self.inner.get());
        let r = ogc_sys::LWP_CondDestroy(self.inner.get() as u32);
        debug_assert_eq!(r, 0);
    }
}
