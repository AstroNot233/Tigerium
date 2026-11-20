# 把 out/ 下的机器表渲染成人类可读的三张表：键位表 / 字根表 / 码表
# 用法： nu tools/mk_tables.nu [out_dir] [forced.tsv]
def main [dir = "out", forced_file = "data/forced.tsv"] {
  let chars = (open --raw $"($dir)/chars.tsv" | lines | each {|l|
    let f = ($l | split row "\t")
    {c: $f.0, w: ($f.1 | into int), n: ($f.2 | into int), sp: $f.3, code: $f.4, full: $f.5, len: ($f.6 | into int), short: ($f.7 | into int), tr: ($f.8 | into int)}
  })
  let keys = (open --raw $"($dir)/keymap.tsv" | lines | each {|l|
    let f = ($l | split row "\t")
    {k: ($f.0 | into int), sym: $f.1, roots: $f.2}
  })
  let roots = (open --raw $"($dir)/roots.tsv" | lines | each {|l|
    let f = ($l | split row "\t")
    {id: ($f.0 | into int), code: $f.1, old: $f.2, glyphs: $f.3, w: ($f.4 | into int)}
  })

  # 一简：每个键上只放一个字；有强制占位（标点）时优先显示它

  # 一简：每个键上只放一个字；有强制占位（标点）时优先显示它
  let one = ($chars | where {|r| $r.len == 1 } | reduce -f {} {|r, a| $a | insert $r.code $r.c })
  let forced = (if ($forced_file | path exists) {
    open --raw $forced_file | lines | where {|l| ($l | str contains "	") }
    | reduce -f {} {|l, a| let f = ($l | split row "	"); $a | insert $f.0 ($f | get 1) }
  } else { {} })
  let one = ($one | merge $forced)
  let rows = ($keys | sort-by k | each {|x| $one | get --optional $x.sym | default "·" })

  let top = (0..2 | each {|row|
    let seg = ($keys | where {|x| ($x.k / 10 | math floor) == $row } | sort-by k)
    let line1 = ($seg | each {|x| $"($x.sym)=($one | get --optional $x.sym | default "·")" } | str join " ")
    $line1
  })

  let header = [
    "Tigerium · 键位表"
    $"生成自 ($dir)，共 ($keys | length) 键 / ($roots | length) 组根"
    ""
    $"上排  ($top | get 0)"
    $"主行  ($top | get 1)"
    $"下排  ($top | get 2)"
    ""
    "明细（键 → 该键上的根，按根权重降序；后缀是根的新码对）"
  ]
  $header | save --force $"($dir)/键位表.txt"

  let detail = ($keys | sort-by k | each {|x| $"($x.sym)\t($x.roots)" })
  $detail | save --append $"($dir)/键位表.txt"

  let rootlines = (["Tigerium · 字根表" $"根 ($roots | length) 组；码对列为新方案，原码列为上游码" ""]
    | append ($roots | sort-by w --reverse | each {|r| $"($r.code)\t($r.old)\t($r.w)\t($r.glyphs)" }))
  $rootlines | save --force $"($dir)/字根表.txt"

  let codelines = (["Tigerium · 码表"
    "格式：字<	>码<	>权<	>码长"
    ""]
    | append ($chars | where {|r| $r.tr == 1 } | sort-by w --reverse | each {|r| $"($r.c)\t($r.code)\t($r.w)\t($r.len)" }))
  $codelines | save --force $"($dir)/码表.txt"

  print $"已渲染：($dir)/键位表.txt、($dir)/字根表.txt、($dir)/码表.txt"
}