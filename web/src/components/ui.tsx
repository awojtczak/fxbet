import type { ReactNode } from 'react'
import type { TooltipContentProps } from 'recharts'

export function Kpi(props: { label: string; value: ReactNode; hint?: ReactNode; emph?: boolean; className?: string }) {
  return (
    <div className={`kpi${props.emph ? ' emph' : ''}`}>
      <div className="label">{props.label}</div>
      <div className={`value ${props.className ?? ''}`}>{props.value}</div>
      {props.hint && <div className="hint">{props.hint}</div>}
    </div>
  )
}

export function Card(props: { title: string; sub?: ReactNode; right?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <section className={`card ${props.className ?? ''}`}>
      <div className="card-head">
        <h2>{props.title}</h2>
        {props.right}
      </div>
      {props.sub && <p className="sub">{props.sub}</p>}
      {props.children}
    </section>
  )
}

export interface LegendItem {
  label: string
  color: string
  kind?: 'line' | 'dash' | 'area'
}

export function Legend({ items }: { items: LegendItem[] }) {
  return (
    <div className="legend">
      {items.map((i) => (
        <span key={i.label}>
          <i
            className={i.kind === 'dash' ? 'dash' : i.kind === 'area' ? 'area' : ''}
            style={{ background: i.color, color: i.color }}
          />
          {i.label}
        </span>
      ))}
    </div>
  )
}

export interface TooltipRow {
  key: string
  label: string
  color?: string
  format: (v: number) => string
}

/** Tooltip body for recharts: rows are declared explicitly so order and wording stay stable. */
export function ChartTooltip(props: {
  rows: TooltipRow[]
  title: (label: string | number) => string
} & Partial<TooltipContentProps<number, string>>) {
  const { active, payload, label, rows, title } = props
  if (!active || !payload?.length) return null
  const datum = payload[0].payload as Record<string, number>
  return (
    <div className="tooltip">
      <div className="t-title">{title(label as string | number)}</div>
      {rows
        .filter((r) => datum[r.key] != null)
        .map((r) => (
          <div className="t-row" key={r.key}>
            <span>
              {r.color && <i style={{ background: r.color }} />}
              {r.label}
            </span>
            <b>{r.format(datum[r.key])}</b>
          </div>
        ))}
    </div>
  )
}

export function Segmented<T extends string | number>(props: {
  options: { value: T; label: string }[]
  value: T
  onChange: (v: T) => void
}) {
  return (
    <div className="seg" role="group">
      {props.options.map((o) => (
        <button key={o.value} className={o.value === props.value ? 'on' : ''} onClick={() => props.onChange(o.value)}>
          {o.label}
        </button>
      ))}
    </div>
  )
}

export function Modal(props: { title: string; onClose: () => void; children: ReactNode }) {
  return (
    <div className="modal-backdrop" onClick={props.onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()} role="dialog" aria-label={props.title}>
        <h2>{props.title}</h2>
        {props.children}
      </div>
    </div>
  )
}

export function ErrorBox({ error }: { error: unknown }) {
  if (!error) return null
  return <div className="error">{error instanceof Error ? error.message : String(error)}</div>
}

/** Shared recharts axis styling: recessive ink, no tick lines. */
export const axis = {
  stroke: 'var(--axis)',
  tick: { fill: 'var(--muted)', fontSize: 11 },
  tickLine: false,
} as const

export const grid = { stroke: 'var(--grid)', strokeDasharray: '0', vertical: false } as const
