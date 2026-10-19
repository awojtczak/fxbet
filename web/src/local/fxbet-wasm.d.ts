// The Rust /api router compiled to WebAssembly (../wasm, built by `npm run build:pages`).
// Declared here so the server build type-checks without the generated package.
declare module 'fxbet-wasm' {
  export default function init(): Promise<unknown>
  export class LocalApi {
    constructor(store: string | null | undefined, fx: string | null | undefined, today: string)
    handle(method: string, url: string, body: string | null | undefined, now: string): string
    store(): string
    fx(): string
    sync_urls(baseUrl: string, historyStart: string, today: string): string[]
    add_fixings(body: string): number
    latest_fixing(): string | undefined
  }
}
