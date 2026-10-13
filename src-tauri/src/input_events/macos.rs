//! Posizione e pulsanti del mouse su macOS tramite CoreGraphics.
//! Nessun permesso aggiuntivo richiesto (niente Accessibilità né Monitoraggio input).

use std::ffi::c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventCreate(source: *const c_void) -> *mut c_void;
    fn CGEventGetLocation(event: *mut c_void) -> CGPoint;
    fn CGEventSourceButtonState(state_id: i32, button: u32) -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *const c_void);
}

/// kCGEventSourceStateCombinedSessionState
const COMBINED_SESSION_STATE: i32 = 0;

/// Posizione del cursore in punti globali (origine in alto a sinistra dello
/// schermo principale), come `SCDisplay::frame`.
pub fn cursor_location() -> Option<(f64, f64)> {
    // SAFETY: CGEventCreate(NULL) restituisce un nuovo evento (o NULL) che
    // rilasciamo con CFRelease dopo averne letto la posizione.
    unsafe {
        let ev = CGEventCreate(std::ptr::null());
        if ev.is_null() {
            return None;
        }
        let p = CGEventGetLocation(ev);
        CFRelease(ev);
        Some((p.x, p.y))
    }
}

/// Stato di sinistro, destro e centrale.
pub fn buttons() -> [bool; 3] {
    // SAFETY: funzione pura di sola lettura dello stato dei pulsanti.
    unsafe {
        [
            CGEventSourceButtonState(COMBINED_SESSION_STATE, 0),
            CGEventSourceButtonState(COMBINED_SESSION_STATE, 1),
            CGEventSourceButtonState(COMBINED_SESSION_STATE, 2),
        ]
    }
}
