# TypeScript

Applies to every hand-written TypeScript package in a tasker-systems repository: tasker-core's
`tasker-ts` FFI package; temper's published clients (`temper-ts`, `temper-telemetry-ts`), its web
packages (`temper-cloud`, `temper-ui`, `temper-design`) and its agent workflows; temper-contrib's
desktop app. Canonical config is [`../typescript/biome.json`](../typescript/biome.json) and
[`../typescript/tsconfig.base.json`](../typescript/tsconfig.base.json); enforcement status per repo
is in [`rules.md`](rules.md). What was taken from Google's TypeScript guide (and its JavaScript and
HTML/CSS guides) is traced in [`../sources/google/typescript.md`](../sources/google/typescript.md).

Our code runs under Node, Bun and Deno, often with type-stripping instead of a `tsc` emit, and
behind Biome rather than Google's internal toolchain. Where Google's rules exist to serve Closure or
downlevelling, they are rejected; where they prevent defects, they are taken and given a gate.

## Tooling

**TS-TOOL-1 — every TypeScript package is linted and typechecked in CI.** Biome (`biome ci`) and
`tsc --noEmit` (or `svelte-check`) run for every package, inside the repository's required
aggregate check ([CI-2](testing.md)). *Why:* a package outside the linter is outside every rule
below. temper's published SDK (`clients/temper-ts`), `temper-telemetry-ts` and both agent workflow
packages are typechecked but never linted.

**TS-TOOL-2 — one shared rule set, and no rule that only warns.** Packages extend the canonical
[`biome.json`](../typescript/biome.json) and add to it. CI runs Biome with
`--error-on-warnings`, so a rule is either on or off. *Why:* `biome ci` exits 0 on warnings. Every
repository had `noExplicitAny` and `noNonNullAssertion` at `warn`, which meant they enforced nothing
— [tenet 2](tenets.md), a gate that cannot fail. temper-ui carries 60 live non-null-assertion
warnings that no check has ever failed on.

**TS-TOOL-3 — one TypeScript and one Biome version per repository, pinned.** *Why:* type semantics
change between compiler versions; a repository with five TypeScript ranges, two of them release
candidates, type-checks each package against a different language. (Pinning mechanics are
[SC-IN-1 and SC-IN-3](supply-chain.md).)

## Compiler settings

**TS-CFG-1 — packages extend one strict `tsconfig` base.** The base sets `strict`,
`noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `noImplicitOverride`,
`noImplicitReturns` and `noFallthroughCasesInSwitch`. A package adds only module, target, path and
output settings. *Why:* `strict` alone still types `array[i]` as defined and lets an optional
property be set to `undefined` explicitly; both are where runtime `undefined` comes from. tasker-ts
already runs this set; temper's packages and the desktop app run `strict` only.

**TS-CFG-2 — erasable syntax only.** No `enum`, `namespace`, parameter properties or `import =`;
type-only imports and exports are marked `type`. `erasableSyntaxOnly` and `verbatimModuleSyntax`
enforce it at the compiler, Biome's `noEnum`, `noNamespace`, `noParameterProperties`,
`useImportType` and `useExportType` at the linter. A closed set of values is an `as const` object
with a union type. *Why:* Node, Bun and Deno strip types without compiling; syntax that needs code
generation either fails there or behaves differently from `tsc`. This is stricter than Google, which
allows plain enums and prefers parameter properties.

## Types

**TS-TYPE-1 — no `any`; untrusted data enters as `unknown` and is validated.** Data from the
network, a file, the FFI boundary or `JSON.parse` is `unknown` until a schema (zod, or the
generated client's types after validation) narrows it. `noExplicitAny` enforces it. *Why:* `any`
switches the type checker off for everything it touches, silently and transitively.

**TS-TYPE-2 — no non-null assertions; a type assertion says why it holds.** `value!` is not used —
narrow with a check instead. An `as` cast carries a comment stating the invariant that makes it
true; `as unknown as T` is a smell to justify. `noNonNullAssertion` enforces the first; the second
is judged in review. *Why:* both tell the compiler to stop checking, and the comment is the only
place the reason can live.

**TS-SUP-1 — a suppression names its rule and its reason.** `// @ts-expect-error <reason>` or
`// biome-ignore lint/<group>/<rule>: <reason>`. Never `@ts-ignore` or `@ts-nocheck`
(`noTsIgnore`). *Why:* [tenet 6](tenets.md), and the same reasoning as
[RUST-LINT-3](rust.md#lints): `@ts-expect-error` fails once the error it silences is gone, while
`@ts-ignore` hides problems forever. Biome checks that a reason exists for `biome-ignore`, not for
`@ts-expect-error`; a tripwire covers that.

## Modules and errors

**TS-MOD-1 — named exports.** No `export default`, except where a framework requires one (config
files, SvelteKit route modules, workflow definitions), listed in a Biome override. `noDefaultExport`
enforces it. *Why:* a default export can be imported under any name, so renames do not propagate and
a reader cannot grep for a symbol's uses.

**TS-ERR-1 — throw and reject only `Error` instances.** `useThrowOnlyError` enforces it. *Why:* a
thrown string has no stack and no type to narrow on, and `catch (e)` code assumes `e.message`.

## Naming

**TS-NAME-1 — the language's casing; the wire's casing on wire types.** Types and classes
PascalCase, values and functions camelCase, module-level constants CONSTANT_CASE, no `I` prefix on
interfaces. A property that mirrors a JSON field keeps the field's snake_case
([API-JSON-1](api.md)). A leading `_` marks only an intentionally unused binding. Biome's
`useNamingConvention` can enforce it once configured to allow snake_case properties; it is not in
the canonical config yet, because with properties exempted it still has 1–37 findings per package
to audit first. *Why:* Google's
lowerCamelCase-everywhere rule would rename our wire types away from the API they describe, and its
ban on `_` conflicts with how tsc and Biome mark an unused parameter.

## Generated code

**TS-GEN-1 — generated TypeScript is drift-gated, not linted.** `schema.ts` from the OpenAPI spec,
ts-rs bindings, napi-rs's `index.d.ts`: excluded from Biome, never edited by hand, regenerated in CI
and compared ([CI-3](testing.md)). *Why:* the same as [PY-GEN-1](python.md#generated-code).

## Documentation

**TS-DOC-1 — the exported API of a published package has doc comments.** `/** */` on every export of
`temper-ts`, `temper-telemetry-ts` and `tasker-ts`, saying what the code does not. *Why:* consumers
read the declaration file, not the source. Biome has no rule for it, so it is a convention.

## Not specified

- **Formatting** — indentation, quotes, semicolons, trailing commas. Biome's formatter is the tool;
  the settings stay per repository for now, and temper's own packages disagree (spaces vs tabs,
  double vs single quotes). Unifying them is a churn-only change for a later ruling.
- **`#private` vs `private`** — Google bans `#private` for downlevelling cost; we target ES2022,
  where it is native. Either is fine.
- **`interface` vs `type`, `readonly`, function declarations vs arrows, braces on one-line blocks,
  static-only classes, decorators** (none in use) — review judgment. Google's preferences here would
  churn hundreds of sites (`useBlockStatements` alone: 680+) for no defect class.
- **Floating promises** — the defect is real, but Biome's `noFloatingPromises` is still a nursery
  rule. Revisit when it is stable.
- **Linting `.svelte` files** — Biome's Svelte support is partial; temper-ui's 103 and the desktop
  app's 88 components are typechecked by `svelte-check` only.
- **Google's HTML/CSS guide** — Tailwind and temper's colour-token guards cover what matters.
