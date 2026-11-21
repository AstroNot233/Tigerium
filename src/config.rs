//! 全部可调参数；TOML 由 serde 反序列化，键名拼错会直接报错。字段含义见示例配置。

use crate::keyboard::NSYM;
use serde::Deserialize;
use std::fs;

#[derive(Clone, Debug)]
pub struct Config {
    /// 30 键的显示字符（字母表）。空字符表示该键不参与（等同禁用）。
    pub alphabet: [char; NSYM],
    /// 30 键的按键费（3 行 × 10 列，行主序）。≤ 0 视为**禁用键**：不会有字根落在那里。
    pub keycost: [f64; NSYM],
    /// 禁用键的符号下标
    pub disabled: Vec<usize>,
    /// 单根字可用的无理码尾（符号下标列表）；空 = 关闭
    pub wuli: Vec<usize>,
    /// 分级候选上限：一简 / 二简 / 三简 / 四码（逻辑全码不受限）
    pub caps: [usize; 4],
    /// 选重基础罚：首选 / 二选三选 / 每后移几位 / 每次加多少
    pub p1: f64,
    pub p2: f64,
    pub sel_span: f64,
    pub sel_step: f64,
    /// 四根及以上字的末位用末根的小码(true) 还是大码(false)
    pub last_small: bool,
    /// 单根字的筛重码策略：Aa(Aax) / Aa(Aaa) / Aa(AaAa)
    pub single_root: SingleRoot,
    /// 训练权重下限：低于它的字不参与重排（只按逻辑全码落表）
    pub min_weight: f64,
    /// 简码资格：1 = 只有一级字，2 = 一二级，3 = 一二三级
    pub short_levels: usize,
    /// 前 dup_n 高频字之间不许撞码（0 = 关闭）
    pub dup_n: usize,
    /// 撞码罚金倍率：罚金 = dup_pen × 涉及的高频字权重之和
    pub dup_pen: f64,
    /// 第一层软约束：前 N 高频字的码长不得超过 top_len，否则按字频罚 top_pen
    pub top_n: usize,
    pub top_len: usize,
    pub top_pen: f64,
    /// 第二层软约束：前 N2 高频字的码长不得超过 top_len2（0 = 关闭）
    pub top_n2: usize,
    pub top_len2: usize,
    pub top_pen2: f64,
    pub data: DataPaths,
    /// 疲劳值的权重（0 = 完全不看人体工学，只比击键与选重）
    pub fatigue_weight: f64,
    pub ergo: ErgoWeights,
    /// 退火移动权重：换大码 / 改大码 / 换小码 / 改小码 / 整格换 / 整键换
    pub moves: [f64; 6],
    pub t0: f64,
    pub t1: f64,
    pub steps: usize,
    /// 重升温轮数（每轮从上一轮最优出发、温度重新拉高）
    pub rounds: usize,
    /// 每轮并行跑几条独立链（取最优）；1 = 单线程
    pub threads: usize,
    /// 非零号岛在开跑前随机扰动的步数（起点多样化）；0 = 所有岛同起点
    pub island_kick: usize,
    pub seed: u64,
}

/// 默认字母表（3 行 × 10 列）。可用 [alphabet] 小节整体替换。
pub const DEFAULT_ALPHABET: [char; NSYM] = [
    'G', 'E', 'C', 'A', 'I', 'J', 'B', 'D', 'F', 'H',
    '7', '5', '3', '1', '9', '0', '2', '4', '6', '8',
    'g', 'e', 'c', 'a', 'i', 'j', 'b', 'd', 'f', 'h',
];

/// 默认键位费用：上排内列与下排内列略贵，主行略便宜；没有指位项。
const DEFAULT_KEYCOST: [[f64; 10]; 3] = [
    [1.0, 1.0, 1.0, 1.0, 1.05, 1.05, 1.0, 1.0, 1.0, 1.0],
    [0.9, 0.9, 0.9, 0.9, 0.95, 0.95, 0.9, 0.9, 0.9, 0.9],
    [1.0, 1.0, 1.0, 1.0, 1.1, 1.1, 1.0, 1.0, 1.0, 1.0],
];

