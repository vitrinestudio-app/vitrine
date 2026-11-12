# CJK font packs – proposal (2026-11-12)

Status: **proposal, not implemented** – the owner decides (see
`docs/plan/STATUS.md`, open questions). Context: ADR 0028.

## Problem

Chinese, Japanese and Korean need fonts of 10–20 MB each; bundling them
would triple the app size (measured sizes below). Today (ADR 0028) CJK
texts use an installed system font chosen per language (Hiragino Sans /
Yu Gothic for `ja`, PingFang SC / Hiragino Sans GB / Microsoft YaHei for
`zh-Hans` …). That works on macOS and Windows desktops, but:

- a build server (Linux CI, Docker) usually has no CJK font → `missing_glyphs`;
- two machines may pick different families (PingFang on one Mac, Hiragino
  Sans GB on another where PingFang is not downloaded) → different line
  breaks and auto-fit sizes;
- storing Apple's or Microsoft's CJK fonts in a project is not allowed for
  public repositories (licence).

An optional, **app-wide** download of OFL CJK fonts gives identical output
everywhere it is installed, and the files may legally go into projects and
templates.

## Candidates (all SIL OFL 1.1)

Sizes of the Google Fonts builds (`github.com/google/fonts/ofl/…`,
variable `wght` 100–900, one file per language; checked 2026-11-11):

| Pack id | Family | Languages | Size |
|---|---|---|---:|
| `cjk-ja` | Noto Sans JP | `ja` | 9.6 MB |
| `cjk-ko` | Noto Sans KR | `ko` | 10.4 MB |
| `cjk-sc` | Noto Sans SC | `zh-Hans`, `zh-CN`, `zh-SG`, `zh` | 17.8 MB |
| `cjk-tc` | Noto Sans TC | `zh-Hant`, `zh-TW` | 11.9 MB |
| `cjk-hk` | Noto Sans HK | `zh-HK`, `zh-MO` | ~12 MB |

Alternatives: the upstream Noto Sans CJK OTC (all languages in one
collection, ~20 MB per weight – larger and static), Source Han Sans (same
design, Adobe, OFL), serif variants (Noto Serif JP/SC/…) as a second pack
later. Subsetting to frequent characters would cut sizes by ~70 %, but
store texts contain rare characters and subsets change glyph coverage
silently – not proposed.

## Proposal

1. **Manifest in the app** (`crates/core/data/font-packs.yaml`, embedded
   like the catalogs): pack id, family, languages, file name, size,
   **SHA-256**, licence (OFL), source URL(s) at a pinned commit, the
   OFL text's SHA-256. The app only installs files whose hash matches –
   no trust in the server.
2. **Hosting**: GitHub release assets of the Vitrine Studio repository
   (`font-packs-1`, one asset per pack, immutable) as the primary URL,
   with the pinned `raw.githubusercontent.com/google/fonts/<commit>/…`
   URL as fallback. `api.vitrinestudio.app` is possible but costs our
   bandwidth (≈ 60 MB per user who installs everything); GitHub's CDN is
   free and fast. Proposed: GitHub release assets + raw fallback.
3. **Install location**: app-wide `<app data>/fonts/packs/<id>/` (font +
   `OFL.txt` + `pack.yaml` with the hash), shared by all projects, like
   imported frames (ADR 0012). Download in the app layer (reqwest + rustls
   are already used by `publish`; core stays network-free), with progress
   events and cancel; resumable is not needed at this size.
4. **Resolution**: a new `FontSource::Pack`, loaded lazily (mmap) like
   system fonts but **deterministic**: order bundled → project → packs →
   installed system fonts. For CJK characters an installed pack of the
   text's language comes before the system preference list, so a Mac with
   the `ja` pack renders exactly like CI with the `ja` pack.
5. **Front ends**:
   - GUI: Project settings → Fonts → "Fonts for Chinese, Japanese,
     Korean": one row per pack with size and *Download* / *Remove*; a
     validation info `cjk_font_pack_available` with a fix button when a
     project has a CJK language and no pack/project font for it.
   - CLI: `vitrine fonts pack list|install <id>…|remove <id>`
     (`--app-dir`), and `VITRINE_FONT_PACKS=ja,sc` for CI images
     (`vitrine fonts pack install` in the Dockerfile).
   - MCP: read-only (`catalog_fonts` lists installed packs); installing
     stays a user action (download size).
6. **Projects and templates**: packs are OFL, so "Copy to project"
   (`fonts/`) is allowed and makes a public repository reproducible
   without the pack; templates may include them (gallery size limit
   permitting – 10–18 MB per language is likely too large for the
   gallery; recommend *required font* entries instead, ADR 0028).

## Effort

About 1.5–2 days: manifest + hash check + install/remove (0.5), loader +
resolution + tests with a tiny fake pack (0.5), GUI rows + validation info
(0.5), CLI + docs (0.25). No change to the project format.

## Recommendation

Do it when the first users with CJK store pages ask, or before CI
documentation promises identical CJK output. Until then: system fonts on
desktops, "put Noto Sans JP into `fonts/`" for CI (documented on the
website).
