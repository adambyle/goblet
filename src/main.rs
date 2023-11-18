use std::{cmp::Ordering, hint::black_box, mem::MaybeUninit, thread::current};

const NUM_SIZES: usize = 4;
const NUM_EACH_SIZE: i32 = 3;
const BOARD_DIM: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Color {
    Empty,
    White,
    Black,
}

impl Color {
    fn other(self) -> Color {
        if self == Color::White {
            Color::Black
        } else {
            Color::White
        }
    }
}

#[derive(Clone)]
struct Stack {
    /// The pieces are stored in an array of sizes, where if an element of the array
    /// is a non-empty color, then a piece of that color with the size equal to the index
    /// is present in the stack.
    pieces: [Color; NUM_SIZES],
}

impl Stack {
    fn empty() -> Stack {
        Stack {
            pieces: [Color::Empty; NUM_SIZES],
        }
    }

    /// Return the next valid space where a piece would go.
    /// If this value is equal to NUM_SIZES, the stack is full.
    fn top(&self) -> usize {
        for i in (0..NUM_SIZES).rev() {
            if self.pieces[i] != Color::Empty {
                return i + 1;
            }
        }
        return 0;
    }

    fn top_color(&self) -> Color {
        for color in self.pieces.into_iter().rev() {
            if color != Color::Empty {
                return color;
            }
        }
        return Color::Empty;
    }
}

impl Default for Stack {
    fn default() -> Self {
        Stack::empty()
    }
}

#[derive(Clone)]
struct Board {
    contents: [[Stack; BOARD_DIM]; BOARD_DIM],
}

impl Board {
    fn empty() -> Board {
        Board {
            contents: Default::default(),
        }
    }
}

#[derive(Clone)]
struct GameState {
    // White and black pieces store how many of each size there are,
    // where the index is the size.
    white_pieces: [i32; NUM_SIZES],
    black_pieces: [i32; NUM_SIZES],

    board: Board,
    turn: Color,
}

impl GameState {
    fn new() -> GameState {
        GameState {
            white_pieces: [NUM_EACH_SIZE; NUM_SIZES],
            black_pieces: [NUM_EACH_SIZE; NUM_SIZES],
            board: Board::empty(),
            turn: Color::White,
        }
    }

    fn next_turn(&mut self) {
        self.turn = self.turn.other();
    }

    fn apply_move(&mut self, game_move: GameMove) {
        match game_move {
            GameMove::Move {
                source: (source_row, source_col),
                dest: (dest_row, dest_col),
            } => {
                self.board.contents[dest_row][dest_col].pieces
                    [self.board.contents[source_row][source_col].top() - 1] = self.turn;
            }
            GameMove::Place {
                size,
                dest: (dest_row, dest_col),
            } => {
                self.board.contents[dest_row][dest_col].pieces[size] = self.turn;
            }
        }
        self.next_turn();
    }

    fn branch(&self) -> Vec<(GameMove, GameState)> {
        let mut children = Vec::new();

        let available_pieces = if self.turn == Color::White {
            self.white_pieces
        } else {
            self.black_pieces
        };

        let tops: [[usize; BOARD_DIM]; BOARD_DIM] = {
            let mut tops: [MaybeUninit<[MaybeUninit<usize>; BOARD_DIM]>; BOARD_DIM] =
                unsafe { std::mem::MaybeUninit::uninit().assume_init() };
            for y in 0..BOARD_DIM {
                let mut row: [MaybeUninit<usize>; BOARD_DIM] =
                    unsafe { MaybeUninit::uninit().assume_init() };
                for x in 0..BOARD_DIM {
                    row[x].write(self.board.contents[y][x].top());
                }
                tops[y].write(row);
            }
            unsafe { std::mem::transmute(tops) }
        };

        for (dest_row, dest_stack_row) in tops.into_iter().enumerate() {
            for (dest_col, dest_top) in dest_stack_row.into_iter().enumerate() {
                if dest_top == BOARD_DIM {
                    continue;
                }
                for (size, count) in available_pieces.into_iter().enumerate() {
                    if count > 0 && size >= dest_top {
                        let mut new_state = self.clone();
                        new_state.board.contents[dest_row][dest_col].pieces[size] = self.turn;
                        new_state.next_turn();
                        children.push((
                            GameMove::Place {
                                size,
                                dest: (dest_row, dest_col),
                            },
                            new_state,
                        ));
                    }
                }

                for (source_row, source_stack_row) in tops.into_iter().enumerate() {
                    for (source_col, source_top) in source_stack_row.into_iter().enumerate() {
                        if source_top > dest_top
                            && (source_row != dest_row || source_col != dest_col)
                        {
                            let mut new_state = self.clone();
                            let mut stacks = new_state.board.contents;
                            let current_top = source_top - 1;
                            stacks[dest_row][dest_col].pieces[current_top] =
                                stacks[source_row][source_col].pieces[current_top];
                            stacks[source_row][source_col].pieces[current_top] = Color::Empty;
                            new_state.board.contents = stacks;
                            new_state.next_turn();
                            children.push((
                                GameMove::Move {
                                    source: (source_row, source_col),
                                    dest: (dest_row, dest_col),
                                },
                                new_state,
                            ));
                        }
                    }
                }
            }
        }

        children
    }

