//! 模拟退火搜索：六类移动，含官方布局与禁用键迁移。

use crate::keyboard::NSYM;
use crate::model::{Evaluator, Layout};
use std::collections::HashMap;
use std::f64::consts::E;

pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// QWERTY 字母 → 本方案 30 键的下标。
/// 上排 q..p 是第 0 行、主行 a..l 是第 1 行、下排 z..m 是第 2 行；
/// 本方案下排有 10 个键而字母只有 7 个，故 z 起索引右移 1。
pub fn letter_to_sym() -> HashMap<char, u8> {
    let qwerty = "qwertyuiopasdfghjklzxcvbnm";
    let mut m = HashMap::new();
    for (i, ch) in qwerty.chars().enumerate() {
        let idx = if i < 19 { i } else { i + 1 };
        m.insert(ch, idx as u8);
    }
    m
}

/// 上游布局：根表的码对（`roots.tsv` 第二列）按物理位置映到 30 键。
pub fn official_layout(data: &crate::data::Data, cfg: &crate::config::Config) -> Layout {
    let pos = letter_to_sym();
    let nr = data.roots.len();
    let mut lay = Layout::new(nr);
    let mut missing = 0usize;
    for (r, root) in data.roots.iter().enumerate() {
        let cs: Vec<char> = root.code.chars().collect();
        match (cs.first().and_then(|c| pos.get(c)), cs.get(1).and_then(|c| pos.get(c))) {
            (Some(&b), Some(&s)) => {
                lay.b[r] = b;
                lay.s[r] = s;
            }
            _ => {
                lay.b[r] = (r % NSYM) as u8;
                lay.s[r] = ((r * 7) % NSYM) as u8;
                missing += 1;
            }
        }
    }
    // 禁用键上的根搬到最便宜的可用键
    let mut moved = 0usize;
    let mut cheapest: Vec<usize> = (0..NSYM).filter(|&k| cfg.keycost[k].is_finite()).collect();
    cheapest.sort_by(|&a, &b| cfg.keycost[a].partial_cmp(&cfg.keycost[b]).unwrap());
    for r in 0..nr {
        let bad_b = !cfg.keycost[lay.b[r] as usize].is_finite();
        let bad_s = !cfg.keycost[lay.s[r] as usize].is_finite();
        if bad_b || bad_s {
            let k = cheapest[r % cheapest.len()];
            lay.b[r] = k as u8;
            lay.s[r] = cheapest[(r / cheapest.len()) % cheapest.len()] as u8;
            moved += 1;
        }
    }
    eprintln!(
        "官方布局：{} 根取官方码对，{} 根缺码占位，{} 根因禁用键迁移",
        nr - missing,
        missing,
        moved
    );
    lay
}

pub struct AnnealResult {
    pub best_cost: f64,
    pub best: Layout,
}

/// 退火主循环。`moves` 是六类移动的相对权重；不施加「每根独占一格」约束。
pub fn anneal(
    ev: &mut Evaluator,
    start: Layout,
    steps: usize,
    t0: f64,
    t1: f64,
    seed: u64,
    moves: [f64; 6],
) -> AnnealResult {
    let nr = start.b.len();
    let mut rng = Rng::new(seed);
    let mut cur = start;
    let mut cur_cost = ev.evaluate(&cur).cost;
    let mut best = Layout {
        b: cur.b.clone(),
        s: cur.s.clone(),
    };
    let mut best_cost = cur_cost;

    let total: f64 = moves.iter().sum();
    for step in 0..steps {
        let temp = t0 * (t1 / t0).powf(step as f64 / steps as f64);
        let pick = rng.unit() * total;
        let mut kind = 0;
        let mut acc = 0.0;
        for (i, &m) in moves.iter().enumerate() {
            acc += m;
            if pick <= acc {
                kind = i;
                break;
            }
        }
        let r1 = rng.below(nr);
        let r2 = rng.below(nr);
        let snap = (cur.b.clone(), cur.s.clone());
        let old = (cur.b[r1], cur.s[r1], cur.b[r2], cur.s[r2]);
        match kind {
            0 => {
                if r1 == r2 {
                    continue;
                }
                cur.b[r1] = old.2;
                cur.b[r2] = old.0;
            }
            1 => cur.b[r1] = rng.below(NSYM) as u8,
            2 => {
                if r1 == r2 {
                    continue;
                }
                cur.s[r1] = old.3;
                cur.s[r2] = old.1;
            }
            3 => cur.s[r1] = rng.below(NSYM) as u8,
            4 => {
                if r1 == r2 {
                    continue;
                }
                cur.b[r1] = old.2;
                cur.s[r1] = old.3;
                cur.b[r2] = old.0;
                cur.s[r2] = old.1;
            }
            _ => {
                let (ka, kb) = (r1 % NSYM, r2 % NSYM);
                if ka == kb {
                    continue;
                }
                for r in 0..nr {
                    if cur.b[r] as usize == ka {
                        cur.b[r] = kb as u8;
                    } else if cur.b[r] as usize == kb {
                        cur.b[r] = ka as u8;
                    }
                }
            }
        }
        let m = ev.evaluate(&cur);
        let delta = m.cost - cur_cost;
        if delta <= 0.0 || rng.unit() < E.powf(-delta / temp) {
            cur_cost = m.cost;
            if m.cost < best_cost {
                best_cost = m.cost;
                best.b.copy_from_slice(&cur.b);
                best.s.copy_from_slice(&cur.s);
            }
        } else {
            cur.b.copy_from_slice(&snap.0);
            cur.s.copy_from_slice(&snap.1);
        }
    }
    AnnealResult { best_cost, best }
}

