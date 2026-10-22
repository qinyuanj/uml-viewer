# JVM parity goldens (#54 / spec #50)

These eight files are the **frozen parity oracle** captured on native Windows
by the JVM implementation at
`4afab4f2b2117a06086bfd812163962c7c3af84f` (Temurin OpenJDK LTS, Clojure CLI
1.12.6.1673, Clojure 1.12.0). They are an independent oracle for the Rust
port, including existing scanner limitations. No viewer window or companion
was involved.

The JVM implementation and its regeneration tooling were deleted by the #72
cutover. Do **not** rewrite these EDN files: the Rust golden tests read them
unchanged. There is no `regenerate-jvm-goldens.ps1`, `capture-jvm-geometry.clj`,
or `test_jvm_goldens.clj` any more; reproducing them would require checking out
the historical JVM commit.

Each fixture has its own policy. The historical generator ran
`clj -M:ir policy.edn <output>` from each fixture directory, adding the JVM
source and pinned dependencies with `-Sdeps`. This preserved relative `:file`
paths and `:policy-file "policy.edn"` without rewriting IR values. The four
`*.ir.edn` files are the untouched CLI output, not hand-authored expectations.

The historical geometry capture called the public `document/compile-view`
interface with the matching fixture as metrics root. It captured the root focus
and every proper namespace prefix (TypeScript 2 views, Rust 3, Python 2, Lua 1)
and used the viewer's public curve operations on the scene's routed points.
Class/edge/view vector order is preserved. The metadata `:version` and
`:absolute-tolerance` are capture format declarations, not viewer outputs.

Each geometry file contains:

- `:views`: vector keyed by `:focus`, with `:size`, package rectangles, class `:id` and `:rect` (`:x :y :w :h`).
- `:edges`: `:from :to :kind`, original routed `:points`, and constrained `:curve` with `:start` and ordered `:ops`. A cubic operation has `:c1`, `:c2`, `:p`; a line operation has `:p`. Straight edges (including Lua's only edge) legitimately have no cubic controls.
- The scene's `:tip`, `:behind`, and `:draw-bounds` for additional geometry comparisons.

For parity, parse EDN and compare keys, IDs, strings, operation kinds,
collection lengths and vector ordering exactly. Compare each finite geometric
number with absolute error **≤ 1e-6 logical canvas units**, including
rectangles, points and controls; integer versus floating representation is
irrelevant. Missing fields, NaN and infinity fail. No relative tolerance,
rounding, rescaling or coordinate translation is allowed. IR comparison is
structural and exact (map key ordering is irrelevant, including vector keys in
`:edge-kinds`).

Fixture provenance (historical scanner spec scenarios materialized):

| Project | Existing scanner scenarios |
| --- | --- |
| `typescript-fixture/` | relative/type/bare/side-effect imports, exported functions/variables/classes/enums/lists, interfaces and implementation, nested file IDs, regex/comment/string masking, skipped tests and declaration files; invoke names are present in input even though IR does not expose command metadata. |
| `rust-fixture/` | Cargo lib/binary linkage, commands/public/private functions, `use` and external roots, declared versus undeclared modules, trait implementation with crate/self paths, directory `mod.rs` and nested module discovery, character escapes. |
| `python-fixture/` | packages and relative/absolute/multiline imports, inheritance and dotted bases, Protocol stereotype, simple versus generic return annotations, private definitions, nested imports, triple/raw/f-strings and comments, skipped tests/conftest/test directories. |
| `lua-fixture/` | Existing real Lua project, unchanged, including its existing metrics snapshots. |

These are scanner fixture projects; comparison needs no npm/Cargo/Python
dependency installation or compilation. The Rust cargo golden tests establish
the current parity result.
