# FX Bet

Self-hosted dashboard for one recurring decision: **I earn USD — do I convert to PLN now and put it on a
daily-capitalization savings account, or keep it in USD (or EUR) savings and wait for a better USD/PLN?**

Analyse a scenario, place it as a **bet**, record what you actually exchanged, and the app replays it
daily against real ECB fixings and your accounts' variable rates — so over time you see how right you were and
what every other path would have earned with the same money.

Rust (axum + SQLite) backend, React (Vite + Recharts) frontend, one Docker container.

## What it answers

| Question | Where |
|---|---|
| How much more PLN if USD/PLN goes 3.89 → 3.93? | Analyzer → *Extra PLN if target hit* |
| How many days of savings interest is that? | Analyzer → *= days of PLN interest* — `ln(target/spot) / ln(1 + r·(1−tax)/365)` |
| How long can I wait before waiting stops paying, given USD still earns interest? | Analyzer → *Break-even wait* — `ln(target/spot) / ln((1+d_pln)/(1+d_usd))` |
| What's the chance it gets there in time? | Model (GBM first-passage, realised volatility) vs **your** probability slider |
| How confident must I be for waiting to make sense? | *Required confidence* = (now − E[miss]) / (E[hit] − E[miss]) from Monte Carlo |
| How bad can waiting go? | Outcome percentile curve: limit order at target, convert at deadline if missed |
| Which targets/days are worth it? | Sensitivity heatmap (target × day hit) |
| Did my bet work? What if I'd converted on any other day, kept USD, or gone via EUR? | Bet page: five paths over time + value-by-conversion-day chart |
| How did a decision I made *before* using this app work out? | Bets → **Record a past decision**: date, amount, target, deadline and your real conversions in one form; replayed like any other bet |
| Is my gut better than the model? | Calibration: Brier scores and reliability diagram, you vs model |

### Model notes

- Interest accrues daily (ACT/365), net of tax (default 19% Belka), capitalized daily or on the 1st of each month.
  Per-capitalization rounding to the grosz is ignored.
- Variable rates: each account has a dated rate schedule. Add a row when your bank changes the rate (future
  dates work for announced cuts) — analyses and bet replays use the schedule.
- FX: ECB reference rates via [Frankfurter](https://frankfurter.dev) (no key), USD base, PLN and EUR,
  forward-filled over weekends. Your bank's rate differs by its spread: set *Bank spread per conversion* and
  record conversions at the rate you actually got.
- Probability: USD/PLN as geometric Brownian motion; volatility defaults to realised (selectable lookback),
  drift defaults to 0 (no directional view). Override both. Your probability re-weights the simulated "hit" vs
  "miss" outcomes, so the cost of a miss stays priced in.
- A target is "hit" if any daily fixing in the window reaches it; the limit-order path fills at the target.
- Past decisions are stored as *retrospective* bets. The model probability is reconstructed from data available
  on the start date; your own probability, written with hindsight, is shown but excluded from calibration.
  Enter the savings rates that applied back then as dated rates under **Accounts**.

## Try it in the browser

https://awojtczak.github.io/fxbet/ runs the whole app without a server: the Rust API is compiled to
WebAssembly, ECB rates are fetched from Frankfurter by the browser, and accounts and bets are saved in that
browser's localStorage, so they don't sync between devices and are lost if site data is cleared. Self-host
for a shared, durable database.

## Run it (self-host)

```bash
docker compose up -d --build
```

Open http://localhost:8080. The SQLite database lives in the `fxbet-data` volume. On first start it fetches FX
history from 2016 and creates three placeholder accounts (PLN 5% daily, USD 3% monthly, EUR 2% monthly) — edit
them under **Accounts** to match your bank.

Compose binds to `127.0.0.1` only; put it behind your reverse proxy (Caddy/Traefik/nginx) with auth if you
expose it. The app has no authentication of its own.

Backup:

```bash
docker compose exec fxbet sh -c 'cat /data/fxbet.db' > fxbet-backup.db
```

### Configuration

| Variable | Default | |
|---|---|---|
| `FXBET_BIND` | `0.0.0.0:8080` | listen address |
| `FXBET_DATABASE_URL` | `sqlite://data/fxbet.db?mode=rwc` (`/data/fxbet.db` in Docker) | SQLite file |
| `FXBET_STATIC_DIR` | `../web/dist` (`/app/web` in Docker) | built frontend |
| `FXBET_FX_PROVIDER_URL` | `https://api.frankfurter.dev/v1` | any Frankfurter-compatible API |
| `FXBET_FX_SYNC_HOURS` | `6` | background sync interval |
| `FXBET_FX_HISTORY_START` | `2016-01-01` | first date fetched on an empty DB |
| `RUST_LOG` | `info` | log filter |

## Develop

```bash
# backend on :8088 (serves web/dist if built)
cd server && FXBET_BIND=127.0.0.1:8088 FXBET_DATABASE_URL='sqlite://../data/dev.db?mode=rwc' cargo run
```

```bash
# frontend with hot reload on :5173, proxying /api to :8088
cd web && npm install && npm run dev
```

```bash
# browser-only (GitHub Pages) build; needs wasm-pack and the wasm32-unknown-unknown target
cd web && npm run build:pages && npx vite preview --mode pages --outDir dist-pages   # http://localhost:4173/fxbet/
```

```bash
for c in core server wasm; do (cd $c && cargo test); done
```

### Layout

```
core/src/domain/     pure model — savings growth, GBM/stats, analysis, bet replay, calibration (unit-tested)
core/src/service/    request validation and response shaping per endpoint, shared by server and wasm
server/src/api/      axum handlers, router tests
server/src/db.rs     SQLite repository (migrations in server/migrations)
server/src/fxsync.rs ECB rate sync
wasm/                the /api router for the browser build, records in localStorage
web/src/pages/       Dashboard, Analyzer, Bets, BetDetail, Calibration, Accounts
```

### API

`GET /api/fx/latest` · `GET /api/fx/history?from=&to=` · `GET /api/fx/stats?lookback_days=` · `POST /api/fx/sync` ·
`GET|POST /api/accounts` · `PUT|DELETE /api/accounts/{id}` · `POST /api/accounts/{id}/rates` ·
`DELETE /api/accounts/{id}/rates/{rate_id}` · `POST /api/analyze` · `GET|POST /api/bets` ·
`GET|PATCH|DELETE /api/bets/{id}` · `POST /api/bets/{id}/conversions` ·
`DELETE /api/bets/{id}/conversions/{cid}` · `GET /api/calibration`

Not financial advice: this is a calculator for your own decisions.
