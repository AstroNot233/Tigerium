//! 贪心定码与精确记账，产出 Metrics。代价模型见 AGENTS.md。

use crate::config::Config;
use crate::data::Data;
use crate::keyboard::{cand1, cand2, cand3, cand4, code_id, decode, Keyboard, NCODE, NSYM};

#[derive(Clone)]
pub struct Layout {
    pub b: Vec<u8>,
    pub s: Vec<u8>,
}

impl Layout {
    pub fn new(n: usize) -> Layout {
        Layout {
            b: vec![0; n],
            s: vec![0; n],
        }
    }
}

pub struct Metrics {
    pub cost: f64,
    /// 击键费（只看用了哪些键，与顺序无关）
    pub keys: f64,
    pub sel: f64,
    /// 疲劳值（相邻两键的顺序代价）
    pub fatigue: f64,
    /// 真实平均码长（按字频加权，不计键权）
    pub len: f64,
    /// 前 N 高频字没拿一简的罚金（单独列出，便于和「真实费用」区分）
    pub top: f64,
    pub len_hist: [usize; 5],
}

struct CodeStats {
    count: Vec<u16>,
    /// 码位里权重最高的字（首选）的权重
    wtop: Vec<f64>,
    wsum: Vec<f64>,
    s1: Vec<f64>,
    m2: Vec<f64>,
    touched: Vec<u32>,
}

impl CodeStats {
    fn new() -> Self {
        CodeStats {
            count: vec![0; NCODE],
            wtop: vec![0.0; NCODE],
            wsum: vec![0.0; NCODE],
            s1: vec![0.0; NCODE],
            m2: vec![0.0; NCODE],
            touched: Vec::with_capacity(8192),
        }
    }
    fn reset(&mut self) {
        for &t in &self.touched {
            let i = t as usize;
            self.count[i] = 0;
            self.wtop[i] = 0.0;
            self.wsum[i] = 0.0;
            self.s1[i] = 0.0;
            self.m2[i] = 0.0;
        }
        self.touched.clear();
    }
}

/// 选重基础罚：首选 p1、二三选 p2、此后每 sel_span 位加 sel_step
pub fn rank_pen(cfg: &Config, rank: usize) -> f64 {
    match rank {
        0 => 0.0,
        1 => cfg.p1,
        2 | 3 => cfg.p2,
        _ => cfg.p2 + cfg.sel_step * (((rank - 2) as f64) / cfg.sel_span).floor(),
    }
}

pub struct Evaluator<'a> {
    pub data: &'a Data,
    pub cfg: &'a Config,
    pub kb: Keyboard,
    /// 30×30 的相邻两键疲劳矩阵
    fat: [[f64; NSYM]; NSYM],
    /// 参与训练的字下标，按权重降序
    pub order: Vec<u32>,
    pub total_w: f64,
    /// 上一次评估的定码结果（导出用）
    pub codes: Vec<u32>,
    pub lens: Vec<u8>,
    /// 第一层：前 N 高频字（必须拿一简）
    top: Vec<bool>,
    /// 每个码位被强制占用的个数（放行 caps 检查用）
    forced: Vec<u16>,
    /// 强制占位表原样保留，导出时要写进码表
    pub forced_list: Vec<crate::data::Forced>,
    /// 被强制占用的码位（预计算，避免每次评估都扫全表）
    forced_ids: Vec<u32>,
    /// 第二层：前 N2 高频字（必须不超过 top_len2 码）
    top2: Vec<bool>,
    stats: CodeStats,
}

