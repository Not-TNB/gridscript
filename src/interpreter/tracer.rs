pub type Point = (f32, f32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProgramTracer {
    pub x: f32, pub y: f32,
    pub direction: u16 // in degrees, kept in [0,359]
}

impl ProgramTracer {
    pub fn new(pos: Point) -> Self {
        Self { x: pos.0, y: pos.1, direction: 0 }
    }

    /// One step: advance by one unit in current direction.
    /// Returns the segment (start, end) to test for intersection against nodes.
    pub fn advance(&mut self) -> (Point, Point) {
        let start: Point = (self.x, self.y);
        let rad: f32 = (self.direction as f32).to_radians();
        self.x += rad.cos(); self.y += rad.sin();
        (start, (self.x, self.y))
    }

    /// Instantaneous relocation (GOTO).
    pub fn teleport(&mut self, pos: Point) {
        (self.x, self.y) = (pos.0, pos.1);
    }

    /// Setter for direction
    pub fn set_direction(&mut self, degrees: i64) {
        self.direction = degrees.rem_euclid(360) as u16;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataTracer {
    pub row: usize,
    pub col: usize,
}

impl DataTracer {
    pub fn new() -> Self {
        Self { row: 0, col: 0 }
    }

    /// HOME; reset to top left cell
    pub fn home(&mut self) {
        self.row = 0;
        self.col = 0;
    }

    /// NEXT VALUE; one col right, wrap to next row (or first if on last) on overrun
    pub fn next_value(&mut self, width: usize, height: usize) {
        self.col += 1;
        if self.col >= width {
            self.col = 0;
            self.next_row(height);
        }
    }

    /// PREVIOUS VALUE; mirror of NEXT VALUE
    pub fn previous_value(&mut self, width: usize, height: usize) {
        if self.col == 0 {
            if self.row == 0 {
                self.row = height - 1;
            } else {
                self.row -= 1;
            }
            self.col = width - 1;
        } else {
            self.col -= 1;
        }
    }

    /// NEXT ROW; jump to start of next row, wrap to first if on last
    pub fn next_row(&mut self, height: usize) {
        if self.row + 1 >= height {
            self.row = 0;
        } else {
            self.row += 1;
        }
        self.col = 0;
    }

    /// PREVIOUS ROW; mirror of NEXT ROW
    pub fn previous_row(&mut self, height: usize) {
        if self.row == 0 {
            self.row = height - 1;
        } else {
            self.row -= 1;
        }
        self.col = 0;
    }
}

impl Default for DataTracer {
    fn default() -> Self {
        Self::new()
    }
}
