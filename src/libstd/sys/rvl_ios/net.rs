use crate::cmp;
use crate::ffi::CStr;
use crate::io::{self, IoSlice, IoSliceMut};
use crate::mem;
use crate::net::{Shutdown, SocketAddr};
use crate::str;
use crate::sys::fd::FileDesc;
use crate::sys_common::net::{getsockopt, setsockopt, sockaddr_to_addr};
use crate::sys_common::{AsInner, FromInner, IntoInner};
use crate::time::{Duration, Instant};

use libc::{c_int, c_void, size_t, sockaddr, socklen_t, MSG_PEEK};

pub use crate::sys::{cvt, cvt_r};

#[allow(unused_extern_crates)]
pub extern crate libc as netc;

pub type wrlen_t = size_t;

const SOCK_CLOEXEC: c_int = 0;
const SO_NOSIGPIPE: c_int = 0;

pub struct Socket(FileDesc);

pub fn init() {}

pub fn cvt_gai(err: c_int) -> io::Result<()> {
    if err == 0 {
        return Ok(());
    }

    // if err == EAI_SYSTEM {
    //     return Err(io::Error::last_os_error());
    // }

    let detail = unsafe {
        str::from_utf8(CStr::from_ptr(libc::gai_strerror(err)).to_bytes()).unwrap().to_owned()
    };
    Err(io::Error::new(
        io::ErrorKind::Other,
        &format!("failed to lookup address information: {}", detail)[..],
    ))
}

impl Socket {
    pub fn new(addr: &SocketAddr, ty: c_int) -> io::Result<Socket> {
        // IPV6 Matching TODO
        let fam = match *addr {
            SocketAddr::V4(..) => libc::AF_INET,
            SocketAddr::V6(..) => libc::AF_INET,
        };
        Socket::new_raw(fam, ty)
    }

    pub fn new_raw(fam: c_int, ty: c_int) -> io::Result<Socket> {
        unsafe {
            let fd = cvt(ogc_sys::net_socket(fam as u32, ty as u32, 0))?;
            let fd = FileDesc::new(fd);
            fd.set_cloexec()?;
            let socket = Socket(fd);
            Ok(socket)
        }
    }

