use std::collections::{HashMap, VecDeque};
use std::ops::{Add, Sub, Mul};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct Orb(u16);
impl Add<u8> for Orb {
    type Output = Option<Orb>;
    fn add(self, rhs: u8) -> Self::Output {
        let v = self.0 as u64 + rhs as u64;
        if v > u16::max_value() as u64 {
            None
        } else {
            Some(Orb(v as u16))
        }
    }
}
impl Sub<u8> for Orb {
    type Output = Option<Orb>;
    fn sub(self, rhs: u8) -> Self::Output {
        if rhs as u16 >= self.0 {
            None
        } else {
            Some(Orb(self.0 - rhs as u16))
        }
    }
}
impl Mul<u8> for Orb {
    type Output = Option<Orb>;
    fn mul(self, rhs: u8) -> Self::Output {
        let v = self.0 as u64 * rhs as u64;
        if v > u16::max_value() as u64 {
            None
        } else {
            Some(Orb(v as u16))
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct State {
    x: usize,
    y: usize,
    orb: Option<Orb>,
    op: Option<CellOp>,
}

impl State {
    fn is_goal(&self) -> bool {
        self.at_goal() && self.at_value()
    }

    fn at_goal(&self) -> bool {
        self.x == 3 && self.y == 0
    }

    fn at_value(&self) -> bool {
        match self.orb {
            Some(Orb(30)) => true,
            _ => false,
        }
    }

    fn is_valid(&self, m: &Move) -> bool {
        match (m, self.x, self.y) {
            (&Move::East, 3, _) => false,
            (&Move::West, 0, _) => false,
            (&Move::West, 1, 3) => false,
            (&Move::North, _, 0) => false,
            (&Move::South, _, 3) => false,
            (&Move::South, 0, 2) => false,
            _ => true,
        }
    }

    fn apply_move(&self, m: &Move) -> State {
        let (x, y) = match m {
            &Move::East => (self.x + 1, self.y),
            &Move::West => (self.x - 1, self.y),
            &Move::North => (self.x, self.y - 1),
            &Move::South => (self.x, self.y + 1),
        };
        let (orb, op) = match (self.op, GRID[y][x]) {
            (None, CellOp::Val(_)) => unreachable!(),
            (Some(CellOp::Val(_)), _) => unreachable!(),
            (None, op) => (self.orb, Some(op)),
            (Some(CellOp::Add), CellOp::Val(v)) => (self.orb.and_then(|orb| orb + v), None),
            (Some(CellOp::Add), _) => unreachable!(),
            (Some(CellOp::Sub), CellOp::Val(v)) => (self.orb.and_then(|orb| orb - v), None),
            (Some(CellOp::Sub), _) => unreachable!(),
            (Some(CellOp::Mul), CellOp::Val(v)) => (self.orb.and_then(|orb| orb * v), None),
            (Some(CellOp::Mul), _) => unreachable!(),
        };
        State {
            x: x,
            y: y,
            orb: orb,
            op: op,
        }
    }

    fn valid_moves(&self) -> Vec<(Move, State)> {
        const MOVES: [Move; 4] = [Move::North, Move::East, Move::South, Move::West];
        MOVES.iter()
            .filter(|m| self.is_valid(m))
            .map(|ref m| (**m, self.apply_move(m)))
            .filter(|&(_, s)| s.orb.is_some() && !(s.at_goal() && !s.at_value()))
            .collect::<Vec<_>>()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum CellOp {
    Add,
    Sub,
    Mul,
    Val(u8),
}

#[derive(Clone, Copy, Debug)]
enum Move {
    North,
    East,
    South,
    West,
}

const GRID: [[CellOp; 4]; 4] = [[CellOp::Mul, CellOp::Val(8), CellOp::Sub, CellOp::Val(1)],
                                [CellOp::Val(4), CellOp::Mul, CellOp::Val(11), CellOp::Mul],
                                [CellOp::Add, CellOp::Val(4), CellOp::Sub, CellOp::Val(18)],
                                [CellOp::Val(22), CellOp::Sub, CellOp::Val(9), CellOp::Mul]];
//
// NW corner is 0,0

fn main() {
    let i = State {
        x: 0,
        y: 3,
        orb: Some(Orb(22)),
        op: None,
    };
    let mut q = VecDeque::new();
    let mut shortest_path: HashMap<State, Vec<Move>> = HashMap::new();
    shortest_path.insert(i, vec![]);
    q.push_back(i);
    while let Some(s) = q.pop_front() {
        let cms = s.valid_moves()
            .into_iter()
            .filter(|&(_, ref s)| !shortest_path.contains_key(s))
            .collect::<Vec<_>>();
        for (m, n) in cms {
            let mut path = shortest_path.get(&s).unwrap().clone();
            path.push(m);
            if n.is_goal() {
                println!("{:?}", path);
                return;
            }
            shortest_path.insert(n, path);
            q.push_back(n);
        }
    }
    panic!("out of states to test!");
}
