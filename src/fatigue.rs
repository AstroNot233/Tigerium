//! 疲劳值：相邻两键的顺序代价。各项定义见 AGENTS.md。

use crate::config::Config;
use crate::keyboard::{col, row, NSYM};

pub fn hand(c: usize) -> usize {
    if c < 5 {
        0
    } else {
        1
    }
}

/// 0 小指 / 1 无名 / 2 中 / 3 食指
pub fn finger(c: usize) -> usize {
    match c {
        0 | 9 => 0,
        1 | 8 => 1,
        2 | 7 => 2,
        _ => 3,
    }
}

pub fn pair_cost(a: usize, b: usize, cfg: &Config) -> f64 {
    let (ra, ca) = (row(a), col(a));
    let (rb, cb) = (row(b), col(b));
    let e = &cfg.ergo;

    // 同键：不计（双击不算坏）
    if a == b {
        return if finger(ca) == 0 { e.pinky } else { 0.0 };
    }

    let mut c = 0.0;
    if hand(ca) != hand(cb) {
        c -= e.alt; // 互击奖励
    } else if finger(ca) == finger(cb) {
        if (ra as i32 - rb as i32).abs() >= 2 {
            c += e.same_big;
        } else {
            c += e.same_small;
        }
    }
    if finger(ca) == 0 || finger(cb) == 0 {
        c += e.pinky;
    }
    let f = finger(ca);
    if hand(ca) == hand(cb) && (f == 1 || f == 2) && ra >= 1 && rb == 0 {
        c += e.wrong;
    }
    c
}

pub fn matrix(cfg: &Config) -> [[f64; NSYM]; NSYM] {
    let mut m = [[0.0; NSYM]; NSYM];
    for a in 0..NSYM {
        for b in 0..NSYM {
            m[a][b] = pair_cost(a, b, cfg);
        }
    }
    m
}