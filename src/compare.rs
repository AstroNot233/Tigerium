//! 与上游方案同口径对比：重码率 / 码长 / 带选码长。口径见 AGENTS.md。

use crate::data::Data;
use crate::model::{Evaluator, Layout};
use std::collections::HashMap;
use std::fs;

pub struct Row {
    pub scope: String,
    pub chars: usize,
    pub mean_len: f64,
    pub mean_len_sel: f64,
    pub codes: usize,
    pub collided_codes: usize,
    pub collided_chars: usize,
    pub collided_w: f64,
}

fn score(scope: &str, items: &[(char, f64, String)]) -> Row {
    // 同一码位内按字频降序排名
    let mut by_code: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, it) in items.iter().enumerate() {
        by_code.entry(it.2.as_str()).or_default().push(i);
    }
    let mut rank = vec![0usize; items.len()];
    for (_, idxs) in by_code.iter() {
        let mut v = idxs.clone();
        v.sort_by(|&a, &b| items[b].1.partial_cmp(&items[a].1).unwrap());
        for (r, &i) in v.iter().enumerate() {
            rank[i] = r + 1;
        }
    }
    let (mut w, mut len_w, mut len_sel_w) = (0.0, 0.0, 0.0);
    let mut collided_codes = 0usize;
    let mut collided_chars = 0usize;
    let mut collided_w = 0.0;
    for (i, it) in items.iter().enumerate() {
        let l = it.2.chars().count();
        let extra = if rank[i] > 1 { 1.0 } else { 0.0 };
        w += it.1;
        len_w += it.1 * l as f64;
        len_sel_w += it.1 * (l as f64 + extra);
    }
    for (_, idxs) in by_code.iter() {
        if idxs.len() >= 2 {
            collided_codes += 1;
            collided_chars += idxs.len();
            for &i in idxs {
                collided_w += items[i].1;
            }
        }
    }
    Row {
        scope: scope.to_string(),
        chars: items.len(),
        mean_len: if w > 0.0 { len_w / w } else { 0.0 },
        mean_len_sel: if w > 0.0 { len_sel_w / w } else { 0.0 },
        codes: by_code.len(),
        collided_codes,
        collided_chars,
        collided_w: if w > 0.0 { 100.0 * collided_w / w } else { 0.0 },
    }
}

fn load_set(path: &str) -> Vec<char> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let t = l.trim_start_matches(|c: char| c == '*' || c == '-' || c == '+' || c.is_whitespace());
            let c = t.chars().next()?;
            if c.is_whitespace() {
                None
            } else {
                Some(c)
            }
        })
        .collect()
}

/// 官方码表：每个字取最短的一条码
fn official_codes(path: &str) -> HashMap<char, String> {
    let mut m: HashMap<char, String> = HashMap::new();
    for line in fs::read_to_string(path).unwrap_or_default().lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 2 {
            continue;
        }
        let c = match f[0].chars().next() {
            Some(c) => c,
            None => continue,
        };
        let code = f[1].to_string();
        m.entry(c)
            .and_modify(|x| {
                if code.chars().count() < x.chars().count() {
                    *x = code.clone();
                }
            })
            .or_insert(code);
    }
    m
}

fn corpus(path: &str) -> Vec<(char, f64)> {
    let mut out = Vec::new();
    for line in fs::read_to_string(path).unwrap_or_default().lines() {
        let toks: Vec<&str> = line
            .split(|c: char| c == '\t' || c == ' ' || c == ',')
            .map(|x| x.trim())
            .filter(|x| !x.is_empty())
            .collect();
        if toks.len() < 2 {
            continue;
        }
        let (mut num, mut ch) = (None, None);
        for t in &toks {
            match t.parse::<f64>() {
                Ok(v) => {
                    if num.is_none() {
                        num = Some(v);
                    }
                }
                Err(_) => {
                    if ch.is_none() {
                        ch = t.chars().next();
                    }
                }
            }
        }
        if let (Some(f), Some(c)) = (num, ch) {
            if f > 0.0 {
                out.push((c, f));
            }
        }
    }
    out
}

pub fn run(data: &Data, ev: &mut Evaluator, lay: &Layout, corpora: &[String], data_dir: &str) {
    ev.evaluate(lay);
    let mut ours: HashMap<char, String> = HashMap::new();
    for (i, c) in data.chars.iter().enumerate() {
        let code = if ev.lens[i] > 0 {
            ev.decode(ev.codes[i], ev.lens[i])
        } else {
            crate::keyboard::render(&ev.full_code(&c.ids, lay), &ev.cfg.alphabet)
        };
        ours.insert(c.glyph, code);
    }
    let off = official_codes(&format!("{data_dir}/原码编码.yaml"));

    let l1 = load_set("std/level-1.txt");
    let l2 = load_set("std/level-2.txt");
    let l3 = load_set("std/level-3.txt");
    let scopes: Vec<(&str, Vec<char>)> = vec![
        ("一级", l1.clone()),
        ("一二级", l1.iter().chain(l2.iter()).copied().collect()),
        (
            "一二三级",
            l1.iter().chain(l2.iter()).chain(l3.iter()).copied().collect(),
        ),
        ("全集", data.chars.iter().map(|c| c.glyph).collect()),
    ];

    let weight: HashMap<char, f64> = data.chars.iter().map(|c| (c.glyph, c.weight)).collect();

    let mut table: Vec<(Row, Row)> = Vec::new();
    for (name, set) in &scopes {
        let items: Vec<(char, f64, String)> = set
            .iter()
            .filter_map(|&c| {
                let w = *weight.get(&c).unwrap_or(&0.0);
                Some((c, w, ours.get(&c)?.clone()))
            })
            .collect();
        let items_off: Vec<(char, f64, String)> = set
            .iter()
            .filter_map(|&c| {
                let w = *weight.get(&c).unwrap_or(&0.0);
                Some((c, w, off.get(&c)?.clone()))
            })
            .collect();
        table.push((score(name, &items), score(name, &items_off)));
    }

    // 语料
    for path in corpora {
        let freq = corpus(path);
        let build = |map: &HashMap<char, String>| -> Vec<(char, f64, String)> {
            freq.iter()
                .filter_map(|&(c, f)| Some((c, f, map.get(&c)?.clone())))
                .collect()
        };
        let a = build(&ours);
        let b = build(&off);
        let name = format!("语料 {}", std::path::Path::new(path).file_stem().unwrap().to_string_lossy());
        table.push((score(&name, &a), score(&name, &b)));
    }

    println!("\n| 范围 | 方案 | 字数 | 码位 | 重码码位 | 重码字 | 重码频次 | 码长 | 带选码长 |");
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for (a, b) in &table {
        for (r, name) in [(a, "Tigerium"), (b, "上游")] {
            println!(
                "| {} | {} | {} | {} | {} | {} | {:.1}% | {:.4} | {:.4} |",
                r.scope,
                name,
                r.chars,
                r.codes,
                r.collided_codes,
                r.collided_chars,
                r.collided_w,
                r.mean_len,
                r.mean_len_sel
            );
        }
    }
}