pub(crate) use agere_utils_fs::test_support::PathBufExt;
pub(crate) use agere_utils_fs::test_support::test_path_buf;

pub(crate) fn test_path_display(path: &str) -> String {
    test_path_buf(path).display().to_string()
}
