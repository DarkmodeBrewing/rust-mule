# Private Alpha Release Checklist

Status: ACTIVE
Last Reviewed: 2026-03-10

This checklist defines the minimum bar for cutting a private alpha tag such as
`v0.1.0-alpha.1`.

## Intended Scope

The private alpha is for controlled evaluation, not public distribution.

Primary goals:

- validate packaged binaries on real user machines
- exercise the full application flow end-to-end
- capture UX, config, interoperability, and runtime friction
- prove that release artifacts and startup/config contracts are coherent

## Supported Alpha Platform Targets

- Linux:
  - native release bundle built on `ubuntu-latest`
- macOS:
  - native arm64 release bundle built on `macos-latest`
  - x86_64 target build produced on `macos-latest`
  - intended private alpha Intel support floor: `macOS 12.0`
  - implemented for the x86_64 build via `MACOSX_DEPLOYMENT_TARGET=12.0`
- Windows:
  - native release bundle built on `windows-latest`

Notes:

- Intel macOS 12 support is intentionally targeted for the x86_64 build, but still requires
  runtime verification on a real macOS 12 Intel machine before claiming it as validated.
- Multi-platform release artifacts are built natively in CI/release workflows, not by
  cross-compiling everything from Linux.

## Artifact Requirements

Before tagging:

1. CI build matrix is green on:
   - Linux
   - macOS
   - Windows
2. Packaged artifact smoke checks pass on each native runner:
   - `--version`
   - `--help`
   - `--check-config --config ./config.example.toml`
3. The tracked repo `config.toml` remains an alpha-safe example baseline:
   - loopback-safe SAM defaults
   - debug endpoints off by default
   - explicit log-file settings
   - explicit empty sharing config

## CLI / Config Contract

The alpha binary must support:

- `rust-mule --config <path>`
- `rust-mule --check-config`
- `rust-mule --help`
- `rust-mule -?`
- `rust-mule --version`

Behavioral expectations:

- normal startup fails clearly if the chosen config path does not exist
- `--help` and `--version` short-circuit parsing
- `--check-config` validates and exits without booting the app

## End-To-End Alpha Flow To Exercise

At least one controlled alpha run should verify:

1. binary starts from packaged artifact
2. config file is readable and valid
3. UI loads successfully
4. shared folder configuration is possible
5. source publishing occurs
6. search/discovery works
7. download starts and progresses
8. upload activity is visible
9. restart/resume behavior works
10. operator telemetry is legible:
   - download/upload rates
   - zero-fill fallback warnings
   - upload session lifecycle history

## Known Alpha Caveats

- macOS 12 is targeted but not yet field-verified until tested on the older Mac.
- The private alpha is expected to surface UX/config rough edges; that is part of the goal.
- This release is still shaped around trusted/private evaluation rather than broad deployment.

## Tagging Decision

Recommended first private alpha tag:

- `v0.1.0-alpha.1`

Cut the tag only after:

1. the merge queue for alpha-readiness work is complete
2. CI is green on `main`
3. at least one final review confirms the checklist items above are satisfied or explicitly
   deferred
