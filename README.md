# Tigerium · 钛铬利按

Tigerium 并不是一份具体的形码输入方案，而是一款基于字根的重排训练器：
给一套字根、一套拆分、一份字频，它就能把字根重新铺到你定制的键位上，并输出键位表与可直接安装的码表。

面向追求高度自定义的用户，训练参数可精细调整。

> **为什么叫Tigerium** \
  程序本身不绑定任何具体方案，但启发自[虎码](https://www.tiger-code.com/)。\
  所谓“吃水不忘挖井人”，即使最终不与任何方案绑定，也保留虎码作为命名的一部分。

## 1. 获取程序

**下载预编译版本**：到 [Releases](../../releases) 页面取对应平台的压缩包，解压即得可执行文件。

| 平台 | 文件 |
| --- | --- |
| Linux x86-64 | `tigerium-x86_64-linux.tar.gz` |
| Windows x86-64 | `tigerium-x86_64-windows.zip`（含 `tigerium.exe`） |

**自行编译**：需要 Rust 工具链（`cargo`）。

```sh
cargo build --release
```

产物：`./target/release/tigerium`。

## 2. 准备数据

本仓库不含上游方案的数据，请自行准备三个文件放进 `data/`：

- 字根表
- 拆分方案 —— 字 → 根序列
- 字频（Rime 风格码表或纯字频都行）

格式与获取方式见 [`data/README.md`](data/README.md)。

## 3. 选配置

请注意：Tigerium使用的编码逻辑可能与你预期的不符！
默认情况下Tigerium采用以下方式对字编码（逻辑全码）：
| 字根数 | 编码形式  | 说明 |
|:------:|:---------:|:----:|
| 一根字 | `Aa(Aax)` | x（如果有）会是你指定的一组无理码（无理码只用于单根字筛重）之一，指定为空集来禁用无理码。 |
| 二根字 | `ABba`    | 逻辑全码有回头码。 |
| 三根字 | `ABCc`    | — |
| 多根字 | `ABCZ`    | — |

打逻辑全码时，一定能找到想要的字，但是可能要选重。
简码是逻辑全码的前缀部分，没有无理简码。
逻辑全码撞码时，没有简码的字优先。

如果你不喜欢一根字的设计，详见以下文件修改一根字策略。
支持的策略还有`Aa(Aaa)`式和`Aa(AaAa)`式，还有其他需要可以借助AI分析本项目，自定义策略。

直接改 [`config/QWERTY-30.toml`](config/QWERTY-30.toml)（30 键）或
[`config/QWERTY-26.toml`](config/QWERTY-26.toml)（26 键）即可，两份文件每个字段都有注释。
若要换上游方案，按 [`profiles/README.md`](profiles/README.md) 写一份自己的 profile。

## 4. 运行

先看一遍全部选项：

```sh
./target/release/tigerium --help
```

常用命令：

```sh
# 从头训练
./target/release/tigerium optimize --config config/QWERTY-30.toml --steps 400000 --seed 1 --out out

# 从已有布局继续（--rounds 是重升温轮数）
./target/release/tigerium optimize --config config/QWERTY-30.toml --layout out/layout-1.tsv \
    --steps 200000 --rounds 3 --seed 2 --out out

# 只给某个布局打分
./target/release/tigerium score --config config/QWERTY-30.toml --layout out/layout-1.tsv

# 出表：chars/keymap/roots/metrics，并可选写出 Rime 码表
./target/release/tigerium export --config config/QWERTY-30.toml --layout out/layout-1.tsv \
    --out out --rime rime/tigerium.dict.yaml

# 人体工学统计
./target/release/tigerium ergo --layout out/layout-1.tsv

# 与上游原方案对比（可给多个语料）
./target/release/tigerium compare --layout out/layout-1.tsv --corpus <语料字频文件>
```

> 建议
  先从头训练出一个不考虑任何人体工程学、只以理论效率为目标的布局，
  然后再基于它逐渐提高人体工程学权重继续训练，直到达到你认为兼顾了两者平衡。
  一开始就将人体工程学权重设得很高，容易优化得过于保守。

## 5. 输出

| 文件 | 内容 |
| --- | --- |
| `out/layout-<seed>.tsv` | 布局：每行 `根id<TAB>大码<TAB>小码`，可再次喂给 `--layout` |
| `out/chars.tsv` | 逐字结果：字、权重、根数、拆分、可打码、逻辑全码、码长… |
| `out/keymap.tsv` | 每键上的字根 |
| `out/roots.tsv` | 字根表：新码对、原码、字形串 |
| `out/metrics.txt` | 各项指标（击键费、选重费、疲劳值、列负载等） |
| `rime/tigerium.dict.yaml` | 加 `--rime` 时写出，可直接装进 Rime；出了简码的字会额外补一条逻辑全码（权重 0），供词组与拆字输入 |

想要人类可读的三张表（键位表 / 字根表 / 码表）：

```sh
nu tools/mk_tables.nu out
```

## 6. 安装到 Rime

把 `rime/tigerium.schema.yaml` 与上一步生成的 `rime/tigerium.dict.yaml` 一起放进 Rime 用户目录，
重新部署即可。

## 许可证

GPL-3.0-only，见 [LICENSE](LICENSE)。

字根划分、拆分方案与码表的版权归各自方案作者所有，本仓库不分发这些数据。
