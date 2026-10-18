import { useMutation, useQuery } from '@tanstack/react-query'
import { useEffect, useMemo, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { api, type Currency, type HistoryPoint } from '../api'
import { Card, ErrorBox } from '../components/ui'
import { addDays, fmtDate, fmtPct, fmtPln, fmtRate, fmtUsd, isoToday } from '../format'

interface Row {
  key: number
  date: string
  usd_amount: string
  rate: string
}

const daysBetween = (a: string, b: string) =>
  Math.round((Date.parse(`${b}T00:00:00Z`) - Date.parse(`${a}T00:00:00Z`)) / 86_400_000)

/** Last fixing on or before `date` (ECB doesn't publish on weekends/holidays). */
function fixingOn(history: HistoryPoint[] | undefined, date: string): HistoryPoint | undefined {
  if (!history || !date) return undefined
  let found: HistoryPoint | undefined
  for (const p of history) {
    if (p.date > date) break
    found = p
  }
  return found
}

export default function PastBet() {
  const navigate = useNavigate()
  const today = isoToday()
  const accounts = useQuery({ queryKey: ['accounts'], queryFn: api.accounts })

  const [title, setTitle] = useState('')
  const [start, setStart] = useState(addDays(today, -90))
  const [amount, setAmount] = useState('10000')
  const [spot, setSpot] = useState('')
  const [target, setTarget] = useState('')
  const [deadline, setDeadline] = useState('')
  const [spread, setSpread] = useState('0')
  const [hasProb, setHasProb] = useState(false)
  const [prob, setProb] = useState(0.5)
  const [notes, setNotes] = useState('')
  const [rows, setRows] = useState<Row[]>([{ key: 1, date: '', usd_amount: '', rate: '' }])
  const [acc, setAcc] = useState<Record<Currency, number | null>>({ PLN: null, USD: null, EUR: null })

  useEffect(() => {
    if (!accounts.data) return
    const first = (c: Currency) => accounts.data.find((a) => a.currency === c)?.id ?? null
    setAcc((a) => ({ PLN: a.PLN ?? first('PLN'), USD: a.USD ?? first('USD'), EUR: a.EUR ?? first('EUR') }))
  }, [accounts.data])

  // One history fetch covering the start date (with a margin for weekends) through today.
  const history = useQuery({
    queryKey: ['history', 'from', start],
    queryFn: () => api.history(addDays(start, -10)),
    enabled: !!start,
  })
  const startFixing = fixingOn(history.data, start)
  const spotValue = spot ? Number(spot) : startFixing?.usd_pln

  const filledRows = rows.filter((r) => r.date)
  const lastConversion = filledRows.map((r) => r.date).sort().at(-1)
  const effectiveDeadline = deadline || lastConversion || addDays(start, 90)
  const horizon = daysBetween(start, effectiveDeadline)
  const effectiveTarget = target ? Number(target) : undefined

  const conversions = filledRows.map((r) => ({
    date: r.date,
    usd_amount: Number(r.usd_amount || 0),
    rate: r.rate ? Number(r.rate) : (fixingOn(history.data, r.date)?.usd_pln ?? 0),
  }))
  const convertedUsd = conversions.reduce((s, c) => s + c.usd_amount, 0)
  const receivedPln = conversions.reduce((s, c) => s + c.usd_amount * c.rate, 0)

  const windowStats = useMemo(() => {
    const pts = (history.data ?? []).filter((p) => p.date >= start && p.date <= effectiveDeadline)
    if (!pts.length) return null
    return { min: Math.min(...pts.map((p) => p.usd_pln)), max: Math.max(...pts.map((p) => p.usd_pln)) }
  }, [history.data, start, effectiveDeadline])

  const problems: string[] = []
  if (!title.trim()) problems.push('Give it a title.')
  if (!(Number(amount) > 0)) problems.push('Amount must be positive.')
  if (!effectiveTarget) problems.push('Enter the rate you were waiting for (or hoping for).')
  if (start > today) problems.push('Start date cannot be in the future.')
  if (horizon < 1 || horizon > 1095) problems.push('Deadline must be 1 day to 3 years after the start.')
  if (filledRows.some((r) => r.date < start || r.date > today)) problems.push('Conversions must be between the start date and today.')
  if (conversions.some((c) => !(c.usd_amount > 0) || !(c.rate > 0))) problems.push('Each conversion needs a USD amount and a rate.')
  if (convertedUsd > Number(amount) * 1.05) problems.push('Conversions exceed the amount (plus interest).')

  const create = useMutation({
    mutationFn: () =>
      api.createBet(
        title,
        notes,
        {
          amount_usd: Number(amount),
          target: effectiveTarget!,
          horizon_days: horizon,
          start_date: start,
          spot: spot ? Number(spot) : undefined,
          spread_pct: Number(spread) || 0,
          pln_account_id: acc.PLN ?? undefined,
          usd_account_id: acc.USD ?? undefined,
          eur_account_id: acc.EUR ?? undefined,
          subjective_prob: hasProb ? prob : undefined,
          paths: 2000,
        },
        { retrospective: true, conversions },
      ),
    onSuccess: (d) => navigate(`/bets/${d.bet.id}`),
  })

  const setRow = (key: number, patch: Partial<Row>) => setRows((rs) => rs.map((r) => (r.key === key ? { ...r, ...patch } : r)))

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Record a past decision</h1>
          <p>
            A decision you already made before using this tool. It is replayed against the real fixings so you can see how it worked out and
            what every other path would have given you.
          </p>
        </div>
      </div>
      <ErrorBox error={create.error ?? accounts.error ?? history.error} />

      <div className="grid cols-2">
        <Card title="1 · The situation" sub="When you had the USD, and what you were waiting for.">
          <div className="form">
            <label className="field">
              Title
              <input placeholder="e.g. Q2 salary — waited for 4.00" value={title} onChange={(e) => setTitle(e.target.value)} />
            </label>
            <div className="row">
              <label className="field">
                Date you had the USD
                <input type="date" max={today} value={start} onChange={(e) => setStart(e.target.value)} />
              </label>
              <label className="field">
                Amount (USD)
                <input type="number" min="1" value={amount} onChange={(e) => setAmount(e.target.value)} />
              </label>
            </div>
            <div className="row">
              <label className="field">
                USD/PLN that day
                <input
                  type="number"
                  step="0.0001"
                  placeholder={startFixing ? fmtRate(startFixing.usd_pln) : ''}
                  value={spot}
                  onChange={(e) => setSpot(e.target.value)}
                />
              </label>
              <label className="field">
                Rate you were waiting for
                <input type="number" step="0.0001" placeholder="e.g. 4.0000" value={target} onChange={(e) => setTarget(e.target.value)} />
              </label>
            </div>
            <div className="row">
              <label className="field">
                Your deadline (blank = last conversion)
                <input type="date" min={addDays(start, 1)} value={deadline} onChange={(e) => setDeadline(e.target.value)} />
              </label>
              <label className="field">
                Bank spread %
                <input type="number" step="0.05" min="0" value={spread} onChange={(e) => setSpread(e.target.value)} />
              </label>
            </div>
            <p className="muted" style={{ margin: 0, fontSize: 12 }}>
              Blank USD/PLN uses the ECB fixing on that date{startFixing && startFixing.date !== start ? ` (${fmtDate(startFixing.date)})` : ''}.
              {windowStats &&
                ` Between ${fmtDate(start)} and ${fmtDate(effectiveDeadline)} USD/PLN ranged ${fmtRate(windowStats.min)} – ${fmtRate(windowStats.max)}.`}
            </p>
          </div>
        </Card>

        <Card title="2 · What you actually exchanged" sub="Each conversion: date, USD sold, and the PLN rate you really got. Blank rate = ECB fixing.">
          <div className="form">
            <table>
              <thead>
                <tr>
                  <th>Date</th>
                  <th>USD sold</th>
                  <th>PLN per USD</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {rows.map((r) => {
                  const fx = fixingOn(history.data, r.date)
                  return (
                    <tr key={r.key}>
                      <td>
                        <input type="date" min={start} max={today} value={r.date} onChange={(e) => setRow(r.key, { date: e.target.value })} />
                      </td>
                      <td>
                        <input type="number" value={r.usd_amount} onChange={(e) => setRow(r.key, { usd_amount: e.target.value })} />
                      </td>
                      <td>
                        <input
                          type="number"
                          step="0.0001"
                          placeholder={fx ? fmtRate(fx.usd_pln) : ''}
                          value={r.rate}
                          onChange={(e) => setRow(r.key, { rate: e.target.value })}
                        />
                      </td>
                      <td className="num">
                        <button className="danger" aria-label="Remove row" onClick={() => setRows((rs) => rs.filter((x) => x.key !== r.key))}>
                          ×
                        </button>
                      </td>
                    </tr>
                  )
                })}
              </tbody>
            </table>
            <div className="row" style={{ justifyContent: 'space-between' }}>
              <button
                style={{ flex: 'none' }}
                onClick={() => setRows((rs) => [...rs, { key: Math.max(0, ...rs.map((x) => x.key)) + 1, date: '', usd_amount: '', rate: '' }])}
              >
                + Add conversion
              </button>
              <span className="muted" style={{ textAlign: 'right' }}>
                {fmtUsd(convertedUsd)} of {fmtUsd(Number(amount))} → {fmtPln(receivedPln)}
                {convertedUsd > 0 && ` @ ${fmtRate(receivedPln / convertedUsd)} avg`}
              </span>
            </div>
            <p className="muted" style={{ margin: 0, fontSize: 12 }}>
              Anything not converted is treated as still sitting in your USD account. No conversions = you are still holding.
            </p>
          </div>
        </Card>

        <Card title="3 · What you believed then (optional)" sub="Only if you remember it honestly.">
          <div className="form">
            <label className="row" style={{ fontSize: 12, color: 'var(--text-2)' }}>
              <input type="checkbox" checked={hasProb} onChange={(e) => setHasProb(e.target.checked)} style={{ flex: 'none' }} />
              <span>I had a sense of how likely the target was</span>
            </label>
            {hasProb && (
              <div className="prob-slider">
                <input type="range" min="0" max="1" step="0.01" value={prob} onChange={(e) => setProb(Number(e.target.value))} />
                <output>{fmtPct(prob, 0)}</output>
              </div>
            )}
            <p className="muted" style={{ margin: 0, fontSize: 12 }}>
              Past decisions are marked <b>retrospective</b>: a probability written down after you know the outcome isn’t a forecast, so it is
              shown on the bet but kept out of your calibration score. The money results still count. The model’s probability is computed as of
              the start date from data available then.
            </p>
            <label className="field">
              Why you decided what you did
              <textarea rows={4} value={notes} onChange={(e) => setNotes(e.target.value)} />
            </label>
          </div>
        </Card>

        <Card title="4 · Savings accounts" sub="Used to value interest on every path.">
          <div className="form">
            {(['PLN', 'USD', 'EUR'] as const).map((c) => (
              <label className="field" key={c}>
                {c}
                <select value={acc[c] ?? ''} onChange={(e) => setAcc((a) => ({ ...a, [c]: Number(e.target.value) }))}>
                  {accounts.data
                    ?.filter((a) => a.currency === c)
                    .map((a) => (
                      <option key={a.id} value={a.id}>
                        {a.name} · {a.current_rate_pct}%
                      </option>
                    ))}
                </select>
              </label>
            ))}
            <p className="muted" style={{ margin: 0, fontSize: 12 }}>
              Rates in effect back then matter. If your bank’s rates were different on {fmtDate(start)}, add a dated rate under{' '}
              <Link to="/accounts">Accounts</Link> (effective from that date or earlier) — before the first entry, the earliest known rate is used.
            </p>
          </div>
        </Card>
      </div>

      <div className="card" style={{ marginTop: 14, display: 'flex', gap: 16, alignItems: 'center', flexWrap: 'wrap' }}>
        <div style={{ flex: 1, minWidth: 240 }}>
          {problems.length ? (
            <span className="muted">{problems[0]}</span>
          ) : (
            <span>
              {fmtUsd(Number(amount))} on {fmtDate(start)} at {fmtRate(spotValue)}, waiting for {fmtRate(effectiveTarget)} until{' '}
              {fmtDate(effectiveDeadline)} ({horizon} days), {conversions.length} conversion{conversions.length === 1 ? '' : 's'}.
            </span>
          )}
        </div>
        <button className="primary" disabled={problems.length > 0 || create.isPending} onClick={() => create.mutate()}>
          {create.isPending ? 'Replaying…' : 'Record and analyse'}
        </button>
      </div>
    </>
  )
}
