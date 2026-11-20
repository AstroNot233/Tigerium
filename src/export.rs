//! 出表：键位表、字根表、码表、指标。

use crate::data::Data;
use crate::keyboard::{code_id, decode, render};
use crate::model::{Evaluator, Layout};
use std::fs::{self, File};
use std::io::Write;

pub fn export(data: &Data, ev: &Evaluator, lay: &Layout, dir: &str) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;

    // 字表：所有字（参与训练的用贪心结果，其余按逻辑全码）
    let mut ev2 = crate::model::Evaluator::new(data, ev.cfg, &ev.forced_list);
    let m = ev2.evaluate(lay);
    let mut f = File::create(format!("{dir}/chars.tsv"))?;
    for (i, c) in data.chars.iter().enumerate() {
        let (code, len) = if ev2.lens[i] > 0 {
            (ev2.decode(ev2.codes[i], ev2.lens[i]), ev2.lens[i])
        } else {
            let full = ev.full_code(&c.ids, lay);
            (render(&full, &ev.cfg.alphabet), full.len() as u8)
        };
        let full = ev.full_code(&c.ids, lay);
        let full_s: String = render(&full, &ev.cfg.alphabet);
        let split: Vec<String> = c
            .ids
            .iter()
            .map(|&r| data.roots[r as usize].glyphs[0].to_string())
            .collect();
        writeln!(
            f,
            "{}\t{:.0}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            c.glyph,
            c.weight,
            c.ids.len(),
            split.join("|"),
            code,
            full_s,
            len,
            if c.short_ok { 1 } else { 0 },
            if ev2.lens[i] > 0 { 1 } else { 0 }
        )?;
    }

    // 键位表
    let mut g = File::create(format!("{dir}/keymap.tsv"))?;
    for k in 0..crate::keyboard::NSYM {
        let mut rs: Vec<usize> = (0..data.roots.len()).filter(|&r| lay.b[r] as usize == k).collect();
        rs.sort_by(|&a, &b| root_weight(data, b).partial_cmp(&root_weight(data, a)).unwrap());
        let list: Vec<String> = rs
            .iter()
            .map(|&r| {
                format!(
                    "{}{}",
                    data.roots[r].glyphs.iter().collect::<String>(),
                    render(&decode(code_id(&[lay.b[r], lay.s[r]]), 2), &ev.cfg.alphabet)
                )
            })
            .collect();
        writeln!(g, "{}\t{}\t{}", k, ev.cfg.alphabet[k], list.join(" "))?;
    }

    // 字根表
    let mut h = File::create(format!("{dir}/roots.tsv"))?;
    for (r, root) in data.roots.iter().enumerate() {
        writeln!(
            h,
            "{}\t{}{}\t{}\t{}\t{:.0}",
            r,
            ev.cfg.alphabet[lay.b[r] as usize],
            ev.cfg.alphabet[lay.s[r] as usize],
            root.code,
            root.glyphs.iter().collect::<String>(),
            root_weight(data, r)
        )?;
    }

    // 指标
    // 边缘列（小指+无名指所在的 0/1/8/9 列）承担的首键字频占比
    let mut col_w = [0f64; 10];
    let mut all_w = 0f64;
    for (i, c) in data.chars.iter().enumerate() {
        if ev2.lens[i] > 0 {
            let first = (ev2.codes[i] as usize - crate::keyboard::OFF[ev2.lens[i] as usize])
                / 30u32.pow(ev2.lens[i] as u32 - 1) as usize;
            col_w[first % 10] += c.weight;
            all_w += c.weight;
        }
    }
    let edge: f64 = col_w[0] + col_w[1] + col_w[8] + col_w[9];

    let mut t = File::create(format!("{dir}/metrics.txt"))?;
    writeln!(t, "cost\t{:.6}", m.cost)?;
    writeln!(t, "keystroke\t{:.6}", m.keys)?;
    writeln!(t, "select\t{:.6}", m.sel)?;
    writeln!(t, "fatigue\t{:.6}", m.fatigue)?;
    writeln!(t, "keys\t{:.6}", m.keys)?;
    writeln!(t, "sel\t{:.6}", m.sel)?;
    writeln!(t, "len\t{:?}", &m.len_hist[1..])?;
    writeln!(t, "edge_col_pct\t{:.1}", 100.0 * edge / all_w)?;
    writeln!(t, "col_share\t{:?}", col_w.map(|x| (x / all_w * 1000.0).round() / 10.0))?;
    println!(
        "费用 {:.4}（按键 {:.4} + 选重 {:.4}），码长 {:.4}，长度分布 {:?}",
        m.cost,
        m.cost - m.sel,
        m.sel,
        m.keys,
        &m.len_hist[1..]
    );
    Ok(())
}