impl<'a> Evaluator<'a> {
    pub fn new(data: &'a Data, cfg: &'a Config, forced_list: &[crate::data::Forced]) -> Self {
        let mut order: Vec<u32> = (0..data.chars.len() as u32)
            .filter(|&i| data.chars[i as usize].train_ok && data.chars[i as usize].weight >= cfg.min_weight)
            .collect();
        order.sort_by(|&x, &y| {
            data.chars[y as usize]
                .weight
                .partial_cmp(&data.chars[x as usize].weight)
                .unwrap()
        });
        let by_weight = order.clone();
        let mut top = vec![false; data.chars.len()];
        for k in 0..cfg.top_n.min(by_weight.len()) {
            top[by_weight[k] as usize] = true;
        }
        let mut top2 = vec![false; data.chars.len()];
        for k in 0..cfg.top_n2.min(by_weight.len()) {
            top2[by_weight[k] as usize] = true;
        }

        // 强制占位：把 (码, 输出) 预先占住对应码位的首选手
        let mut forced = vec![0u16; NCODE];
        for f in forced_list {
            let syms: Vec<u8> = f.code.chars().filter_map(|c| cfg.alphabet.iter().position(|&x| x == c).map(|i| i as u8)).collect();
            if !syms.is_empty() && syms.len() <= 4 {
                forced[code_id(&syms) as usize] = forced[code_id(&syms) as usize].saturating_add(1);
            }
        }

        let total_w: f64 = order
            .iter()
            .map(|&i| data.chars[i as usize].weight)
            .sum();
        Evaluator {
            data,
            cfg,
            kb: Keyboard::new(cfg),
            fat: crate::fatigue::matrix(cfg),
            order,
            total_w,
            codes: vec![0; data.chars.len()],
            lens: vec![0; data.chars.len()],
            top: top,
            top2: top2,
            forced_ids: forced.iter().enumerate().filter(|(_, &n)| n > 0).map(|(i, _)| i as u32).collect(),
            forced: forced,
            forced_list: forced_list.to_vec(),
            stats: CodeStats::new(),
        }
    }

    /// 一个字的可选码：返回 (符号串, 是否逻辑全码)
    pub fn options(&self, ids: &[u16], b: &[u8], s: &[u8]) -> Vec<(Vec<u8>, bool)> {
        let n = ids.len();
        let b0 = b[ids[0] as usize];
        let s0 = s[ids[0] as usize];
        let mut out = vec![(vec![b0], false)];
        if n == 1 {
            // Aa 是普通一层（受候选上限约束），筛重码才是逻辑全码
            out.push((vec![b0, s0], false));
            match self.cfg.single_root {
                crate::config::SingleRoot::Wuli => {
                    for (k, &w) in self.cfg.wuli.iter().enumerate() {
                        out.push((vec![b0, s0, w as u8], k == 0));
                    }
                }
                crate::config::SingleRoot::Aaa => out.push((vec![b0, s0, s0], true)),
                crate::config::SingleRoot::AaAa => out.push((vec![b0, s0, b0, s0], true)),
            }
            return out;
        }
        let b1 = b[ids[1] as usize];
        out.push((vec![b0, b1], false));
        if n == 2 {
            let s1 = s[ids[1] as usize];
            out.push((vec![b0, b1, s1], false));
            out.push((vec![b0, b1, s1, s0], true));
            return out;
        }
        let b2 = b[ids[2] as usize];
        out.push((vec![b0, b1, b2], false));
        if n == 3 {
            let s2 = s[ids[2] as usize];
            out.push((vec![b0, b1, b2, s2], true));
            return out;
        }
        let last = *ids.last().unwrap() as usize;
        let z = if self.cfg.last_small { s[last] } else { b[last] };
        out.push((vec![b0, b1, b2, z], true));
        out
    }

