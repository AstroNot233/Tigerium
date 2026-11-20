# 方案 profile

`config/default.toml` 放**算法参数**（键费矩阵、候选上限、选重罚、疲劳权重、退火策略）；
`profiles/<方案>.toml` 只放**数据源路径**（`[data]` 段）。两者叠加使用：

```sh
tigerium optimize --config config/default.toml --config profiles/tiger.toml \
                  --layout layouts/best.tsv --steps 300000 --seed 1 --out out
```

`--config` 可以给多次，后面的覆盖前面的；只给一个也能跑（默认就用 `config/default.toml`）。

## 接一个新方案需要准备三样东西

| 内容 | 格式 | 说明 |
| --- | --- | --- |
| 根集合 | `id<TAB>码对<TAB>字形数<TAB>字形串` | 每个根一行；字形串包含该根的所有同形变体，用于把拆分字形归并到根 |
| 拆分 | `字<TAB>拆分字形串` | 每字一行，拆分串里每个字符是一个字形 |
| 字频 | Rime 风格 `字<TAB>码<TAB>权` 或纯 `字<TAB>权` | 三列/两列自动识别；权重越大越常用 |

可选：`nfkc`（字形归一化）、`forced`（强制占位，如标点）、`level1/2/3`（简码资格字表）。

## 新增一个 profile

```toml
# profiles/myscheme.toml
[data]
roots = "schemes/myscheme/roots.tsv"
splits = "schemes/myscheme/splits.txt"
weights = "schemes/myscheme/weights.yaml"
```

然后 `--config config/default.toml --config profiles/myscheme.toml`。
如果该方案的键位数量或字母表不同，在同一份 profile 里再加 `[keyboard]`、`[alphabet]` 覆盖即可。
