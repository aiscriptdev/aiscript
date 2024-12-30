#[allow(unused)]
pub fn normalize_path(path: &str) -> String {
    let mut result = String::with_capacity(path.len());
    let mut last_was_slash = false;

    for c in path.chars() {
        if c == '/' {
            if !last_was_slash {
                result.push(c);
            }
            last_was_slash = true;
        } else {
            result.push(c);
            last_was_slash = false;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("//path"), "/path");
        assert_eq!(normalize_path("/path///a"), "/path/a");
        assert_eq!(normalize_path("///"), "/");
        assert_eq!(normalize_path("path"), "path");
        assert_eq!(normalize_path("/path/to///file"), "/path/to/file");
        assert_eq!(normalize_path("//path//to////dir/"), "/path/to/dir/");
    }
}
