import { useMutation, useQuery } from '@tanstack/react-query'
import { useEffect, useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  Area,
  CartesianGrid,
  ComposedChart,
  Line,
  LineChart,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'
import { api, type Account, type AnalyzeRequest, type AnalyzeResponse, type Compounding, type Currency } from '../api'
import { Card, ChartTooltip, ErrorBox, Kpi, Legend, Modal, axis, grid } from '../components/ui'
import { addDays, fmtDate, fmtDays, fmtPct, fmtPln, fmtRate, fmtSignedPln, isoToday, signClass } from '../format'

interface FormState {
  amount_usd: string
  target: string
  horizon_days: number
  spot: string
  usd_eur: string
  spread_pct: string
  start_date: string
  pln_account_id: number | null
  usd_account_id: number | null
  eur_account_id: number | null
  whatIfRates: boolean
  pln_rate: string
  usd_rate: string
  eur_rate: string
  vol_pct: string
  drift_pct: string
  lookback_days: number
  useSubjective: boolean
  subjective_prob: number
}

const INITIAL: FormState = {
  amount_usd: '10000',
  target: '3.93',
  horizon_days: 90,
  spot: '',
  usd_eur: '',
  spread_pct: '0',
  start_date: '',
  pln_account_id: null,
  usd_account_id: null,
  eur_account_id: null,
  whatIfRates: false,
  pln_rate: '',
  usd_rate: '',
  eur_rate: '',
  vol_pct: '',
  drift_pct: '',
  lookback_days: 365,
  useSubjective: true,
  subjective_prob: 0.5,
}

const num = (s: string) => (s.trim() === '' ? undefined : Number(s))

function toRequest(f: FormState, accounts: Account[]): AnalyzeRequest {
  const acc = (id: number | null) => accounts.find((a) => a.id === id)
  const override = (id: number | null, rate: string) => {
    const a = acc(id)
    const r = num(rate)
    return f.whatIfRates && r !== undefined
      ? { annual_rate_pct: r, compounding: (a?.compounding ?? 'daily') as Compounding, tax_pct: a?.tax_pct ?? 19 }
      : undefined
  }
  return {
    amount_usd: Number(f.amount_usd),
    target: Number(f.target),
    horizon_days: f.horizon_days,
    spot: num(f.spot),
    usd_eur: num(f.usd_eur),
    spread_pct: num(f.spread_pct) ?? 0,
    start_date: f.start_date || undefined,
    pln_account_id: f.pln_account_id ?? undefined,
    usd_account_id: f.usd_account_id ?? undefined,
    eur_account_id: f.eur_account_id ?? undefined,
    pln: override(f.pln_account_id, f.pln_rate),
    usd: override(f.usd_account_id, f.usd_rate),
    eur: override(f.eur_account_id, f.eur_rate),
    vol_pct: num(f.vol_pct),
    drift_pct: num(f.drift_pct),
    lookback_days: f.lookback_days,
    subjective_prob: f.useSubjective ? f.subjective_prob : undefined,
    paths: 5000,
  }
}

function useDebounced<T>(value: T, ms: number): T {
  const [v, setV] = useState(value)
  useEffect(() => {
    const t = setTimeout(() => setV(value), ms)
    return () => clearTimeout(t)
  }, [value, ms])
  return v
}

export default function Analyzer() {
  const navigate = useNavigate()
  const accounts = useQuery({ queryKey: ['accounts'], queryFn: api.accounts })
  const [form, setForm] = useState<FormState>(INITIAL)
  const [placing, setPlacing] = useState(false)
  const set = <K extends keyof FormState>(k: K, v: FormState[K]) => setForm((f) => ({ ...f, [k]: v }))

  // Pick the first account of each currency once accounts arrive.
  useEffect(() => {
    if (!accounts.data) return
    const first = (c: Currency) => accounts.data.find((a) => a.currency === c)?.id ?? null
    setForm((f) => ({
      ...f,
      pln_account_id: f.pln_account_id ?? first('PLN'),
      usd_account_id: f.usd_account_id ?? first('USD'),
      eur_account_id: f.eur_account_id ?? first('EUR'),
    }))
  }, [accounts.data])

  const req = useMemo(() => toRequest(form, accounts.data ?? []), [form, accounts.data])
  const debounced = useDebounced(req, 300)
  const ready = !!accounts.data && debounced.pln_account_id != null && debounced.usd_account_id != null && debounced.eur_account_id != null
  const result = useQuery({
    queryKey: ['analyze', debounced],
    queryFn: () => api.analyze(debounced),
    enabled: ready,
    placeholderData: (prev) => prev,
    staleTime: Infinity,
  })

  const byCcy = (c: Currency) => accounts.data?.filter((a) => a.currency === c) ?? []
  const r = result.data

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Analyzer</h1>
          <p>Convert USD now and earn PLN interest, or keep earning USD interest and wait for a better USD/PLN?</p>
        </div>
        <button className="primary" disabled={!r} onClick={() => setPlacing(true)}>
          Place this bet
        </button>
      </div>
      <div className="analyzer">
        <aside className="card form">
          <section>
            <h3>Scenario</h3>
            <label className="field">
              Amount (USD)
              <input type="number" min="1" value={form.amount_usd} onChange={(e) => set('amount_usd', e.target.value)} />
            </label>
            <div className="row">
              <label className="field">
                USD/PLN now
                <input
                  type="number"
                  step="0.0001"
                  placeholder={r ? fmtRate(r.inputs.spot) : 'latest'}
                  value={form.spot}
                  onChange={(e) => set('spot', e.target.value)}
                />
              </label>
              <label className="field">
                Target USD/PLN
                <input type="number" step="0.0001" value={form.target} onChange={(e) => set('target', e.target.value)} />
              </label>
            </div>
            <label className="field">
              Deadline: convert anyway after {form.horizon_days} days
              <input
                type="range"
                min="7"
                max="365"
                step="1"
                value={form.horizon_days}
                onChange={(e) => set('horizon_days', Number(e.target.value))}
              />
            </label>
            <label className="field">
              Bank spread per conversion (%)
              <input type="number" step="0.05" min="0" value={form.spread_pct} onChange={(e) => set('spread_pct', e.target.value)} />
            </label>
          </section>

          <section>
            <h3>Your view</h3>
            <label className="row" style={{ fontSize: 12, color: 'var(--text-2)' }}>
              <input
                type="checkbox"
                checked={form.useSubjective}
                onChange={(e) => set('useSubjective', e.target.checked)}
                style={{ flex: 'none' }}
              />
              <span>I have my own probability that the target is hit by the deadline</span>
            </label>
            {form.useSubjective && (
              <div className="prob-slider">
                <input
                  type="range"
                  min="0"
                  max="1"
                  step="0.01"
                  value={form.subjective_prob}
                  onChange={(e) => set('subjective_prob', Number(e.target.value))}
                />
                <output>{fmtPct(form.subjective_prob, 0)}</output>
              </div>
            )}
            {r && (
              <p className="muted" style={{ margin: 0, fontSize: 12 }}>
                Model says {fmtPct(r.analysis.probability.model_hit_prob, 0)}. Waiting pays off above{' '}
                {fmtPct(r.analysis.decision.required_prob, 0)}.
              </p>
            )}
          </section>

          <section>
            <h3>Savings accounts</h3>
            {(['PLN', 'USD', 'EUR'] as const).map((c) => {
              const key = `${c.toLowerCase()}_account_id` as 'pln_account_id' | 'usd_account_id' | 'eur_account_id'
              return (
                <label className="field" key={c}>
                  {c}
                  <select value={form[key] ?? ''} onChange={(e) => set(key, Number(e.target.value))}>
                    {byCcy(c).map((a) => (
                      <option key={a.id} value={a.id}>
                        {a.name} · {a.current_rate_pct}% · {a.compounding}
                      </option>
                    ))}
                  </select>
                </label>
              )
            })}
            <label className="row" style={{ fontSize: 12, color: 'var(--text-2)' }}>
              <input type="checkbox" checked={form.whatIfRates} onChange={(e) => set('whatIfRates', e.target.checked)} style={{ flex: 'none' }} />
              <span>What-if: override annual rates (gross %)</span>
            </label>
            {form.whatIfRates && (
              <div className="row">
                {(['pln_rate', 'usd_rate', 'eur_rate'] as const).map((k) => (
                  <label className="field" key={k}>
                    {k.slice(0, 3).toUpperCase()}
                    <input
                      type="number"
                      step="0.05"
                      placeholder={r ? String(r.inputs[`${k}_pct` as 'pln_rate_pct']) : ''}
                      value={form[k]}
                      onChange={(e) => set(k, e.target.value)}
                    />
                  </label>
                ))}
              </div>
            )}
          </section>

          <section>
            <h3>Model</h3>
            <div className="row">
              <label className="field">
                Volatility % p.a.
                <input
                  type="number"
                  step="0.1"
                  placeholder={r ? r.inputs.vol_pct.toFixed(1) : ''}
                  value={form.vol_pct}
                  onChange={(e) => set('vol_pct', e.target.value)}
                />
              </label>
              <label className="field">
                Drift % p.a.
                <input type="number" step="0.5" placeholder="0" value={form.drift_pct} onChange={(e) => set('drift_pct', e.target.value)} />
              </label>
            </div>
            <label className="field">
              Volatility lookback
              <select value={form.lookback_days} onChange={(e) => set('lookback_days', Number(e.target.value))}>
                <option value={90}>3 months</option>
                <option value={180}>6 months</option>
                <option value={365}>1 year</option>
                <option value={1095}>3 years</option>
                <option value={1826}>5 years</option>
              </select>
            </label>
            <label className="field">
              Analysis date (backdate to test a past decision)
              <input type="date" max={isoToday()} value={form.start_date} onChange={(e) => set('start_date', e.target.value)} />
            </label>
            {r && (
              <p className="muted" style={{ margin: 0, fontSize: 12 }}>
                Vol {r.inputs.vol_pct.toFixed(2)}% ({r.inputs.vol_source}), fixing {r.inputs.fixing_date ? fmtDate(r.inputs.fixing_date) : '—'}
              </p>
            )}
          </section>
        </aside>

        <div style={{ minWidth: 0, opacity: result.isFetching ? 0.75 : 1, transition: 'opacity .15s' }}>
          <ErrorBox error={result.error ?? accounts.error} />
          {r ? <Results r={r} target={Number(form.target)} /> : <div className="card empty">Loading analysis…</div>}
        </div>
      </div>
      {placing && r && (
        <PlaceBet
          req={req}
          r={r}
          onClose={() => setPlacing(false)}
          onPlaced={(id) => navigate(`/bets/${id}`)}
        />
      )}
    </>
  )
}

