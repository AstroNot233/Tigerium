//! 从 `pdftotext -layout` 的文本里抽字根表。折行规则见 AGENTS.md。

use std::fs;
use std::io::Write;

pub fn run(input: &str, out: &str) -> std::io::Result<()> {
    let txt = fs::read_to_string(input)?;
    let mut rows: Vec<(String, String, bool)> = Vec::new(); // (码, 字形, 本行是否内联)
    let mut pending = String::new();

    for raw in txt.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.contains("主根+变体")
            || line.contains("虎码全字根归并")
            || line.starts_with("注：")
            || line == "编"
            || line == "码"
            || line.starts_with('😀')
            || line.chars().next().map(|c| c.is_ascii_digit()) == Some(true)
        {
            continue;
        }
        let bytes: Vec<char> = line.chars().collect();
        let is_code = bytes.len() >= 2
            && bytes[0].is_ascii_lowercase()
            && bytes[1].is_ascii_lowercase()
            && (bytes.len() == 2 || bytes[2].is_whitespace());
        if is_code {
            let code: String = bytes[0..2].iter().collect();
            let glyphs: String = bytes[2..].iter().filter(|c| !c.is_whitespace()).collect();
            if !pending.is_empty() {
                // 上一行是内联字形行 → 这段折行属于本行；否则属于上一个裸码行
                let prev_inline = rows.last().map(|r| r.2).unwrap_or(true);
                if !prev_inline {
                    if let Some(last) = rows.last_mut() {
                        last.1.push_str(&pending);
                    }
                    pending.clear();
                }
            }
            // 第三个字段记录「码这一行本身是否带字形」，与是否吸收了折行段无关
            let had_inline = !glyphs.is_empty();
            if !pending.is_empty() {
                let mut g = std::mem::take(&mut pending);
                g.push_str(&glyphs);
                rows.push((code, g, had_inline));
            } else {
                rows.push((code, glyphs, had_inline));
            }
        } else {
            // 纯字形行
            let joined: String = line.chars().filter(|c| !c.is_whitespace()).collect();
            match rows.last() {
                Some((_, _, inline)) if !*inline => {
                    if let Some(last) = rows.last_mut() {
                        last.1.push_str(&joined);
                    }
                }
                _ => pending.push_str(&joined),
            }
        }
    }
    if !pending.is_empty() {
        if let Some(last) = rows.last_mut() {
            last.1.push_str(&pending);
        }
    }

    let mut f = fs::File::create(out)?;
    let mut total = 0usize;
    for (i, (code, glyphs, _)) in rows.iter().enumerate() {
        let n = glyphs.chars().count();
        total += n;
        writeln!(f, "{i}\t{code}\t{n}\t{glyphs}")?;
    }
    println!("抽出 {} 组根，字形合计 {}", rows.len(), total);
    Ok(())
}