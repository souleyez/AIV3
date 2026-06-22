pub(crate) fn escape_markdown_table_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_markdown_table_cell_escapes_pipe_and_flattens_newline() {
        assert_eq!(
            escape_markdown_table_cell("候选人|项目\n交付"),
            "候选人\\|项目 交付"
        );
    }

    #[test]
    fn escape_markdown_table_cell_preserves_plain_text() {
        assert_eq!(escape_markdown_table_cell("普通文本"), "普通文本");
    }
}
