//! 键位几何、码位编号与击键费（与顺序无关）。

use crate::config::Config;

pub const NSYM: usize = 30;

/// 码位编码用的前缀和：长度 1..4 各自起始偏移，全码空间 30 + 900 + 27000 + 810000。
pub const OFF: [usize; 5] = [0, 0, 30, 930, 27930];
pub const NCODE: usize = 27930 + 810000;

pub fn col(k: usize) -> usize {
    k % 10
}
pub fn row(k: usize) -> usize {
    k / 10
}

pub struct Keyboard {
    pub key_cost: [f64; NSYM],
}

impl Keyboard {
    pub fn new(cfg: &Config) -> Keyboard {
        Keyboard {
            key_cost: cfg.keycost,
        }
    }

    pub fn code_cost(&self, syms: &[u8]) -> f64 {
        syms.iter().map(|&s| self.key_cost[s as usize]).sum()
    }
}

/// 码位编号：长度 + 各位符号（30 进制）。
pub fn code_id(syms: &[u8]) -> u32 {
    let mut v = 0usize;
    for &s in syms {
        v = v * NSYM + s as usize;
    }
    (OFF[syms.len()] + v) as u32
}

pub fn decode(cid: u32, len: usize) -> Vec<u8> {
    let mut v = cid as usize - OFF[len];
    let mut syms = vec![0u8; len];
    for i in (0..len).rev() {
        syms[i] = (v % NSYM) as u8;
        v /= NSYM;
    }
    syms
}

/// 键下标序列 → 显示用的码串
pub fn render(idxs: &[u8], alphabet: &[char; NSYM]) -> String {
    idxs.iter().map(|&i| alphabet[i as usize]).collect()
}

pub fn cand1(a: usize) -> u32 {
    code_id(&[a as u8])
}
pub fn cand2(a: usize, b: usize) -> u32 {
    code_id(&[a as u8, b as u8])
}
pub fn cand3(a: usize, b: usize, c: usize) -> u32 {
    code_id(&[a as u8, b as u8, c as u8])
}
pub fn cand4(a: usize, b: usize, c: usize, d: u8) -> u32 {
    code_id(&[a as u8, b as u8, c as u8, d])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn key_costs_come_from_matrix() {
        let cfg = Config::default();
        let kb = Keyboard::new(&cfg);
        for k in 0..NSYM {
            assert!((kb.key_cost[k] - cfg.keycost[k]).abs() < 1e-12);
        }
    }

    #[test]
    fn keystroke_cost_is_order_independent() {
        let cfg = Config::default();
        let kb = Keyboard::new(&cfg);
        // fgbg 与 ddtb 只要落在同样的键上，击键费相同（与顺序无关）
        let a = kb.code_cost(&[13, 14, 24, 14]);
        let b = kb.code_cost(&[14, 24, 13, 14]);
        assert!((a - b).abs() < 1e-12);
        assert!(kb.code_cost(&[13, 14]) < kb.code_cost(&[13, 14, 24]));
    }

    #[test]
    fn code_id_roundtrip() {
        for syms in [vec![3u8], vec![3, 7], vec![3, 7, 11], vec![3, 7, 11, 29]] {
            let id = code_id(&syms);
            assert_eq!(decode(id, syms.len()), syms);
        }
    }
}