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

    // 同键不算坏，但也不等于互击：仍要付「没有互击」那份钱，只是不加跨排费用
    if a == b {
        return if finger(ca) == 0 { e.alt + e.pinky } else { e.alt };
    }

    let mut c = 0.0;
    // 不互击就付钱（而不是互击给钱）：一简没有相邻键，不该因此吃亏，
    // 也不会出现「把高频字拉长成 fgfg 去多吃几次奖励」这种占便宜
    if hand(ca) == hand(cb) {
        c += e.alt;
        if finger(ca) == finger(cb) {
            if (ra as i32 - rb as i32).abs() >= 2 {
                c += e.same_big;
            } else {
                c += e.same_small;
            }
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