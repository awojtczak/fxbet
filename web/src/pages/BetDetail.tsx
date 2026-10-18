import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useMemo, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  ComposedChart,
  Line,
  LineChart,
  ReferenceDot,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'
import { api, type BetDetail as Detail } from '../api'
import { Card, ChartTooltip, ErrorBox, Kpi, Legend, axis, grid } from '../components/ui'
import { fmtDate, fmtDays, fmtPct, fmtPln, fmtRate, fmtShortDate, fmtSignedPln, fmtUsd, isoToday, signClass } from '../format'

const PATHS = [
  { key: 'actual', label: 'What you did', color: 'var(--s1)' },
  { key: 'convert_day0', label: 'Convert all on day 0', color: 'var(--s2)' },
  { key: 'hold_usd', label: 'Keep all in USD', color: 'var(--s3)' },
  { key: 'eur_route', label: 'Move to EUR savings', color: 'var(--s4)' },
  { key: 'target_order', label: 'Limit order at target', color: 'var(--s5)' },
] as const

export default function BetDetail() {
  const id = Number(useParams().id)
  const navigate = useNavigate()
  const qc = useQueryClient()
  const q = useQuery({ queryKey: ['bet', id], queryFn: () => api.bet(id) })
  const onChange = (d: Detail) => {
    qc.setQueryData(['bet', id], d)
    qc.invalidateQueries({ queryKey: ['bets'] })
    qc.invalidateQueries({ queryKey: ['calibration'] })
  }
  const remove = useMutation({
    mutationFn: () => api.deleteBet(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['bets'] })
      navigate('/bets')
    },
  })

  if (q.error) return <ErrorBox error={q.error} />
  if (!q.data) return <div className="empty">Loading…</div>
  const d = q.data
  const { bet, tracking: t } = d
  const s = t.summary
  const o = t.outcome

  return (
    <>
      <div className="page-head">
        <div>
          <h1>
            {bet.title} <span className={`badge ${d.status}`}>{d.status}</span>
          </h1>
          <p>
            {fmtUsd(bet.amount_usd)} at {fmtRate(bet.spot_usd_pln)} on {fmtDate(bet.start_date)} · target {fmtRate(bet.target_rate)} by{' '}
            {fmtDate(bet.deadline)} · valued as of {fmtDate(t.as_of)}
          </p>
        </div>
        <button
          className="danger"
          onClick={() => {
            if (confirm('Delete this bet and its conversions?')) remove.mutate()
          }}
        >
          Delete bet
        </button>
      </div>
      <ErrorBox error={remove.error} />

      <div className="kpis">
        <Kpi label="Your position now" value={fmtPln(s.actual)} hint={s.usd_remaining > 0.01 ? `${fmtUsd(s.usd_remaining)} still in USD` : 'fully converted'} emph />
        <Kpi
          label="Edge vs converting on day 0"
          value={fmtSignedPln(s.edge_vs_day0_pln)}
          className={signClass(s.edge_vs_day0_pln)}
          hint={s.edge_in_pln_interest_days != null ? `= ${fmtDays(s.edge_in_pln_interest_days)} of PLN interest` : undefined}
          emph
        />
        <Kpi
          label="Best single conversion day"
          value={s.best_day ? fmtPln(s.best_day.final_value) : '—'}
          hint={s.best_day ? `${fmtDate(s.best_day.date)} at ${fmtRate(s.best_day.usd_pln)} · regret ${fmtPln(s.regret_vs_best_pln)}` : undefined}
        />
        <Kpi label="You beat" value={fmtPct(s.actual_beats_share, 0)} hint="of all possible single-day conversions" />
        <Kpi
          label="Target"
          value={o.target_hit ? `hit ${o.first_hit_date ? fmtShortDate(o.first_hit_date) : ''}` : o.resolved ? 'missed' : 'not yet'}
          hint={`window range ${fmtRate(o.min_rate_in_window)} – ${fmtRate(o.max_rate_in_window)}`}
        />
        <Kpi
          label="Brier score (lower is better)"
          value={o.brier_subjective != null ? o.brier_subjective.toFixed(3) : o.resolved ? '—' : 'pending'}
          hint={`you ${fmtPct(bet.subjective_prob, 0)} · model ${fmtPct(bet.model_prob, 0)}${o.brier_model != null ? ` (Brier ${o.brier_model.toFixed(3)})` : ''}`}
        />
      </div>

      <div className="grid cols-2">
        <Card title="Paths: value in PLN over time" sub="Same USD amount, five ways. Unconverted USD/EUR is marked at that day’s fixing minus spread.">
          <PathsChart d={d} />
        </Card>
        <Card title="USD/PLN during the bet" sub="Fixings, your target, starting rate and your conversions.">
          <RateChart d={d} />
        </Card>
        <Card
          title="If you had converted everything on day X"
          sub={`Value today (${fmtDate(t.as_of)}) for each possible conversion day, including interest before and after. Line: your actual position.`}
        >
          <ConversionDaysChart d={d} />
        </Card>
        <Card title="At the time of the bet" sub="Snapshot of the analysis when you placed it.">
          <Snapshot d={d} />
        </Card>
      </div>

      <div className="grid cols-2" style={{ marginTop: 14 }}>
        <Card title="Conversions" sub="Record each actual exchange: date, USD sold and the PLN rate you really got (after spread).">
          <Conversions d={d} onChange={onChange} />
        </Card>
        <Card title="Notes">
          <Notes d={d} onChange={onChange} />
        </Card>
      </div>
    </>
  )
}

