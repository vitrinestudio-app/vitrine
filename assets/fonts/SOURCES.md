# Bundled fonts – sources and checksums

All fonts are licensed under the **SIL Open Font License 1.1**. Each family
ships its own licence file with its copyright line (`OFL.txt` in the
family folder; Inter: `assets/fonts/OFL.txt`). The files are unmodified
(no subsetting, no renaming inside the font), so the Reserved Font Names
some families declare (Dancing Script, Lora, Playfair Display) are not
affected.

The manifest the code uses (family, weights, italic, category, licence
file) is `crates/core/src/layout/bundled.rs`; its tests check it against
the files and check every file's name table (ID 13 licence description,
ID 14 licence URL) for the OFL.

## Inter 4.1

Bundled since Phase 0 (static instances; source: the Inter 4.1 release,
https://github.com/rsms/inter).

| File | Bytes | SHA-256 |
|---|---:|---|
| `Inter-Regular.ttf` | 411640 | `40d692fce188e4471e2b3cba937be967878f631ad3ebbbdcd587687c7ebe0c82` |
| `Inter-SemiBold.ttf` | 419744 | `78a843fade9d4612a5567302fb595b56976eb5fcebf4fea5a5912d638bafcde3` |
| `InterDisplay-Bold.ttf` | 418856 | `b74c8e0dd744b3347faca4c96bc7b2e32f7d6f62300a79b1d1a99331e44a5bc4` |
| `InterDisplay-Black.ttf` | 417540 | `25460b0d5b3afd9764d63cf838f94145af624f8d4c9d20e0f74116be42878b32` |
| `OFL.txt` | | `262481e844521b326f5ecd053e59b98c8b2da78c8ee1bdbb6e8174305e54935a` |

## Added 2026-11-11

Owner's decision 2026-11-11 (`docs/plan/STATUS.md`, proposal
`docs/research/2026-11-11-bundled-fonts.md`). Source: the Google Fonts
repository **github.com/google/fonts**, folder `ofl/<family>/`, pinned
commit **`5bb28733cec313b63fbd415b574b5ae35c8aa8b8`** (main on
2026-11-11), downloaded from
`https://raw.githubusercontent.com/google/fonts/5bb28733cec313b63fbd415b574b5ae35c8aa8b8/ofl/<family>/<file>`.
Variable fonts (`[wght]` = one file for the whole weight range).

Name table check (2026-11-11): every file below has ID 13 = "This Font
Software is licensed under the SIL Open Font License, Version 1.1. …" and
ID 14 = `http(s)://scripts.sil.org/OFL` (Montserrat:
`https://openfontlicense.org`).

