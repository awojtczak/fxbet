// Typed client for the fxbet Rust API. Shapes mirror the server's serde output.

export type Currency = 'USD' | 'EUR' | 'PLN'
export type Compounding = 'daily' | 'monthly'

export interface SeriesStats {
  lookback_days: number
  observations: number
  vol_annual: number
  drift_annual: number
  min: number
  max: number
  last: number
  percentile_of_last: number
}

export interface Latest {
  date: string
  usd_pln: number
  usd_eur: number
  eur_pln: number
  usd_pln_change_1d: number | null
  usd_pln_change_30d: number | null
  stats_365d: SeriesStats | null
}

export interface HistoryPoint {
  date: string
  usd_pln: number
  usd_eur: number
  eur_pln: number
}

export interface AccountRate {
  id: number
  account_id: number
  effective_from: string
  annual_rate_pct: number
}

export interface Account {
  id: number
  name: string
  currency: Currency
  compounding: Compounding
  tax_pct: number
  current_rate_pct: number
  rates: AccountRate[]
}

export interface TermsOverride {
  annual_rate_pct: number
  compounding: Compounding
  tax_pct: number
}

export interface AnalyzeRequest {
  amount_usd: number
  target: number
  horizon_days: number
  start_date?: string
  spot?: number
  usd_eur?: number
  spread_pct?: number
  pln_account_id?: number
  usd_account_id?: number
  eur_account_id?: number
  pln?: TermsOverride
  usd?: TermsOverride
  eur?: TermsOverride
  vol_pct?: number
  drift_pct?: number
  lookback_days?: number
  subjective_prob?: number
  paths?: number
  seed?: number
}

export interface Headline {
  pln_now: number
  pln_at_target: number
  fx_gain_pln: number
  pln_interest_per_day: number
  usd_interest_per_day_pln: number
  fx_gain_in_pln_interest_days: number
  breakeven_wait_days: number | null
  required_move_pct: number
  required_move_pips: number
  convert_now_at_horizon: number
  hold_usd_unchanged_rate_at_horizon: number
  eur_route_unchanged_rate_at_horizon: number
  carry_cost_per_day_pln: number
}

export interface Probability {
  vol_annual: number
  drift_annual: number
  model_hit_prob: number
  subjective_prob: number | null
  one_sigma_low: number
  one_sigma_high: number
}

export interface QuantilePoint {
  q: number
  edge_pln: number
}

export interface MonteCarlo {
  paths: number
  hit_rate: number
  mean_days_to_hit: number | null
  mean_value: number
  mean_value_if_hit: number | null
  mean_value_if_miss: number | null
  prob_beats_convert_now: number
  p05: number
  p25: number
  p50: number
  p75: number
  p95: number
  edge_quantiles: QuantilePoint[]
}

export type Verdict = 'wait' | 'convert_now' | 'toss'

export interface Decision {
  required_prob: number | null
  model_edge_pln: number
  subjective_edge_pln: number | null
  verdict: Verdict
}

export interface CurvePoint {
  day: number
  convert_now: number
  hold_usd_unchanged: number
  hold_usd_at_target: number
  eur_route_unchanged: number
  breakeven_rate: number
  rate_p05: number
  rate_p25: number
  rate_p50: number
  rate_p75: number
  rate_p95: number
  hit_prob_by_day: number
}

export interface Sensitivity {
  targets: number[]
  hit_days: number[]
  edge_pln: number[][]
}

export interface ResolvedInputs {
  start_date: string
  deadline: string
  spot: number
  usd_eur: number
  fixing_date: string | null
  vol_pct: number
  vol_source: 'manual' | 'realised' | 'fallback'
  drift_pct: number
  pln_rate_pct: number
  usd_rate_pct: number
  eur_rate_pct: number
}

export interface AnalyzeResponse {
  inputs: ResolvedInputs
  analysis: {
    headline: Headline
    probability: Probability
    monte_carlo: MonteCarlo
    decision: Decision
    curves: CurvePoint[]
    sensitivity: Sensitivity
  }
}

export interface Bet {
  id: number
  created_at: string
  title: string
  notes: string
  amount_usd: number
  start_date: string
  deadline: string
  spot_usd_pln: number
  spot_usd_eur: number
  target_rate: number
  spread_pct: number
  subjective_prob: number | null
  model_prob: number
  pln_account_id: number
  usd_account_id: number
  eur_account_id: number
  retrospective: boolean
  snapshot: {
    request: AnalyzeRequest
    inputs: ResolvedInputs
    headline: Headline
    probability: Probability
    decision: Decision
    monte_carlo: Pick<MonteCarlo, 'hit_rate' | 'mean_value' | 'prob_beats_convert_now' | 'p05' | 'p50' | 'p95'>
  }
}

export type BetStatus = 'open' | 'hit' | 'missed'

export interface ConversionDay {
  date: string
  usd_pln: number
  final_value: number
}

