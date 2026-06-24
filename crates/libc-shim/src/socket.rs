#![allow(non_camel_case_types, unused)]

use std::ffi::c_char;
use crate::types::bionic_addrinfo;
use crate::bionic_conv;

pub unsafe extern "C" fn socket(domain: i32, type_: i32, protocol: i32) -> i32 { libc::socket(domain, type_, protocol) }
pub unsafe extern "C" fn bind(sockfd: i32, addr: *const libc::sockaddr, addrlen: u32) -> i32 { libc::bind(sockfd, addr, addrlen) }
pub unsafe extern "C" fn connect(sockfd: i32, addr: *const libc::sockaddr, addrlen: u32) -> i32 { libc::connect(sockfd, addr, addrlen) }
pub unsafe extern "C" fn listen(sockfd: i32, backlog: i32) -> i32 { libc::listen(sockfd, backlog) }
pub unsafe extern "C" fn accept(sockfd: i32, addr: *mut libc::sockaddr, addrlen: *mut u32) -> i32 { libc::accept(sockfd, addr, addrlen) }
pub unsafe extern "C" fn accept4(sockfd: i32, addr: *mut libc::sockaddr, addrlen: *mut u32, _flags: i32) -> i32 { libc::accept(sockfd, addr, addrlen) }
pub unsafe extern "C" fn send(sockfd: i32, buf: *const std::ffi::c_void, len: usize, flags: i32) -> isize { libc::send(sockfd, buf, len, flags) }
pub unsafe extern "C" fn recv(sockfd: i32, buf: *mut std::ffi::c_void, len: usize, flags: i32) -> isize { libc::recv(sockfd, buf, len, flags) }
pub unsafe extern "C" fn sendto(sockfd: i32, buf: *const std::ffi::c_void, len: usize, flags: i32, dest_addr: *const libc::sockaddr, addrlen: u32) -> isize { libc::sendto(sockfd, buf, len, flags, dest_addr, addrlen) }
pub unsafe extern "C" fn recvfrom(sockfd: i32, buf: *mut std::ffi::c_void, len: usize, flags: i32, src_addr: *mut libc::sockaddr, addrlen: *mut u32) -> isize { libc::recvfrom(sockfd, buf, len, flags, src_addr, addrlen) }
pub unsafe extern "C" fn getsockname(sockfd: i32, addr: *mut libc::sockaddr, addrlen: *mut u32) -> i32 { libc::getsockname(sockfd, addr, addrlen) }
pub unsafe extern "C" fn getpeername(sockfd: i32, addr: *mut libc::sockaddr, addrlen: *mut u32) -> i32 { libc::getpeername(sockfd, addr, addrlen) }
pub unsafe extern "C" fn getsockopt(sockfd: i32, level: i32, optname: i32, optval: *mut std::ffi::c_void, optlen: *mut u32) -> i32 { libc::getsockopt(sockfd, level, optname, optval, optlen) }
pub unsafe extern "C" fn setsockopt(sockfd: i32, level: i32, optname: i32, optval: *const std::ffi::c_void, optlen: u32) -> i32 { libc::setsockopt(sockfd, level, optname, optval, optlen) }
pub unsafe extern "C" fn shutdown(sockfd: i32, how: i32) -> i32 { libc::shutdown(sockfd, how) }
pub unsafe extern "C" fn getsockopt_ip(sockfd: i32, level: i32, optname: i32, optval: *mut std::ffi::c_void, optlen: *mut u32) -> i32 { libc::getsockopt(sockfd, level, optname, optval, optlen) }
pub unsafe extern "C" fn setsockopt_ip(sockfd: i32, level: i32, optname: i32, optval: *const std::ffi::c_void, optlen: u32) -> i32 { libc::setsockopt(sockfd, level, optname, optval, optlen) }
pub unsafe extern "C" fn socketpair(domain: i32, type_: i32, protocol: i32, sv: *mut i32) -> i32 { libc::socketpair(domain, type_, protocol, sv) }
pub unsafe extern "C" fn sendmsg(sockfd: i32, msg: *const libc::msghdr, flags: i32) -> isize { libc::sendmsg(sockfd, msg, flags) }
pub unsafe extern "C" fn recvmsg(sockfd: i32, msg: *mut libc::msghdr, flags: i32) -> isize { libc::recvmsg(sockfd, msg, flags) }
pub unsafe extern "C" fn getaddrinfo(node: *const c_char, service: *const c_char, hints: *const bionic_addrinfo, res: *mut *mut bionic_addrinfo) -> i32 {
    bionic_conv::getaddrinfo_impl(node, service, hints, res)
}
pub unsafe extern "C" fn freeaddrinfo(res: *mut bionic_addrinfo) { bionic_conv::freeaddrinfo_impl(res); }
pub unsafe extern "C" fn getnameinfo(addr: *const libc::sockaddr, addrlen: u32, host: *mut c_char, hostlen: u32, serv: *mut c_char, servlen: u32, flags: i32) -> i32 {
    bionic_conv::getnameinfo_impl(addr, addrlen, host, hostlen, serv, servlen, flags)
}
pub unsafe extern "C" fn if_nametoindex(ifname: *const c_char) -> u32 { libc::if_nametoindex(ifname) }
pub unsafe extern "C" fn if_indextoname(ifindex: u32, ifname: *mut c_char) -> *mut c_char { libc::if_indextoname(ifindex, ifname) }
pub unsafe extern "C" fn sendfile(out_fd: i32, in_fd: i32, offset: *mut i64, count: usize) -> isize { libc::sendfile(out_fd, in_fd, offset, count) }
pub unsafe extern "C" fn inet_pton(af: i32, src: *const c_char, dst: *mut std::ffi::c_void) -> i32 {
    extern "C" { fn inet_pton(af: i32, src: *const c_char, dst: *mut std::ffi::c_void) -> i32; }
    inet_pton(af, src, dst)
}
pub unsafe extern "C" fn inet_ntop(af: i32, src: *const std::ffi::c_void, dst: *mut c_char, size: u32) -> *mut c_char {
    extern "C" { fn inet_ntop(af: i32, src: *const std::ffi::c_void, dst: *mut c_char, size: u32) -> *mut c_char; }
    inet_ntop(af, src, dst, size)
}
pub unsafe extern "C" fn inet_addr(cp: *const c_char) -> u32 {
    extern "C" { fn inet_addr(cp: *const c_char) -> u32; }
    inet_addr(cp)
}
pub unsafe extern "C" fn inet_ntoa(in_: libc::in_addr) -> *mut c_char {
    extern "C" { fn inet_ntoa(in_: libc::in_addr) -> *mut c_char; }
    inet_ntoa(in_)
}
pub unsafe extern "C" fn gethostbyaddr(addr: *const std::ffi::c_void, len: u32, type_: i32) -> *mut std::ffi::c_void {
    extern "C" { fn gethostbyaddr(addr: *const std::ffi::c_void, len: u32, type_: i32) -> *mut std::ffi::c_void; }
    gethostbyaddr(addr, len, type_)
}
pub unsafe extern "C" fn gethostbyname(name: *const c_char) -> *mut std::ffi::c_void {
    extern "C" { fn gethostbyname(name: *const c_char) -> *mut std::ffi::c_void; }
    gethostbyname(name)
}
pub unsafe extern "C" fn gethostbyname2(name: *const c_char, af: i32) -> *mut std::ffi::c_void {
    extern "C" { fn gethostbyname2(name: *const c_char, af: i32) -> *mut std::ffi::c_void; }
    gethostbyname2(name, af)
}
pub unsafe extern "C" fn gethostent() -> *mut std::ffi::c_void {
    extern "C" { fn gethostent() -> *mut std::ffi::c_void; }
    gethostent()
}
pub unsafe extern "C" fn gai_strerror(errcode: i32) -> *const c_char {
    extern "C" { fn gai_strerror(errcode: i32) -> *const c_char; }
    gai_strerror(errcode)
}
pub unsafe extern "C" fn if_nameindex() -> *mut libc::if_nameindex { libc::if_nameindex() }
pub unsafe extern "C" fn if_freenameindex(ptr: *mut libc::if_nameindex) { libc::if_freenameindex(ptr); }

