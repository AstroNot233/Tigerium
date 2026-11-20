//! 数据加载：根表、拆分、字频、字形归并。格式见 profiles/README.md。

use crate::keyboard::NSYM;
use std::collections::HashMap;
use std::fs;

/// 一个字：根序列（根 id）、权重、是否具备简码资格。
pub struct Char {
    pub glyph: char,
    pub weight: f64,
    pub ids: Vec<u16>,
    /// 有简码资格（只有一级字）
    pub short_ok: bool,
    /// 参与重排（一、二、三级字都在内）
    pub train_ok: bool,
}

pub struct Root {
    pub code: String,
    pub glyphs: Vec<char>,
}

pub struct Data {
    pub chars: Vec<Char>,
    pub roots: Vec<Root>,
}

fn read_opt(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("读取 {path} 失败：{e}"))
}

impl Data {
    /// 按配置里的路径加载；换上游方案时只改 config 的 [data] 段。
    pub fn load(cfg: &crate::config::Config) -> Data {
        let paths = &cfg.data;
        let roots = load_roots(&paths.roots);
        let (weights, full_codes) = load_weights(&paths.weights);
        let splits = load_splits(&paths.splits);
        let glyph2root = group_glyphs(&roots, &splits, &full_codes, paths.nfkc.as_deref());
        let l1 = paths.level1.as_deref().map(load_charset).unwrap_or_default();
        let l2 = paths.level2.as_deref().map(load_charset).unwrap_or_default();
        let l3 = paths.level3.as_deref().map(load_charset).unwrap_or_default();
        let mut chars = Vec::with_capacity(splits.len());
        let all_levels: Vec<char> = l1.iter().chain(l2.iter()).chain(l3.iter()).copied().collect();
        let mut unresolved: Vec<char> = Vec::new();
        for (glyph, toks) in &splits {
            let mut ids = Vec::with_capacity(toks.len());
            let mut bad = false;
            for t in toks {
                match glyph2root.get(t) {
                    Some(&id) => ids.push(id),
                    None => {
                        bad = true;
                        if !unresolved.contains(t) {
                            unresolved.push(*t);
                        }
                    }
                }
            }
            if bad {
                continue;
            }
            chars.push(Char {
                glyph: *glyph,
                weight: weights.get(glyph).copied().unwrap_or(0.0),
                ids,
                short_ok: match cfg.short_levels {
                    3 => all_levels.contains(glyph),
                    2 => l1.contains(glyph) || l2.contains(glyph),
                    _ => l1.contains(glyph),
                },
                train_ok: all_levels.contains(glyph),
            });
        }
        println!(
            "字表：一级 {} 字（入简）／一至三级合计 {} 字",
            chars.iter().filter(|c| c.short_ok).count(),
            all_levels.len()
        );
        println!(
            "数据：字 {}，根 {}，未归并字形 {}",
            chars.len(),
            roots.len(),
            unresolved.len()
        );
        if !unresolved.is_empty() {
            println!("  未归并：{unresolved:?}");
        }
        Data {
            chars,
            roots,
        }
    }
}

fn load_roots(path: &str) -> Vec<Root> {
    let mut out = Vec::new();
    for line in read(path).lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 4 {
            continue;
        }
        out.push(Root {
            code: f[1].to_string(),
            glyphs: f[3].chars().collect(),
        });
    }
    out
}

/// 字频与码表：兼容两种格式
/// - Rime 风格 `字<TAB>码<TAB>权`（每字可多条，取最大权与最短码）
fn load_weights(path: &str) -> (HashMap<char, f64>, HashMap<char, Vec<String>>) {
    let mut w: HashMap<char, f64> = HashMap::new();
    let mut code: HashMap<char, Vec<String>> = HashMap::new();
    for line in read(path).lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 2 {
            continue;
        }
        let c = match f[0].chars().next() {
            Some(c) => c,
            None => continue,
        };
        // 两列视为「字<TAB>权」，三列视为「字<TAB>码<TAB>权」
        let (cd_opt, weight_str) = if f.len() >= 3 {
            (Some(f[1]), f[2])
        } else {
            (None, f[1])
        };
        let weight: f64 = weight_str.trim().parse().unwrap_or(0.0);
        w.entry(c)
            .and_modify(|x| {
                if weight > *x {
                    *x = weight;
                }
            })
            .or_insert(weight);
        if let Some(cd) = cd_opt {
            let e = code.entry(c).or_default();
            let cd = cd.to_string();
            if !e.contains(&cd) {
                e.push(cd);
            }
        }
    }
    (w, code)
}

