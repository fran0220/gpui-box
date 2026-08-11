# Asset provenance

## Fonts

Geist and Geist Mono are Copyright 2023 Vercel Inc. and licensed under the
SIL Open Font License 1.1. The variable faces and static Medium, SemiBold, and
Bold faces were copied unmodified from Comet at
`fb22e269ac57331ee7aa4a9673530acf3299a886`.

`KeySymbols.ttf` carries the seven keyboard symbols `Kbd` can emit that no
Geist face draws. Geist covers `⇧` U+21E7 and `⇥` U+21E5; it does not cover
`⌘` U+2318, `⌃` U+2303, `⌥` U+2325, `⏎` U+23CE, `⌦` U+2326, `⌫` U+232B, or
`␣` U+2423. Until this face existed those glyphs were drawn by whatever font
the host machine happened to install, so the library's own output depended on
the machine rather than on the caller's data, and no baseline could record it.

It is a subset of two Noto families, both Copyright 2022 The Noto Project
Authors and licensed under the SIL Open Font License 1.1 with no Reserved Font
Name, taken from `notofonts/notofonts.github.io` at
`c16b117609abbe4e60b3f2bd4433bdb3d0accb2e`:

| Source                                | Version | Symbols                |
| ------------------------------------- | ------- | ---------------------- |
| `NotoSansSymbols2-Regular.ttf`        | 2.008   | `⌘ ⌥ ⏎ ⌦ ⌫ ␣`           |
| `NotoSansSymbols-Regular.ttf`         | 2.003   | `⌃`                    |

To rebuild it, subset each source to its own symbols with
`python3 -m fontTools.subset <source> --unicodes=… --layout-features=
--no-hinting --desubroutinize --drop-tables+=DSIG --name-IDs='*'`, merge the
two results with `fontTools.merge`, then rewrite the name records to the family
`GPUI Kit Key Symbols` and copy the `hhea` and `OS/2` vertical metrics from
`Geist.ttf`. The rename keeps the subset from shadowing a full Noto family on
a host that has one; the copied metrics keep a fallback glyph from changing the
line box of the Geist run it appears in.

## Icons

Most icons are Solar Icons, Linear weight, by 480 Design, licensed under
CC BY 4.0. Attribution: “Solar Icons by 480 Design.”

The terminal, plus, close, stop, check, copy, return, info-circle, git-branch,
and mirrored sidebar glyphs are product-neutral icons derived from Comet,
Copyright (c) 2026 Wing, MIT licensed.

Product and provider marks from the source application are deliberately not
included in this generic asset crate.

See the repository `THIRD_PARTY_NOTICES` for source URLs, exact revisions, and
license references.
