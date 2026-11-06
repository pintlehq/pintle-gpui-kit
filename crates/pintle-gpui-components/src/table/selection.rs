use gpui_kit::SharedString;
use std::collections::BTreeSet;

/// Stable cell identity. Keys refer to source data, never sorted row indices.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellAddress {
    pub row: SharedString,
    pub column: SharedString,
}
impl CellAddress {
    pub fn new(row: impl Into<SharedString>, column: impl Into<SharedString>) -> Self {
        Self {
            row: row.into(),
            column: column.into(),
        }
    }
}
/// A set of selected cells with a keyboard cursor and range anchor.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CellSelection {
    pub cells: BTreeSet<CellAddress>,
    pub active: Option<CellAddress>,
    pub anchor: Option<CellAddress>,
}
impl CellSelection {
    pub fn select(&mut self, cell: CellAddress) {
        self.cells.clear();
        self.cells.insert(cell.clone());
        self.anchor = Some(cell.clone());
        self.active = Some(cell);
    }
    pub fn contains(&self, cell: &CellAddress) -> bool {
        self.cells.contains(cell)
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Serialize selected cells in display order. Disjoint rows with the same
/// contiguous columns, or disjoint columns with the same contiguous rows, pack
/// together. Incompatible shapes fail without changing the system clipboard.
pub fn selection_tsv(
    rows: &[SharedString],
    columns: &[SharedString],
    selection: &CellSelection,
    mut value: impl FnMut(&CellAddress) -> String,
) -> Result<String, &'static str> {
    let used_rows: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| selection.cells.iter().any(|c| &c.row == *row))
        .collect();
    let used_cols: Vec<_> = columns
        .iter()
        .enumerate()
        .filter(|(_, col)| selection.cells.iter().any(|c| &c.column == *col))
        .collect();
    if used_rows.is_empty() || used_cols.is_empty() {
        return Err("Select cells to copy");
    }
    let contiguous = |indices: Vec<usize>| indices.windows(2).all(|pair| pair[1] == pair[0] + 1);
    if !contiguous(used_rows.iter().map(|(i, _)| *i).collect())
        && !contiguous(used_cols.iter().map(|(i, _)| *i).collect())
    {
        return Err("Copy requires ranges with matching rows or columns");
    }
    let mut lines = Vec::new();
    for (_, row) in used_rows {
        let mut line = Vec::new();
        for (_, col) in &used_cols {
            let address = CellAddress::new(row.clone(), (*col).clone());
            if !selection.contains(&address) {
                return Err("Copy requires ranges with matching rows or columns");
            }
            let text = value(&address);
            line.push(if text.contains(['\t', '\r', '\n', '"']) {
                format!("\"{}\"", text.replace('"', "\"\""))
            } else {
                text
            });
        }
        lines.push(line.join("\t"));
    }
    Ok(lines.join("\r\n"))
}

/// Parse rectangular Excel-style TSV, including quoted tabs/newlines and
/// doubled quotes. A final line ending terminates the last row, not a new row.
/// Reject malformed/ragged input instead of silently shifting cell values.
pub fn parse_tsv(text: &str) -> Result<Vec<Vec<String>>, &'static str> {
    if text.is_empty() {
        return Err("Clipboard contains no text cells");
    }
    let mut chars = text.chars().peekable();
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut closed = false;
    let mut ended_row = false;
    while let Some(ch) = chars.next() {
        if quoted {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                    closed = true;
                }
            } else {
                field.push(ch);
            }
            continue;
        }
        if closed && !matches!(ch, '\t' | '\r' | '\n') {
            return Err("Unexpected text after a quoted cell");
        }
        ended_row = false;
        match ch {
            '"' if field.is_empty() && !closed => quoted = true,
            '\t' => {
                row.push(std::mem::take(&mut field));
                closed = false;
            }
            '\r' | '\n' => {
                if ch == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                closed = false;
                ended_row = true;
            }
            _ => field.push(ch),
        }
    }
    if quoted {
        return Err("Unclosed quoted cell");
    }
    if !ended_row {
        row.push(field);
        rows.push(row);
    }
    let width = rows[0].len();
    if rows.iter().any(|row| row.len() != width) {
        return Err("Paste requires the same number of columns in every row");
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paste_tsv_quotes_unicode_empty_cells_and_line_endings() {
        assert_eq!(
            parse_tsv("ไทย\t\"line\n\"\"two\"\"\"\r\nJosé\t\r\n").unwrap(),
            vec![vec!["ไทย", "line\n\"two\""], vec!["José", ""]]
        );
        assert_eq!(
            parse_tsv("a\tb\nc\td").unwrap(),
            vec![vec!["a", "b"], vec!["c", "d"]]
        );
        assert_eq!(
            parse_tsv("\t\r\n\t").unwrap(),
            vec![vec!["", ""], vec!["", ""]]
        );
        assert_eq!(parse_tsv("\"\"").unwrap(), vec![vec![""]]);
        assert_eq!(parse_tsv("\n").unwrap(), vec![vec![""]]);
        assert_eq!(parse_tsv("a\rb\r").unwrap(), vec![vec!["a"], vec!["b"]]);
        for invalid in ["", "a\tb\nc", "\"unclosed", "\"a\"garbage"] {
            assert!(parse_tsv(invalid).is_err(), "{invalid:?}");
        }
    }
    #[test]
    fn copy_shapes_and_escaping() {
        let rows = ["r1", "r2", "r3"].map(SharedString::from);
        let cols = ["a", "b", "c"].map(SharedString::from);
        let mut s = CellSelection::default();
        for r in ["r1", "r3"] {
            for c in ["a", "b"] {
                s.cells.insert(CellAddress::new(r, c));
            }
        }
        let tsv = selection_tsv(&rows, &cols, &s, |c| {
            if c.column == "a" {
                "ไทย\t\"x\"".into()
            } else {
                "line\n2".into()
            }
        })
        .unwrap();
        assert_eq!(
            tsv,
            "\"ไทย\t\"\"x\"\"\"\t\"line\n2\"\r\n\"ไทย\t\"\"x\"\"\"\t\"line\n2\""
        );
        assert_eq!(
            parse_tsv(&tsv).unwrap(),
            vec![vec!["ไทย\t\"x\"", "line\n2"], vec!["ไทย\t\"x\"", "line\n2"]]
        );
        s.cells.remove(&CellAddress::new("r3", "b"));
        assert!(selection_tsv(&rows, &cols, &s, |_| String::new()).is_err());
        s.select(CellAddress::new("r1", "a"));
        s.cells.insert(CellAddress::new("r1", "c"));
        assert_eq!(
            selection_tsv(&rows, &cols, &s, |c| c.column.to_string()).unwrap(),
            "a\tc"
        );
        s.cells.insert(CellAddress::new("r3", "a"));
        s.cells.insert(CellAddress::new("r3", "c"));
        assert!(selection_tsv(&rows, &cols, &s, |_| String::new()).is_err());
    }
}
