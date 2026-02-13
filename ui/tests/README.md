# UI Smoke Tests (Playwright)

These tests validate that the core UI pages render and that auth bootstrap (`/auth`) can reach the UI shell.

## Prerequisites

1. Start `rust-mule` locally with API/UI enabled.
2. Ensure `http://127.0.0.1:17835/auth` is reachable.
3. Install Node.js 20+.

## Install

```bash
cd ui
npm install
npx playwright install
```

## Run

```bash
cd ui
npm run test:ui:smoke
```

Use another base URL:

```bash
cd ui
UI_BASE_URL=http://127.0.0.1:17836 npm run test:ui:smoke
```

Headed/debug:

```bash
cd ui
npm run test:ui:smoke:headed
npm run test:ui:smoke:debug
```
