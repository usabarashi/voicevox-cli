/// Asks the platform allocator to return unused process memory to the OS.
///
/// This is a best-effort RSS reduction hook for large native allocations released
/// by VOICEVOX Core. The return value is the allocator-reported released byte
/// count when the platform exposes one.
#[cfg(target_os = "macos")]
pub fn release_unused_allocator_memory() -> usize {
    use libc::size_t;
    use std::ffi::c_void;

    unsafe extern "C" {
        fn malloc_zone_pressure_relief(zone: *mut c_void, goal: size_t) -> size_t;
    }

    // A null zone asks libmalloc to examine all zones. A zero goal requests as
    // much relief as the allocator can currently provide.
    unsafe { malloc_zone_pressure_relief(std::ptr::null_mut(), 0) as usize }
}

#[cfg(not(target_os = "macos"))]
pub fn release_unused_allocator_memory() -> usize {
    0
}
