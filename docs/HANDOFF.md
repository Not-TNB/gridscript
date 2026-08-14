# GridScript — Handoff

Rust implementation of GridScript, an esolang by SuperJedi224. This is a from-scratch
interpreter: lexer → parser → (interpreter, not yet started). Written by Tristan,
Claude acting as a design/code-review partner (tips + code blocks, not full solutions —
Tristan writes the code himself).

The authoritative spec is `GridScript.md` (clean, amended version). A second file,
`GridScript-amended.md`, is the same spec with every change annotated inline against
the original — useful if you need to know *why* a rule is what it is. Read
`GridScript.md` first; it's what the code should match.

## Where things stand

**Stage 0 (project scaffolding): done.** Cargo project `gridscript`, dependencies
`clap`, `thiserror`, `nom` (added but unused so far — hand-rolled recursive descent
was used instead, see below), `rand`, `rand_chacha`, `strum`/`strum_macros`.

**Stage 1 (parser): ~90% done.** Lexer complete. All 24 commands parse. Node/checkpoint
line parsing complete. Scope assembly (metadata + body + validation) complete.
`parse()` — the top-level entry point that splits main program from subroutines and
assembles a `Program` — is **in progress, not finished**. See "Immediate next steps."

**Interpreter: not started.** `interpreter/tracer.rs` and `interpreter/state.rs` are
done (from Stage 0/early Stage 1 work). `interpreter/exec.rs` is still `todo!()`.

## Module layout
src/
├── main.rs # clap CLI: gridscript <script> [--seed N]
├── lib.rs # pub fn run(source: &str, seed_override: Option<u64>) -> Result<i32>
├── error.rs # GridScriptError, GridScriptWarning, Result<T> alias, Error::syntax()
├── types.rs # Value, DataType, casting, Display impls — DONE, tested
├── rng.rs # GridScriptRng — DONE, tested
├── program.rs # Metadata, Scope, Program — DONE
├── parser.rs # lexer submodule + all parsing logic — IN PROGRESS
│ └── parser/ast.rs # AST types (Node, Checkpoint, Command, ValueExpr, etc.)
│ └── parser/lexer.rs # Token, Keyword, tokenize() — DONE, tested
└── interpreter/
├── tracer.rs # ProgramTracer, DataTracer — DONE
├── state.rs # per-instance dataspace/buffer/variables — DONE
└── exec.rs # todo!() — NOT STARTED

Everything for the parser (lexer aside) currently lives in one file, `parser.rs`
(~800 lines). We discussed splitting it into `parser/metadata.rs`, `parser/expr.rs`,
`parser/command.rs` etc., but decided **not to**, because `Parser`'s fields/primitives
would need to become `pub(super)`/`pub(crate)`, weakening encapsulation for a benefit
(navigation) that wasn't yet worth the cost. Revisit only if the file becomes genuinely
hard to navigate.

## Key architectural decision: the `Parser` cursor struct

Early in Stage 1 the parser was written as free functions threading
`(tokens: &[Token]) -> Result<(T, &[Token])>` — take a slice, return the parsed thing
plus whatever's left. This worked but caused several real bugs (a missed `&tokens[1..]`
in `parse_go`, shadowing bugs where a `let (x, rest) = ...` inside a block shadowed the
outer `rest` instead of advancing it, etc.).

**We refactored the whole parser to a cursor-based `Parser<'a>` struct** partway through
Stage 1:

```rust
struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}
```

All parsing functions are now methods (`&mut self`) returning `Result<T>` (no more
tuple-with-remaining-slice). This eliminates the whole class of "did I advance
correctly" bugs by construction. **This is the established pattern — do not go back
to the free-function/slice-threading style.**

Core primitives on `Parser` (in "PARSER STRUCT AND PRIMITIVES" section of `parser.rs`):

- `peek() -> Option<&Token>`, `peek_at(n)`, `advance()`, `at_end()`, `at_line_end()`
  (true at a `Newline` or end-of-input)
