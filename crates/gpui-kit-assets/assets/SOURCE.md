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

`NotoSansArabic.ttf` and `NotoSansHebrew.ttf` are the unmodified variable
families from `google/fonts` revision
`352f6b7d9d6cc4fa9e242b931291d31b21a6dc84`:

| File | Upstream path | Version | SHA-256 |
|---|---|---:|---|
| `NotoSansArabic.ttf` | `ofl/notosansarabic/NotoSansArabic[wdth,wght].ttf` | 2.012 | `63111b5b2e074dd48cc67692e0a2726d86ee94c1c37fe8598257b7b4e87e869e` |
| `NotoSansHebrew.ttf` | `ofl/notosanshebrew/NotoSansHebrew[wdth,wght].ttf` | 3.001 | `7ef36a2c3593758cdb622e1bdef4f84523e92fbc3ccc667438dd80ff54c2de88` |

Both are SIL Open Font License 1.1 fonts with no Reserved Font Name. They are
registered with Geist and carried as an explicit ordered fallback chain, so
Arabic and Hebrew output is reproducible in headless, native, and browser
renderers instead of depending on machine fonts.

## Icons

Most icons are Solar Icons, Linear weight, by 480 Design, licensed under
CC BY 4.0. Attribution: “Solar Icons by 480 Design.”

The terminal, plus, close, check, copy, return, info-circle, git-branch,
and mirrored sidebar glyphs are product-neutral icons derived from Comet,
Copyright (c) 2026 Wing, MIT licensed.

The calendar, checkbox-empty, checkbox-checked, double-arrow-left,
double-arrow-right, drag-handle, filter, forbidden, image, minus, pause, play,
sound-wave, star, star-filled, stop and video glyphs are original drawings for
this repository, on
the same sixteen-unit grid and 1.25 stroke as the set around them. They exist
because the components that needed them were drawing a character the embedded
fonts do not cover — a task list rendered its boxes as tofu, a date field
borrowed the checklist glyph, a transport had no play or pause of its own, and
a refused run wore the stop square that a running one stops with.

Product and provider marks from the source application are deliberately not
included in this generic asset crate.

See the repository `THIRD_PARTY_NOTICES` for source URLs, exact revisions, and
license references.
