/// Region used for locale-aware remote fetches.
///
/// `1` = China (system locale contains `zh`); `2` = International (default).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Region {
    Cn = 1,
    Intl = 2,
}

impl Region {
    pub(crate) fn query_value(self) -> u8 {
        self as u8
    }
}

pub(crate) fn detect_region() -> Region {
    if cfg!(test)
        && let Ok(val) = std::env::var("AGERE_REGION_TEST")
    {
        return match val.as_str() {
            "1" | "cn" | "CN" | "zh" | "zh-CN" | "zh-Hans" => Region::Cn,
            _ => Region::Intl,
        };
    }
    let lower = sys_locale::get_locale().unwrap_or_default();
    let lower = lower.to_ascii_lowercase();
    if lower == "zh-cn"
        || lower.starts_with("zh-cn-")
        || lower == "zh-hans"
        || lower.starts_with("zh-hans-")
    {
        Region::Cn
    } else {
        Region::Intl
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial(agere_region_test_env)]
    fn detect_region_test_override() {
        unsafe { std::env::set_var("AGERE_REGION_TEST", "cn") };
        assert_eq!(detect_region(), Region::Cn);
        unsafe { std::env::set_var("AGERE_REGION_TEST", "en_US") };
        assert_eq!(detect_region(), Region::Intl);
        unsafe { std::env::remove_var("AGERE_REGION_TEST") };
        let _ = detect_region();
    }
}
