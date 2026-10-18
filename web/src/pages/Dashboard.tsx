import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useMemo, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { CartesianGrid, Line, LineChart, ReferenceLine, ResponsiveContainer, Tooltip, XAxis, YAxis } from 'recharts'
import { api } from '../api'
import { Card, ChartTooltip, ErrorBox, Kpi, Legend, Segmented, axis, grid } from '../components/ui'
import { fmtDate, fmtPct, fmtRate, fmtShortDate, fmtSignedPln, isoDaysAgo, signClass } from '../format'

const RANGES = [
  { value: 30, label: '1M' },
  { value: 91, label: '3M' },
  { value: 182, label: '6M' },
  { value: 365, label: '1Y' },
  { value: 1826, label: '5Y' },
  { value: 3652, label: '10Y' },
]

export default function Dashboard() {
  const qc = useQueryClient()
  const navigate = useNavigate()
  const [range, setRange] = useState(365)
  const [pair, setPair] = useState<'usd_pln' | 'eur_pln' | 'usd_eur'>('usd_pln')
  const latest = useQuery({ queryKey: ['latest'], queryFn: api.latest })
  const history = useQuery({ queryKey: ['history', range], queryFn: () => api.history(isoDaysAgo(range)) })
  const bets = useQuery({ queryKey: ['bets'], queryFn: api.bets })
  const sync = useMutation({
    mutationFn: api.sync,
    onSuccess: () => qc.invalidateQueries(),
  })

  const l = latest.data
  const s = l?.stats_365d
  const open = bets.data?.filter((b) => b.status === 'open') ?? []
  const totalEdge = bets.data?.reduce((acc, b) => acc + b.summary.edge_vs_day0_pln, 0) ?? 0

  const series = useMemo(() => {
    const pts = history.data ?? []
    const vals = pts.map((p) => p[pair])
    const min = Math.min(...vals)
    const max = Math.max(...vals)
    const pad = (max - min) * 0.08 || 0.01
    return { pts, domain: [min - pad, max + pad] as [number, number] }
  }, [history.data, pair])

  const pairLabel = { usd_pln: 'USD/PLN', eur_pln: 'EUR/PLN', usd_eur: 'USD/EUR' }[pair]

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Dashboard</h1>
          <p>ECB reference rates{l ? `, last fixing ${fmtDate(l.date)}` : ''}. Synced automatically every few hours.</p>
        </div>
        <div className="row" style={{ flex: 'none' }}>
          <button onClick={() => sync.mutate()} disabled={sync.isPending}>
            {sync.isPending ? 'Syncing…' : 'Sync rates now'}
          </button>
          <button className="primary" onClick={() => navigate('/analyze')}>
            New analysis
          </button>
        </div>
      </div>
      <ErrorBox error={latest.error ?? sync.error} />

      <div className="kpis">
        <Kpi
          label="USD/PLN"
          value={fmtRate(l?.usd_pln)}
          hint={
            <>
              <span className={signClass(l?.usd_pln_change_1d)}>{fmtSigned(l?.usd_pln_change_1d)}</span> 1d ·{' '}
              <span className={signClass(l?.usd_pln_change_30d)}>{fmtSigned(l?.usd_pln_change_30d)}</span> 30d
            </>
          }
          emph
        />
        <Kpi label="EUR/PLN" value={fmtRate(l?.eur_pln)} hint="cross via USD" />
        <Kpi label="EUR/USD" value={fmtRate(l ? 1 / l.usd_eur : undefined)} />
        <Kpi
          label="1Y realised volatility"
          value={fmtPct(s?.vol_annual)}
          hint={s ? `≈ ±${(s.last * s.vol_annual * Math.sqrt(30 / 365)).toFixed(3)} PLN 1σ in 30 days` : undefined}
        />
        <Kpi
          label="Where is today in the 1Y range"
          value={fmtPct(s?.percentile_of_last, 0)}
          hint={s ? `range ${fmtRate(s.min)} – ${fmtRate(s.max)}` : undefined}
        />
        <Kpi
          label="All bets vs converting on day 0"
          value={fmtSignedPln(totalEdge)}
          className={signClass(totalEdge)}
          hint={`${bets.data?.length ?? 0} bets, ${open.length} open`}
        />
      </div>

      <div className="grid">
        <Card
          title={`${pairLabel} history`}
          sub="Daily ECB fixing. Dashed line: today's rate."
          right={
            <div className="row" style={{ flex: 'none' }}>
              <Segmented
                options={[
                  { value: 'usd_pln', label: 'USD/PLN' },
                  { value: 'eur_pln', label: 'EUR/PLN' },
                  { value: 'usd_eur', label: 'USD/EUR' },
                ]}
                value={pair}
                onChange={setPair}
              />
              <Segmented options={RANGES} value={range} onChange={setRange} />
            </div>
          }
        >
          <div style={{ height: 340 }}>
            <ResponsiveContainer>
              <LineChart data={series.pts} margin={{ top: 8, right: 12, bottom: 0, left: 0 }}>
                <CartesianGrid {...grid} />
                <XAxis dataKey="date" {...axis} tickFormatter={fmtShortDate} minTickGap={50} />
                <YAxis {...axis} domain={series.domain} tickFormatter={(v: number) => v.toFixed(2)} width={48} />
                <Tooltip
                  content={
                    <ChartTooltip
                      title={(d) => fmtDate(String(d))}
                      rows={[{ key: pair, label: pairLabel, color: 'var(--s1)', format: (v) => fmtRate(v) }]}
                    />
                  }
                  cursor={{ stroke: 'var(--axis)' }}
                />
                {l && <ReferenceLine y={l[pair]} stroke="var(--muted)" strokeDasharray="4 4" />}
                <Line dataKey={pair} stroke="var(--s1)" strokeWidth={2} dot={false} isAnimationActive={false} />
              </LineChart>
            </ResponsiveContainer>
          </div>
        </Card>

        <Card
          title="Open bets"
          sub="Live value of what you did vs converting everything on the day you placed the bet."
          right={<Link to="/bets">All bets →</Link>}
        >
          {open.length === 0 ? (
            <div className="empty">
              No open bets. <Link to="/analyze">Analyse a scenario</Link> and place one.
            </div>
          ) : (
            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>Bet</th>
                    <th>Deadline</th>
                    <th className="num">Target</th>
                    <th className="num">Now</th>
                    <th className="num">Distance</th>
                    <th className="num">Your p</th>
                    <th className="num">Model p</th>
                    <th className="num">Edge vs day 0</th>
                  </tr>
                </thead>
                <tbody>
                  {open.map(({ bet, summary }) => (
                    <tr key={bet.id} className="link" onClick={() => navigate(`/bets/${bet.id}`)}>
                      <td>{bet.title}</td>
                      <td>{fmtDate(bet.deadline)}</td>
                      <td className="num">{fmtRate(bet.target_rate)}</td>
                      <td className="num">{fmtRate(l?.usd_pln)}</td>
                      <td className="num">{l ? `${((bet.target_rate / l.usd_pln - 1) * 100).toFixed(2)}%` : '—'}</td>
                      <td className="num">{fmtPct(bet.subjective_prob, 0)}</td>
                      <td className="num">{fmtPct(bet.model_prob, 0)}</td>
                      <td className={`num ${signClass(summary.edge_vs_day0_pln)}`}>{fmtSignedPln(summary.edge_vs_day0_pln)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </Card>
        {history.data && history.data.length === 0 && (
          <Legend items={[{ label: 'No FX history yet — press “Sync rates now”.', color: 'var(--warn)' }]} />
        )}
      </div>
    </>
  )
}

function fmtSigned(v: number | null | undefined) {
  if (v == null) return '—'
  return `${v >= 0 ? '+' : '−'}${Math.abs(v).toFixed(4)}`
}
