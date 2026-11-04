use std::path::PathBuf;

use terma_core::dirs::UserDirs;

/// Der AUR-Helfer erwartet Pakete fest unter ~/.cache/terma/pkg, deshalb zählt XDG_CACHE_HOME hier nicht.
#[test]
fn cache_is_always_under_home() {
    // SAFETY: einziger Test in diesem Binary, keine parallelen Zugriffe auf die Umgebung.
    unsafe {
        std::env::set_var("HOME", "/h");
        std::env::set_var("XDG_CACHE_HOME", "/anderswo");
    }
    assert_eq!(UserDirs::from_env().cache, PathBuf::from("/h/.cache/terma"));
}