export interface Summary {
  actual: number
  convert_day0: number
  hold_usd: number
  eur_route: number
  target_order: number
  edge_vs_day0_pln: number
  edge_in_pln_interest_days: number | null
  best_day: ConversionDay | null
  worst_day: ConversionDay | null
  actual_beats_share: number
  regret_vs_best_pln: number | null
  usd_remaining: number
  converted_usd: number
  avg_conversion_rate: number | null
}

export interface Outcome {
  resolved: boolean
  target_hit: boolean
  first_hit_date: string | null
  max_rate_in_window: number
  min_rate_in_window: number
  brier_subjective: number | null
  brier_model: number | null
}

export interface DayValue {
  date: string
  usd_pln: number
  eur_pln: number
  actual: number
  convert_day0: number
  hold_usd: number
  eur_route: number
  target_order: number
}

export interface ConversionRecord {
  id: number
  bet_id: number
  date: string
  usd_amount: number
  rate: number
  note: string
}

export interface BetListItem {
  bet: Bet
  status: BetStatus
  summary: Summary
  outcome: Outcome
}

export interface BetDetail {
  bet: Bet
  status: BetStatus
  conversions: ConversionRecord[]
  accounts: Account[]
  tracking: {
    as_of: string
    days: DayValue[]
    conversion_days: ConversionDay[]
    summary: Summary
    outcome: Outcome
  }
}

export interface Bucket {
  from: number
  to: number
  count: number
  mean_predicted: number
  hit_rate: number
}

export interface CalibrationResponse {
  calibration: {
    resolved: number
    hits: number
    brier_subjective: number | null
    brier_model: number | null
    brier_model_on_subjective_subset: number | null
    total_edge_vs_day0_pln: number
    buckets_subjective: Bucket[]
    buckets_model: Bucket[]
  }
  points: {
    bet_id: number
    title: string
    start_date: string
    subjective_prob: number | null
    model_prob: number
    target_hit: boolean
    edge_vs_day0_pln: number
    retrospective: boolean
  }[]
}

export interface NewConversion {
  date: string
  usd_amount: number
  rate: number
  note?: string
}

export class ApiError extends Error {
  readonly status: number
  constructor(status: number, message: string) {
    super(message)
    this.status = status
  }
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(path, {
    method,
    headers: body === undefined ? undefined : { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  if (!res.ok) {
    const text = await res.text()
    let message = text || res.statusText
    try {
      message = (JSON.parse(text) as { error?: string }).error ?? message
    } catch {
      // Non-JSON error body (proxy, crash page): keep the raw text.
    }
    throw new ApiError(res.status, message)
  }
  return res.status === 204 ? (undefined as T) : ((await res.json()) as T)
}

export const api = {
  latest: () => request<Latest>('GET', '/api/fx/latest'),
  history: (from: string, to?: string) =>
    request<HistoryPoint[]>('GET', `/api/fx/history?from=${from}${to ? `&to=${to}` : ''}`),
  stats: (lookbackDays: number) => request<SeriesStats>('GET', `/api/fx/stats?lookback_days=${lookbackDays}`),
  sync: () => request<{ written: number; latest: string | null }>('POST', '/api/fx/sync'),

  accounts: () => request<Account[]>('GET', '/api/accounts'),
  createAccount: (a: Omit<Account, 'id' | 'rates' | 'current_rate_pct'> & { annual_rate_pct: number }) =>
    request<Account>('POST', '/api/accounts', a),
  updateAccount: (id: number, a: Pick<Account, 'name' | 'currency' | 'compounding' | 'tax_pct'>) =>
    request<Account>('PUT', `/api/accounts/${id}`, a),
  deleteAccount: (id: number) => request<void>('DELETE', `/api/accounts/${id}`),
  addRate: (id: number, effective_from: string, annual_rate_pct: number) =>
    request<Account>('POST', `/api/accounts/${id}/rates`, { effective_from, annual_rate_pct }),
  deleteRate: (id: number, rateId: number) => request<Account>('DELETE', `/api/accounts/${id}/rates/${rateId}`),

  analyze: (req: AnalyzeRequest) => request<AnalyzeResponse>('POST', '/api/analyze', req),

  bets: () => request<BetListItem[]>('GET', '/api/bets'),
  bet: (id: number) => request<BetDetail>('GET', `/api/bets/${id}`),
  createBet: (
    title: string,
    notes: string,
    analysis: AnalyzeRequest,
    extra: { retrospective?: boolean; conversions?: NewConversion[] } = {},
  ) => request<BetDetail>('POST', '/api/bets', { title, notes, analysis, ...extra }),
  updateBet: (id: number, title: string, notes: string) => request<BetDetail>('PATCH', `/api/bets/${id}`, { title, notes }),
  deleteBet: (id: number) => request<void>('DELETE', `/api/bets/${id}`),
  addConversion: (id: number, c: NewConversion) =>
    request<BetDetail>('POST', `/api/bets/${id}/conversions`, c),
  deleteConversion: (id: number, cid: number) => request<BetDetail>('DELETE', `/api/bets/${id}/conversions/${cid}`),
  calibration: () => request<CalibrationResponse>('GET', '/api/calibration'),
}
