# Docs Directory Guide

`docs/index.md` is the canonical documentation landing page (used by the docs site and for top-level navigation).

This `README.md` explains the folder layout at a glance:

- `00_overview/`: contracts and high-level design principles.
- `10_architecture/`: architecture/design references (API, UI, download, refactor plans).
- `20_protocol/`: protocol compatibility/parity notes.
- `30_operations/`: operational runbooks and API usage examples.
- `governance/`: review checklist, execution tasks, and handoff log.
- `rfcs/`: RFC-style technical notes.
- `90_archive/`: legacy or historical docs kept for reference.
- `public/`: static assets for docs-site publishing.

Related paths:

- `../site/`: VitePress configuration and site scaffolding.
- `../scripts/docs/`: docs helper scripts for API interactions.
- `../scripts/test/`: soak/scenario test harnesses.
- `../scripts/build/`: build/release helper scripts.

If links drift, update `docs/index.md` first, then keep this file aligned with folder-level intent only.
