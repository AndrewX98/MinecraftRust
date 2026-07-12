pub fn split(s: &str, delimiters: &str) -> Vec<String> {
    assert!(!delimiters.is_empty());
    let mut result = Vec::new();
    let mut base = 0;
    loop {
        let found = s[base..].find(|c| delimiters.contains(c));
        match found {
            Some(pos) => {
                result.push(s[base..base + pos].to_string());
                base = base + pos + 1;
            }
            None => {
                result.push(s[base..].to_string());
                break;
            }
        }
    }
    result
}

pub fn trim(s: &str) -> String {
    s.trim().to_string()
}

pub fn starts_with(s: &str, prefix: &str) -> bool {
    s.starts_with(prefix)
}

pub fn starts_with_char(s: &str, prefix: char) -> bool {
    s.starts_with(prefix)
}

pub fn starts_with_ignore_case(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len()
        && s[..prefix.len()].to_lowercase() == prefix.to_lowercase()
}

pub fn ends_with(s: &str, suffix: &str) -> bool {
    s.ends_with(suffix)
}

pub fn ends_with_char(s: &str, suffix: char) -> bool {
    s.ends_with(suffix)
}

pub fn ends_with_ignore_case(s: &str, suffix: &str) -> bool {
    s.len() >= suffix.len()
        && s[s.len() - suffix.len()..].to_lowercase() == suffix.to_lowercase()
}

pub fn equals_ignore_case(lhs: &str, rhs: &str) -> bool {
    lhs.len() == rhs.len() && lhs.to_lowercase() == rhs.to_lowercase()
}

pub fn string_replace(s: &str, from: &str, to: &str, all: bool) -> String {
    if from.is_empty() {
        return s.to_string();
    }
    let mut result = String::new();
    let mut start_pos = 0;
    loop {
        match s[start_pos..].find(from) {
            Some(pos) => {
                result.push_str(&s[start_pos..start_pos + pos]);
                result.push_str(to);
                start_pos = start_pos + pos + from.len();
                if !all {
                    result.push_str(&s[start_pos..]);
                    break;
                }
            }
            None => {
                result.push_str(&s[start_pos..]);
                break;
            }
        }
    }
    result
}

pub fn join(strings: &[String], separator: &str) -> String {
    strings.join(separator)
}

pub fn join_str(strings: &[&str], separator: &str) -> String {
    strings.join(separator)
}

pub enum ParseBoolResult {
    True,
    False,
    Error,
}

pub fn parse_bool(s: &str) -> ParseBoolResult {
    match s {
        "1" | "y" | "yes" | "on" | "true" => ParseBoolResult::True,
        "0" | "n" | "no" | "off" | "false" => ParseBoolResult::False,
        _ => ParseBoolResult::Error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split() {
        let r = split("a,b,c", ",");
        assert_eq!(r, vec!["a", "b", "c"]);

        let r = split("hello world foo", " ");
        assert_eq!(r, vec!["hello", "world", "foo"]);

        let r = split("single", ",");
        assert_eq!(r, vec!["single"]);

        let r = split("", ",");
        assert_eq!(r, vec![""]);
    }

    #[test]
    fn test_split_multiple_delimiters() {
        let r = split("a,b;c", ",;");
        assert_eq!(r, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_trim() {
        assert_eq!(trim("  hello  "), "hello");
        assert_eq!(trim("hello"), "hello");
        assert_eq!(trim("  "), "");
        assert_eq!(trim(""), "");
    }

    #[test]
    fn test_starts_with() {
        assert!(starts_with("hello", "he"));
        assert!(!starts_with("hello", "wo"));
        assert!(starts_with_char("hello", 'h'));
        assert!(!starts_with_char("hello", 'x'));
    }

    #[test]
    fn test_ends_with() {
        assert!(ends_with("hello", "lo"));
        assert!(!ends_with("hello", "el"));
        assert!(ends_with_char("hello", 'o'));
        assert!(!ends_with_char("hello", 'x'));
    }

    #[test]
    fn test_ignore_case() {
        assert!(starts_with_ignore_case("HelloWorld", "hello"));
        assert!(!starts_with_ignore_case("Hello", "world"));
        assert!(ends_with_ignore_case("HelloWorld", "world"));
        assert!(equals_ignore_case("Hello", "hELLO"));
        assert!(!equals_ignore_case("Hello", "World"));
    }

    #[test]
    fn test_string_replace() {
        assert_eq!(string_replace("hello", "l", "x", false), "hexlo");
        assert_eq!(string_replace("hello", "l", "x", true), "hexxo");
        assert_eq!(string_replace("hello", "", "x", true), "hello");
        assert_eq!(string_replace("hello", "z", "x", true), "hello");
    }

    #[test]
    fn test_join() {
        let v = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(join(&v, ","), "a,b,c");
        assert_eq!(join(&v, " "), "a b c");

        assert_eq!(join_str(&["a", "b", "c"], ","), "a,b,c");
    }

    #[test]
    fn test_parse_bool() {
        assert!(matches!(parse_bool("true"), ParseBoolResult::True));
        assert!(matches!(parse_bool("1"), ParseBoolResult::True));
        assert!(matches!(parse_bool("yes"), ParseBoolResult::True));
        assert!(matches!(parse_bool("y"), ParseBoolResult::True));
        assert!(matches!(parse_bool("on"), ParseBoolResult::True));

        assert!(matches!(parse_bool("false"), ParseBoolResult::False));
        assert!(matches!(parse_bool("0"), ParseBoolResult::False));
        assert!(matches!(parse_bool("no"), ParseBoolResult::False));
        assert!(matches!(parse_bool("n"), ParseBoolResult::False));
        assert!(matches!(parse_bool("off"), ParseBoolResult::False));

        assert!(matches!(parse_bool("invalid"), ParseBoolResult::Error));
        assert!(matches!(parse_bool(""), ParseBoolResult::Error));
    }

    #[test]
    fn test_split_preserves_order() {
        let r = split("path1:path2:path3", ":");
        assert_eq!(r.len(), 3);
        assert_eq!(r[0], "path1");
        assert_eq!(r[1], "path2");
        assert_eq!(r[2], "path3");
    }

    #[test]
    fn test_string_replace_all_none() {
        assert_eq!(string_replace("no match here", "zzz", "yyy", true), "no match here");
    }

    #[test]
    fn test_trim_whitespace() {
        assert_eq!(trim("\t\n hello \r\n"), "hello");
    }
}