function PathsChart({ d }: { d: Detail }) {
  const conv = new Set(d.conversions.map((c) => c.date))
  return (
    <>
      <Legend items={PATHS.map((p) => ({ label: p.label, color: p.color }))} />
      <div style={{ height: 300 }}>
        <ResponsiveContainer>
          <LineChart data={d.tracking.days} margin={{ top: 6, right: 12, bottom: 0, left: 8 }}>
            <CartesianGrid {...grid} />
            <XAxis dataKey="date" {...axis} tickFormatter={fmtShortDate} minTickGap={40} />
            <YAxis {...axis} domain={['auto', 'auto']} tickFormatter={(v: number) => `${(v / 1000).toFixed(1)}k`} width={48} />
            <Tooltip
              cursor={{ stroke: 'var(--axis)' }}
              content={
                <ChartTooltip
                  title={(x) => `${fmtDate(String(x))}${conv.has(String(x)) ? ' · conversion' : ''}`}
                  rows={[
                    ...PATHS.map((p) => ({ key: p.key, label: p.label, color: p.color, format: (v: number) => fmtPln(v) })),
                    { key: 'usd_pln', label: 'USD/PLN', format: (v: number) => fmtRate(v) },
                  ]}
                />
              }
            />
            {[...PATHS].reverse().map((p) => (
              <Line
                key={p.key}
                dataKey={p.key}
                stroke={p.color}
                strokeWidth={p.key === 'actual' ? 2.5 : 1.5}
                dot={false}
                isAnimationActive={false}
              />
            ))}
          </LineChart>
        </ResponsiveContainer>
      </div>
    </>
  )
}

function RateChart({ d }: { d: Detail }) {
  const { bet, tracking } = d
  const days = tracking.days
  const domain = useMemo(() => {
    const vals = [...days.map((x) => x.usd_pln), bet.target_rate, bet.spot_usd_pln, ...d.conversions.map((c) => c.rate)]
    const min = Math.min(...vals)
    const max = Math.max(...vals)
    const pad = (max - min) * 0.1 || 0.01
    return [min - pad, max + pad] as [number, number]
  }, [days, bet, d.conversions])
  return (
    <>
      <Legend
        items={[
          { label: 'USD/PLN fixing', color: 'var(--s1)' },
          { label: 'Target', color: 'var(--s3)' },
          { label: 'Start rate', color: 'var(--muted)', kind: 'dash' },
          { label: 'Your conversions', color: 'var(--s2)' },
        ]}
      />
      <div style={{ height: 300 }}>
        <ResponsiveContainer>
          <ComposedChart data={days} margin={{ top: 6, right: 12, bottom: 0, left: 0 }}>
            <CartesianGrid {...grid} />
            <XAxis dataKey="date" {...axis} tickFormatter={fmtShortDate} minTickGap={40} />
            <YAxis {...axis} domain={domain} tickFormatter={(v: number) => v.toFixed(3)} width={52} />
            <Tooltip
              cursor={{ stroke: 'var(--axis)' }}
              content={
                <ChartTooltip
                  title={(x) => fmtDate(String(x))}
                  rows={[
                    { key: 'usd_pln', label: 'USD/PLN', color: 'var(--s1)', format: (v) => fmtRate(v) },
                    { key: 'eur_pln', label: 'EUR/PLN', format: (v) => fmtRate(v) },
                  ]}
                />
              }
            />
            <ReferenceLine y={bet.target_rate} stroke="var(--s3)" strokeWidth={2} />
            <ReferenceLine y={bet.spot_usd_pln} stroke="var(--muted)" strokeDasharray="4 4" />
            {bet.deadline <= tracking.as_of && (
              <ReferenceLine x={bet.deadline} stroke="var(--muted)" label={{ value: 'deadline', fill: 'var(--text-2)', fontSize: 11, position: 'insideTopRight' }} />
            )}
            <Line dataKey="usd_pln" stroke="var(--s1)" strokeWidth={2} dot={false} isAnimationActive={false} type="stepAfter" />
            {d.conversions.map((c) => (
              <ReferenceDot key={c.id} x={c.date} y={c.rate} r={6} fill="var(--s2)" stroke="var(--surface)" strokeWidth={2} />
            ))}
          </ComposedChart>
        </ResponsiveContainer>
      </div>
    </>
  )
}