fn root_weight(data: &Data, r: usize) -> f64 {
    data.chars
        .iter()
        .filter(|c| c.ids.contains(&(r as u16)))
        .map(|c| c.weight)
        .sum()
}

/// 读入布局文件：每行 `根id<TAB>大码<TAB>小码`
pub fn read_layout(path: &str) -> Layout {
    let txt = fs::read_to_string(path).unwrap_or_else(|e| panic!("读布局 {path} 失败：{e}"));
    let mut rows: Vec<(usize, u8, u8)> = Vec::new();
    for line in txt.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 3 {
            continue;
        }
        if let (Ok(id), Ok(b), Ok(s)) = (f[0].parse(), f[1].parse(), f[2].parse()) {
            rows.push((id, b, s));
        }
    }
    let n = rows.iter().map(|r| r.0 + 1).max().unwrap_or(0);
    let mut lay = Layout::new(n);
    for (id, b, s) in rows {
        lay.b[id] = b;
        lay.s[id] = s;
    }
    lay
}

pub fn write_layout(lay: &Layout, path: &str) -> std::io::Result<()> {
    let mut f = File::create(path)?;
    for r in 0..lay.b.len() {
        writeln!(f, "{}\t{}\t{}", r, lay.b[r], lay.s[r])?;
    }
    Ok(())
}

/// 输出 Rime 码表（`---` 头 + `字<TAB>码<TAB>权`），可直接放进 Rime 的 dicts 目录试打。
pub fn write_rime(
    data: &Data,
    ev: &Evaluator,
    lay: &Layout,
    path: &str,
    name: &str,
) -> std::io::Result<()> {
    let mut ev2 = crate::model::Evaluator::new(data, ev.cfg, &ev.forced_list);
    ev2.evaluate(lay);
    let mut rows: Vec<(f64, String, String)> = Vec::with_capacity(data.chars.len() * 2);
    for (i, c) in data.chars.iter().enumerate() {
        let full = render(&ev.full_code(&c.ids, lay), &ev.cfg.alphabet);
        let code = if ev2.lens[i] > 0 {
            ev2.decode(ev2.codes[i], ev2.lens[i])
        } else {
            full.clone()
        };
        rows.push((c.weight, c.glyph.to_string(), code.clone()));
        // 出了简码的字仍保留逻辑全码，权重压到末尾（词组与拆字输入要用）
        if c.short_ok && code != full {
            rows.push((0.0, c.glyph.to_string(), full));
        }
    }
    rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

    let mut f = File::create(path)?;
    writeln!(f, "---")?;
    writeln!(f, "name: {name}")?;
    writeln!(f, "version: \"0.1\"")?;
    writeln!(f, "sort: by_weight")?;
    writeln!(f, "use_preset_vocabulary: false")?;
    writeln!(f, "columns:")?;
    writeln!(f, "  - text")?;
    writeln!(f, "  - code")?;
    writeln!(f, "  - weight")?;
    writeln!(f, "...")?;
    let rows_out = rows.len();
    for (w, text, code) in rows {
        writeln!(f, "{text}\t{code}\t{w:.0}")?;
    }
    // 强制占位（标点等）也写进码表，权重给到最高，保证排在首位
    for fc in &ev.forced_list {
        writeln!(f, "{}\t{}\t{}", fc.text, fc.code, 1_000_000_000_000u64)?;
    }
    println!("已写出 Rime 码表 {path}（{} 行）", rows_out + ev.forced_list.len());
    Ok(())
}
