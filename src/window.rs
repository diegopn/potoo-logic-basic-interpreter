use std::fmt;

const ROWS: usize = 27;
const COLUMNS: usize = 79;
const MAX_CELLS: usize = 1_000_000;

/// The default Logic Basic window, represented as a zero-based text grid.
#[derive(Clone, Debug)]
pub struct Window {
    cells: Vec<Vec<char>>,
    row: usize,
    column: usize,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            cells: vec![vec![' '; COLUMNS]; ROWS],
            row: 0,
            column: 0,
        }
    }
}

impl Window {
    /// Returns the current number of text rows.
    pub fn rows(&self) -> usize {
        self.cells.len()
    }

    /// Returns the current number of text columns.
    pub fn columns(&self) -> usize {
        self.cells.first().map_or(0, Vec::len)
    }

    /// Returns the current zero-based text cursor row and column.
    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.column)
    }

    /// Returns the character at the given zero-based row and column.
    pub fn cell(&self, row: usize, column: usize) -> Option<char> {
        self.cells.get(row)?.get(column).copied()
    }

    pub(crate) fn resize(&mut self, rows: usize, columns: usize) -> Result<(), String> {
        if rows == 0 || columns == 0 {
            return Err("Janela resolution must be positive".into());
        }
        rows.checked_mul(columns)
            .filter(|cells| *cells <= MAX_CELLS)
            .ok_or_else(|| format!("Janela resolution cannot exceed {MAX_CELLS} cells"))?;
        let mut resized = Vec::new();
        resized
            .try_reserve_exact(rows)
            .map_err(|error| format!("cannot allocate Janela rows: {error}"))?;
        for row_index in 0..rows {
            let mut row = Vec::new();
            row.try_reserve_exact(columns)
                .map_err(|error| format!("cannot allocate Janela columns: {error}"))?;
            row.resize(columns, ' ');
            if let Some(previous) = self.cells.get(row_index) {
                let preserved = columns.min(previous.len());
                row[..preserved].copy_from_slice(&previous[..preserved]);
            }
            resized.push(row);
        }
        self.row = self.row.min(rows - 1);
        self.column = self.column.min(columns - 1);
        self.cells = resized;
        Ok(())
    }

    pub(crate) fn position(&mut self, row: f64, column: f64) -> Result<(), String> {
        fn coordinate(value: f64, limit: usize, name: &str) -> Result<usize, String> {
            if value.fract() != 0.0 || value < 0.0 || value >= limit as f64 {
                return Err(format!(
                    "Posicione {name} must be an integer from 0 to {}",
                    limit - 1
                ));
            }
            Ok(value as usize)
        }

        self.row = coordinate(row, self.cells.len(), "row")?;
        self.column = coordinate(column, self.columns(), "column")?;
        Ok(())
    }

    pub(crate) fn write(&mut self, text: &str) {
        if let Some(row) = self.cells.get_mut(self.row) {
            for (cell, character) in row[self.column..].iter_mut().zip(text.chars()) {
                *cell = character;
            }
            self.row += 1;
        }
        self.column = 0;
    }

    pub(crate) fn clear(&mut self) {
        for row in &mut self.cells {
            row.fill(' ');
        }
    }
}

impl fmt::Display for Window {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut blank_rows = 0;
        for row in &self.cells {
            let visible = row.iter().rposition(|&character| character != ' ');
            if let Some(last) = visible {
                for _ in 0..blank_rows {
                    writeln!(formatter)?;
                }
                for &character in &row[..=last] {
                    // Keep source and input control characters out of the terminal.
                    write!(
                        formatter,
                        "{}",
                        if character.is_control() {
                            '�'
                        } else {
                            character
                        }
                    )?;
                }
                writeln!(formatter)?;
                blank_rows = 0;
            } else {
                blank_rows += 1;
            }
        }
        Ok(())
    }
}