- `keyword() -> Option<Keyword>`, `keyword_at(n)`, `at_keyword(kw) -> bool`
- `expect_keyword(kw) -> Result<()>` (consumes-or-errors)
- `eat_keyword(kw) -> bool` (consumes-if-present, reports whether)
- `expect_token(Token) -> Result<()>` (same as expect_keyword but for punctuation)
- `expect_int() -> Result<i64>` (reads one `IntLiteral`, widened; **generic message,
  no context parameter** — we deliberately dropped per-call-site context strings
  because in practice the surrounding line makes the error unambiguous)
- `eat_data_type() -> Option<DataType>` (consumes `INT`/`FLOAT`/`STRING`/`BOOL` keyword
  if present)
- `skip_newlines()`

Naming convention going forward: `expect_*` = consume-or-error, `eat_*` = consume-if-
present (bool or Option return), `at_*` = peek-only, no consumption. Keep new helpers
consistent with this.

## Key design decisions from the spec-amendment phase (before any code was written)

These are already baked into `GridScript.md` and the type design; listed here only
because they explain *why* the code looks the way it does. Full reasoning is in
`GridScript-amended.md`.

- **CHECKPOINT is not a `Command` variant.** A checkpoint is a static, load-time-only
  marker — no radius, never entered, never executes. This is why `parse_command` has
  no `Checkpoint` arm, and why node-line parsing produces a `BodyLine` enum
  (`Node(Node) | Checkpoint(Checkpoint)`) that branches at the `(x,y):` level, not
  inside command parsing.
