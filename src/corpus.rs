//! 语料复核：用真实语料字频给已定好的码表打分，不参与优化。

use crate::model::{rank_pen, Evaluator, Layout};
use std::collections::HashMap;
use std::fs;

pub fn eval_file(ev: &Evaluator, lay: &Layout, path: &str) {
    let txt = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("读语料 {path} 失败：{e}");
            return;
        }
    };
    let mut freq: HashMap<char, f64> = HashMap::new();
    for line in txt.lines() {
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
                freq.entry(c).or_insert(f);
            }
        }
    }
    if freq.is_empty() {
        return;
    }

    // 训练侧的码位总权
    let mut code_w: HashMap<u32, f64> = HashMap::new();
    for (i, ch) in ev.data.chars.iter().enumerate() {
        if ev.lens[i] > 0 {
            *code_w.entry(ev.codes[i]).or_insert(0.0) += ch.weight;
        }
    }
    // 码位 → (码长, 表内排名)
    let mut info: HashMap<char, (u32, u8, u16, f64, f64)> = HashMap::new();
    let mut seen: HashMap<u32, u16> = HashMap::new();
    let mut order: Vec<usize> = (0..ev.data.chars.len()).filter(|&i| ev.lens[i] > 0).collect();
    order.sort_by(|&a, &b| {
        ev.data.chars[b]
            .weight
            .partial_cmp(&ev.data.chars[a].weight)
            .unwrap()
    });
    for i in order {
        let cid = ev.codes[i];
        let r = seen.entry(cid).or_insert(0);
        *r += 1;
        let c = &ev.data.chars[i];
        info.insert(
            c.glyph,
            (cid, ev.lens[i], *r, code_w.get(&cid).copied().unwrap_or(0.0), c.weight),
        );
    }
    // 非训练字按逻辑全码计（与码表实际行为一致）
    for (i, c) in ev.data.chars.iter().enumerate() {
        if ev.lens[i] == 0 {
            let full = ev.full_code(&c.ids, lay);
            let cid = crate::keyboard::code_id(&full);
            let r = seen.entry(cid).or_insert(0);
            *r += 1;
            let total = code_w.get(&cid).copied().unwrap_or(0.0);
            info.insert(c.glyph, (cid, full.len() as u8, *r, total, c.weight));
        }
    }

    let (mut wsum, mut phys, mut keys, mut sel, mut hit) = (0.0, 0.0, 0.0, 0.0, 0usize);
    for (&c, &f) in &freq {
        if let Some(&(cid, len, rank, code_total, w)) = info.get(&c) {
            let syms = ev.symbols(cid, len);
            let pc = ev.kb.code_cost(&syms);
            let pen = if rank > 1 && code_total > 0.0 {
                rank_pen(ev.cfg, rank as usize) * (1.0 - w / code_total)
            } else {
                0.0
            };
            wsum += f;
            phys += f * pc;
            keys += f * len as f64;
            sel += f * pen;
            hit += 1;
        }
    }
    if wsum <= 0.0 {
        println!("语料 {path}：无可用行");
        return;
    }
    println!(
        "语料 {}：覆盖 {:.2}%（{}/{} 字），每字 击键 {:.4} + 选重 {:.4} = {:.4}，码长 {:.4}",
        path,
        100.0 * hit as f64 / freq.len() as f64,
        hit,
        freq.len(),
        phys / wsum,
        sel / wsum,
        (phys + sel) / wsum,
        keys / wsum
    );
}