function ConversionDaysChart({ d }: { d: Detail }) {
  const s = d.tracking.summary
  const data = d.tracking.conversion_days
  return (
    <>
      <Legend
        items={[
          { label: 'Beats what you did', color: 'var(--div-neg)', kind: 'area' },
          { label: 'Worse than what you did', color: 'var(--div-pos)', kind: 'area' },
          { label: 'Your position', color: 'var(--text)', kind: 'dash' },
        ]}
      />
      <div style={{ height: 300 }}>
        <ResponsiveContainer>
          <BarChart data={data} margin={{ top: 6, right: 12, bottom: 0, left: 8 }} barCategoryGap={1}>
            <CartesianGrid {...grid} />
            <XAxis dataKey="date" {...axis} tickFormatter={fmtShortDate} minTickGap={40} />
            <YAxis
              {...axis}
              domain={[(min: number) => Math.min(min, s.actual) * 0.998, (max: number) => Math.max(max, s.actual) * 1.002]}
              tickFormatter={(v: number) => `${(v / 1000).toFixed(1)}k`}
              width={48}
              allowDataOverflow
            />
            <Tooltip
              cursor={{ fill: 'var(--surface-2)' }}
              content={
                <ChartTooltip
                  title={(x) => `Convert all on ${fmtDate(String(x))}`}
                  rows={[
                    { key: 'usd_pln', label: 'Rate', format: (v) => fmtRate(v) },
                    { key: 'final_value', label: 'Worth today', format: (v) => fmtPln(v) },
                    { key: 'final_value', label: 'vs you', format: (v) => fmtSignedPln(v - s.actual) },
                  ]}
                />
              }
            />
            <ReferenceLine y={s.actual} stroke="var(--text)" strokeDasharray="5 4" strokeWidth={1.5} />
            <Bar dataKey="final_value" isAnimationActive={false} radius={[2, 2, 0, 0]}>
              {data.map((c) => (
                <Cell key={c.date} fill={c.final_value > s.actual ? 'var(--div-neg)' : 'var(--div-pos)'} />
              ))}
            </Bar>
          </BarChart>
        </ResponsiveContainer>
      </div>
    </>
  )
}

function Snapshot({ d }: { d: Detail }) {
  const snap = d.bet.snapshot
  const acc = (id: number) => d.accounts.find((a) => a.id === id)
  return (
    <dl className="facts">
      <dt>Spot / target</dt>
      <dd>
        {fmtRate(snap.inputs.spot)} → {fmtRate(d.bet.target_rate)} ({snap.headline.required_move_pct.toFixed(2)}%)
      </dd>
      <dt>Extra PLN if hit</dt>
      <dd>
        {fmtSignedPln(snap.headline.fx_gain_pln)} = {fmtDays(snap.headline.fx_gain_in_pln_interest_days)} of PLN interest
      </dd>
      <dt>Break-even wait</dt>
      <dd>{fmtDays(snap.headline.breakeven_wait_days)}</dd>
      <dt>Volatility used</dt>
      <dd>
        {snap.inputs.vol_pct.toFixed(2)}% ({snap.inputs.vol_source}), drift {snap.inputs.drift_pct.toFixed(1)}%
      </dd>
      <dt>P(hit): you / model</dt>
      <dd>
        {fmtPct(d.bet.subjective_prob, 0)} / {fmtPct(d.bet.model_prob, 0)} · break-even {fmtPct(snap.decision.required_prob, 0)}
      </dd>
      <dt>Verdict then</dt>
      <dd>
        {snap.decision.verdict.replace('_', ' ')} ({fmtSignedPln(snap.decision.subjective_edge_pln ?? snap.decision.model_edge_pln)} expected)
      </dd>
      <dt>Accounts</dt>
      <dd>
        {[d.bet.pln_account_id, d.bet.usd_account_id, d.bet.eur_account_id]
          .map((i) => acc(i))
          .map((a) => (a ? `${a.currency} ${a.current_rate_pct}%` : '?'))
          .join(' · ')}
      </dd>
      <dt>Spread</dt>
      <dd>{d.bet.spread_pct}%</dd>
    </dl>
  )
}