function Results({ r, target }: { r: AnalyzeResponse; target: number }) {
  const { headline: h, probability: p, monte_carlo: mc, decision: d, curves, sensitivity } = r.analysis
  const horizon = curves.length - 1
  const verdictText = {
    wait: { icon: '⏳', title: 'Waiting has positive expected value', sym: '' },
    convert_now: { icon: '⇄', title: 'Converting now has higher expected value', sym: '' },
    toss: { icon: '≈', title: 'Too close to call', sym: '' },
  }[d.verdict]
  const usedProb = p.subjective_prob ?? p.model_hit_prob
  const edge = d.subjective_edge_pln ?? d.model_edge_pln

  return (
    <>
      <div className={`verdict ${d.verdict}`}>
        <div className="icon" aria-hidden>
          {verdictText.icon}
        </div>
        <div>
          <strong>{verdictText.title}</strong>
          <p>
            With {p.subjective_prob != null ? 'your' : 'the model’s'} {fmtPct(usedProb, 0)} chance of hitting {fmtRate(target)} within{' '}
            {horizon} days, waiting is worth <b className={signClass(edge)}>{fmtSignedPln(edge)}</b> vs converting today (value at{' '}
            {fmtDate(r.inputs.deadline)}). Break-even confidence: <b>{fmtPct(d.required_prob, 0)}</b>.
          </p>
        </div>
      </div>

      <div className="kpis">
        <Kpi
          label="Extra PLN if target hit"
          value={fmtSignedPln(h.fx_gain_pln)}
          hint={`${fmtPln(h.pln_now)} → ${fmtPln(h.pln_at_target)} · ${h.required_move_pips.toFixed(0)} pips (${h.required_move_pct.toFixed(2)}%)`}
          emph
        />
        <Kpi
          label="= days of PLN interest"
          value={fmtDays(h.fx_gain_in_pln_interest_days)}
          hint={`PLN account earns ${fmtPln(h.pln_interest_per_day, 2)}/day net`}
          emph
        />
        <Kpi
          label="Break-even wait"
          value={fmtDays(h.breakeven_wait_days)}
          hint={`USD still earns ${fmtPln(h.usd_interest_per_day_pln, 2)}/day; carry cost ${fmtPln(h.carry_cost_per_day_pln, 2)}/day`}
        />
        <Kpi
          label="Hit probability"
          value={`${fmtPct(p.model_hit_prob, 0)} model`}
          hint={`${p.subjective_prob != null ? `${fmtPct(p.subjective_prob, 0)} you · ` : ''}MC ${fmtPct(mc.hit_rate, 0)}, avg ${mc.mean_days_to_hit?.toFixed(0) ?? '—'} d to hit`}
        />
        <Kpi
          label="Required confidence"
          value={fmtPct(d.required_prob, 0)}
          hint="probability at which waiting = converting now"
        />
        <Kpi
          label="Waiting beats converting"
          value={fmtPct(mc.prob_beats_convert_now, 0)}
          hint={`of ${mc.paths.toLocaleString()} simulated paths`}
        />
      </div>

      <div className="grid cols-2">
        <Card
          title="Rate cone vs break-even"
          sub="Simulated USD/PLN range (GBM). Anywhere above the orange line on a given day, converting then beats having converted today."
        >
          <Legend
            items={[
              { label: '5–95%', color: 'var(--band-outer)', kind: 'area' },
              { label: '25–75%', color: 'var(--band-inner)', kind: 'area' },
              { label: 'Median', color: 'var(--s1)', kind: 'dash' },
              { label: 'Break-even rate', color: 'var(--s2)' },
              { label: 'Target', color: 'var(--s3)' },
            ]}
          />
          <div style={{ height: 280 }}>
            <ResponsiveContainer>
              <ComposedChart data={curves} margin={{ top: 6, right: 12, bottom: 0, left: 0 }}>
                <CartesianGrid {...grid} />
                <XAxis dataKey="day" {...axis} tickFormatter={(v: number) => `${v}d`} minTickGap={30} />
                <YAxis {...axis} domain={['auto', 'auto']} tickFormatter={(v: number) => v.toFixed(2)} width={48} />
                <Tooltip
                  cursor={{ stroke: 'var(--axis)' }}
                  content={
                    <ChartTooltip
                      title={(dd) => `Day ${dd} · ${fmtDate(addDays(r.inputs.start_date, Number(dd)))}`}
                      rows={[
                        { key: 'rate_p95', label: '95th pct', format: (v) => fmtRate(v) },
                        { key: 'rate_p75', label: '75th pct', format: (v) => fmtRate(v) },
                        { key: 'rate_p50', label: 'Median', color: 'var(--s1)', format: (v) => fmtRate(v) },
                        { key: 'rate_p25', label: '25th pct', format: (v) => fmtRate(v) },
                        { key: 'rate_p05', label: '5th pct', format: (v) => fmtRate(v) },
                        { key: 'breakeven_rate', label: 'Break-even', color: 'var(--s2)', format: (v) => fmtRate(v) },
                      ]}
                    />
                  }
                />
                <Area dataKey={(c) => [c.rate_p05, c.rate_p95]} stroke="none" fill="var(--band-outer)" isAnimationActive={false} />
                <Area dataKey={(c) => [c.rate_p25, c.rate_p75]} stroke="none" fill="var(--band-inner)" isAnimationActive={false} />
                <Line dataKey="rate_p50" stroke="var(--s1)" strokeDasharray="4 4" strokeWidth={1.5} dot={false} isAnimationActive={false} />
                <Line dataKey="breakeven_rate" stroke="var(--s2)" strokeWidth={2} dot={false} isAnimationActive={false} />
                <ReferenceLine y={target} stroke="var(--s3)" strokeWidth={2} label={{ value: `target ${target}`, fill: 'var(--text-2)', fontSize: 11, position: 'insideTopLeft' }} />
              </ComposedChart>
            </ResponsiveContainer>
          </div>
        </Card>

        <Card title="Value of each path (PLN)" sub="Converted today vs holding USD and converting on day t. Interest is net of tax.">
          <Legend
            items={[
              { label: 'Convert now → PLN savings', color: 'var(--s1)' },
              { label: 'Hold USD, rate unchanged', color: 'var(--s2)' },
              { label: 'Hold USD, convert at target on day t', color: 'var(--s3)' },
              { label: 'Via EUR savings, rates unchanged', color: 'var(--s4)' },
            ]}
          />
          <div style={{ height: 280 }}>
            <ResponsiveContainer>
              <LineChart data={curves} margin={{ top: 6, right: 12, bottom: 0, left: 8 }}>
                <CartesianGrid {...grid} />
                <XAxis dataKey="day" {...axis} tickFormatter={(v: number) => `${v}d`} minTickGap={30} />
                <YAxis {...axis} domain={['auto', 'auto']} tickFormatter={(v: number) => `${(v / 1000).toFixed(1)}k`} width={48} />
                <Tooltip
                  cursor={{ stroke: 'var(--axis)' }}
                  content={
                    <ChartTooltip
                      title={(dd) => `Day ${dd}`}
                      rows={[
                        { key: 'convert_now', label: 'Convert now', color: 'var(--s1)', format: (v) => fmtPln(v) },
                        { key: 'hold_usd_unchanged', label: 'Hold USD', color: 'var(--s2)', format: (v) => fmtPln(v) },
                        { key: 'hold_usd_at_target', label: 'Hit target', color: 'var(--s3)', format: (v) => fmtPln(v) },
                        { key: 'eur_route_unchanged', label: 'Via EUR', color: 'var(--s4)', format: (v) => fmtPln(v) },
                      ]}
                    />
                  }
                />
                {h.breakeven_wait_days != null && h.breakeven_wait_days <= horizon && (
                  <ReferenceLine
                    x={Math.round(h.breakeven_wait_days)}
                    stroke="var(--muted)"
                    strokeDasharray="4 4"
                    label={{ value: 'break-even', fill: 'var(--text-2)', fontSize: 11, position: 'insideTopRight' }}
                  />
                )}
                <Line dataKey="convert_now" stroke="var(--s1)" strokeWidth={2} dot={false} isAnimationActive={false} />
                <Line dataKey="hold_usd_unchanged" stroke="var(--s2)" strokeWidth={2} dot={false} isAnimationActive={false} />
                <Line dataKey="hold_usd_at_target" stroke="var(--s3)" strokeWidth={2} dot={false} isAnimationActive={false} />
                <Line dataKey="eur_route_unchanged" stroke="var(--s4)" strokeWidth={2} dot={false} isAnimationActive={false} />
              </LineChart>
            </ResponsiveContainer>
          </div>
        </Card>

        <Card
          title="Chance the target is touched by day t"
          sub="First-passage probability of a GBM (closed form). Compare with your own estimate and the break-even confidence."
        >
          <Legend
            items={[
              { label: 'Model', color: 'var(--s1)' },
              { label: 'You', color: 'var(--s5)', kind: 'dash' },
              { label: 'Break-even confidence', color: 'var(--s2)', kind: 'dash' },
            ]}
          />
          <div style={{ height: 240 }}>
            <ResponsiveContainer>
              <ComposedChart data={curves} margin={{ top: 6, right: 12, bottom: 0, left: 0 }}>
                <CartesianGrid {...grid} />
                <XAxis dataKey="day" {...axis} tickFormatter={(v: number) => `${v}d`} minTickGap={30} />
                <YAxis {...axis} domain={[0, 1]} tickFormatter={(v: number) => `${Math.round(v * 100)}%`} width={40} />
                <Tooltip
                  cursor={{ stroke: 'var(--axis)' }}
                  content={
                    <ChartTooltip
                      title={(dd) => `By day ${dd}`}
                      rows={[{ key: 'hit_prob_by_day', label: 'Model P(hit)', color: 'var(--s1)', format: (v) => fmtPct(v, 1) }]}
                    />
                  }
                />
                <Area dataKey="hit_prob_by_day" stroke="var(--s1)" strokeWidth={2} fill="var(--band-outer)" isAnimationActive={false} />
                {p.subjective_prob != null && <ReferenceLine y={p.subjective_prob} stroke="var(--s5)" strokeDasharray="5 4" strokeWidth={2} />}
                {d.required_prob != null && <ReferenceLine y={d.required_prob} stroke="var(--s2)" strokeDasharray="5 4" strokeWidth={2} />}
              </ComposedChart>
            </ResponsiveContainer>
          </div>
        </Card>

        <Card
          title="Outcome distribution of waiting (PLN, by percentile)"
          sub={`Monte Carlo of “hold USD, convert at ${fmtRate(target)} if touched, else at the deadline” minus converting now.`}
        >
          <Legend
            items={[
              { label: 'Waiting loses', color: 'var(--div-neg)', kind: 'area' },
              { label: 'Waiting wins', color: 'var(--div-pos)', kind: 'area' },
            ]}
          />
          <div style={{ height: 240 }}>
            <ResponsiveContainer>
              <ComposedChart
                data={mc.edge_quantiles.map((x) => ({ ...x, neg: Math.min(0, x.edge_pln), pos: Math.max(0, x.edge_pln) }))}
                margin={{ top: 6, right: 12, bottom: 0, left: 8 }}
              >
                <CartesianGrid {...grid} />
                <XAxis dataKey="q" type="number" domain={[0, 1]} {...axis} tickFormatter={(v: number) => `${Math.round(v * 100)}%`} ticks={[0, 0.25, 0.5, 0.75, 1]} />
                <YAxis {...axis} tickFormatter={(v: number) => `${Math.round(v)}`} width={48} />
                <Tooltip
                  cursor={{ stroke: 'var(--axis)' }}
                  content={
                    <ChartTooltip
                      title={(q) => `${Math.round(Number(q) * 100)}% of paths end below`}
                      rows={[{ key: 'edge_pln', label: 'Edge vs converting now', format: (v) => fmtSignedPln(v) }]}
                    />
                  }
                />
                <ReferenceLine y={0} stroke="var(--axis)" />
                <Area dataKey="neg" stroke="none" fill="var(--div-neg)" fillOpacity={0.55} isAnimationActive={false} />
                <Area dataKey="pos" stroke="none" fill="var(--div-pos)" fillOpacity={0.55} isAnimationActive={false} />
                <Line dataKey="edge_pln" stroke="var(--text-2)" strokeWidth={1.5} dot={false} isAnimationActive={false} />
              </ComposedChart>
            </ResponsiveContainer>
          </div>
          <dl className="facts" style={{ marginTop: 8 }}>
            <dt>If hit (avg)</dt>
            <dd>{fmtSignedPln(mc.mean_value_if_hit != null ? mc.mean_value_if_hit - h.convert_now_at_horizon : null)}</dd>
            <dt>If missed (avg)</dt>
            <dd>{fmtSignedPln(mc.mean_value_if_miss != null ? mc.mean_value_if_miss - h.convert_now_at_horizon : null)}</dd>
            <dt>5% / 50% / 95%</dt>
            <dd>
              {fmtSignedPln(mc.p05 - h.convert_now_at_horizon)} / {fmtSignedPln(mc.p50 - h.convert_now_at_horizon)} /{' '}
              {fmtSignedPln(mc.p95 - h.convert_now_at_horizon)}
            </dd>
          </dl>
        </Card>
      </div>

      <div className="grid" style={{ marginTop: 14 }}>
        <Card
          title="Sensitivity: gain vs converting now, by target and day it is hit"
          sub={`PLN at ${fmtDate(r.inputs.deadline)}. Red cells: even hitting that rate on that day loses to converting today, because of the interest you gave up.`}
        >
          <Heatmap sensitivity={sensitivity} selected={target} />
        </Card>
        <Card title="How the numbers are computed">
          <div className="explain">
            <p>
              <b>Days of interest.</b> The PLN account grows by <code>(1 + r·(1−tax)/365)</code> per day. The better rate is worth{' '}
              <code>ln(target/spot) / ln(1 + daily)</code> days of that growth — {fmtDays(h.fx_gain_in_pln_interest_days)} here.
            </p>
            <p>
              <b>Break-even wait.</b> While waiting, USD still earns interest, so you only lose the difference:{' '}
              <code>ln(target/spot) / ln((1+d_pln)/(1+d_usd))</code> — if the target arrives later than {fmtDays(h.breakeven_wait_days)},
              converting today would have been better even though you got your rate.
            </p>
            <p>
              <b>Probability.</b> USD/PLN is modelled as geometric Brownian motion with {(p.vol_annual * 100).toFixed(2)}% annual volatility
              and {(p.drift_annual * 100).toFixed(1)}% drift. Hit probability uses the reflection principle; the distribution comes from{' '}
              {mc.paths.toLocaleString()} daily-step simulations. Your probability replaces the model’s weight between the simulated “hit” and
              “miss” outcomes, so the downside of a miss is still priced in.
            </p>
            <p>
              <b>Required confidence</b> = (now − miss) / (hit − miss), using average simulated values at the deadline.
            </p>
          </div>
        </Card>
      </div>
    </>
  )
}

