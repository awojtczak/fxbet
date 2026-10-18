const pln = new Intl.NumberFormat('en-GB', { maximumFractionDigits: 0 })
const pln2 = new Intl.NumberFormat('en-GB', { minimumFractionDigits: 2, maximumFractionDigits: 2 })

export const fmtPln = (v: number | null | undefined, digits: 0 | 2 = 0) =>
  v == null || !Number.isFinite(v) ? '—' : `${(digits === 2 ? pln2 : pln).format(v)} zł`

export const fmtSignedPln = (v: number | null | undefined, digits: 0 | 2 = 0) =>
  v == null || !Number.isFinite(v) ? '—' : `${v > 0 ? '+' : v < 0 ? '−' : ''}${fmtPln(Math.abs(v), digits)}`

export const fmtUsd = (v: number | null | undefined) =>
  v == null || !Number.isFinite(v) ? '—' : `$${pln.format(v)}`

export const fmtRate = (v: number | null | undefined, digits = 4) =>
  v == null || !Number.isFinite(v) ? '—' : v.toFixed(digits)

export const fmtPct = (v: number | null | undefined, digits = 1) =>
  v == null || !Number.isFinite(v) ? '—' : `${(v * 100).toFixed(digits)}%`

export const fmtDays = (v: number | null | undefined) =>
  v == null ? 'never' : !Number.isFinite(v) ? '∞' : `${Math.round(v)} d`

export const fmtDate = (iso: string) =>
  new Date(`${iso}T00:00:00`).toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric' })

export const fmtShortDate = (iso: string) =>
  new Date(`${iso}T00:00:00`).toLocaleDateString('en-GB', { day: 'numeric', month: 'short' })

export const isoToday = () => new Date().toISOString().slice(0, 10)

export const isoDaysAgo = (days: number) => new Date(Date.now() - days * 86_400_000).toISOString().slice(0, 10)

export const addDays = (iso: string, days: number) => {
  const d = new Date(`${iso}T00:00:00Z`)
  d.setUTCDate(d.getUTCDate() + days)
  return d.toISOString().slice(0, 10)
}

export const signClass = (v: number | null | undefined) => (v == null ? '' : v > 0 ? 'pos' : v < 0 ? 'neg' : '')
