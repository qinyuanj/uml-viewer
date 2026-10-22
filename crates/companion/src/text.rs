//! Pure argv/text quoting shared by the Herdr boundary. A pane command
//! string is parsed by the pane's own shell, so every argument is quoted for
//! that shell (port of `love/uml/domain/text.lua`).

/// Quote one argument for a POSIX shell: single quotes, with embedded
/// single quotes emitted as `'"'"'`.
pub fn shell_word(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_spaces_unicode_and_metacharacters_literally() {
        for (input, expected) in [
            ("plain", "'plain'"),
            ("hi there", "'hi there'"),
            ("$HOME & `x`;", "'$HOME & `x`;'"),
            ("中文 名称", "'中文 名称'"),
            ("", "''"),
            ("trailing\\", "'trailing\\'"),
        ] {
            assert_eq!(expected, shell_word(input), "{input}");
        }
    }

    #[test]
    fn escapes_embedded_single_quotes() {
        assert_eq!("'it'\"'\"'s'", shell_word("it's"));
    }
}