fn load_splits(path: &str) -> Vec<(char, Vec<char>)> {
    let mut out = Vec::new();
    for line in read(path).lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 2 {
            continue;
        }
        let c = match f[0].trim().chars().next() {
            Some(c) => c,
            None => continue,
        };
        let toks: Vec<char> = f[1].trim().chars().collect();
        if toks.is_empty() {
            continue;
        }
        out.push((c, toks));
    }
    out
}

/// 读字表文件（`* 字` 或每行一字）
fn load_charset(path: &str) -> Vec<char> {
    read(path)
        .lines()
        .filter_map(|l| {
            let t = l.trim_start_matches(|c: char| c == '*' || c == '-' || c == '+' || c.is_whitespace());
            let c = t.chars().next()?;
            if c.is_whitespace() { None } else { Some(c) }
        })
        .collect()
}

/// 把拆分字形并到根上。证据顺序：上游字形 → NFKC 归一化 →
/// 「码 ↔ 拆分」反推码对 → 人工校正。返回 字形 → 根 id。
fn group_glyphs(
    roots: &[Root],
    splits: &[(char, Vec<char>)],
    full_codes: &HashMap<char, Vec<String>>,
    nfkc_path: Option<&str>,
) -> HashMap<char, u16> {
    let mut map: HashMap<char, u16> = HashMap::new();
    for (id, r) in roots.iter().enumerate() {
        for &g in &r.glyphs {
            map.insert(g, id as u16);
        }
    }
    let exact = map.len();

    // NFKC：康熙部首、兼容汉字等归一化后若命中官方字形，即并入该根
    let nfkc_text = nfkc_path.map(read_opt).unwrap_or_default();
    for line in nfkc_text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 2 || f[0] == f[1] {
            continue;
        }
        if let (Some(tok), Some(id)) = (f[0].chars().next(), map.get(&f[1].chars().next().unwrap()).copied()) {
            if !map.contains_key(&tok) {
                map.insert(tok, id);
            }
        }
    }
    let after_nfkc = map.len();

    // 码对反推：按「根数 / 全码长度」的规则给每个拆分字形投票
    let mut vote_b: HashMap<char, HashMap<char, usize>> = HashMap::new();
    let mut vote_s: HashMap<char, HashMap<char, usize>> = HashMap::new();
    let bump = |m: &mut HashMap<char, HashMap<char, usize>>, t: char, c: char| {
        m.entry(t).or_default().entry(c).and_modify(|x| *x += 1).or_insert(1);
    };
    for (c, toks) in splits {
        let code = match full_codes.get(c).and_then(|v| v.iter().max_by_key(|x| x.len())) {
            Some(x) => x,
            None => continue,
        };
        let cd: Vec<char> = code.chars().collect();
        let n = toks.len();
        let l = cd.len();
        match (n, l) {
            (1, 2) => {
                bump(&mut vote_b, toks[0], cd[0]);
                bump(&mut vote_s, toks[0], cd[1]);
            }
            (2, 3) => {
                bump(&mut vote_b, toks[0], cd[0]);
                bump(&mut vote_b, toks[1], cd[1]);
                bump(&mut vote_s, toks[1], cd[2]);
            }
            (2, 4) => {
                bump(&mut vote_b, toks[0], cd[0]);
                bump(&mut vote_b, toks[1], cd[1]);
                bump(&mut vote_s, toks[1], cd[2]);
                bump(&mut vote_s, toks[0], cd[3]);
            }
            (3, 4) => {
                bump(&mut vote_b, toks[0], cd[0]);
                bump(&mut vote_b, toks[1], cd[1]);
                bump(&mut vote_b, toks[2], cd[2]);
                bump(&mut vote_s, toks[2], cd[3]);
            }
            (k, 4) if k >= 4 => {
                bump(&mut vote_b, toks[0], cd[0]);
                bump(&mut vote_b, toks[1], cd[1]);
                bump(&mut vote_b, toks[2], cd[2]);
                bump(&mut vote_b, *toks.last().unwrap(), cd[3]);
            }
            _ => {}
        }
    }
    let best = |m: &HashMap<char, HashMap<char, usize>>, t: char| -> Option<char> {
        m.get(&t)
            .and_then(|h| h.iter().max_by_key(|(_, &n)| n).map(|(c, _)| *c))
    };
    // 同一码对可能对应多个根，取第一个
    let mut code2root: HashMap<String, u16> = HashMap::new();
    for (id, r) in roots.iter().enumerate() {
        code2root.entry(r.code.clone()).or_insert(id as u16);
    }
    let mut by_code = 0;
    for (c, _) in splits {
        let _ = c;
    }
    let mut tokens: Vec<char> = Vec::new();
    for (_, toks) in splits {
        for &t in toks {
            if !tokens.contains(&t) {
                tokens.push(t);
            }
        }
    }
    for t in &tokens {
        if map.contains_key(t) {
            continue;
        }
        if let (Some(b), Some(s)) = (best(&vote_b, *t), best(&vote_s, *t)) {
            let key: String = [b, s].iter().collect();
            if let Some(&id) = code2root.get(&key) {
                map.insert(*t, id);
                by_code += 1;
            }
        }
    }

    // 兜底：仍无归属的字形，按大码并入该大码下第一个根，并计数
    let mut fallback = 0;
    for t in &tokens {
        if map.contains_key(t) {
            continue;
        }
        if let Some(b) = best(&vote_b, *t) {
            eprintln!(
                "  兜底依据 {} U+{:04X}: 大码票 {:?} 小码票 {:?}",
                t,
                *t as u32,
                vote_b.get(t).map(|m| m.iter().map(|(k, v)| (k.to_string(), *v)).collect::<Vec<_>>()),
                vote_s.get(t).map(|m| m.iter().map(|(k, v)| (k.to_string(), *v)).collect::<Vec<_>>())
            );
            // 同一大码可能对应多个根：优先选「字形与它属于同一 Unicode 区块」的那个
            let cands: Vec<usize> = roots
                .iter()
                .enumerate()
                .filter(|(_, r)| r.code.chars().next() == Some(b))
                .map(|(i, _)| i)
                .collect();
            let blk = block(*t);
            let pick = cands
                .iter()
                .copied()
                .find(|&i| roots[i].glyphs.iter().any(|&g| block(g) == blk))
                .or_else(|| cands.first().copied());
            if let Some(id) = pick {
                eprintln!("兜底: {} U+{:04X} -> 根 {}（大码 {}，同区块命中 {}）", t, *t as u32, id, b, cands.len());
                map.insert(*t, id as u16);
                fallback += 1;
            }
        }
    }

    println!(
        "字形归并：官方 {exact}，NFKC 后 {after_nfkc}，码对反推 +{by_code}，兜底 {fallback}，合计 {}（拆分字形 {}）",
        map.len(),
        tokens.len()
    );
    let _ = NSYM;
    map
}