| Family | Designer | File | Weights | Bytes | SHA-256 |
|---|---|---|---|---:|---|
| Montserrat | Julieta Ulanovsky, Sol Matas, Juan Pablo del Peral, Jacques Le Bailly | `montserrat/Montserrat[wght].ttf` | 100–900 | 744936 | `0f7b311b2f3279e4eef9b2f968bcdbab6e28f4daeb1f049f4f278a902bcd82f7` |
| | | `montserrat/Montserrat-Italic[wght].ttf` | 100–900 italic | 762376 | `51607f316bc020e59f03cbf51543eecffbea501c0b31d73e5b82927c5cca442c` |
| | | `montserrat/OFL.txt` | | | `8b7141c03fa4f8d44e6345d5d4931709290f0f67875e452e95ac1fd3a027802e` |
| Playfair Display | Claus Eggers Sørensen | `playfairdisplay/PlayfairDisplay[wght].ttf` | 400–900 | 300724 | `c40f2293766a503bc70cce9e512ef844a4ccb7cbcde792fe2ea31d191917d8d6` |
| | | `playfairdisplay/PlayfairDisplay-Italic[wght].ttf` | 400–900 italic | 278688 | `a5e26dc5e2e77fb2803a0bf02fd4f81ee136ec8dea863ccdb0c59a263b21378b` |
| | | `playfairdisplay/OFL.txt` | | | `566be814f8e96e93dfa16101331557eb6b5467e9e03f627c0910fe93ca12300e` |
| Lora | Cyreal | `lora/Lora[wght].ttf` | 400–700 | 212196 | `822a6621ccbe8d97d20ac88c1c41f5615c9c2c202eaa75f272cd452aac6475a7` |
| | | `lora/Lora-Italic[wght].ttf` | 400–700 italic | 221232 | `22d8d8854b53807aa664ca34f2031a9ed57a1d0dea296b8b96cdd3aad937a2b3` |
| | | `lora/OFL.txt` | | | `1d9a970809ac804b582a6ce7f0ebc4e7fefcbfd7ff6299cad35ee656a21be716` |
| Nunito | Vernon Adams, Cyreal, Jacques Le Bailly | `nunito/Nunito[wght].ttf` | 200–1000 | 276932 | `bb55a5ca5c2042335b3991af27c4d0705d0ef41cac6164ac737fd8f2a1e85207` |
| | | `nunito/Nunito-Italic[wght].ttf` | 200–1000 italic | 281832 | `b520cc871868b0acfca1beda875df7f4a44ebce914f8a89f83977fc9c09529c8` |
| | | `nunito/OFL.txt` | | | `580df76c95a1ec5ab878ceb25bb3d85c6a076804e9c970c8c6972aea775fdf65` |
| Oswald | Vernon Adams, Kalapi Gajjar, Cyreal | `oswald/Oswald[wght].ttf` | 200–700 | 172088 | `5b38c246e255a12f5712d640d56bcced0472466fc68983d2d0410ec0457c2817` |
| | | `oswald/OFL.txt` | | | upstream `0fd731a904b729a4e02eaf5e8ebd06783edd9abe400e8882760160230675b652` (CRLF); stored with LF line endings (`.gitattributes`): `0687c51a3126ae6963389181135a2eaea02f2e88f31230d27409952f28e84da5` |
| Bebas Neue | Ryoichi Tsunekawa (Dharma Type) | `bebasneue/BebasNeue-Regular.ttf` | 400 | 61400 | `08e4623805102d819f58601e46e345648846075e363b2ceb23313c2d1c83ec73` |
| | | `bebasneue/OFL.txt` | | | `72082f6cb4d04be2ecf7cc7d9e1e7d73787f0af8a5a278a47cade70c16b78341` |
| Caveat | Impallari Type | `caveat/Caveat[wght].ttf` | 400–700 | 403648 | `0bdb6b660482d31531b3945849fba5916b3ef8695da7024a9e6b9ee3c4157988` |
| | | `caveat/OFL.txt` | | | `1f9d81d094273d82f3898a1ee8b598a717d050ecbf5ff7bede105b704880157b` |
| Dancing Script | Impallari Type | `dancingscript/DancingScript[wght].ttf` | 400–700 | 133636 | `21808625578fe8d8cd10cb684be546dca077b27cd03a53a2f1ec11dc743c924c` |
| | | `dancingscript/OFL.txt` | | | `6f090277c00af96651ce6dbcc38ff1591047a3bffef486e80b6a32e8276a8201` |
| JetBrains Mono | JetBrains, Philipp Nurullin, Konstantin Bulenkov | `jetbrainsmono/JetBrainsMono[wght].ttf` | 100–800 | 187208 | `48715a42ec242c21e9f02692891e147d022299a52e48d5e413e1a942193ffeda` |
| | | `jetbrainsmono/JetBrainsMono-Italic[wght].ttf` | 100–800 italic | 191556 | `85ae2a5cd3f56baf1ce1c21a851322c58e3d8fbe8e8ad4a4d090a820dd7fe558` |
| | | `jetbrainsmono/OFL.txt` | | | `b2fe5e8987594e9ffd1d2ca52a2f5d73eb8335243893c5d6254b5ad69269591d` |

Total added: 4.23 MB (14 font files). Italics only for Montserrat,
Playfair Display, Lora, Nunito and JetBrains Mono (proposal); Oswald,
Bebas Neue, Caveat and Dancing Script have no italic upstream – italic
text in them is slanted synthetically.

To update a family: download the new files from a newer pinned commit,
re-run `cargo test -p vitrine-core bundled` (the manifest test reports
changed weights/families), update this table.