    fn raw_score(&self) -> Score {
        // Check for victory.
        let check_winner = self.turn.other();

        let top_colors: [[Color; BOARD_DIM]; BOARD_DIM] = {
            let mut top_colors: [MaybeUninit<[MaybeUninit<Color>; BOARD_DIM]>; BOARD_DIM] =
                unsafe { std::mem::MaybeUninit::uninit().assume_init() };
            for y in 0..BOARD_DIM {
                let mut row: [MaybeUninit<Color>; BOARD_DIM] =
                    unsafe { MaybeUninit::uninit().assume_init() };
                for x in 0..BOARD_DIM {
                    row[x].write(self.board.contents[y][x].top_color());
                }
                top_colors[y].write(row);
            }
            unsafe { std::mem::transmute(top_colors) }
        };

        for i in 0..BOARD_DIM {
            if (0..BOARD_DIM).all(|col| self.board.contents[i][col].top_color() == check_winner)
                || (0..BOARD_DIM).all(|row| self.board.contents[row][i].top_color() == check_winner)
            {
                return Score::for_color(check_winner);
            }
        }

        if (0..BOARD_DIM).all(|i| self.board.contents[i][i].top_color() == check_winner)
            || (0..BOARD_DIM)
                .all(|i| self.board.contents[i][BOARD_DIM - i - 1].top_color() == check_winner)
        {
            return Score::for_color(check_winner);
        }

        let mut score = 0;

        for row in 0..BOARD_DIM {
            for col in 0..BOARD_DIM {
                let color = top_colors[row][col];
                if color == Color::Empty {
                    continue;
                }
                let base_score = if row == col || row == BOARD_DIM - col - 1 {
                    3
                } else {
                    2
                };
                let score_multiplier = if top_colors[row][col] == Color::White {
                    1
                } else {
                    -1
                };
                score += score_multiplier * base_score;
            }
        }

        Score::Balanced(score)
    }
}

enum GameMove {
    Place {
        size: usize,
        dest: (usize, usize),
    },
    Move {
        source: (usize, usize),
        dest: (usize, usize),
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Score {
    WhiteFavored,
    BlackFavored,
    Balanced(i32),
}

impl Score {
    fn for_color(color: Color) -> Score {
        if color == Color::White {
            Score::WhiteFavored
        } else {
            Score::BlackFavored
        }
    }
}

impl PartialOrd for Score {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self == other {
            return Some(Ordering::Equal);
        }
        Some(match (*self, *other) {
            (Score::WhiteFavored, _) | (_, Score::BlackFavored) => Ordering::Greater,
            (Score::BlackFavored, _) | (_, Score::WhiteFavored) => Ordering::Less,
            (Score::Balanced(self_score), Score::Balanced(other_score)) => {
                self_score.cmp(&other_score)
            }
        })
    }
}

impl Ord for Score {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

enum NodeState {
    GameState(Box<GameState>),
    Branches(Vec<(GameMove, Node)>),
    Resolved,
}

struct Node {
    score: Score,
    turn: Color,
    state: NodeState,
}

impl Node {
    fn new(game: GameState) -> Node {
        Node {
            score: game.raw_score(),
            turn: game.turn,
            state: NodeState::GameState(Box::new(game)),
        }
    }

    fn update_score(&mut self) {
        if let NodeState::Branches(ref branches) = self.state {
            let branch_scores = branches.iter().map(|(_, node)| node.score);
            let optimized_score = if self.turn == Color::White {
                branch_scores.max()
            } else {
                branch_scores.min()
            };
            self.score = optimized_score.unwrap();
            if matches!(self.score, Score::WhiteFavored | Score::BlackFavored) {
                self.state = NodeState::Resolved;
            }
        }
    }

    fn branch(&mut self, depth: i32) {
        match self.state {
            NodeState::GameState(ref game_state) => {
                let branch_states = game_state.branch();
                let mut branches: Vec<(GameMove, Node)> = branch_states
                    .into_iter()
                    .map(|(branch_move, branch_state)| {
                        (
                            branch_move,
                            Node {
                                score: branch_state.raw_score(),
                                turn: branch_state.turn,
                                state: NodeState::GameState(Box::new(branch_state)),
                            },
                        )
                    })
                    .collect();

                if depth > 1 {
                    for (_, branch) in &mut branches {
                        branch.branch(depth - 1);
                    }
                }

                self.update_score();
                self.state = NodeState::Branches(branches);
            }
            NodeState::Branches(ref mut branches) => {
                if depth == 1 {
                    return;
                }
                for (_, branch) in branches {
                    branch.branch(depth - 1);
                }
                self.update_score();
            }
            _ => (),
        }
    }
}

fn main() {
    let state = GameState::new();
    black_box(state.branch());

    let numbers = vec![78, 90, 20];

    let mut raw_string = "Adam";

    let mut owned_string = String::from("Adam");

    owned_string.insert(2, 'h');
}