impl Default for Config {
    fn default() -> Self {
        let mut keycost = [0.0; NSYM];
        for r in 0..3 {
            for c in 0..10 {
                keycost[r * 10 + c] = DEFAULT_KEYCOST[r][c];
            }
        }
        Config {
            alphabet: DEFAULT_ALPHABET,
            keycost,
            disabled: Vec::new(),
            wuli: vec![14],
            caps: [1, 1, 1, 9],
            p1: 0.5,
            p2: 4.5,
            sel_span: 2.0,
            sel_step: 2.0,
            last_small: false,
            single_root: SingleRoot::Wuli,
            min_weight: 1.0,
            short_levels: 1,
            dup_n: 0,
            dup_pen: 0.0,
            top_n: 0,
            top_len: 1,
            top_pen: 1.0,
            top_n2: 0,
            top_len2: 2,
            top_pen2: 0.6,
            data: DataPaths::default(),
            fatigue_weight: 1.0,
            ergo: ErgoWeights::default(),
            moves: [3.0, 2.0, 2.0, 1.0, 1.0, 1.0],
            t0: 0.004,
            t1: 1e-5,
            steps: 400_000,
            rounds: 1,
            threads: 8,
            island_kick: 0,
            seed: 1,
        }
    }
}

impl Config {
    /// 依次加载多个配置文件，后面的覆盖前面的（用于「公共参数 + 方案 profile」两段式）。
    pub fn load_all(paths: &[String]) -> Config {
        let mut c = Config::default();
        for p in paths {
            c.apply(p);
        }
        c.normalize();
        c
    }

    pub fn load(path: &str) -> Config {
        let mut c = Config::default();
        c.apply(path);
        c.normalize();
        c
    }
    /// 把一份配置叠加到当前值上（不负责 normalize）。读取或解析失败直接退出。
    fn apply(&mut self, path: &str) {
        let txt = fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("config: 读不到 {path}: {e}");
            std::process::exit(2);
        });
        let raw: Raw = toml::from_str(&txt).unwrap_or_else(|e| {
            eprintln!("config: {path} 解析失败：\n{e}");
            std::process::exit(2);
        });
        raw.apply(self);
    }

    /// 守卫：键费 ≤ 0、或字母表里是空白字符、或字符重复，都视为禁用键。
    fn normalize(&mut self) {
        self.wuli.retain(|&k| k < NSYM);
        self.disabled.clear();
        for k in 0..NSYM {
            let blank = self.alphabet[k].is_whitespace();
            let dup = (0..k).any(|j| self.alphabet[j] == self.alphabet[k]);
            if !(self.keycost[k] > 0.0) || blank || dup {
                self.disabled.push(k);
                self.keycost[k] = f64::INFINITY;
            }
        }
        if !self.disabled.is_empty() {
            eprintln!("config: 禁用键 {} 个（下标 {:?}）", self.disabled.len(), self.disabled);
        }
        // Rime 的 speller 里这些字符有特殊含义，作编码符号可能被吞掉或改变语义
        let reserved: &[char] = &[',', '/', '.', '*', '+', '[', ']', '{', '}', '(', ')', '|', '&', '#', '%', '@', '!', '<', '>', ':'];
        let bad: Vec<(usize, char)> = (0..NSYM)
            .filter(|&k| reserved.contains(&self.alphabet[k]))
            .map(|k| (k, self.alphabet[k]))
            .collect();
        if !bad.is_empty() {
            eprintln!("config: 注意，字母表里用了 Rime 保留字符，可能被 speller 吞掉：{bad:?}（如确实要用，请自行在 schema 里处理）");
        }
    }
}

#[derive(Clone, Debug)]
pub struct ErgoWeights {
    /// 左右互击（正值表示奖励，内部取负成本）
    pub alt: f64,
    /// 同指、行距 ≥ 2
    pub same_big: f64,
    /// 同指、行距 ≤ 1
    pub same_small: f64,
    /// 任一键是小指
    pub pinky: f64,
    /// 同一只手、从中/无名指的中下排跳上上排
    pub wrong: f64,
    /// 三连击（连续三键同键）——多数由拆分决定、布局改不动，故只统计不计入目标
    #[allow(dead_code)]
    pub triple: f64,
}

impl Default for ErgoWeights {
    fn default() -> Self {
        ErgoWeights {
            alt: 0.05,
            same_big: 1.2,
            same_small: 0.35,
            pinky: 0.15,
            wrong: 0.5,
            triple: 0.25,
        }
    }
}