/// 把落在禁用键上的根迁到最便宜的可用键（用户给的自定义布局可能违规）。
pub fn sanitize(lay: &mut Layout, cfg: &crate::config::Config) {
    let mut enabled: Vec<usize> = (0..NSYM).filter(|&k| cfg.keycost[k].is_finite()).collect();
    if enabled.is_empty() {
        return;
    }
    enabled.sort_by(|&a, &b| cfg.keycost[a].partial_cmp(&cfg.keycost[b]).unwrap());
    let ne = enabled.len();
    let mut moved = 0usize;
    let mut cursor = 0usize;
    for r in 0..lay.b.len() {
        if !cfg.keycost[lay.b[r] as usize].is_finite() || !cfg.keycost[lay.s[r] as usize].is_finite() {
            lay.b[r] = enabled[cursor % ne] as u8;
            lay.s[r] = enabled[(cursor / ne) % ne] as u8;
            cursor += 1;
            moved += 1;
        }
    }
    if moved > 0 {
        eprintln!("布局修正：{moved} 根因禁用键被迁走");
    }
}
/// 多线程岛屿退火：N 条链各自独立跑，取最优。
///
/// 每条链有自己的 Evaluator（统计数组约 27 MB），互不共享可变状态；
/// 代价是同时跑很多条会挤占内存带宽，实测加速比低于核数。
pub fn anneal_parallel(
    data: &crate::data::Data,
    cfg: &crate::config::Config,
    forced: &[crate::data::Forced],
    start: &Layout,
    steps: usize,
    t0: f64,
    t1: f64,
    seed: u64,
    threads: usize,
    moves: [f64; 6],
    kick: usize,
) -> AnnealResult {
    if threads <= 1 {
        let mut ev = Evaluator::new(data, cfg, forced);
        return anneal(&mut ev, start.clone(), steps, t0, t1, seed, moves);
    }
    let results: Vec<AnnealResult> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|i| {
                scope.spawn(move || {
                    let mut ev = Evaluator::new(data, cfg, forced);
                    let iseed = seed.wrapping_add(i as u64 * 7919);
                    let mut s0 = start.clone();
                    // 岛 0 保持原起点，其余岛先随机踢几脚，增加起点多样性
                    if i > 0 && kick > 0 {
                        kick_layout(&mut s0, ev.kb.key_cost.iter().enumerate().filter(|(_, c)| c.is_finite()).map(|(k, _)| k).collect(), kick, iseed);
                    }
                    let r = anneal(&mut ev, s0, steps, t0, t1, iseed, moves);
                    eprintln!("  岛 {i}（seed {iseed}）: {:.6}", r.best_cost);
                    r
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    results
        .into_iter()
        .min_by(|a, b| a.best_cost.partial_cmp(&b.best_cost).unwrap())
        .unwrap()
}

/// 随机踢几脚：随机挑根、随机换到可用键上，用于岛屿的起点多样化。
fn kick_layout(lay: &mut Layout, keys: Vec<usize>, steps: usize, seed: u64) {
    if keys.len() < 2 {
        return;
    }
    let mut rng = Rng::new(seed ^ 0x5DEE_CE66_D1CE_u64);
    let nk = keys.len();
    for _ in 0..steps {
        let r = rng.below(lay.b.len());
        lay.b[r] = keys[rng.below(nk)] as u8;
        lay.s[r] = keys[rng.below(nk)] as u8;
    }
}
