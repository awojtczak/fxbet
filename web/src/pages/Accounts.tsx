import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { api, type Account, type Compounding, type Currency } from '../api'
import { Card, ErrorBox } from '../components/ui'
import { fmtDate, isoToday } from '../format'

export default function Accounts() {
  const q = useQuery({ queryKey: ['accounts'], queryFn: api.accounts })
  return (
    <>
      <div className="page-head">
        <div>
          <h1>Savings accounts</h1>
          <p>
            Variable rates are a schedule: add a new rate with its effective date whenever the bank changes it (future dates work for
            announced cuts). Bets are replayed with this history.
          </p>
        </div>
      </div>
      <ErrorBox error={q.error} />
      <div className="grid cols-3">
        {q.data?.map((a) => <AccountCard key={a.id} a={a} />)}
        <NewAccount />
      </div>
    </>
  )
}

function AccountCard({ a }: { a: Account }) {
  const qc = useQueryClient()
  const refresh = () => qc.invalidateQueries({ queryKey: ['accounts'] })
  const [name, setName] = useState(a.name)
  const [compounding, setCompounding] = useState<Compounding>(a.compounding)
  const [tax, setTax] = useState(String(a.tax_pct))
  const [from, setFrom] = useState(isoToday())
  const [rate, setRate] = useState('')
  const save = useMutation({
    mutationFn: () => api.updateAccount(a.id, { name, currency: a.currency, compounding, tax_pct: Number(tax) }),
    onSuccess: refresh,
  })
  const addRate = useMutation({
    mutationFn: () => api.addRate(a.id, from, Number(rate)),
    onSuccess: () => {
      setRate('')
      refresh()
    },
  })
  const delRate = useMutation({ mutationFn: (rid: number) => api.deleteRate(a.id, rid), onSuccess: refresh })
  const del = useMutation({ mutationFn: () => api.deleteAccount(a.id), onSuccess: refresh })
  const dirty = name !== a.name || compounding !== a.compounding || Number(tax) !== a.tax_pct
  const today = isoToday()

  return (
    <Card title={`${a.currency} · ${a.current_rate_pct}%`} sub={a.name}>
      <div className="form">
        <ErrorBox error={save.error ?? addRate.error ?? delRate.error ?? del.error} />
        <label className="field">
          Name
          <input value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        <div className="row">
          <label className="field">
            Capitalization
            <select value={compounding} onChange={(e) => setCompounding(e.target.value as Compounding)}>
              <option value="daily">daily</option>
              <option value="monthly">monthly</option>
            </select>
          </label>
          <label className="field">
            Tax on interest %
            <input type="number" step="1" value={tax} onChange={(e) => setTax(e.target.value)} />
          </label>
        </div>
        <div className="actions">
          <button className="danger" onClick={() => confirm(`Delete ${a.name}?`) && del.mutate()}>
            Delete
          </button>
          <button className="primary" disabled={!dirty} onClick={() => save.mutate()}>
            Save
          </button>
        </div>
        <h3>Rate history (gross % p.a.)</h3>
        <table>
          <tbody>
            {[...a.rates].reverse().map((r) => (
              <tr key={r.id}>
                <td>
                  {fmtDate(r.effective_from)}
                  {r.effective_from > today && <span className="muted"> (scheduled)</span>}
                </td>
                <td className="num">{r.annual_rate_pct}%</td>
                <td className="num">
                  <button className="danger" disabled={a.rates.length <= 1} onClick={() => delRate.mutate(r.id)}>
                    ×
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        <div className="row">
          <input type="date" value={from} onChange={(e) => setFrom(e.target.value)} aria-label="Effective from" />
          <input type="number" step="0.05" placeholder="rate %" value={rate} onChange={(e) => setRate(e.target.value)} aria-label="Rate" />
          <button style={{ flex: 'none' }} disabled={!rate} onClick={() => addRate.mutate()}>
            Add
          </button>
        </div>
      </div>
    </Card>
  )
}

function NewAccount() {
  const qc = useQueryClient()
  const [name, setName] = useState('')
  const [currency, setCurrency] = useState<Currency>('PLN')
  const [compounding, setCompounding] = useState<Compounding>('daily')
  const [tax, setTax] = useState('19')
  const [rate, setRate] = useState('')
  const create = useMutation({
    mutationFn: () => api.createAccount({ name, currency, compounding, tax_pct: Number(tax), annual_rate_pct: Number(rate) }),
    onSuccess: () => {
      setName('')
      setRate('')
      qc.invalidateQueries({ queryKey: ['accounts'] })
    },
  })
  return (
    <Card title="Add account" sub="E.g. a competing bank’s promo savings account to compare.">
      <div className="form">
        <ErrorBox error={create.error} />
        <label className="field">
          Name
          <input value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        <div className="row">
          <label className="field">
            Currency
            <select value={currency} onChange={(e) => setCurrency(e.target.value as Currency)}>
              <option>PLN</option>
              <option>USD</option>
              <option>EUR</option>
            </select>
          </label>
          <label className="field">
            Capitalization
            <select value={compounding} onChange={(e) => setCompounding(e.target.value as Compounding)}>
              <option value="daily">daily</option>
              <option value="monthly">monthly</option>
            </select>
          </label>
        </div>
        <div className="row">
          <label className="field">
            Rate % p.a.
            <input type="number" step="0.05" value={rate} onChange={(e) => setRate(e.target.value)} />
          </label>
          <label className="field">
            Tax %
            <input type="number" value={tax} onChange={(e) => setTax(e.target.value)} />
          </label>
        </div>
        <div className="actions">
          <button className="primary" disabled={!name.trim() || !rate || create.isPending} onClick={() => create.mutate()}>
            Create
          </button>
        </div>
      </div>
    </Card>
  )
}
