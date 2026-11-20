//! Tigerium —— 基于字根的重排训练器。
//!
//! 用法：
//! ```text
//! tigerium optimize [--data data] [--config config/default.toml] [--out out] [--seed 1] [--steps 400000]
//! tigerium score    --layout out/layout.tsv
//! tigerium export   --layout out/layout.tsv --out out
//! ```
//!
//! 命令行参数与 `config/default.toml` 同名项等价，命令行优先。

mod config;
mod compare;
mod corpus;
mod data;
mod ergo;
mod export;
mod fatigue;
mod keyboard;
mod model;
mod search;

use config::Config;
use data::Data;
use model::Evaluator;

fn usage() {
    println!(
        r#"Tigerium · 钛铬利按

用法： tigerium <子命令> [选项]

子命令：
  optimize        搜索新的字根布局（默认）
  score           给一个布局打分
  export          出表（chars/keymap/roots/metrics，可选写出 Rime 码表）
  ergo            人体工学统计
  compare         与上游原方案在同口径下对比

常用选项：
  --config <文件>   配置文件，可重复给出，后面的覆盖前面的
                    （默认 config/default.toml；示例见 config/QWERTY-30.toml）
  --layout <文件>   读入已有布局（每行 根id<TAB>大码<TAB>小码）
  --out <目录>      输出目录，默认 out
  --seed <数>       随机种子
  --steps <数>      每轮退火步数
  --rounds <数>     重升温轮数
  --corpus <文件>   语料字频（可重复），用于 compare
  --rime <文件>     export 时额外写出 Rime 码表
  --p1/--p2 <数>    选重罚（覆盖配置）
  --caps <a,b,c,d>  分级候选上限（覆盖配置）
  --min-weight <数> 参与重排的权重下限
  --data <路径>     数据目录（默认 data）

示例：
  tigerium optimize --config config/QWERTY-30.toml --steps 400000 --seed 1 --out out
  tigerium score    --config config/QWERTY-30.toml --layout out/layout-1.tsv
  tigerium export   --config config/QWERTY-30.toml --layout out/layout-1.tsv --out out --rime rime/tigerium.dict.yaml
"#
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 1 || args[1] == "--help" || args[1] == "-h" || args[1] == "help" {
        usage();
        return;
    }
    let mut cmd = "optimize".to_string();
    let mut data_dir = "data".to_string();
    // 可重复给 --config：后面的覆盖前面的（公共参数 + 方案 profile）
    let mut config_paths: Vec<String> = Vec::new();
    let mut out_dir = "out".to_string();
    let mut layout_path = String::new();
    let mut corpora: Vec<String> = Vec::new();
    let mut rime_path = String::new();
    let mut cfg = Config::load("config/default.toml");

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "optimize" | "score" | "export" | "compare" | "ergo" => {
                cmd = args[i].clone();
                i += 1;
            }
            "--data" => {
                data_dir = args[i + 1].clone();
                i += 2;
            }
            "--config" => {
                config_paths.push(args[i + 1].clone());
                cfg = Config::load_all(&config_paths);
                i += 2;
            }
            "--out" => {
                out_dir = args[i + 1].clone();
                i += 2;
            }
            "--layout" => {
                layout_path = args[i + 1].clone();
                i += 2;
            }
            "--rime" => {
                rime_path = args[i + 1].clone();
                i += 2;
            }
            "--corpus" => {
                corpora.push(args[i + 1].clone());
                i += 2;
            }
            "--seed" => {
                cfg.seed = args[i + 1].parse().unwrap();
                i += 2;
            }
            "--steps" => {
                cfg.steps = args[i + 1].parse().unwrap();
                i += 2;
            }
            "--rounds" => {
                cfg.rounds = args[i + 1].parse().unwrap();
                i += 2;
            }
            "--p1" => {
                cfg.p1 = args[i + 1].parse().unwrap();
                i += 2;
            }
            "--p2" => {
                cfg.p2 = args[i + 1].parse().unwrap();
                i += 2;
            }
            "--caps" => {
                let v: Vec<usize> = args[i + 1]
                    .split(',')
                    .filter_map(|x| x.parse().ok())
                    .collect();
                for (j, x) in v.iter().take(4).enumerate() {
                    cfg.caps[j] = *x;
                }
                i += 2;
            }
            "--min-weight" => {
                cfg.min_weight = args[i + 1].parse().unwrap();
                i += 2;
            }
            other => {
                eprintln!("未知参数 {other}");
                std::process::exit(2);
            }
        }
    }


    println!(
        "Tigerium：data={data_dir} config={:?} 参数 p1={} p2={} caps={:?} last_small={}",
        if config_paths.is_empty() { vec!["config/default.toml".to_string()] } else { config_paths.clone() },
        cfg.p1, cfg.p2, cfg.caps, cfg.last_small
    );
    let data = Data::load(&cfg);
    let forced = match cfg.data.forced.as_deref() {
        Some(path) => data::load_forced(path),
        None => Vec::new(),
    };
    if !forced.is_empty() {
        eprintln!("强制占位 {} 条（{}）", forced.len(), cfg.data.forced.as_deref().unwrap_or("-"));
    }
    let mut ev = Evaluator::new(&data, &cfg, &forced);

    match cmd.as_str() {
        "score" => {
            let mut lay = export::read_layout(&layout_path);
            search::sanitize(&mut lay, &cfg);
            let m = ev.evaluate(&lay);
            for c in &corpora {
                corpus::eval_file(&ev, &lay, c);
            }
            println!(
                "费用 {:.6}（击键 {:.6} + 选重 {:.6} + 疲劳 {:.6} + 一简罚 {:.6}），码长 {:.6}，长度分布 {:?}",
                m.cost,
                m.keys,
                m.sel,
                m.fatigue,
                m.top,
                m.keys,
                &m.len_hist[1..]
            );
        }
        "compare" => {
            let lay = export::read_layout(&layout_path);
            compare::run(&data, &mut ev, &lay, &corpora, &data_dir);
        }
        "ergo" => {
            let lay = export::read_layout(&layout_path);
            ev.evaluate(&lay);
            let e = ergo::measure(&data, &ev);
            let o2 = ergo::measure_official(&data, &format!("{data_dir}/原码编码.yaml"));
            println!("上游：左右互击 {:.2}%｜同指大跨排 {:.2}%｜同指小跨排 {:.2}%｜小指干扰 {:.2}%｜错手 {:.2}%｜三连击 {:.2}%", o2.alt, o2.same_big, o2.same_small, o2.pinky, o2.wrong, o2.triple);
            println!("码内相邻转移 (字频加权) 共 {:.0} 次", e.bigrams);
            println!("左右互击 {:.2}%｜同指大跨排 {:.2}%｜同指小跨排 {:.2}%", e.alt, e.same_big, e.same_small);
            println!("小指干扰 {:.2}%｜错手 {:.2}%｜三连击 {:.2}%", e.pinky, e.wrong, e.triple);
        }
        "export" => {
            let lay = export::read_layout(&layout_path);
            export::export(&data, &ev, &lay, &out_dir).unwrap();
            if !rime_path.is_empty() {
                export::write_rime(&data, &ev, &lay, &rime_path, "tigerium").unwrap();
            }
            println!("已导出到 {out_dir}/");
        }
        _ => {
            let mut start = if layout_path.is_empty() {
                // 默认从官方码表反推的布局出发
                search::official_layout(&data, &cfg)
            } else {
                export::read_layout(&layout_path)
            };
            search::sanitize(&mut start, &cfg);
            let m0 = ev.evaluate(&start);
            for c in &corpora {
                corpus::eval_file(&ev, &start, c);
            }
            eprintln!("初值：费用 {:.4} 码长 {:.4}", m0.cost, m0.keys);
            // 多轮重升温：每轮从上一轮最优的布局出发，温度重新拉高
            let mut cur = start;
            let mut res = search::anneal(
                &mut ev,
                cur.clone(),
                cfg.steps,
                cfg.t0,
                cfg.t1,
                cfg.seed,
                cfg.moves,
            );
            for r in 1..cfg.rounds {
                eprintln!("第 {r} 轮重升温（起点 {:.4}）", res.best_cost);
                cur = res.best.clone();
                let again = search::anneal(
                    &mut ev,
                    cur,
                    cfg.steps,
                    cfg.t0,
                    cfg.t1,
                    cfg.seed + r as u64 * 7919,
                    cfg.moves,
                );
                if again.best_cost < res.best_cost {
                    res = again;
                }
            }
            eprintln!("收敛：费用 {:.4}", res.best_cost);
            std::fs::create_dir_all(&out_dir).unwrap();
            export::write_layout(&res.best, &format!("{out_dir}/layout-{}.tsv", cfg.seed)).unwrap();
            export::export(&data, &ev, &res.best, &out_dir).unwrap();
        }
    }
}