/// 粗分 Unicode 区块，用于判断两个字形是否「同族写法」。
fn block(c: char) -> u8 {
    let u = c as u32;
    match u {
        0x2E80..=0x2EFF => 1,   // 康熙部首
        0x2F00..=0x2FDF => 1,
        0x3005..=0x3007 => 2,   // 〇
        0x3040..=0x309F => 3,   // 平假名
        0x30A0..=0x30FF => 3,   // 片假名
        0x31C0..=0x31EF => 4,   // 笔画
        0x3400..=0x4DBF => 5,   // 扩展 A
        0x4E00..=0x9FFF => 5,   // 基本区
        0xF900..=0xFAFF => 5,   // 兼容汉字
        0xE000..=0xF8FF => 6,   // 私用区
        0x20000..=0x2FA1F => 7, // 扩展 B 及以后
        _ => 0,
    }
}



/// 强制占位表（`data/forced.tsv`）：`码<TAB>输出`，用于把标点等固定在某个码的首选位。
#[derive(Clone)]
pub struct Forced {
    /// 码的字符串（按键位符号书写，例如 "、" 或 ",,"）
    pub code: String,
    pub text: String,
}

pub fn load_forced(path: &str) -> Vec<Forced> {
    read_opt(path)
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() < 2 {
                return None;
            }
            let code = f[0].trim().to_string();
            let text = f[1].trim().to_string();
            if code.is_empty() || text.is_empty() {
                None
            } else {
                Some(Forced { code, text })
            }
        })
        .collect()
}