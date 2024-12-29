//! Game logic for Snake

#![no_std]
#![deny(missing_docs)]
#![deny(unsafe_code)]

use core::fmt::Write;

use neotron_sdk::console;

/// Represents the Snake application
///
/// An application can play multiple games.
pub struct App {
    game: Game,
    width: u8,
    height: u8,
    stdout: neotron_sdk::File,
    stdin: neotron_sdk::File,
}

impl App {
    /// Make a new snake application.
    ///
    /// You can give the screen size in characters. There will be a border and
    /// the board will be two units smaller in each axis.
    pub const fn new(width: u8, height: u8) -> App {
        App {
            game: Game::new(width - 2, height - 2, console::Position { row: 1, col: 1 }),
            width,
            height,
            stdout: neotron_sdk::stdout(),
            stdin: neotron_sdk::stdin(),
        }
    }

    /// Play multiple games of snake.
    ///
    /// Loops playing games and printing scores.
    pub fn play(&mut self) {
        console::cursor_off(&mut self.stdout);
        self.clear_screen();
        self.title_screen();

        let mut seed: u16 = 0x4f34;

        'outer: loop {
            'inner: loop {
                let key = self.wait_for_key();
                seed = seed.wrapping_add(1);
                if key == b'q' || key == b'Q' {
                    break 'outer;
                }
                if key == b'p' || key == b'P' {
                    break 'inner;
                }
            }

            self.clear_screen();

            neotron_sdk::srand(seed);

            let score = self.game.play(&mut self.stdin, &mut self.stdout);

            self.winning_message(score);
        }

        // show cursor
        console::cursor_on(&mut self.stdout);
        self.clear_screen();
    }

    /// Clear the screen and draw the board.
    fn clear_screen(&mut self) {
        console::set_sgr(&mut self.stdout, [console::SgrParam::Reset]);
        console::clear_screen(&mut self.stdout);
        console::set_sgr(
            &mut self.stdout,
            [
                console::SgrParam::Bold,
                console::SgrParam::FgYellow,
                console::SgrParam::BgBlack,
            ],
        );
        console::move_cursor(&mut self.stdout, console::Position::origin());
        let _ = self.stdout.write_char('╔');
        for _ in 1..self.width - 1 {
            let _ = self.stdout.write_char('═');
        }
        let _ = self.stdout.write_char('╗');
        console::move_cursor(
            &mut self.stdout,
            console::Position {
                row: self.height - 1,
                col: 0,
            },
        );
        let _ = self.stdout.write_char('╚');
        for _ in 1..self.width - 1 {
            let _ = self.stdout.write_char('═');
        }
        let _ = self.stdout.write_char('╝');
        for row in 1..self.height - 1 {
            console::move_cursor(&mut self.stdout, console::Position { row, col: 0 });
            let _ = self.stdout.write_char('║');
            console::move_cursor(
                &mut self.stdout,
                console::Position {
                    row,
                    col: self.width - 1,
                },
            );
            let _ = self.stdout.write_char('║');
        }
        console::set_sgr(&mut self.stdout, [console::SgrParam::Reset]);
    }

    /// Show the title screen
    fn title_screen(&mut self) {
        console::set_sgr(&mut self.stdout, [console::SgrParam::Reset]);
        let message = "Neotron Snake by theJPster";
        let pos = console::Position {
            row: self.height / 2,
            col: (self.width - message.chars().count() as u8) / 2,
        };
        console::move_cursor(&mut self.stdout, pos);
        let _ = self.stdout.write_str(message);
        let message = "Q to Quit | 'P' to Play";
        let pos = console::Position {
            row: pos.row + 1,
            col: (self.width - message.chars().count() as u8) / 2,
        };
        console::move_cursor(&mut self.stdout, pos);
        let _ = self.stdout.write_str(message);
    }

    /// Spin until a key is pressed
    fn wait_for_key(&mut self) -> u8 {
        loop {
            let mut buffer = [0u8; 1];
            if let Ok(1) = self.stdin.read(&mut buffer) {
                return buffer[0];
            }
            neotron_sdk::delay(core::time::Duration::from_millis(10));
        }
    }

    /// Print the game over message with the given score
    fn winning_message(&mut self, score: u32) {
        console::set_sgr(&mut self.stdout, [console::SgrParam::Reset]);
        let pos = console::Position {
            row: self.height / 2,
            col: (self.width - 13u8) / 2,
        };
        console::move_cursor(&mut self.stdout, pos);
        let _ = writeln!(self.stdout, "Score: {:06}", score);
        let message = "Q to Quit | 'P' to Play";
        let pos = console::Position {
            row: pos.row + 1,
            col: (self.width - message.chars().count() as u8) / 2,
        };
        console::move_cursor(&mut self.stdout, pos);
        let _ = self.stdout.write_str(message);
    }
}

/// Something we can send to the ANSI console
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Piece {
    Head,
    Food,
    Body,
}

impl Piece {
    /// Get the Unicode char for this piece
    fn get_char(self) -> char {
        match self {
            Piece::Body => '▓',
            Piece::Head => '█',
            Piece::Food => '▲',
        }
    }

