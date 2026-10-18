import { useQuery } from '@tanstack/react-query'
import { Link, useNavigate } from 'react-router-dom'
import {
  CartesianGrid,
  ComposedChart,
  Line,
  ResponsiveContainer,
  Scatter,
  Tooltip,
  XAxis,
  YAxis,
  ZAxis,
} from 'recharts'
import { api } from '../api'
import { Card, ChartTooltip, ErrorBox, Kpi, Legend, axis, grid } from '../components/ui'
import { fmtDate, fmtPct, fmtSignedPln, signClass } from '../format'

export default function Calibration() {
  const navigate = useNavigate()
  const q = useQuery({ queryKey: ['calibration'], queryFn: api.calibration })
  const c = q.data?.calibration
  const points = q.data?.points ?? []
  const toPts = (b: NonNullable<typeof c>['buckets_model']) =>
    b.map((x) => ({ predicted: x.mean_predicted, observed: x.hit_rate, count: x.count }))
  const youBetter =
    c?.brier_subjective != null && c.brier_model_on_subjective_subset != null
      ? c.brier_subjective < c.brier_model_on_subjective_subset
      : null

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Calibration</h1>
          <p>
            How right were you? Resolved bets only: target touched before the deadline, or deadline passed. Probabilities on retrospective
            bets are hindsight, so they are not scored.
          </p>
        </div>
      </div>
      <ErrorBox error={q.error} />
      <div className="kpis">
        <Kpi label="Resolved bets" value={c?.resolved ?? '—'} hint={c ? `${c.hits} hit the target` : undefined} />
        <Kpi
          label="Your Brier score"
          value={c?.brier_subjective != null ? c.brier_subjective.toFixed(3) : '—'}
          hint="0 = perfect, 0.25 = coin flip"
          emph
        />
        <Kpi
          label="Model Brier (same bets)"
          value={c?.brier_model_on_subjective_subset != null ? c.brier_model_on_subjective_subset.toFixed(3) : '—'}
          hint={youBetter == null ? 'need resolved bets with your probability' : youBetter ? 'your experience beats the model' : 'the model beats your gut'}
        />
        <Kpi
          label="Total edge vs day-0 conversion"
          value={fmtSignedPln(c?.total_edge_vs_day0_pln)}
          className={signClass(c?.total_edge_vs_day0_pln)}
          hint="money made (or lost) by waiting, resolved bets"
        />
      </div>

      {!c || c.resolved === 0 ? (
        <Card title="Reliability diagram">
          <div className="empty">
            Nothing resolved yet. Place bets from the <Link to="/analyze">Analyzer</Link>; they resolve when the target is touched or the
            deadline passes. Tip: backdate an analysis to test your judgement on past periods.
          </div>
        </Card>
      ) : (
        <div className="grid cols-2">
          <Card
            title="Reliability diagram"
            sub="When you say 70%, does it happen 70% of the time? Points on the diagonal are perfectly calibrated. Buckets of 20 percentage points."
          >
            <Legend
              items={[
                { label: 'You', color: 'var(--s1)' },
                { label: 'Model', color: 'var(--s2)' },
                { label: 'Perfect calibration', color: 'var(--muted)', kind: 'dash' },
              ]}
            />
            <div style={{ height: 320 }}>
              <ResponsiveContainer>
                <ComposedChart margin={{ top: 14, right: 16, bottom: 0, left: 0 }}>
                  <ZAxis range={[110, 110]} />
                  <CartesianGrid {...grid} vertical />
                  <XAxis type="number" dataKey="predicted" domain={[0, 1]} {...axis} tickFormatter={(v: number) => `${v * 100}%`} name="Predicted" />
                  <YAxis type="number" dataKey="observed" domain={[0, 1]} {...axis} tickFormatter={(v: number) => `${v * 100}%`} width={40} />
                  <Tooltip
                    content={
                      <ChartTooltip
                        title={() => 'Bucket'}
                        rows={[
                          { key: 'predicted', label: 'Predicted', format: (v) => fmtPct(v, 0) },
                          { key: 'observed', label: 'Happened', format: (v) => fmtPct(v, 0) },
                          { key: 'count', label: 'Bets', format: (v) => String(v) },
                        ]}
                      />
                    }
                  />
                  <Line
                    data={[{ predicted: 0, observed: 0 }, { predicted: 1, observed: 1 }]}
                    dataKey="observed"
                    stroke="var(--muted)"
                    strokeDasharray="5 4"
                    dot={false}
                    isAnimationActive={false}
                    activeDot={false}
                  />
                  <Scatter data={toPts(c.buckets_subjective)} fill="var(--s1)" stroke="var(--surface)" strokeWidth={2} isAnimationActive={false} shape="circle" />
                  <Scatter data={toPts(c.buckets_model)} fill="var(--s2)" stroke="var(--surface)" strokeWidth={2} isAnimationActive={false} shape="diamond" />
                </ComposedChart>
              </ResponsiveContainer>
            </div>
          </Card>
          <Card title="Resolved bets">
            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>Bet</th>
                    <th>Placed</th>
                    <th className="num">You</th>
                    <th className="num">Model</th>
                    <th>Outcome</th>
                    <th className="num">Edge</th>
                  </tr>
                </thead>
                <tbody>
                  {points.map((p) => (
                    <tr key={p.bet_id} className="link" onClick={() => navigate(`/bets/${p.bet_id}`)}>
                      <td>
                        {p.title}
                        {p.retrospective && <span className="muted"> · retro</span>}
                      </td>
                      <td>{fmtDate(p.start_date)}</td>
                      <td className="num" title={p.retrospective ? 'hindsight — not scored' : undefined}>
                        {p.retrospective ? <s className="muted">{fmtPct(p.subjective_prob, 0)}</s> : fmtPct(p.subjective_prob, 0)}
                      </td>
                      <td className="num">{fmtPct(p.model_prob, 0)}</td>
                      <td>
                        <span className={`badge ${p.target_hit ? 'hit' : 'missed'}`}>{p.target_hit ? 'hit' : 'missed'}</span>
                      </td>
                      <td className={`num ${signClass(p.edge_vs_day0_pln)}`}>{fmtSignedPln(p.edge_vs_day0_pln)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </Card>
        </div>
      )}
    </>
  )
}
