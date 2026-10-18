# JVM parity goldens (#54 / spec #50)

These eight files were generated on native Windows by the existing JVM implementation at `4afab4f2b2117a06086bfd812163962c7c3af84f` (Temurin OpenJDK LTS, Clojure CLI 1.12.6.1673, Clojure 1.12.0). They are the independent oracle for the Lua rewrite, including existing scanner limitations. No viewer window or companion is involved.

From the repository root in PowerShell 7, with Java 21+ and Clojure CLI installed:

```powershell
./scripts/regenerate-jvm-goldens.ps1
clj -M scripts/test_jvm_goldens.clj
./scripts/regenerate-jvm-goldens.ps1 -Check
```

Regeneration works **only while the JVM implementation exists**. Preserve this script and its JVM source revision if regeneration is needed after cutover. `-Check` captures into a unique temporary directory and compares all eight files byte for byte; a mismatch retains that directory for inspection. Generation failures stop the command. The baseline is never overwritten in check mode.

Each fixture has its own policy. The script runs `clj -M:ir policy.edn <output>` from each fixture directory, adding the repository JVM source and pinned dependencies with `-Sdeps`. This preserves relative `:file` paths and `:policy-file "policy.edn"` without rewriting IR values. The four `*.ir.edn` files are the untouched CLI output, not hand-authored expectations. Existing `lua-fixture/lua-fixture.edn` and `examples/uml-viewer.edn` are not used as outputs.

`capture-jvm-geometry.clj` reads those same IRs and calls the public `document/compile-view` interface with the matching fixture as metrics root. It captures the root focus and every proper namespace prefix (TypeScript 2 views, Rust 3, Python 2, Lua 1). It uses the viewer's public `curve/basis-path` and `curve/constrain-ends` operations on the scene's routed points, matching the rendering path. Geometry values are JVM-generated; the exporter only projects fields and sorts map keys for stable serialization. Class/edge/view vector order is preserved. The metadata `:version` and `:absolute-tolerance` are capture format declarations, not viewer outputs.

Each geometry file contains:

- `:views`: vector keyed by `:focus`, with `:size`, package rectangles, class `:id` and `:rect` (`:x :y :w :h`).
- `:edges`: `:from :to :kind`, original routed `:points`, and constrained `:curve` with `:start` and ordered `:ops`. A cubic operation has `:c1`, `:c2`, `:p`; a line operation has `:p`. Straight edges (including Lua's only edge) legitimately have no cubic controls.
- The scene's `:tip`, `:behind`, and `:draw-bounds` for additional geometry comparisons.

For Lua parity, parse EDN and compare keys, IDs, strings, operation kinds, collection lengths and vector ordering exactly. Compare each finite geometric number with absolute error **≤ 1e-6 logical canvas units**, including rectangles, points and controls; integer versus floating representation is irrelevant. Missing fields, NaN and infinity fail. No relative tolerance, rounding, rescaling or coordinate translation is allowed. IR comparison is structural and exact (map key ordering is irrelevant, including vector keys in `:edge-kinds`). The stricter byte comparison above checks JVM regeneration only.

Fixture provenance:

| Project | Existing scanner spec scenarios materialized |
| --- | --- |
| `typescript-fixture/` | `graph_typescript_spec.clj`: relative/type/bare/side-effect imports, exported functions/variables/classes/enums/lists, interfaces and implementation, nested file IDs, regex/comment/string masking, skipped tests and declaration files; invoke names are present in input even though IR does not expose command metadata. |
| `rust-fixture/` | `graph_rust_spec.clj`: Cargo lib/binary linkage, commands/public/private functions, `use` and external roots, declared versus undeclared modules, trait implementation with crate/self paths, directory `mod.rs` and nested module discovery, character escapes. |
| `python-fixture/` | `graph_python_spec.clj`: packages and relative/absolute/multiline imports, inheritance and dotted bases, Protocol stereotype, simple versus generic return annotations, private definitions, nested imports, triple/raw/f-strings and comments, skipped tests/conftest/test directories. |
| `lua-fixture/` | Existing real Lua project, unchanged, including its existing metrics snapshots. |

These are scanner fixture projects; generation needs no npm/Cargo/Python dependency installation or compilation. No Lua implementation is added by #54, so no busted or dual-interpreter result is claimed. The scripts and checks capture the oracle independently of the #52 prototype gate.