pub unsafe extern "C" fn __recvfrom_chk(sockfd: i32, buf: *mut std::ffi::c_void, len: usize, _buf_len: usize, flags: i32, src_addr: *mut libc::sockaddr, addrlen: *mut u32) -> isize { libc::recvfrom(sockfd, buf, len, flags, src_addr, addrlen) }
pub unsafe extern "C" fn __sendto_chk(sockfd: i32, buf: *const std::ffi::c_void, len: usize, _buf_len: usize, flags: i32, dest_addr: *const libc::sockaddr, addrlen: u32) -> isize { libc::sendto(sockfd, buf, len, flags, dest_addr, addrlen) }
pub unsafe extern "C" fn inet_network(cp: *const c_char) -> u32 {
    extern "C" { fn inet_network(cp: *const c_char) -> u32; }
    inet_network(cp)
}
pub unsafe extern "C" fn inet_lnaof(in_: libc::in_addr) -> u32 {
    extern "C" { fn inet_lnaof(in_: libc::in_addr) -> u32; }
    inet_lnaof(in_)
}
pub unsafe extern "C" fn inet_makeaddr(net: u32, host: u32) -> libc::in_addr {
    extern "C" { fn inet_makeaddr(net: u32, host: u32) -> libc::in_addr; }
    inet_makeaddr(net, host)
}
pub unsafe extern "C" fn inet_netof(in_: libc::in_addr) -> u32 {
    extern "C" { fn inet_netof(in_: libc::in_addr) -> u32; }
    inet_netof(in_)
}
pub unsafe extern "C" fn __cmsg_nxthdr(hdr: *mut libc::msghdr, cmsg: *mut libc::cmsghdr) -> *mut libc::cmsghdr {
    extern "C" { fn __cmsg_nxthdr(hdr: *mut libc::msghdr, cmsg: *mut libc::cmsghdr) -> *mut libc::cmsghdr; }
    __cmsg_nxthdr(hdr, cmsg)
}