    /// Get the ANSI colour for this piece
    fn get_colour(self) -> console::SgrParam {
        match self {
            Piece::Body => console::SgrParam::FgMagenta,
            Piece::Head => console::SgrParam::FgYellow,
            Piece::Food => console::SgrParam::FgGreen,
        }
    }
}

/// Represents one game of Snake
struct Game {
    board: Board<{ Self::MAX_WIDTH }, { Self::MAX_HEIGHT }>,
    width: u8,
    height: u8,
    offset: console::Position,
    head: console::Position,
    tail: console::Position,
    direction: Direction,
    score: u32,
    digesting: u32,
    tick_interval_ms: u16,
}

impl Game {
    /// The maximum width board we can handle
    pub const MAX_WIDTH: usize = 78;
    /// The maximum height board we can handle
    pub const MAX_HEIGHT: usize = 23;
    /// How many ms per tick do we start at?
    const STARTING_TICK: u16 = 100;

    /// Make a new game.
    ///
    /// Give the width and the height of the game board, and where on the screen
    /// the board should be located.
    const fn new(width: u8, height: u8, offset: console::Position) -> Game {
        Game {
            board: Board::new(),
            width,
            height,
            offset,
            head: console::Position { row: 0, col: 0 },
            tail: console::Position { row: 0, col: 0 },
            direction: Direction::Up,
            score: 0,
            digesting: 3,
            tick_interval_ms: Self::STARTING_TICK,
        }
    }

    /// Play a game
    fn play(&mut self, stdin: &mut neotron_sdk::File, stdout: &mut neotron_sdk::File) -> u32 {
        // Reset score and speed, and start with a bit of snake
        self.score = 0;
        self.tick_interval_ms = Self::STARTING_TICK;
        self.digesting = 2;
        // Wipe board
        self.board.reset();
        // Add offset snake
        self.head = console::Position {
            row: self.height / 4,
            col: self.width / 4,
        };
        self.tail = self.head;
        self.board.store_body(self.head, self.direction);
        self.write_at(stdout, self.head, Some(Piece::Head));
        // Add random food
        let pos = self.random_empty_position();
        self.board.store_food(pos);
        self.write_at(stdout, pos, Some(Piece::Food));

        'game: loop {
            // Wait for frame tick
            neotron_sdk::delay(core::time::Duration::from_millis(
                self.tick_interval_ms as u64,
            ));

            // 1 point for not being dead
            self.score += 1;

            // Read input
            'input: loop {
                let mut buffer = [0u8; 1];
                if let Ok(1) = stdin.read(&mut buffer) {
                    match buffer[0] {
                        b'w' | b'W' => {
                            // Going up
                            if self.direction.is_horizontal() {
                                self.direction = Direction::Up;
                            }
                        }
                        b's' | b'S' => {
                            // Going down
                            if self.direction.is_horizontal() {
                                self.direction = Direction::Down;
                            }
                        }
                        b'a' | b'A' => {
                            // Going left
                            if self.direction.is_vertical() {
                                self.direction = Direction::Left;
                            }
                        }
                        b'd' | b'D' => {
                            // Going right
                            if self.direction.is_vertical() {
                                self.direction = Direction::Right;
                            }
                        }
                        b'q' | b'Q' => {
                            // Quit game
                            break 'game;
                        }
                        _ => {
                            // ignore
                        }
                    }
                } else {
                    break 'input;
                }
            }

            // Mark which way we're going in the old head position
            self.board.store_body(self.head, self.direction);
            self.write_at(stdout, self.head, Some(Piece::Body));

            // Update head position
            match self.direction {
                Direction::Up => {
                    if self.head.row == 0 {
                        break 'game;
                    }
                    self.head.row -= 1;
                }
                Direction::Down => {
                    if self.head.row == self.height - 1 {
                        break 'game;
                    }
                    self.head.row += 1;
                }
                Direction::Left => {
                    if self.head.col == 0 {
                        break 'game;
                    }
                    self.head.col -= 1;
                }
                Direction::Right => {
                    if self.head.col == self.width - 1 {
                        break 'game;
                    }
                    self.head.col += 1;
                }
            }

            // Check what we just ate
            //   - Food => get longer
            //   - Ourselves => die
            if self.board.is_food(self.head) {
                // yum
                self.score += 10;
                self.digesting = 2;
                // Drop 10% on the tick interval
                self.tick_interval_ms *= 9;
                self.tick_interval_ms /= 10;
                if self.tick_interval_ms < 5 {
                    // Maximum speed
                    self.tick_interval_ms = 5;
                }
                // Add random food
                let pos = self.random_empty_position();
                self.board.store_food(pos);
                self.write_at(stdout, pos, Some(Piece::Food));
            } else if self.board.is_body(self.head) {
                // oh no
                break 'game;
            }

            // Write the new head
            self.board.store_body(self.head, self.direction);
            self.write_at(stdout, self.head, Some(Piece::Head));

