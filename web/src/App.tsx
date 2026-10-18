import { useQuery } from '@tanstack/react-query'
import { NavLink, Route, Routes } from 'react-router-dom'
import { api } from './api'
import { fmtRate, fmtShortDate } from './format'
import Accounts from './pages/Accounts'
import Analyzer from './pages/Analyzer'
import BetDetail from './pages/BetDetail'
import Bets from './pages/Bets'
import Calibration from './pages/Calibration'
import Dashboard from './pages/Dashboard'

export default function App() {
  const latest = useQuery({ queryKey: ['latest'], queryFn: api.latest })
  return (
    <div className="shell">
      <nav className="nav">
        <div className="brand">
          <img src="/favicon.svg" alt="" />
          FX Bet
        </div>
        <NavLink to="/" end>
          Dashboard
        </NavLink>
        <NavLink to="/analyze">Analyzer</NavLink>
        <NavLink to="/bets">Bets</NavLink>
        <NavLink to="/calibration">Calibration</NavLink>
        <NavLink to="/accounts">Accounts</NavLink>
        {latest.data && (
          <div className="ticker">
            USD/PLN · ECB {fmtShortDate(latest.data.date)}
            <b>{fmtRate(latest.data.usd_pln)}</b>
          </div>
        )}
      </nav>
      <main>
        <Routes>
          <Route path="/" element={<Dashboard />} />
          <Route path="/analyze" element={<Analyzer />} />
          <Route path="/bets" element={<Bets />} />
          <Route path="/bets/:id" element={<BetDetail />} />
          <Route path="/calibration" element={<Calibration />} />
          <Route path="/accounts" element={<Accounts />} />
          <Route path="*" element={<div className="empty">Not found</div>} />
        </Routes>
      </main>
    </div>
  )
}