    pub fn connect_timeout(&self, addr: &SocketAddr, timeout: Duration) -> io::Result<()> {
        self.set_nonblocking(true)?;
        let r = unsafe {
            let (addrp, len) = addr.into_inner();
            // ogc_sys::net_connect(self.0.raw(), addrp, len)
            cvt(101)
        };
        self.set_nonblocking(false)?;

        match r {
            Ok(_) => return Ok(()),
            // there's no ErrorKind for EINPROGRESS :(
            Err(ref e) if e.raw_os_error() == Some(libc::EINPROGRESS) => {}
            Err(e) => return Err(e),
        }

        let mut pollfd = ogc_sys::pollsd { socket: self.0.raw(), events: ogc_sys::POLLOUT, revents: 0 };

        if timeout.as_secs() == 0 && timeout.subsec_nanos() == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot set a 0 duration timeout",
            ));
        }

        let start = Instant::now();

        loop {
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "connection timed out"));
            }

            let timeout = timeout - elapsed;
            let mut timeout = timeout
                .as_secs()
                .saturating_mul(1_000)
                .saturating_add(timeout.subsec_nanos() as u64 / 1_000_000);
            if timeout == 0 {
                timeout = 1;
            }

            let timeout = cmp::min(timeout, c_int::max_value() as u64) as c_int;

            match unsafe { ogc_sys::net_poll(&mut pollfd, 1, timeout) } {
                -1 => {
                    let err = io::Error::last_os_error();
                    if err.kind() != io::ErrorKind::Interrupted {
                        return Err(err);
                    }
                }
                0 => {}
                _ => {
                    // linux returns POLLOUT|POLLERR|POLLHUP for refused connections (!), so look
                    // for POLLHUP rather than read readiness
                    if pollfd.revents & ogc_sys::POLLHUP != 0 {
                        let e = self.take_error()?.unwrap_or_else(|| {
                            io::Error::new(io::ErrorKind::Other, "no error set after POLLHUP")
                        });
                        return Err(e);
                    }

                    return Ok(());
                }
            }
        }
    }

    pub fn accept(&self, storage: *mut ogc_sys::sockaddr, len: *mut socklen_t) -> io::Result<Socket> {
        let fd = cvt_r(|| unsafe { ogc_sys::net_accept(self.0.raw(), storage, len) })?;
        let fd = FileDesc::new(fd);
        fd.set_cloexec()?;
        Ok(Socket(fd))
    }

    pub fn duplicate(&self) -> io::Result<Socket> {
        self.0.duplicate().map(Socket)
    }

    fn recv_with_flags(&self, buf: &mut [u8], flags: c_int) -> io::Result<usize> {
        let ret = cvt(unsafe {
            ogc_sys::net_recv(self.0.raw(), buf.as_mut_ptr() as *mut c_void, buf.len() as i32, flags as u32)
        })?;
        Ok(ret as usize)
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.recv_with_flags(buf, 0)
    }

    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.recv_with_flags(buf, MSG_PEEK)
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        //self.0.read_vectored(bufs)
        unimplemented!()
    }

    fn recv_from_with_flags(
        &self,
        buf: &mut [u8],
        flags: c_int,
    ) -> io::Result<(usize, SocketAddr)> {
        let mut storage: libc::sockaddr_storage = unsafe { mem::zeroed() };
        let mut addrlen = mem::size_of_val(&storage) as libc::socklen_t;

        let n = cvt(unsafe {
            ogc_sys::net_recvfrom(
                self.0.raw(),
                buf.as_mut_ptr() as *mut c_void,
                buf.len() as i32,
                flags as u32,
                &mut storage as *mut _ as *mut _,
                &mut addrlen,
            )
        })?;
        Ok((n as usize, sockaddr_to_addr(&storage, addrlen as usize)?))
    }

    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.recv_from_with_flags(buf, 0)
    }

    pub fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.recv_from_with_flags(buf, MSG_PEEK)
    }

    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        //self.0.write_vectored(bufs)
        unimplemented!()
    }

    pub fn set_timeout(&self, dur: Option<Duration>, kind: libc::c_int) -> io::Result<()> {
        let timeout = match dur {
            Some(dur) => {
                if dur.as_secs() == 0 && dur.subsec_nanos() == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "cannot set a 0 duration timeout",
                    ));
                }

                let secs = if dur.as_secs() > libc::time_t::max_value() as u64 {
                    libc::time_t::max_value()
                } else {
                    dur.as_secs() as libc::time_t
                };
                let mut timeout = libc::timeval {
                    tv_sec: secs,
                    tv_usec: (dur.subsec_nanos() / 1000) as libc::suseconds_t,
                };
                if timeout.tv_sec == 0 && timeout.tv_usec == 0 {
                    timeout.tv_usec = 1;
                }
                timeout
            }
            None => libc::timeval { tv_sec: 0, tv_usec: 0 },
        };
        setsockopt(self, libc::SOL_SOCKET, kind, timeout)
    }

    pub fn timeout(&self, kind: libc::c_int) -> io::Result<Option<Duration>> {
        // let raw: libc::timeval = getsockopt(self, libc::SOL_SOCKET, kind)?;
        // if raw.tv_sec == 0 && raw.tv_usec == 0 {
        //     Ok(None)
        // } else {
        //     let sec = raw.tv_sec as u64;
        //     let nsec = (raw.tv_usec as u32) * 1000;
        //     Ok(Some(Duration::new(sec, nsec)))
        // }
        unimplemented!()
    }

    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        let how = match how {
            Shutdown::Write => libc::SHUT_WR,
            Shutdown::Read => libc::SHUT_RD,
            Shutdown::Both => libc::SHUT_RDWR,
        };
        cvt(unsafe { ogc_sys::net_shutdown(self.0.raw(), how as u32) })?;
        Ok(())
    }

    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        setsockopt(self, libc::IPPROTO_TCP, libc::TCP_NODELAY, nodelay as c_int)
    }

    pub fn nodelay(&self) -> io::Result<bool> {
        // let raw: c_int = getsockopt(self, libc::IPPROTO_TCP, libc::TCP_NODELAY)?;
        // Ok(raw != 0)
        unimplemented!()
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        let mut nonblocking = nonblocking as i32 as *mut libc::c_void;
        cvt(unsafe { ogc_sys::net_ioctl(*self.as_inner(), libc::FIONBIO, nonblocking) })
            .map(|_| ())
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        // let raw: c_int = getsockopt(self, libc::SOL_SOCKET, libc::SO_ERROR)?;
        // if raw == 0 {
        //     Ok(None)
        // } else {
        //     Ok(Some(io::Error::from_raw_os_error(raw as i32)))
        // }
        unimplemented!()
    }
}

impl AsInner<c_int> for Socket {
    fn as_inner(&self) -> &c_int {
        self.0.as_inner()
    }
}

impl FromInner<c_int> for Socket {
    fn from_inner(fd: c_int) -> Socket {
        Socket(FileDesc::new(fd))
    }
}

impl IntoInner<c_int> for Socket {
    fn into_inner(self) -> c_int {
        self.0.into_raw()
    }
}