    pub fn evaluate(&mut self, lay: &Layout) -> Metrics {
        let cfg = self.cfg;
        let kb = &self.kb;
        let mut st = std::mem::replace(&mut self.stats, CodeStats::new());
        st.reset();
        for &id in &self.forced_ids {
            let i = id as usize;
            st.touched.push(id);
            st.count[i] = self.forced[i];
            st.wsum[i] = 1e12 * self.forced[i] as f64;
            st.wtop[i] = 1e12;
        }
        let mut key_sum = 0.0;
        let mut len_sum = 0.0;
        let mut fat_sum = 0.0;
        let mut topsum = 0.0;
        // 击键费直接从 phys 得出，无需单独累加
        let mut len_hist = [0usize; 5];
        let codes = &mut self.codes;
        let lens = &mut self.lens;

        // 每个字最多 4 个可选码；这里用栈上定长数组算，避免每字分配
        for &ci in &self.order {
            let c = &self.data.chars[ci as usize];
            let w = c.weight;
            let ids = &c.ids;
            let n = ids.len();
            let b0 = lay.b[ids[0] as usize] as usize;
            let s0 = lay.s[ids[0] as usize] as usize;
            let mut cand = [0u32; 4];
            let mut cost = [0f64; 4];
            let mut len = [0u8; 4];
            // 疲劳值：相邻两键的顺序代价，与击键费分开累计
            let mut fat = [0f64; 4];
            let f = &self.fat;
            let full_len: u8;

            let k0 = kb.key_cost[b0];
            cand[0] = cand1(b0);
            cost[0] = k0;
            len[0] = 1;

            if n == 1 {
                // 逻辑全码 = 筛重码（Aax / Aaa / AaAa），Aa 只是受上限约束的普通一层
                full_len = match cfg.single_root {
                    crate::config::SingleRoot::AaAa => 4,
                    _ => 3,
                };
                let k1 = k0 + kb.key_cost[s0];
                cand[1] = cand2(b0, s0);
                cost[1] = k1;
                len[1] = 2;
                fat[1] = f[b0][s0];
                // 单根字的筛重码：三种策略
                match cfg.single_root {
                    crate::config::SingleRoot::Wuli => {
                        let nw = cfg.wuli.len().min(2);
                        for j in 0..2 {
                            if j < nw {
                                let wu = cfg.wuli[j];
                                cand[2 + j] = cand3(b0, s0, wu);
                                cost[2 + j] = k1 + kb.key_cost[wu];
                                len[2 + j] = 3;
                                fat[2 + j] = fat[1] + f[s0][wu];
                            } else {
                                cost[2 + j] = f64::INFINITY;
                                len[2 + j] = 3;
                            }
                        }
                    }
                    crate::config::SingleRoot::Aaa => {
                        // Aa(Aaa)：第三码复用**小码**（大小小）
                        cand[2] = cand3(b0, s0, s0);
                        cost[2] = k1 + kb.key_cost[s0];
                        len[2] = 3;
                        fat[2] = fat[1] + f[s0][s0];
                        cost[3] = f64::INFINITY;
                        len[3] = 3;
                    }
                    crate::config::SingleRoot::AaAa => {
                        // Aa(AaAa)：重复整条全码
                        cand[2] = cand4(b0, s0, b0, s0 as u8);
                        cost[2] = k1 + kb.key_cost[b0] + kb.key_cost[s0];
                        len[2] = 4;
                        fat[2] = fat[1] + f[s0][b0] + f[b0][s0];
                        cost[3] = f64::INFINITY;
                        len[3] = 3;
                    }
                }
            } else {
                full_len = 4;
                let b1 = lay.b[ids[1] as usize] as usize;
                let k1 = k0 + kb.key_cost[b1];
                cand[1] = cand2(b0, b1);
                cost[1] = k1;
                len[1] = 2;
                fat[1] = f[b0][b1];
                if n == 2 {
                    let s1 = lay.s[ids[1] as usize] as usize;
                    let k2 = k1 + kb.key_cost[s1];
                    cand[2] = cand3(b0, b1, s1);
                    cost[2] = k2;
                    len[2] = 3;
                    fat[2] = fat[1] + f[b1][s1];
                    let k3 = k2 + kb.key_cost[s0];
                    cand[3] = cand4(b0, b1, s1, s0 as u8);
                    cost[3] = k3;
                    len[3] = 4;
                    fat[3] = fat[2] + f[s1][s0];
                } else {
                    let b2 = lay.b[ids[2] as usize] as usize;
                    let k2 = k1 + kb.key_cost[b2];
                    cand[2] = cand3(b0, b1, b2);
                    cost[2] = k2;
                    len[2] = 3;
                    fat[2] = fat[1] + f[b1][b2];
                    // 三根字第 4 码取第三根的小码；四根以上取末根的大码（或小码）
                    let z = if n == 3 {
                        lay.s[ids[2] as usize] as usize
                    } else {
                        let last = *ids.last().unwrap() as usize;
                        if cfg.last_small { lay.s[last] as usize } else { lay.b[last] as usize }
                    };
                    cand[3] = cand4(b0, b1, b2, z as u8);
                    cost[3] = k2 + kb.key_cost[z];
                    len[3] = 4;
                    fat[3] = fat[2] + f[b2][z];
                }
            }

            let nopt = if n == 1 {
                match cfg.single_root {
                    crate::config::SingleRoot::Wuli => 2 + cfg.wuli.len().min(2),
                    _ => 3,
                }
            } else {
                4
            };
            let first = if c.short_ok { 0 } else { nopt - 1 };
            let mut best = usize::MAX;
            let mut best_cost = f64::INFINITY;
            for i in first..nopt {
                let id = cand[i] as usize;
                let occ = st.count[id] as usize;
                // 强制占位也算占用：被标点占住的短码不再接收字
                if len[i] != full_len && occ >= cfg.caps[(len[i] - 1) as usize] {
                    continue;
                }
                let mut cst = cost[i];
                if occ > 0 {
                    let base = rank_pen(cfg, occ + 1);
                    let wprev = st.wsum[id];
                    let surprise = if wprev + w > 0.0 { 1.0 - w / (wprev + w) } else { 0.0 };
                    cst += base * surprise;
                    if wprev > 0.0 && w > 0.0 {
                        cst += st.m2[id] * (1.0 / wprev - 1.0 / (wprev + w)) / w;
                        // 首选本身也要为「多了一个同码者」付费，按其惊讶度增量算
                        let wt = st.wtop[id];
                        cst += wt * cfg.p1 * wt * (1.0 / wprev - 1.0 / (wprev + w)) / w;
                    }
                }
                if cst < best_cost - 1e-12
                    || ((cst - best_cost).abs() <= 1e-12 && best != usize::MAX && len[i] < len[best])
                {
                    best_cost = cst;
                    best = i;
                }
            }
            if best == usize::MAX {
                best = nopt - 1;
            }
            let id = cand[best] as usize;
            if st.count[id] == 0 {
                st.touched.push(id as u32);
                st.wtop[id] = w;
            }
            st.count[id] += 1;
            let rank = st.count[id] as usize;
            let p = if rank == 1 { 0.0 } else { rank_pen(cfg, rank) };
            st.wsum[id] += w;
            st.s1[id] += w * p;
            st.m2[id] += w * w * p;

            // 前 N 高频字没拿到一简：按字频加权罚 top_pen
            let mut top_pen = 0.0;
            if self.top[ci as usize] && (len[best] as usize) > cfg.top_len {
                top_pen += cfg.top_pen;
            }
            if self.top2[ci as usize] && (len[best] as usize) > cfg.top_len2 {
                top_pen += cfg.top_pen2;
            }
            key_sum += w * cost[best];
            len_sum += w * len[best] as f64;
            fat_sum += w * fat[best];
            topsum += w * top_pen;

            len_hist[len[best] as usize] += 1;
            codes[ci as usize] = cand[best];
            lens[ci as usize] = len[best];
        }

        let mut sel = 0.0;
        for &t in &st.touched {
            let i = t as usize;
            if st.count[i] >= 2 && st.wsum[i] > 0.0 {
                sel += st.s1[i] - st.m2[i] / st.wsum[i];
                // 首选的惊讶度：1 − 自己占该码位的权重比
                let wt = st.wtop[i];
                sel += wt * cfg.p1 * (1.0 - wt / st.wsum[i]);
            }
        }
        self.stats = st;
        Metrics {
            cost: (key_sum + topsum + sel + cfg.fatigue_weight * fat_sum) / self.total_w,
            top: topsum / self.total_w,
            keys: key_sum / self.total_w,
            len: len_sum / self.total_w,
            sel: sel / self.total_w,
            fatigue: fat_sum / self.total_w,
            len_hist,
        }
    }


    /// 导出用：某字的可选码与其逻辑全码（符号串）
    pub fn full_code(&self, ids: &[u16], lay: &Layout) -> Vec<u8> {
        self.options(ids, &lay.b, &lay.s)
            .into_iter()
            .find(|o| o.1)
            .map(|o| o.0)
            .unwrap_or_default()
    }

    pub fn decode(&self, cid: u32, len: u8) -> String {
        crate::keyboard::render(&decode(cid, len as usize), &self.cfg.alphabet)
    }
}

impl<'a> Evaluator<'a> {
    /// 码位号 → 键下标序列（语料复核用）
    pub fn symbols(&self, cid: u32, len: u8) -> Vec<u8> {
        crate::keyboard::decode(cid, len as usize)
    }
}