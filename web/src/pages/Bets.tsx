import { useQuery } from '@tanstack/react-query'
import { Link, useNavigate } from 'react-router-dom'
import { api } from '../api'
import { Card, ErrorBox, Kpi } from '../components/ui'
import { fmtDate, fmtPct, fmtPln, fmtRate, fmtSignedPln, fmtUsd, signClass } from '../format'

export default function Bets() {
  const navigate = useNavigate()
  const bets = useQuery({ queryKey: ['bets'], queryFn: api.bets })
  const list = bets.data ?? []
  const total = list.reduce((a, b) => a + b.summary.edge_vs_day0_pln, 0)
  const regret = list.reduce((a, b) => a + (b.summary.regret_vs_best_pln ?? 0), 0)
  const resolved = list.filter((b) => b.status !== 'open')
  const hits = resolved.filter((b) => b.status === 'hit').length

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Bets</h1>
          <p>Every decision you recorded, valued today against what you could have done instead.</p>
        </div>
        <Link to="/analyze">
          <button className="primary">New analysis</button>
        </Link>
      </div>
      <ErrorBox error={bets.error} />
      <div className="kpis">
        <Kpi label="Bets" value={list.length} hint={`${list.length - resolved.length} open · ${resolved.length} resolved`} />
        <Kpi label="Target hit rate" value={resolved.length ? fmtPct(hits / resolved.length, 0) : '—'} hint={`${hits} of ${resolved.length} resolved`} />
        <Kpi label="Total edge vs day-0 conversion" value={fmtSignedPln(total)} className={signClass(total)} emph />
        <Kpi label="Left on the table vs best day" value={fmtPln(regret)} hint="perfect-hindsight conversion day" />
      </div>
      <Card title="All bets">
        {list.length === 0 ? (
          <div className="empty">
            No bets yet. Run the <Link to="/analyze">Analyzer</Link> and press “Place this bet”.
          </div>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Status</th>
                  <th>Bet</th>
                  <th>Placed</th>
                  <th>Deadline</th>
                  <th className="num">Amount</th>
                  <th className="num">Start rate</th>
                  <th className="num">Target</th>
                  <th className="num">Your p</th>
                  <th className="num">Model p</th>
                  <th className="num">Converted</th>
                  <th className="num">Value now</th>
                  <th className="num">Edge vs day 0</th>
                  <th className="num">Beats % of days</th>
                </tr>
              </thead>
              <tbody>
                {list.map(({ bet, status, summary }) => (
                  <tr key={bet.id} className="link" onClick={() => navigate(`/bets/${bet.id}`)}>
                    <td>
                      <span className={`badge ${status}`}>{status}</span>
                    </td>
                    <td>{bet.title}</td>
                    <td>{fmtDate(bet.start_date)}</td>
                    <td>{fmtDate(bet.deadline)}</td>
                    <td className="num">{fmtUsd(bet.amount_usd)}</td>
                    <td className="num">{fmtRate(bet.spot_usd_pln)}</td>
                    <td className="num">{fmtRate(bet.target_rate)}</td>
                    <td className="num">{fmtPct(bet.subjective_prob, 0)}</td>
                    <td className="num">{fmtPct(bet.model_prob, 0)}</td>
                    <td className="num">{fmtPct(summary.converted_usd / bet.amount_usd, 0)}</td>
                    <td className="num">{fmtPln(summary.actual)}</td>
                    <td className={`num ${signClass(summary.edge_vs_day0_pln)}`}>{fmtSignedPln(summary.edge_vs_day0_pln)}</td>
                    <td className="num">{fmtPct(summary.actual_beats_share, 0)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>
    </>
  )
}
