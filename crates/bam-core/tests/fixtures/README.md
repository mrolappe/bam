# Fixtures

Source: `https://ftp.fau.de/aminet/` — fetched 2026-08-06.

- `index_sample.txt` — 506 lines curated from a real `INDEX` (85,433 lines,
  ~84,700 packages). The 3-line header/preamble plus a contiguous run of the
  first ~487 entries, with 16 additional lines appended from elsewhere in the
  file to guarantee coverage of the awkward cases a parser must handle:
  - **zero-size entries** (`0K`) — 7 lines, e.g. `AlphaBase_keyfile.lha`
  - **non-ASCII bytes** (Latin-1, e.g. `Ã©`/`Â´`/em dash) — 5 lines, e.g.
    `Audithec.lha`
  - **descriptions with internal whitespace runs** (double spaces) — 5 lines,
    e.g. `hardchecker1_8.lha`
  - **long filenames** overflowing the fixed-width filename column — 5 lines,
    e.g. `gcc-4.2.2-x86_64-cygwin.tar.bz2`
  - **header/preamble lines** — the leading `|` banner, 3 lines
- `recent_sample.txt` — a real `RECENT`, committed in full (74 lines).
- `tree_sample.txt` — a real `TREE`, committed in full (381 lines).