function Conversions({ d, onChange }: { d: Detail; onChange: (d: Detail) => void }) {
  const remaining = d.tracking.summary.usd_remaining
  const today = isoToday()
  const [date, setDate] = useState(today)
  const [amount, setAmount] = useState('')
  const [rate, setRate] = useState('')
  const fixing = d.tracking.days.find((x) => x.date === date)?.usd_pln
  const add = useMutation({
    mutationFn: () => api.addConversion(d.bet.id, { date, usd_amount: Number(amount || remaining.toFixed(2)), rate: Number(rate || fixing) }),
    onSuccess: (x) => {
      setAmount('')
      setRate('')
      onChange(x)
    },
  })
  const del = useMutation({ mutationFn: (cid: number) => api.deleteConversion(d.bet.id, cid), onSuccess: onChange })
  useEffect(() => setRate(''), [date])

  return (
    <>
      <ErrorBox error={add.error ?? del.error} />
      {d.conversions.length > 0 && (
        <table style={{ marginBottom: 12 }}>
          <thead>
            <tr>
              <th>Date</th>
              <th className="num">USD</th>
              <th className="num">Rate</th>
              <th className="num">PLN</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {d.conversions.map((c) => (
              <tr key={c.id}>
                <td>{fmtDate(c.date)}</td>
                <td className="num">{fmtUsd(c.usd_amount)}</td>
                <td className="num">{fmtRate(c.rate)}</td>
                <td className="num">{fmtPln(c.usd_amount * c.rate)}</td>
                <td className="num">
                  <button className="danger" onClick={() => del.mutate(c.id)}>
                    Remove
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {remaining > 0.01 ? (
        <div className="form">
          <div className="row">
            <label className="field">
              Date
              <input type="date" min={d.bet.start_date} max={today} value={date} onChange={(e) => setDate(e.target.value)} />
            </label>
            <label className="field">
              USD sold
              <input type="number" placeholder={remaining.toFixed(2)} value={amount} onChange={(e) => setAmount(e.target.value)} />
            </label>
            <label className="field">
              PLN per USD
              <input type="number" step="0.0001" placeholder={fixing ? fixing.toFixed(4) : ''} value={rate} onChange={(e) => setRate(e.target.value)} />
            </label>
          </div>
          <div className="actions">
            <button className="primary" disabled={add.isPending || !(rate || fixing)} onClick={() => add.mutate()}>
              Record conversion
            </button>
          </div>
          <p className="muted" style={{ margin: 0, fontSize: 12 }}>
            Blank fields default to everything left ({fmtUsd(remaining)} incl. interest) at that day’s ECB fixing.
          </p>
        </div>
      ) : (
        <p className="muted">Fully converted.</p>
      )}
    </>
  )
}

function Notes({ d, onChange }: { d: Detail; onChange: (d: Detail) => void }) {
  const [title, setTitle] = useState(d.bet.title)
  const [notes, setNotes] = useState(d.bet.notes)
  const save = useMutation({ mutationFn: () => api.updateBet(d.bet.id, title, notes), onSuccess: onChange })
  const dirty = title !== d.bet.title || notes !== d.bet.notes
  return (
    <div className="form">
      <ErrorBox error={save.error} />
      <label className="field">
        Title
        <input value={title} onChange={(e) => setTitle(e.target.value)} />
      </label>
      <label className="field">
        Reasoning and lessons
        <textarea rows={6} value={notes} onChange={(e) => setNotes(e.target.value)} />
      </label>
      <div className="actions">
        <button className="primary" disabled={!dirty || save.isPending} onClick={() => save.mutate()}>
          Save
        </button>
      </div>
    </div>
  )
}