/// 数据源路径。换一个上游方案时只改这里，程序不再硬编码任何文件名。
#[derive(Clone, Debug)]
pub struct DataPaths {
    /// 字根表：`id<TAB>码对<TAB>字形数<TAB>字形串`
    pub roots: String,
    /// 拆分表：`字<TAB>拆分字形串`
    pub splits: String,
    /// 字频表：Rime 风格 `字<TAB>码<TAB>权` 或纯 `字<TAB>权`
    pub weights: String,
    /// 可选：字形归一化（`原字形<TAB>目标字形`）
    pub nfkc: Option<String>,
    /// 可选：强制占位（`码<TAB>输出`）
    pub forced: Option<String>,
    /// 可选：简码资格字表（一级 / 二级 / 三级）
    pub level1: Option<String>,
    pub level2: Option<String>,
    pub level3: Option<String>,
}

impl Default for DataPaths {
    fn default() -> Self {
        DataPaths {
            roots: "data/roots.tsv".into(),
            splits: "data/拆分方案.txt".into(),
            weights: "data/原码编码.yaml".into(),
            nfkc: Some("data/nfkc.tsv".into()),
            forced: Some("data/forced.tsv".into()),
            level1: Some("std/level-1.txt".into()),
            level2: Some("std/level-2.txt".into()),
            level3: Some("std/level-3.txt".into()),
        }
    }
}

/// 单根字的筛重码策略。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SingleRoot {
    /// `Aa(Aax)`：第三码从 keyboard.wuli 给的符号里挑（默认）
    Wuli,
    /// `Aa(Aaa)`：第三码复用小码（大小小）
    Aaa,
    /// `Aa(AaAa)`：重复整条全码
    AaAa,
}