            if self.digesting == 0 {
                let old_tail = self.tail;
                match self.board.remove_piece(self.tail) {
                    Some(Direction::Up) => {
                        self.tail.row -= 1;
                    }
                    Some(Direction::Down) => {
                        self.tail.row += 1;
                    }
                    Some(Direction::Left) => {
                        self.tail.col -= 1;
                    }
                    Some(Direction::Right) => {
                        self.tail.col += 1;
                    }
                    None => {
                        panic!("Bad game state");
                    }
                }
                self.write_at(stdout, old_tail, None);
            } else {
                self.digesting -= 1;
            }
        }

        self.score
    }

    /// Draw a piece on the ANSI console at the given location
    fn write_at(
        &self,
        console: &mut neotron_sdk::File,
        position: console::Position,
        piece: Option<Piece>,
    ) {
        let adjusted_position = console::Position {
            row: position.row + self.offset.row,
            col: position.col + self.offset.col,
        };
        console::move_cursor(console, adjusted_position);
        if let Some(piece) = piece {
            let colour = piece.get_colour();
            let ch = piece.get_char();
            console::set_sgr(console, [colour]);
            let _ = console.write_char(ch);
        } else {
            let _ = console.write_char(' ');
        }
    }

    /// Find a spot on the board that is empty
    fn random_empty_position(&mut self) -> console::Position {
        loop {
            // This isn't equally distributed. I don't really care.
            let pos = console::Position {
                row: (neotron_sdk::rand() % self.height as u16) as u8,
                col: (neotron_sdk::rand() % self.width as u16) as u8,
            };
            if self.board.is_empty(pos) {
                return pos;
            }
        }
    }
}

/// A direction in which a body piece can face
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Direction {
    /// Facing up
    Up,
    /// Facing down
    Down,
    /// Facing left
    Left,
    /// Facing right
    Right,
}

impl Direction {
    /// Is this left/right?
    fn is_horizontal(self) -> bool {
        self == Direction::Left || self == Direction::Right
    }

    /// Is this up/down?
    fn is_vertical(self) -> bool {
        self == Direction::Up || self == Direction::Down
    }
}

/// Something we can put on a board.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
enum BoardPiece {
    /// Nothing here
    Empty,
    /// A body, and the next piece is up
    Up,
    /// A body, and the next piece is down
    Down,
    /// A body, and the next piece is left
    Left,
    /// A body, and the next piece is right
    Right,
    /// A piece of food
    Food,
}

/// Tracks where the snake is in 2D space.
///
/// We do this rather than maintain a Vec of body positions and a Vec of food
/// positions because it's fixed size and faster to see if a space is empty, or
/// body, or food.
struct Board<const WIDTH: usize, const HEIGHT: usize> {
    cells: [[BoardPiece; WIDTH]; HEIGHT],
}

impl<const WIDTH: usize, const HEIGHT: usize> Board<WIDTH, HEIGHT> {
    /// Make a new empty board
    const fn new() -> Board<WIDTH, HEIGHT> {
        Board {
            cells: [[BoardPiece::Empty; WIDTH]; HEIGHT],
        }
    }

    /// Clean up the board so everything is empty.
    fn reset(&mut self) {
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                self.cells[y][x] = BoardPiece::Empty;
            }
        }
    }

    /// Store a body piece on the board, based on which way it is facing
    fn store_body(&mut self, position: console::Position, direction: Direction) {
        self.cells[usize::from(position.row)][usize::from(position.col)] = match direction {
            Direction::Up => BoardPiece::Up,
            Direction::Down => BoardPiece::Down,
            Direction::Left => BoardPiece::Left,
            Direction::Right => BoardPiece::Right,
        }
    }

    /// Put some food on the board
    fn store_food(&mut self, position: console::Position) {
        self.cells[usize::from(position.row)][usize::from(position.col)] = BoardPiece::Food;
    }

    /// Is there food on the board here?
    fn is_food(&mut self, position: console::Position) -> bool {
        self.cells[usize::from(position.row)][usize::from(position.col)] == BoardPiece::Food
    }

    /// Is there body on the board here?
    fn is_body(&mut self, position: console::Position) -> bool {
        let cell = self.cells[usize::from(position.row)][usize::from(position.col)];
        cell == BoardPiece::Up
            || cell == BoardPiece::Down
            || cell == BoardPiece::Left
            || cell == BoardPiece::Right
    }

    /// Is this position empty?
    fn is_empty(&mut self, position: console::Position) -> bool {
        self.cells[usize::from(position.row)][usize::from(position.col)] == BoardPiece::Empty
    }

    /// Remove a piece from the board
    fn remove_piece(&mut self, position: console::Position) -> Option<Direction> {
        let old = match self.cells[usize::from(position.row)][usize::from(position.col)] {
            BoardPiece::Up => Some(Direction::Up),
            BoardPiece::Down => Some(Direction::Down),
            BoardPiece::Left => Some(Direction::Left),
            BoardPiece::Right => Some(Direction::Right),
            _ => None,
        };
        self.cells[usize::from(position.row)][usize::from(position.col)] = BoardPiece::Empty;
        old
    }
}
