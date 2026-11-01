//! Keeps the Mac awake while the daemon sends DMX (OUT-38, OUT-39).

/// OUT-38: hold the assertion until the process exits.
#[cfg(target_os = "macos")]
pub fn hold() -> Result<(), i32> {
    use core_foundation_sys::base::{kCFAllocatorDefault, CFRelease};
    use core_foundation_sys::string::{
        kCFStringEncodingUTF8, CFStringCreateWithCString, CFStringRef,
    };

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOPMAssertionCreateWithName(
            assertion_type: CFStringRef,
            level: u32,
            name: CFStringRef,
            id: *mut u32,
        ) -> i32;
    }
    const LEVEL_ON: u32 = 255; // kIOPMAssertionLevelOn

    // SAFETY: the C string literals are NUL-terminated and live for 'static.
    let kind = unsafe {
        CFStringCreateWithCString(
            kCFAllocatorDefault,
            c"PreventUserIdleSystemSleep".as_ptr(),
            kCFStringEncodingUTF8,
        )
    };
    // SAFETY: as above.
    let name = unsafe {
        CFStringCreateWithCString(
            kCFAllocatorDefault,
            c"CMDMX DMX output".as_ptr(),
            kCFStringEncodingUTF8,
        )
    };
    let mut id = 0u32;
    let code = if kind.is_null() || name.is_null() {
        -1
    } else {
        // SAFETY: both strings are valid CFStrings and `id` is a valid out pointer.
        unsafe { IOPMAssertionCreateWithName(kind, LEVEL_ON, name, &mut id) }
    };
    // SAFETY: each non-null string is owned by us (Create rule) and not used again.
    unsafe {
        if !kind.is_null() {
            CFRelease(kind.cast());
        }
        if !name.is_null() {
            CFRelease(name.cast());
        }
    }
    // The assertion is never released: it lasts until the process exits, when macOS drops it.
    if code == 0 {
        Ok(())
    } else {
        Err(code)
    }
}

/// OUT-39: other platforms do nothing.
#[cfg(not(target_os = "macos"))]
pub fn hold() -> Result<(), i32> {
    Ok(())
}

/// OUT-39: a failure is one log line and one warning; the daemon carries on.
pub fn keep_awake(hold: impl FnOnce() -> Result<(), i32>, pending: &mut Vec<String>) {
    if let Err(code) = hold() {
        let msg = format!(
            "Cannot keep the Mac awake (IOKit error {code:#x}): if it sleeps, DMX output stops"
        );
        eprintln!("warning: {msg}");
        pending.push(msg);
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn out_39_a_failed_assertion_is_one_warning() {
        let mut pending = vec!["earlier".to_string()];
        keep_awake(|| Err(-536_870_212), &mut pending);
        assert_eq!(pending.len(), 2, "{pending:?}");
        assert_eq!(pending[0], "earlier");
        assert!(pending[1].contains("awake"), "{pending:?}");
    }

    #[test]
    fn out_39_a_held_assertion_gives_no_warning() {
        let mut pending = Vec::new();
        keep_awake(|| Ok(()), &mut pending);
        assert!(pending.is_empty(), "{pending:?}");
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn out_39_other_platforms_do_nothing() {
        assert_eq!(hold(), Ok(()));
    }
}