// ---------------- 反序列化用的一层原始结构 ----------------
// 只负责「文件里写了什么」，全 Option 以便多份配置叠加；映射到 Config 时再补默认值。

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Raw {
    keyboard: Option<RawKeyboard>,
    candidates: Option<RawCandidates>,
    selection: Option<RawSelection>,
    code: Option<RawCode>,
    training: Option<RawTraining>,
    cost: Option<RawCost>,
    ergo: Option<RawErgo>,
    search: Option<RawSearch>,
    alphabet: Option<RawAlphabet>,
    data: Option<RawData>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawKeyboard {
    row0: Option<Vec<f64>>,
    row1: Option<Vec<f64>>,
    row2: Option<Vec<f64>>,
    #[serde(default, deserialize_with = "one_or_many")]
    wuli: Option<Vec<usize>>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawCandidates {
    caps: Option<Vec<usize>>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawSelection {
    p1: Option<f64>,
    p2: Option<f64>,
    sel_span: Option<f64>,
    sel_step: Option<f64>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawCode {
    last_small: Option<bool>,
    single_root: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawTraining {
    min_weight: Option<f64>,
    short_levels: Option<usize>,
    dup_n: Option<usize>,
    dup_pen: Option<f64>,
    top_n: Option<usize>,
    top_len: Option<usize>,
    top_pen: Option<f64>,
    top_n2: Option<usize>,
    top_len2: Option<usize>,
    top_pen2: Option<f64>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawCost {
    fatigue_weight: Option<f64>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawErgo {
    alt: Option<f64>,
    same_big: Option<f64>,
    same_small: Option<f64>,
    pinky: Option<f64>,
    wrong: Option<f64>,
    triple: Option<f64>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawSearch {
    moves: Option<Vec<f64>>,
    t0: Option<f64>,
    t1: Option<f64>,
    steps: Option<usize>,
    rounds: Option<usize>,
    threads: Option<usize>,
    island_kick: Option<usize>,
    seed: Option<u64>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawAlphabet {
    row0: Option<Vec<String>>,
    row1: Option<Vec<String>>,
    row2: Option<Vec<String>>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawData {
    roots: Option<String>,
    splits: Option<String>,
    weights: Option<String>,
    nfkc: Option<String>,
    forced: Option<String>,
    level1: Option<String>,
    level2: Option<String>,
    level3: Option<String>,
}

/// `wuli` 允许写标量或数组；负数表示关闭。
fn one_or_many<'de, D>(d: D) -> Result<Option<Vec<usize>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(i64),
        Many(Vec<i64>),
    }
    let v = OneOrMany::deserialize(d)?;
    Ok(Some(match v {
        OneOrMany::One(x) => {
            if x < 0 {
                Vec::new()
            } else {
                vec![x as usize]
            }
        }
        OneOrMany::Many(xs) => xs.iter().filter(|&&x| x >= 0).map(|&x| x as usize).collect(),
    }))
}

fn rows3(v: &Option<Vec<f64>>, dst: &mut [f64; NSYM], row: usize) {
    if let Some(v) = v {
        for (i, x) in v.iter().take(10).enumerate() {
            dst[row * 10 + i] = *x;
        }
    }
}

fn chars3(v: &Option<Vec<String>>, dst: &mut [char; NSYM], row: usize) {
    if let Some(v) = v {
        for (i, s) in v.iter().take(10).enumerate() {
            dst[row * 10 + i] = s.chars().next().unwrap_or(' ');
        }
    }
}

impl Raw {
    fn apply(&self, c: &mut Config) {
        if let Some(k) = &self.keyboard {
            rows3(&k.row0, &mut c.keycost, 0);
            rows3(&k.row1, &mut c.keycost, 1);
            rows3(&k.row2, &mut c.keycost, 2);
            if let Some(w) = &k.wuli {
                c.wuli = w.clone();
            }
        }
        if let Some(a) = &self.alphabet {
            chars3(&a.row0, &mut c.alphabet, 0);
            chars3(&a.row1, &mut c.alphabet, 1);
            chars3(&a.row2, &mut c.alphabet, 2);
        }
        if let Some(x) = &self.candidates {
            if let Some(v) = &x.caps {
                for (i, n) in v.iter().take(4).enumerate() {
                    c.caps[i] = *n;
                }
            }
        }
        if let Some(x) = &self.selection {
            if let Some(v) = x.p1 { c.p1 = v; }
            if let Some(v) = x.p2 { c.p2 = v; }
            if let Some(v) = x.sel_span { c.sel_span = v; }
            if let Some(v) = x.sel_step { c.sel_step = v; }
        }
        if let Some(x) = &self.code {
            if let Some(v) = x.last_small { c.last_small = v; }
            if let Some(v) = &x.single_root {
                c.single_root = match v.as_str() {
                    "aaa" => SingleRoot::Aaa,
                    "aaaa" => SingleRoot::AaAa,
                    _ => SingleRoot::Wuli,
                };
            }
        }
        if let Some(x) = &self.training {
            if let Some(v) = x.min_weight { c.min_weight = v; }
            if let Some(v) = x.short_levels { c.short_levels = v; }
            if let Some(v) = x.dup_n { c.dup_n = v; }
            if let Some(v) = x.dup_pen { c.dup_pen = v; }
            if let Some(v) = x.top_n { c.top_n = v; }
            if let Some(v) = x.top_len { c.top_len = v; }
            if let Some(v) = x.top_pen { c.top_pen = v; }
            if let Some(v) = x.top_n2 { c.top_n2 = v; }
            if let Some(v) = x.top_len2 { c.top_len2 = v; }
            if let Some(v) = x.top_pen2 { c.top_pen2 = v; }
        }
        if let Some(x) = &self.cost {
            if let Some(v) = x.fatigue_weight { c.fatigue_weight = v; }
        }
        if let Some(x) = &self.ergo {
            if let Some(v) = x.alt { c.ergo.alt = v; }
            if let Some(v) = x.same_big { c.ergo.same_big = v; }
            if let Some(v) = x.same_small { c.ergo.same_small = v; }
            if let Some(v) = x.pinky { c.ergo.pinky = v; }
            if let Some(v) = x.wrong { c.ergo.wrong = v; }
            if let Some(v) = x.triple { c.ergo.triple = v; }
        }
        if let Some(x) = &self.search {
            if let Some(v) = &x.moves {
                for (i, m) in v.iter().take(6).enumerate() {
                    c.moves[i] = *m;
                }
            }
            if let Some(v) = x.t0 { c.t0 = v; }
            if let Some(v) = x.t1 { c.t1 = v; }
            if let Some(v) = x.steps { c.steps = v; }
            if let Some(v) = x.rounds { c.rounds = v; }
            if let Some(v) = x.threads { c.threads = v.max(1); }
            if let Some(v) = x.island_kick { c.island_kick = v; }
            if let Some(v) = x.seed { c.seed = v; }
        }
        if let Some(x) = &self.data {
            if let Some(v) = &x.roots { c.data.roots = v.clone(); }
            if let Some(v) = &x.splits { c.data.splits = v.clone(); }
            if let Some(v) = &x.weights { c.data.weights = v.clone(); }
            c.data.nfkc = x.nfkc.clone().or(c.data.nfkc.take());
            c.data.forced = x.forced.clone().or(c.data.forced.take());
            c.data.level1 = x.level1.clone().or(c.data.level1.take());
            c.data.level2 = x.level2.clone().or(c.data.level2.take());
            c.data.level3 = x.level3.clone().or(c.data.level3.take());
        }
    }
}