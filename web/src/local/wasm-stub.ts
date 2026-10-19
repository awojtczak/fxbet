// Stands in for 'fxbet-wasm' in the server build, where the browser backend is never loaded.
export default function init(): Promise<never> {
  return Promise.reject(new Error('browser storage backend is not part of this build'))
}
export class LocalApi {}
