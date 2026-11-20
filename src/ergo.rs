//! 人体工学统计：按字频加权统计码内相邻两键的类别，口径与疲劳值一致。

use crate::data::Data;
use crate::model::Evaluator;

fn hand(col: usize) -> usize {
    if col < 5 {
        0
    } else {
        1
    }
}

/// 手指：0 小指 / 1 无名 / 2 中 / 3 食指
fn finger(col: usize) -> usize {
    match col {
        0 | 9 => 0,
        1 | 8 => 1,
        2 | 7 => 2,
        _ => 3,
    }
}

pub struct Ergo {
    pub bigrams: f64,
    pub alt: f64,      // 左右互击占比
    pub same_big: f64, // 同指大跨排
    pub same_small: f64,
    pub pinky: f64,
    pub wrong: f64,
    pub triple: f64,
}

pub fn measure(data: &Data, ev: &Evaluator) -> Ergo {
    let (mut total, mut alt, mut big, mut small, mut pinky, mut wrong, mut triple) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for (i, c) in data.chars.iter().enumerate() {
        if ev.lens[i] == 0 {
            continue;
        }
        let syms = crate::keyboard::decode(ev.codes[i], ev.lens[i] as usize);
        let cols: Vec<usize> = syms.iter().map(|&s| (s as usize) % 10).collect();
        let rows: Vec<usize> = syms.iter().map(|&s| (s as usize) / 10).collect();
        let w = c.weight;
        for k in 1..cols.len() {
            total += w;
            let (a, b) = (cols[k - 1], cols[k]);
            if hand(a) != hand(b) {
                alt += w;
            } else if finger(a) == finger(b) {
                if (rows[k - 1] as i32 - rows[k] as i32).abs() >= 2 {
                    big += w;
                } else {
                    small += w;
                }
            }
            if finger(a) == 0 || finger(b) == 0 {
                pinky += w;
            }
            // 错手：同一只手，从中/无名指的中排或下排跳上上排
            let f = finger(a);
            if hand(a) == hand(b)
                && (f == 1 || f == 2)
                && rows[k - 1] >= 1
                && rows[k] == 0
            {
                wrong += w;
            }
        }
        // 三连击按「连续三键同键」计（众、森、桑这类三个同根的字必然如此）
        for k in 1..cols.len() {
            if k >= 2 && syms[k - 2] == syms[k - 1] && syms[k - 1] == syms[k] {
                triple += w;
            }
        }
    }
    let pct = |x: f64| if total > 0.0 { 100.0 * x / total } else { 0.0 };
    Ergo {
        bigrams: total,
        alt: pct(alt),
        same_big: pct(big),
        same_small: pct(small),
        pinky: pct(pinky),
        wrong: pct(wrong),
        triple: pct(triple),
    }
}

/// 官方虎码：字母码 → QWERTY 物理位置后，用同一套规则统计。
pub fn measure_official(data: &Data, path: &str) -> Ergo {
    use std::collections::HashMap;
    let mut codes: HashMap<char, String> = HashMap::new();
    for line in std::fs::read_to_string(path).unwrap_or_default().lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 2 {
            continue;
        }
        if let Some(c) = f[0].chars().next() {
            let code = f[1].to_string();
            codes
                .entry(c)
                .and_modify(|x| {
                    if code.chars().count() < x.chars().count() {
                        *x = code.clone();
                    }
                })
                .or_insert(code);
        }
    }
    // QWERTY 字母 → (行, 列)
    let pos = |c: char| -> Option<(usize, usize)> {
        let top = "qwertyuiop";
        let home = "asdfghjkl";
        let bot = "zxcvbnm";
        if let Some(i) = top.find(c) {
            Some((0, i))
        } else if let Some(i) = home.find(c) {
            Some((1, i))
        } else {
            bot.find(c).map(|i| (2, i))
        }
    };
    let (mut total, mut alt, mut big, mut small, mut pinky, mut wrong, mut triple) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for ch in &data.chars {
        let code = match codes.get(&ch.glyph) {
            Some(c) => c,
            None => continue,
        };
        let ps: Vec<(usize, usize)> = code.chars().filter_map(pos).collect();
        let w = ch.weight;
        for k in 1..ps.len() {
            total += w;
            let (ra, ca) = ps[k - 1];
            let (rb, cb) = ps[k];
            if hand(ca) != hand(cb) {
                alt += w;
            } else if finger(ca) == finger(cb) {
                if (ra as i32 - rb as i32).abs() >= 2 {
                    big += w;
                } else {
                    small += w;
                }
            }
            if finger(ca) == 0 || finger(cb) == 0 {
                pinky += w;
            }
            let f = finger(ca);
            if hand(ca) == hand(cb) && (f == 1 || f == 2) && ra >= 1 && rb == 0 {
                wrong += w;
            }
        }
        let ls: Vec<char> = code.chars().collect();
        for k in 2..ls.len() {
            if ls[k - 2] == ls[k - 1] && ls[k - 1] == ls[k] {
                triple += w;
            }
        }
    }
    let pct = |x: f64| if total > 0.0 { 100.0 * x / total } else { 0.0 };
    Ergo {
        bigrams: total,
        alt: pct(alt),
        same_big: pct(big),
        same_small: pct(small),
        pinky: pct(pinky),
        wrong: pct(wrong),
        triple: pct(triple),
    }
}