function Heatmap({ sensitivity, selected }: { sensitivity: AnalyzeResponse['analysis']['sensitivity']; selected: number }) {
  const max = Math.max(1, ...sensitivity.edge_pln.flat().map(Math.abs))
  const color = (v: number) => {
    const t = Math.min(1, Math.abs(v) / max)
    const pole = v >= 0 ? 'var(--div-pos)' : 'var(--div-neg)'
    return `color-mix(in srgb, ${pole} ${Math.round(8 + t * 72)}%, var(--div-mid))`
  }
  return (
    <div className="table-wrap">
      <table className="heatmap">
        <thead>
          <tr>
            <th>Target ↓ / hit on day →</th>
            {sensitivity.hit_days.map((d) => (
              <th key={d}>{d}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {sensitivity.targets.map((t, i) => (
            <tr key={t} className={Math.abs(t - selected) < 1e-9 ? 'sel' : ''}>
              <th>{t.toFixed(4)}</th>
              {sensitivity.edge_pln[i].map((v, j) => (
                <td key={j} style={{ background: color(v) }} title={`${fmtSignedPln(v)} if ${t} is hit on day ${sensitivity.hit_days[j]}`}>
                  {Math.round(v)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

function PlaceBet(props: { req: AnalyzeRequest; r: AnalyzeResponse; onClose: () => void; onPlaced: (id: number) => void }) {
  const { r } = props
  const [title, setTitle] = useState(`Wait for ${props.req.target} by ${fmtDate(r.inputs.deadline)}`)
  const [notes, setNotes] = useState('')
  const create = useMutation({
    mutationFn: () => api.createBet(title, notes, { ...props.req, pln: undefined, usd: undefined, eur: undefined }),
    onSuccess: (d) => props.onPlaced(d.bet.id),
  })
  return (
    <Modal title="Place bet" onClose={props.onClose}>
      <p className="muted" style={{ margin: 0 }}>
        Records this analysis ({fmtPln(r.analysis.headline.pln_now)} at {fmtRate(r.inputs.spot)}, target {props.req.target}, your p{' '}
        {fmtPct(props.req.subjective_prob, 0)}, model p {fmtPct(r.analysis.probability.model_hit_prob, 0)}). Afterwards, record actual
        conversions on the bet page; it is replayed daily against real fixings and your accounts’ rate history.
        {props.req.pln || props.req.usd || props.req.eur ? ' What-if rate overrides are dropped: the bet uses the accounts’ rates.' : ''}
      </p>
      <ErrorBox error={create.error} />
      <label className="field">
        Title
        <input value={title} onChange={(e) => setTitle(e.target.value)} />
      </label>
      <label className="field">
        Why do you think so? (your reasoning, for later review)
        <textarea rows={4} value={notes} onChange={(e) => setNotes(e.target.value)} />
      </label>
      <div className="actions">
        <button onClick={props.onClose}>Cancel</button>
        <button className="primary" disabled={create.isPending || !title.trim()} onClick={() => create.mutate()}>
          {create.isPending ? 'Placing…' : 'Place bet'}
        </button>
      </div>
    </Modal>
  )
}
