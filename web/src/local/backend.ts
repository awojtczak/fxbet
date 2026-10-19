// Browser-only backend for the GitHub Pages build: the Rust /api router runs as WebAssembly,
// accounts and bets live in localStorage, ECB rates come straight from Frankfurter.

import init, { LocalApi } from 'fxbet-wasm'

const STORE_KEY = 'fxbet.store.v1'
const FX_KEY = 'fxbet.fx.v1'
const SYNCED_AT_KEY = 'fxbet.fx.syncedAt'
const FX_BASE_URL = 'https://api.frankfurter.dev/v1'
const HISTORY_START = '2016-01-01'
const SYNC_EVERY_MS = 6 * 3600_000

export interface LocalReply {
  status: number
  body: unknown
}

let ready: Promise<LocalApi> | undefined

export async function localRequest(method: string, path: string, body?: unknown): Promise<LocalReply> {
  const api = await load()
  if (method === 'POST' && path === '/api/fx/sync') {
    try {
      return { status: 200, body: await sync(api) }
    } catch (e) {
      return { status: 503, body: { error: `FX sync failed: ${(e as Error).message}` } }
    }
  }
  const reply = JSON.parse(
    api.handle(method, path, body === undefined ? undefined : JSON.stringify(body), new Date().toISOString()),
  ) as LocalReply
  if (method !== 'GET' && reply.status < 400 && !save(STORE_KEY, api.store())) {
    return { status: 507, body: { error: 'Could not save to browser storage (full or blocked); the change is lost on reload' } }
  }
  return reply
}

function load(): Promise<LocalApi> {
  ready ??= (async () => {
    await init()
    const api = new LocalApi(read(STORE_KEY), read(FX_KEY), today())
    save(STORE_KEY, api.store())
    const syncedAt = Number(read(SYNCED_AT_KEY) ?? 0)
    if (!api.latest_fixing() || Date.now() - syncedAt > SYNC_EVERY_MS) {
      // Without any history the dashboard can't render, so the first sync is awaited; later
      // refreshes run in the background against the rates already stored.
      const pending = sync(api).catch((e: unknown) => console.warn('FX sync failed', e))
      if (!api.latest_fixing()) await pending
    }
    return api
  })()
  return ready
}

async function sync(api: LocalApi): Promise<{ written: number; latest: string | null }> {
  const urls = api.sync_urls(FX_BASE_URL, HISTORY_START, today())
  const bodies = await Promise.all(
    urls.map(async (url) => {
      const res = await fetch(url)
      if (!res.ok) throw new Error(`rates provider returned ${res.status}`)
      return res.text()
    }),
  )
  const written = bodies.reduce((n, b) => n + api.add_fixings(b), 0)
  save(FX_KEY, api.fx())
  save(SYNCED_AT_KEY, String(Date.now()))
  return { written, latest: api.latest_fixing() ?? null }
}

/** UTC date, matching the server's clock. */
function today(): string {
  return new Date().toISOString().slice(0, 10)
}

function read(key: string): string | null {
  try {
    return localStorage.getItem(key)
  } catch {
    return null
  }
}

function save(key: string, value: string): boolean {
  try {
    localStorage.setItem(key, value)
    return true
  } catch (e) {
    console.error(`localStorage write failed key=${key}`, e)
    return false
  }
}
