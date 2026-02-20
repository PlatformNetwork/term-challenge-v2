use platform_core::Hotkey;

pub fn parse_hotkey(ss58: &str) -> Option<Hotkey> {
    Hotkey::from_ss58(ss58)
}