- **`@maxdepth` is a single global setting, main-program-only.** Not part of
  `Metadata` (which is per-Scope) — it lives on `Program` directly. Subroutine-level
  `@maxdepth` declarations are parsed (so they don't error) but ignored.
- Program-space position uses `f32` (not the originally-spec'd 16-bit) — matches the
  language's own FLOAT precision.
- `GOTO`/teleports use **point-containment** at the destination, never a segment test
  — teleporting isn't movement through intervening space.
- STRING is raw bytes (`Vec<u8>`), not UTF-8-validated — `Value::Str(Vec<u8>)`.
- Only the literal string `"0"` is falsy for BOOL casts — the word `"FALSE"` is
  truthy (confirmed-intentional spec quirk, has a regression test in `types.rs`).
- INT arithmetic overflow and divide-by-zero both throw (`GridScriptError::
  IntegerOverflow` / `DivisionByZero`) — not wrapping/saturating.
- STRING↔INT/FLOAT casting shares one numeric parser (trim, sign, decimal, exponent);
  INT-cast truncates toward zero (not floor) — confirmed against spec's literal
  wording ("rounded down (towards 0)").

## `types.rs` / `error.rs` / `rng.rs` — stable, don't need revisiting

- `Value` enum: `Int(i32) | Float(f32) | Str(Vec<u8>) | Bool(bool) | Null`
- `DataType` enum: `Int | Float | Str | Bool` (no Null variant — never a cast target)
- `Value::cast_to(&self, DataType) -> Option<Value>` is the single entry point for all
  casting logic
- `GridScriptError` (fatal) and `GridScriptWarning` (non-fatal) — thiserror-derived.
  `type Result<T> = std::result::Result<T, GridScriptError>` alias lives in `error.rs`.
  `impl GridScriptError { pub fn syntax(msg: impl Into<String>) -> Self }`. Parser
  imports it as `use crate::error::{GridScriptError as Error, Result};` and writes
  `Error::syntax(format!(...))`.
- `GridScriptRng` wraps `ChaCha8Rng` (seeded or OS-entropy), five methods:
  `random_direction`, `random_unit_float`, `coin_flip`, `random_index`,
  `shuffle<T>`. `random_index(len)` panics on `len == 0` — documented, caller's
  responsibility to check emptiness first.

**Toolchain note:** the sandbox this was built in had an old system Rust (1.75)
requiring pinned dependency versions (`rand = "=0.8.5"` era). Tristan's own machine
has a newer toolchain where `rand`/`rand_chacha` resolved to a much newer, still-
churning API generation (0.10.x) with different method names. **If you hit rand API
errors, check `cargo tree | grep rand` first and look up docs.rs for the exact
resolved version** — don't assume either generation's names.

## `program.rs` — stable

```rust
pub struct Metadata {
    pub width: i32, pub height: i32,               // required, from_raw errors if missing/<1
    pub data_width: i32, pub data_height: i32,      // default 64
    pub radius: i32,                                // default 1
    pub steps: Option<u64>,                         // None = unlimited
    pub debug: DebugMode,                           // default False
    pub seed: Option<u64>,
}
impl Metadata {
    pub fn from_raw(raw: RawMetadata) -> Result<Metadata>  // validates, defaults
}
pub struct Scope { pub title: String, pub metadata: Metadata, pub nodes: Vec<Node>,
                    pub checkpoints: Vec<Checkpoint> }
pub struct Program { pub main: Scope, pub subroutines: HashMap<String, Scope>,
                      pub max_depth: u32 }
```

`RawMetadata` (parser output, pre-validation) lives in `parser/ast.rs` — has
`Option<i64>` for every numeric field plus `max_depth: Option<i64>` (only meaningful
when it's the *main* program's raw metadata).

## `parser/lexer.rs` — done, tested

`Token` variants: `Keyword(Keyword)`, `Identifier(String)` (lowercase words),
`UpperName(String)` (**all-caps non-keyword words — used for subroutine names in
CALL**), `IntLiteral(i32)`, `FloatLiteral(f32)`, `StringLiteral(Vec<u8>)`, `Newline`,
`Comma`, `Colon`, `LParen`, `RParen`, `At`, `Equals`, `Bang`, `BangEquals`.

`Keyword` enum: one variant per GridScript reserved word (~55), derives
`strum::EnumString` with `#[strum(serialize_all = "UPPERCASE")]`. One override:
`#[strum(serialize = "STRING")] Str` (Rust variant name `Str`, spec keyword `STRING`).

`tokenize(source: &str) -> Result<Vec<Token>>` handles: whitespace/`\r` (skipped),
comments (`!!` to end of line, newline preserved), all punctuation, signed int/float
literals, quoted strings (single-quote only), keywords vs. lowercase identifiers vs.
all-caps names vs. mixed-case (error). `scan_word` rejects a leading digit before
checking case.

**Title lines are NOT tokenized.** `split_title(source, octothorpes) -> Result<(&str,
&str)>` in `parser.rs` strips `#TITLE.` / `##TITLE.` as a raw-string pre-pass, because
title text is free-form uppercase prose that can't lex as GridScript tokens.

## `parser.rs` — current state, section by section

Sections (each is one or more `impl<'a> Parser<'a>` blocks, grouped by subsystem —
keep new code in the matching section):

1. **PARSER STRUCT AND PRIMITIVES** — described above.
2. **SOURCE PREPROCESSING** — `split_title` (free fn, operates on `&str` not tokens).
3. **METADATA PARSING** — `expect_debug_mode` (lowercase `true`/`false`/`auto` as
   `Token::Identifier`, NOT keywords — deliberate, matches spec's literal lowercase
   wording), `parse_metadata_line`, `parse_metadata`.
4. **VALUE EXPRESSION PARSING** — `parse_value_expr` (dispatches literal/var/`THE`),
   `parse_dynamic_value_expr` (`THE [VARIABLE|type] NAMED name` — `name` is
   `Box<ValueExpr>`, recursive), `parse_optional_clause(kw)` (generic "if kw present,
   consume + parse one value expr").
5. **COMMAND PARSING** — one method per command family, all 24 commands done:
   `parse_value_or_row`, `parse_push`, `parse_goto`, `parse_arithmetic` (covers
   INCREMENT/DECREMENT/MULTIPLY/DIVIDE via one `ArithOp` param), `parse_throw`,
   `parse_warn`, `parse_go` (6 forms + RELATIVE TO validity check), `parse_switch`
   (RANDOM/bare/!/=/!= via a `fn(ValueExpr) -> SwitchCond` constructor-as-value
   trick), `parse_store`, `parse_to_clause` (shared `TO [type] variable` clause —
   used by PEEK, REMOVE, LOAD FILE; returns `Option<ToClause>` where `ToClause
   { cast: Option<DataType>, target: ValueExpr }`), `parse_peek`, `parse_split`,
   `parse_return`, `parse_print` (4 forms), `parse_remove` (4 position forms, has a
   guard for bare `REMOVE TO x`), `parse_load_file`, `parse_move_last_node`,
   `parse_call` (subroutine name must be `Token::UpperName`; gives a specific error
   if a reserved keyword is used as a name; argument list is unbounded,
   self-delimiting, terminated by GIVING/newline/EOF). Then `parse_command`, the
   dispatcher.
6. **NODE/CHECKPOINT LINE PARSING** — `BodyLine` enum (`Node(Node) | Checkpoint
   (Checkpoint)`), `expect_coord()` (i64→i32 with range check), `parse_position()`
   (parses `(x,y)`), `parse_body_line()` (parses one full `(x,y):COMMAND` or
   `(x,y):CHECKPOINT id` line — checkpoint id must be non-negative →
   `Error::InvalidCheckpointId`), `parse_body()` (loops `parse_body_line` until
   `at_end()`, skipping newlines between lines — **uses `while !self.at_end() { ...;
   self.skip_newlines(); }` shape with an initial `skip_newlines()` before the loop
   too, NOT a bare `loop {}`** — deliberate choice).
7. **SCOPE ASSEMBLY** (may not be labeled as its own section yet — check) —
   `parse_scope(source: &str, octothorpes: usize) -> Result<(Scope, Option<i64>)>`.
   Splits title, tokenizes rest, parses metadata → `Metadata::from_raw`, parses body,
   validates exactly-one-START (`MissingStart`/`DuplicateStart`, both take the scope
   title as context), validates every node AND every checkpoint position is within
   `[1,width] x [1,height]` (separate error variants `NodeCenterOutOfBounds` vs.
   `CheckpointCenterOutOfBounds` — **two near-identical loops here trigger a
   RustRover "duplicated code" inspection; it's been suppressed with a
   `//noinspection` comment because the loops genuinely differ and clippy itself has
   no complaint — don't "fix" this by trying to unify them further**). Returns
   `(Scope, Option<i64>)` — the second element is `raw.max_depth`, since
   `Metadata::from_raw` consumes `RawMetadata` by value and `Scope` has nowhere to
   put `max_depth`.

## What's NOT finished — pick up here

**1. `split_scopes(source: &str) -> (&str, Vec<&str>)`** — free function, finds every
line starting with `##` (after `trim_start()`) via `source.match_indices("\n##")`
(landing offset `+1` to point at the `#` not the `\n`), slices source into
(main_chunk, Vec of subroutine chunks each still containing its own `##TITLE.` line).
Last version sketched (not yet finalized/tested/written into the file):

```rust
fn split_scopes(source: &str) -> (&str, Vec<&str>) {
    let cuts: Vec<usize> = source.match_indices("\n##")
        .map(|(i, _)| i + 1)
        .collect();
    let main_end = cuts.first().copied().unwrap_or(source.len());
    let main = &source[..main_end];
    let subs = cuts.iter().enumerate().map(|(i, &start)| {
        let end = cuts.get(i + 1).copied().unwrap_or(source.len());
        &source[start..end]
    }).collect();
    (main, subs)
}
```
Needs to actually be written into the file and tested — verify against the
Ackermann example in `GridScript.md` (has exactly one subroutine).

**2. `pub fn parse(source: &str) -> Result<Program>`** — currently still `todo!()`.
Needs to:
- call `split_scopes`
- `parse_scope(main_src, 1)` → `(main, max_depth_raw)`
- loop `parse_scope(sub_src, 2)` for each subroutine chunk, discard the returned
  `Option<i64>` (subroutine `@maxdepth` is ignored), insert into a
  `HashMap<String, Scope>` keyed by `scope.title`
- **OPEN DECISION, not yet made:** what happens on a duplicate subroutine title
  (`HashMap::insert` silently overwrites)? We agreed this probably warrants a new
  `GridScriptError` variant rather than silent last-wins, but the variant doesn't
  exist yet and the decision wasn't finalized — decide and implement.
- validate/default/convert `max_depth_raw: Option<i64>` → `u32`, default 1000,
  error if `< 1` (last sketch used `u32::try_from` + a `filter(|&n| n >= 1)` chain,
  reusing `GridScriptError::InvalidMetadata { key: "maxdepth", value }` — not yet
  written into the file)
- construct and return `Program { main, subroutines, max_depth }`

Needs `use std::collections::HashMap;` added to `parser.rs`'s imports if not already
present, and `Scope`, `Metadata` pulled in from `crate::program` (check current
import list).

**3. Once `parse()` returns real values instead of `todo!()`**, a batch of
"never used" dead-code warnings across the whole parser (and possibly `peek_at`,
`keyword_at`, `at_end` specifically, added speculatively — check after wiring is
done) will resolve or become real signal. Worth a full `cargo build` + `cargo
clippy` pass at that point.

**4. Write an actual end-to-end test**: tokenize/parse one of the full sample
programs from `GridScript.md` (Hello World is simplest; Ackermann exercises
subroutine-splitting) through `parse()` and assert on the resulting `Program`
structure. This doesn't exist yet.

## After `parse()` is done: Stage 1 is complete, move to the interpreter

`interpreter/exec.rs`'s `run()` is still `todo!()`. The PROGRAM EXECUTION algorithm
(steps 1-3 from `GridScript.md`, including the segment-circle node intersection, the
fixed per-step batch snapshot, GOTO's point-containment-only landing check, CALL
recursion with `@maxdepth` tracking, `@steps` enforcement) hasn't been designed in
code yet, only in the spec document. `interpreter/tracer.rs`
(`ProgramTracer::advance()` already returns a `(start, end)` segment specifically so
exec.rs can run intersection tests against it) and `interpreter/state.rs`
(dataspace/buffer/variables) are ready and waiting to be driven by this loop.

## Testing conventions established

- Test helper functions at the top of each test module: `cmd(source) -> Result
  <Command>`, `expr(source) -> Result<ValueExpr>`, `meta(source) -> Result
  <RawMetadata>` — each tokenizes + runs one parser entry point. Follow this pattern
  for new top-level test helpers (e.g. a `scope(source, n)` helper would make sense
  once `parse_scope` needs more direct testing).
- Small constructor helpers: `int(n)`, `var(s)`, `str_lit(s)` build `ValueExpr`s
  tersely for assertions.
- Both positive (exact `assert_eq!`) and negative (`assert!(...is_err())`) cases in
  the same test function per command, rather than separate `parses_x`/`rejects_x`
  functions.
- Cursor-position assertions (checking `p.peek()` after a partial parse) are used
  specifically where over/under-consumption is a real risk — not needed on every
  test, only where the parsing logic has a plausible failure mode.

## Tristan's working style (for whoever picks this up)

- Wants tips/pseudocode/targeted code blocks, not full solutions handed over —
  writes the code himself, pastes it back for review.
- Wants concise, direct answers — no filler, no restating the question.
- Prior Rust background: RPN calculator, Brainfuck interpreter, Conway's Game of
  Life (separate completed projects, same learning arc). Comfortable with
  ownership/borrowing basics, still building intuition around lifetime elision,
  trait-based generic constraints, match-ergonomics edge cases.
- Consistently pushes for the shortest/cleanest version of anything that looks
  repetitive. Keep offering the genuinely-simpler option AND being honest when
  something can't be meaningfully shortened.
- Catches inconsistencies himself often before being told (e.g. noticed `parse_peek`
  had its own helper while `THROW`/`WARN` didn't, prompting the "give every
  non-trivial command its own helper" rule).