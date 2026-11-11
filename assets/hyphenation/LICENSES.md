# Hyphenation patterns: sources and licences

The `hyph-*.pat.txt` (patterns) and `hyph-*.hyp.txt` (exceptions) files
are **unmodified** copies of the plain-text pattern files of the
[hyph-utf8](https://github.com/hyphenation/tex-hyphen) project
(`hyph-utf8/tex/generic/hyph-utf8/patterns/txt/`, commit
`5684c0f51c0b81133db2efbe60a408b4155a3ff5`, fetched 2026-11-10). They are
embedded into `vitrine-core` (`core::layout::hyphen`, ADR 0009) and read
with Liang's algorithm.

Each language has its own licence, taken from the metadata header of the
corresponding `tex/hyph-*.tex` file. Vitrine Studio is licensed
GPL-3.0-or-later (ADR 0022); only patterns under permissive,
GPL-compatible licences (MIT, BSD, or a "free distribution" notice) are
bundled. Where a file is dual-licensed,
the permissive option is used. Copyright notices and licence texts are
reproduced below as the licences require; they must ship with binary
distributions (e.g. in an "acknowledgements" document).

| Set | Language | Copyright | Licence used | Bundled |
|---|---|---|---|---|
| `en-us` | American English | © 1990, 2004, 2005 Gerard D.C. Kuiken | permissive notice (A) | yes |
| `en-gb` | British English | © 1992, 1996, 2005, 2016 Dominik Wujastyk, Graham Toal | MIT | yes |
| `de-1996` | German, reformed orthography | © 2013–2024 Stephan Hennig, Werner Lemberg, Günter Milde, Sander van Geloven, Georg Pfeiffer, Gisbert W. Selke, Tobias Wendorf, Keno Wehr (Deutschsprachige Trennmustermannschaft) | MIT | yes |
| `fr` | French | © 1994–2002 Daniel Flipo, Bernard Gaulle, 2016 Arthur Reutenauer | MIT | yes |
| `es` | Spanish | © 1993, 1997 Javier Bezos, 2001–2019 Javier Bezos, CervanTeX | MIT | yes |
| `it` | Italian | © 2008–2011 Claudio Beccari | MIT (dual LPPL 1.3 / MIT) | yes |
| `nl` | Dutch | © 1996 Piet Tutelaers | MIT | yes |
| `pt` | Portuguese | © 1987, 1994, 1996, 2015 Pedro J. de Rezende, 1996, 2015 J. Joao Dias Almeida, 2024 Leonardo Araujo and Aline Benevides | BSD 3-clause (B) | yes |
| `da` | Danish | © 1994 Frank Jensen | MIT (dual LPPL 1.3 / MIT) | yes |
| `nb` | Norwegian Bokmål (`no`, `nb`) | © 2004–2005 Rune Kleveland, Ole Michael Selberg, 2007 Karl Ove Hufthammer | permissive notice (A) | yes |
| `fi` | Finnish | © 1986, 1988, 1989 Kauko Saarinen | "Patterns may be freely distributed" | yes |
| `sv` | Swedish | © 1994 Jan Michael Rynning | LPPL 1.2 only | **no** – LPPL 1.2 is GPL-incompatible (FSF) and the patterns are compiled into the binary; left out (ADR 0022). Possible later: separate unmodified data file, or ask the author for MIT |

Other hyph-utf8 languages were not reviewed and are not bundled. To add
one: check the licence in its `tex/hyph-<lang>.tex` header, copy the
unmodified `txt/` files here, add a row and the notice below, and list
the set in `crates/core/src/layout/hyphen.rs`.

## Licence texts

### (A) Permissive notice (`en-us`, `nb`)

> Copying and distribution of this file, with or without modification,
> are permitted in any medium without royalty provided the copyright
> notice and this notice are preserved.

### MIT (`en-gb`, `de-1996`, `fr`, `es`, `it`, `nl`, `da`)

> Permission is hereby granted, free of charge, to any person obtaining a
> copy of this software and associated documentation files (the
> "Software"), to deal in the Software without restriction, including
> without limitation the rights to use, copy, modify, merge, publish,
> distribute, sublicense, and/or sell copies of the Software, and to
> permit persons to whom the Software is furnished to do so, subject to
> the following conditions:
>
> The above copyright notice and this permission notice shall be included
> in all copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
> OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
> MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
> IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
> CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
> TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
> SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

### (B) BSD 3-clause (`pt`)

> Redistribution and use in source and binary forms, with or without
> modification, are permitted provided that the following conditions are
> met:
>
> * Redistributions of source code must retain the above copyright
>   notice, this list of conditions and the following disclaimer.
> * Redistributions in binary form must reproduce the above copyright
>   notice, this list of conditions and the following disclaimer in the
>   documentation and/or other materials provided with the distribution.
> * Neither the name of the University of Campinas, of the University of
>   Minho nor the names of its contributors may be used to endorse or
>   promote products derived from this software without specific prior
>   written permission.
>
> THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS
> IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED
> TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A
> PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL PEDRO J. DE REZENDE
> OR J.JOAO DIAS ALMEIDA BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
> SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
> TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
> PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
> LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
> NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
> SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

### Finnish (`fi`)

> Patterns may be freely distributed.
