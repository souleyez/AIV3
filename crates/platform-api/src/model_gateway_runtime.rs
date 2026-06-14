use chrono::{DateTime, Duration, Utc};
use llm_gateway::model_gateway_lane_env_prefix;
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration as StdDuration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum GatewayLimitError {
    QueueFull,
    QueueTimeout,
    CircuitOpen,
    RequestRateLimit,
    TokenRateLimit,
}

#[allow(dead_code)]
pub(crate) struct GatewayRuntimeLimiter {
    lanes: Mutex<HashMap<String, Arc<GatewayLimitBucket>>>,
    profiles: Mutex<HashMap<String, Arc<GatewayLimitBucket>>>,
    profile_rate_budgets: Mutex<HashMap<String, GatewayProfileRateBudget>>,
    provider_stats: Mutex<HashMap<String, GatewayProviderRuntimeStats>>,
    default_lane_max_concurrency: usize,
    default_profile_max_concurrency: usize,
    default_queue_limit: usize,
    default_queue_timeout: StdDuration,
    circuit_breaker_failure_threshold: u32,
    circuit_breaker_cooldown: StdDuration,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GatewayLimitBucketSnapshot {
    pub(crate) max_concurrency: usize,
    pub(crate) active: usize,
    pub(crate) queued: usize,
    pub(crate) queue_limit: usize,
    pub(crate) queue_timeout_ms: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GatewayRateLimitBudgetSnapshot {
    pub(crate) request_count: u64,
    pub(crate) token_count: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct GatewayRateLimitReservation {
    pub(crate) profile_id: String,
    pub(crate) estimated_tokens: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GatewayProviderStatsSnapshot {
    pub(crate) consecutive_failures: u32,
    pub(crate) success_count: u64,
    pub(crate) failure_count: u64,
    pub(crate) timeout_count: u64,
    pub(crate) rate_limit_count: u64,
    pub(crate) p50_latency_ms: Option<u64>,
    pub(crate) p95_latency_ms: Option<u64>,
    pub(crate) circuit_open: bool,
    pub(crate) opened_until: Option<DateTime<Utc>>,
    pub(crate) last_success_at: Option<DateTime<Utc>>,
    pub(crate) last_failure_at: Option<DateTime<Utc>>,
    pub(crate) last_failure_reason: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct GatewayProviderRuntimeStats {
    pub(crate) consecutive_failures: u32,
    pub(crate) success_count: u64,
    pub(crate) failure_count: u64,
    pub(crate) timeout_count: u64,
    pub(crate) rate_limit_count: u64,
    pub(crate) latency_samples_ms: VecDeque<u64>,
    pub(crate) opened_until: Option<Instant>,
    pub(crate) opened_until_wall_time: Option<DateTime<Utc>>,
    pub(crate) last_success_at: Option<DateTime<Utc>>,
    pub(crate) last_failure_at: Option<DateTime<Utc>>,
    pub(crate) last_failure_reason: Option<String>,
}

#[allow(dead_code)]
impl GatewayRuntimeLimiter {
    pub(crate) fn new() -> Self {
        Self {
            lanes: Mutex::new(HashMap::new()),
            profiles: Mutex::new(HashMap::new()),
            profile_rate_budgets: Mutex::new(HashMap::new()),
            provider_stats: Mutex::new(HashMap::new()),
            default_lane_max_concurrency: 64,
            default_profile_max_concurrency: 16,
            default_queue_limit: 256,
            default_queue_timeout: StdDuration::from_millis(3_000),
            circuit_breaker_failure_threshold: 3,
            circuit_breaker_cooldown: StdDuration::from_secs(60),
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self::new()
    }

    #[cfg(test)]
    pub(crate) fn with_lane_limit(
        self,
        lane: &str,
        max_concurrency: usize,
        queue_limit: usize,
    ) -> Self {
        self.set_lane_limit(
            lane,
            max_concurrency,
            queue_limit,
            self.default_queue_timeout,
        );
        self
    }

    #[cfg(test)]
    pub(crate) fn with_profile_limit(
        self,
        profile_id: &str,
        max_concurrency: usize,
        queue_limit: usize,
    ) -> Self {
        self.set_profile_limit(
            profile_id,
            max_concurrency,
            queue_limit,
            self.default_queue_timeout,
        );
        self
    }

    #[cfg(test)]
    pub(crate) fn with_profile_rate_limit(
        self,
        profile_id: &str,
        requests_per_minute: Option<u32>,
        tokens_per_minute: Option<u32>,
    ) -> Self {
        self.ensure_profile_rate_limit(profile_id, requests_per_minute, tokens_per_minute);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_circuit_breaker(
        mut self,
        failure_threshold: u32,
        cooldown: StdDuration,
    ) -> Self {
        self.circuit_breaker_failure_threshold = failure_threshold.max(1);
        self.circuit_breaker_cooldown = cooldown;
        self
    }

    pub(crate) fn set_lane_limit(
        &self,
        lane: &str,
        max_concurrency: usize,
        queue_limit: usize,
        queue_timeout: StdDuration,
    ) {
        let mut lanes = self.lanes.lock().expect("gateway lane limiter lock");
        lanes.insert(
            lane.to_string(),
            Arc::new(GatewayLimitBucket::new(
                max_concurrency,
                queue_limit,
                queue_timeout,
            )),
        );
    }

    pub(crate) fn set_profile_limit(
        &self,
        profile_id: &str,
        max_concurrency: usize,
        queue_limit: usize,
        queue_timeout: StdDuration,
    ) {
        let mut profiles = self.profiles.lock().expect("gateway profile limiter lock");
        profiles.insert(
            profile_id.to_string(),
            Arc::new(GatewayLimitBucket::new(
                max_concurrency,
                queue_limit,
                queue_timeout,
            )),
        );
    }

    pub(crate) fn ensure_lane_limit(
        &self,
        lane: &str,
        max_concurrency: Option<u32>,
        queue_limit: Option<u32>,
        queue_timeout_ms: Option<u64>,
    ) {
        let mut lanes = self.lanes.lock().expect("gateway lane limiter lock");
        lanes.entry(lane.to_string()).or_insert_with(|| {
            Arc::new(GatewayLimitBucket::new(
                max_concurrency
                    .map(|value| value as usize)
                    .unwrap_or(self.default_lane_max_concurrency),
                queue_limit
                    .map(|value| value as usize)
                    .unwrap_or(self.default_queue_limit),
                queue_timeout_ms
                    .map(StdDuration::from_millis)
                    .unwrap_or(self.default_queue_timeout),
            ))
        });
    }

    pub(crate) fn ensure_profile_limit(
        &self,
        profile_id: &str,
        max_concurrency: Option<u32>,
        queue_limit: Option<u32>,
        queue_timeout_ms: Option<u64>,
    ) {
        let mut profiles = self.profiles.lock().expect("gateway profile limiter lock");
        profiles.entry(profile_id.to_string()).or_insert_with(|| {
            Arc::new(GatewayLimitBucket::new(
                max_concurrency
                    .map(|value| value as usize)
                    .unwrap_or(self.default_profile_max_concurrency),
                queue_limit
                    .map(|value| value as usize)
                    .unwrap_or(self.default_queue_limit),
                queue_timeout_ms
                    .map(StdDuration::from_millis)
                    .unwrap_or(self.default_queue_timeout),
            ))
        });
    }

    pub(crate) fn ensure_profile_rate_limit(
        &self,
        profile_id: &str,
        requests_per_minute: Option<u32>,
        tokens_per_minute: Option<u32>,
    ) {
        let mut budgets = self
            .profile_rate_budgets
            .lock()
            .expect("gateway profile rate budget lock");
        budgets
            .entry(profile_id.to_string())
            .or_default()
            .configure(requests_per_minute, tokens_per_minute);
    }

    pub(crate) fn reserve_profile_rate_budget(
        &self,
        profile_id: &str,
        estimated_tokens: u64,
    ) -> Result<GatewayRateLimitReservation, GatewayLimitError> {
        let mut budgets = self
            .profile_rate_budgets
            .lock()
            .expect("gateway profile rate budget lock");
        budgets.entry(profile_id.to_string()).or_default().reserve(
            profile_id,
            estimated_tokens,
            Instant::now(),
        )
    }

    pub(crate) fn record_profile_actual_token_usage(
        &self,
        reservation: &GatewayRateLimitReservation,
        actual_tokens: u64,
    ) {
        let additional_tokens = actual_tokens.saturating_sub(reservation.estimated_tokens);
        if additional_tokens == 0 {
            return;
        }
        let mut budgets = self
            .profile_rate_budgets
            .lock()
            .expect("gateway profile rate budget lock");
        budgets
            .entry(reservation.profile_id.clone())
            .or_default()
            .add_tokens(additional_tokens, Instant::now());
    }

    pub(crate) fn profile_rate_snapshot(&self, profile_id: &str) -> GatewayRateLimitBudgetSnapshot {
        let mut budgets = self
            .profile_rate_budgets
            .lock()
            .expect("gateway profile rate budget lock");
        budgets
            .entry(profile_id.to_string())
            .or_default()
            .snapshot(Instant::now())
    }

    pub(crate) fn would_throttle_lane_and_profile(
        &self,
        lane: &str,
        profile_id: &str,
        estimated_tokens: u64,
    ) -> Option<GatewayLimitError> {
        if self.provider_available(profile_id).is_none() {
            return Some(GatewayLimitError::CircuitOpen);
        }
        let lane_snapshot = self.lane_snapshot(lane);
        if lane_snapshot.active >= lane_snapshot.max_concurrency
            && lane_snapshot.queued >= lane_snapshot.queue_limit
        {
            return Some(GatewayLimitError::QueueFull);
        }
        let profile_snapshot = self.profile_snapshot(profile_id);
        if profile_snapshot.active >= profile_snapshot.max_concurrency
            && profile_snapshot.queued >= profile_snapshot.queue_limit
        {
            return Some(GatewayLimitError::QueueFull);
        }
        let mut budgets = self
            .profile_rate_budgets
            .lock()
            .expect("gateway profile rate budget lock");
        budgets
            .entry(profile_id.to_string())
            .or_default()
            .would_limit(estimated_tokens, Instant::now())
    }

    pub(crate) async fn acquire_lane(
        &self,
        lane: &str,
    ) -> Result<GatewayRuntimePermit, GatewayLimitError> {
        let bucket = self.lane_bucket(lane);
        let permit = bucket.acquire().await?;
        Ok(GatewayRuntimePermit {
            _lane: Some(permit),
            _profile: None,
        })
    }

    pub(crate) async fn acquire_profile(
        &self,
        profile_id: &str,
    ) -> Result<GatewayRuntimePermit, GatewayLimitError> {
        let bucket = self.profile_bucket(profile_id);
        let permit = bucket.acquire().await?;
        Ok(GatewayRuntimePermit {
            _lane: None,
            _profile: Some(permit),
        })
    }

    pub(crate) async fn acquire_lane_and_profile(
        &self,
        lane: &str,
        profile_id: &str,
    ) -> Result<GatewayRuntimePermit, GatewayLimitError> {
        let lane_bucket = self.lane_bucket(lane);
        let profile_bucket = self.profile_bucket(profile_id);
        let lane_permit = lane_bucket.acquire().await?;
        let profile_permit = match profile_bucket.acquire().await {
            Ok(permit) => permit,
            Err(error) => {
                drop(lane_permit);
                return Err(error);
            }
        };
        Ok(GatewayRuntimePermit {
            _lane: Some(lane_permit),
            _profile: Some(profile_permit),
        })
    }

    pub(crate) fn provider_available(
        &self,
        profile_id: &str,
    ) -> Option<GatewayProviderStatsSnapshot> {
        let now = Instant::now();
        let mut stats_by_profile = self
            .provider_stats
            .lock()
            .expect("gateway provider stats lock");
        let Some(stats) = stats_by_profile.get_mut(profile_id) else {
            return Some(GatewayProviderStatsSnapshot::default());
        };
        if stats.circuit_open(now) {
            return None;
        }
        stats.clear_expired_circuit(now);
        Some(stats.snapshot(now))
    }

    pub(crate) fn record_provider_success(&self, profile_id: &str, latency_ms: Option<u64>) {
        let mut stats_by_profile = self
            .provider_stats
            .lock()
            .expect("gateway provider stats lock");
        stats_by_profile
            .entry(profile_id.to_string())
            .or_default()
            .record_success(latency_ms);
    }

    pub(crate) fn record_provider_failure(&self, profile_id: &str, reason: &str) {
        let now = Instant::now();
        let now_wall_time = Utc::now();
        let opened_until_wall_time = Duration::from_std(self.circuit_breaker_cooldown)
            .ok()
            .map(|duration| now_wall_time + duration);
        let mut stats_by_profile = self
            .provider_stats
            .lock()
            .expect("gateway provider stats lock");
        stats_by_profile
            .entry(profile_id.to_string())
            .or_default()
            .record_failure(
                reason,
                now,
                now_wall_time,
                self.circuit_breaker_failure_threshold,
                self.circuit_breaker_cooldown,
                opened_until_wall_time,
            );
    }

    pub(crate) fn lane_snapshot(&self, lane: &str) -> GatewayLimitBucketSnapshot {
        self.lane_bucket(lane).snapshot()
    }

    pub(crate) fn profile_snapshot(&self, profile_id: &str) -> GatewayLimitBucketSnapshot {
        self.profile_bucket(profile_id).snapshot()
    }

    pub(crate) fn provider_stats_snapshot(&self, profile_id: &str) -> GatewayProviderStatsSnapshot {
        let now = Instant::now();
        let mut stats_by_profile = self
            .provider_stats
            .lock()
            .expect("gateway provider stats lock");
        let Some(stats) = stats_by_profile.get_mut(profile_id) else {
            return GatewayProviderStatsSnapshot::default();
        };
        stats.clear_expired_circuit(now);
        stats.snapshot(now)
    }

    fn lane_bucket(&self, lane: &str) -> Arc<GatewayLimitBucket> {
        let mut lanes = self.lanes.lock().expect("gateway lane limiter lock");
        lanes
            .entry(lane.to_string())
            .or_insert_with(|| {
                Arc::new(GatewayLimitBucket::new(
                    self.default_lane_max_concurrency,
                    self.default_queue_limit,
                    self.default_queue_timeout,
                ))
            })
            .clone()
    }

    fn profile_bucket(&self, profile_id: &str) -> Arc<GatewayLimitBucket> {
        let mut profiles = self.profiles.lock().expect("gateway profile limiter lock");
        profiles
            .entry(profile_id.to_string())
            .or_insert_with(|| {
                Arc::new(GatewayLimitBucket::new(
                    self.default_profile_max_concurrency,
                    self.default_queue_limit,
                    self.default_queue_timeout,
                ))
            })
            .clone()
    }
}

impl GatewayProviderRuntimeStats {
    pub(crate) fn circuit_open(&self, now: Instant) -> bool {
        self.opened_until
            .map(|opened_until| opened_until > now)
            .unwrap_or(false)
    }

    pub(crate) fn clear_expired_circuit(&mut self, now: Instant) {
        if self
            .opened_until
            .map(|opened_until| opened_until <= now)
            .unwrap_or(false)
        {
            self.opened_until = None;
            self.opened_until_wall_time = None;
        }
    }

    pub(crate) fn record_success(&mut self, latency_ms: Option<u64>) {
        self.consecutive_failures = 0;
        self.success_count = self.success_count.saturating_add(1);
        self.opened_until = None;
        self.opened_until_wall_time = None;
        self.last_success_at = Some(Utc::now());
        if let Some(latency_ms) = latency_ms {
            self.latency_samples_ms.push_back(latency_ms);
            while self.latency_samples_ms.len() > 256 {
                self.latency_samples_ms.pop_front();
            }
        }
    }

    pub(crate) fn record_failure(
        &mut self,
        reason: &str,
        now: Instant,
        now_wall_time: DateTime<Utc>,
        failure_threshold: u32,
        cooldown: StdDuration,
        opened_until_wall_time: Option<DateTime<Utc>>,
    ) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        self.failure_count = self.failure_count.saturating_add(1);
        if gateway_provider_failure_is_timeout(reason) {
            self.timeout_count = self.timeout_count.saturating_add(1);
        }
        if gateway_provider_failure_is_rate_limit(reason) {
            self.rate_limit_count = self.rate_limit_count.saturating_add(1);
        }
        self.last_failure_at = Some(now_wall_time);
        self.last_failure_reason = Some(reason.to_string());
        if self.consecutive_failures >= failure_threshold {
            self.opened_until = Some(now + cooldown);
            self.opened_until_wall_time = opened_until_wall_time;
        }
    }

    pub(crate) fn snapshot(&self, now: Instant) -> GatewayProviderStatsSnapshot {
        let circuit_open = self.circuit_open(now);
        let mut latencies: Vec<u64> = self.latency_samples_ms.iter().copied().collect();
        latencies.sort_unstable();
        GatewayProviderStatsSnapshot {
            consecutive_failures: self.consecutive_failures,
            success_count: self.success_count,
            failure_count: self.failure_count,
            timeout_count: self.timeout_count,
            rate_limit_count: self.rate_limit_count,
            p50_latency_ms: percentile_latency(&latencies, 50),
            p95_latency_ms: percentile_latency(&latencies, 95),
            circuit_open,
            opened_until: circuit_open
                .then(|| self.opened_until_wall_time.clone())
                .flatten(),
            last_success_at: self.last_success_at.clone(),
            last_failure_at: self.last_failure_at.clone(),
            last_failure_reason: self.last_failure_reason.clone(),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct GatewayProfileRateBudget {
    pub(crate) requests_per_minute: Option<u32>,
    pub(crate) tokens_per_minute: Option<u32>,
    pub(crate) window_started_at: Option<Instant>,
    pub(crate) request_count: u32,
    pub(crate) token_count: u64,
}

impl GatewayProfileRateBudget {
    pub(crate) fn configure(
        &mut self,
        requests_per_minute: Option<u32>,
        tokens_per_minute: Option<u32>,
    ) {
        self.requests_per_minute = requests_per_minute.filter(|value| *value > 0);
        self.tokens_per_minute = tokens_per_minute.filter(|value| *value > 0);
    }

    pub(crate) fn reserve(
        &mut self,
        profile_id: &str,
        estimated_tokens: u64,
        now: Instant,
    ) -> Result<GatewayRateLimitReservation, GatewayLimitError> {
        self.reset_elapsed_window(now);
        if let Some(limit) = self.requests_per_minute {
            if self.request_count.saturating_add(1) > limit {
                return Err(GatewayLimitError::RequestRateLimit);
            }
        }
        let estimated_tokens = estimated_tokens.max(1);
        if let Some(limit) = self.tokens_per_minute {
            if self.token_count.saturating_add(estimated_tokens) > limit as u64 {
                return Err(GatewayLimitError::TokenRateLimit);
            }
        }
        self.request_count = self.request_count.saturating_add(1);
        self.token_count = self.token_count.saturating_add(estimated_tokens);
        Ok(GatewayRateLimitReservation {
            profile_id: profile_id.to_string(),
            estimated_tokens,
        })
    }

    pub(crate) fn add_tokens(&mut self, additional_tokens: u64, now: Instant) {
        self.reset_elapsed_window(now);
        self.token_count = self.token_count.saturating_add(additional_tokens);
    }

    pub(crate) fn would_limit(
        &mut self,
        estimated_tokens: u64,
        now: Instant,
    ) -> Option<GatewayLimitError> {
        self.reset_elapsed_window(now);
        if let Some(limit) = self.requests_per_minute {
            if self.request_count.saturating_add(1) > limit {
                return Some(GatewayLimitError::RequestRateLimit);
            }
        }
        if let Some(limit) = self.tokens_per_minute {
            if self.token_count.saturating_add(estimated_tokens.max(1)) > limit as u64 {
                return Some(GatewayLimitError::TokenRateLimit);
            }
        }
        None
    }

    pub(crate) fn snapshot(&mut self, now: Instant) -> GatewayRateLimitBudgetSnapshot {
        self.reset_elapsed_window(now);
        GatewayRateLimitBudgetSnapshot {
            request_count: self.request_count as u64,
            token_count: self.token_count,
        }
    }

    pub(crate) fn reset_elapsed_window(&mut self, now: Instant) {
        let should_reset = self
            .window_started_at
            .map(|started_at| now.duration_since(started_at) >= StdDuration::from_secs(60))
            .unwrap_or(true);
        if should_reset {
            self.window_started_at = Some(now);
            self.request_count = 0;
            self.token_count = 0;
        }
    }
}

pub(crate) fn percentile_latency(sorted_latencies: &[u64], percentile: usize) -> Option<u64> {
    if sorted_latencies.is_empty() {
        return None;
    }
    let percentile = percentile.min(100);
    let index = ((sorted_latencies.len().saturating_sub(1)) * percentile + 99) / 100;
    sorted_latencies.get(index).copied()
}

pub(crate) fn gateway_limit_error_reason(error: GatewayLimitError) -> &'static str {
    match error {
        GatewayLimitError::QueueFull => "model_gateway_queue_full",
        GatewayLimitError::QueueTimeout => "model_gateway_queue_timeout",
        GatewayLimitError::CircuitOpen => "model_gateway_circuit_open",
        GatewayLimitError::RequestRateLimit => "model_gateway_request_rate_limit",
        GatewayLimitError::TokenRateLimit => "model_gateway_token_rate_limit",
    }
}

pub(crate) fn model_gateway_estimated_input_tokens(input: &str) -> u64 {
    ((input.chars().count() as u64) / 4).max(1)
}

pub(crate) fn model_gateway_lane_routing_mode(lane: &str) -> String {
    let prefix = model_gateway_lane_env_prefix(lane);
    std::env::var(format!("{prefix}_MODE"))
        .or_else(|_| std::env::var(format!("{prefix}_ROUTING_MODE")))
        .unwrap_or_else(|_| "observe_only".to_string())
        .trim()
        .to_ascii_lowercase()
}

pub(crate) fn model_gateway_lane_canary_percent(lane: &str) -> Option<u32> {
    let prefix = model_gateway_lane_env_prefix(lane);
    std::env::var(format!("{prefix}_CANARY_PERCENT"))
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .map(|value| value.min(100))
}

pub(crate) fn gateway_provider_failure_is_timeout(reason: &str) -> bool {
    reason.contains("timeout")
}

pub(crate) fn gateway_provider_failure_is_rate_limit(reason: &str) -> bool {
    reason.contains("429") || reason.contains("rate_limit") || reason.contains("too_many_requests")
}

#[allow(dead_code)]
struct GatewayLimitBucket {
    max_concurrency: usize,
    semaphore: Arc<Semaphore>,
    queue_limit: usize,
    queue_timeout: StdDuration,
    queued: AtomicUsize,
}

#[allow(dead_code)]
impl GatewayLimitBucket {
    fn new(max_concurrency: usize, queue_limit: usize, queue_timeout: StdDuration) -> Self {
        let max_concurrency = max_concurrency.max(1);
        Self {
            max_concurrency,
            semaphore: Arc::new(Semaphore::new(max_concurrency)),
            queue_limit,
            queue_timeout,
            queued: AtomicUsize::new(0),
        }
    }

    async fn acquire(&self) -> Result<OwnedSemaphorePermit, GatewayLimitError> {
        if let Ok(permit) = self.semaphore.clone().try_acquire_owned() {
            return Ok(permit);
        }

        if self.queue_limit == 0 {
            return Err(GatewayLimitError::QueueFull);
        }

        let queued = self.queued.fetch_add(1, Ordering::AcqRel);
        if queued >= self.queue_limit {
            self.queued.fetch_sub(1, Ordering::AcqRel);
            return Err(GatewayLimitError::QueueFull);
        }

        let permit_result =
            tokio::time::timeout(self.queue_timeout, self.semaphore.clone().acquire_owned()).await;
        self.queued.fetch_sub(1, Ordering::AcqRel);

        match permit_result {
            Ok(Ok(permit)) => Ok(permit),
            Ok(Err(_closed)) => Err(GatewayLimitError::QueueFull),
            Err(_elapsed) => Err(GatewayLimitError::QueueTimeout),
        }
    }

    fn snapshot(&self) -> GatewayLimitBucketSnapshot {
        let active = self
            .max_concurrency
            .saturating_sub(self.semaphore.available_permits().min(self.max_concurrency));
        GatewayLimitBucketSnapshot {
            max_concurrency: self.max_concurrency,
            active,
            queued: self.queued.load(Ordering::Acquire),
            queue_limit: self.queue_limit,
            queue_timeout_ms: self.queue_timeout.as_millis() as u64,
        }
    }
}

#[allow(dead_code)]
pub(crate) struct GatewayRuntimePermit {
    _lane: Option<OwnedSemaphorePermit>,
    _profile: Option<OwnedSemaphorePermit>,
}

#[allow(dead_code)]
pub(crate) struct GatewayModelPermit {
    pub(crate) _runtime: Option<GatewayRuntimePermit>,
    pub(crate) rate_reservation: Option<GatewayRateLimitReservation>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock")
    }

    fn clear_lane_env(lane: &str) {
        let prefix = model_gateway_lane_env_prefix(lane);
        std::env::remove_var(format!("{prefix}_MODE"));
        std::env::remove_var(format!("{prefix}_ROUTING_MODE"));
        std::env::remove_var(format!("{prefix}_CANARY_PERCENT"));
    }

    #[test]
    fn gateway_limit_error_reason_preserves_public_reason_strings() {
        assert_eq!(
            gateway_limit_error_reason(GatewayLimitError::QueueFull),
            "model_gateway_queue_full"
        );
        assert_eq!(
            gateway_limit_error_reason(GatewayLimitError::QueueTimeout),
            "model_gateway_queue_timeout"
        );
        assert_eq!(
            gateway_limit_error_reason(GatewayLimitError::CircuitOpen),
            "model_gateway_circuit_open"
        );
        assert_eq!(
            gateway_limit_error_reason(GatewayLimitError::RequestRateLimit),
            "model_gateway_request_rate_limit"
        );
        assert_eq!(
            gateway_limit_error_reason(GatewayLimitError::TokenRateLimit),
            "model_gateway_token_rate_limit"
        );
    }

    #[test]
    fn model_gateway_estimated_input_tokens_uses_quarter_char_floor() {
        assert_eq!(model_gateway_estimated_input_tokens(""), 1);
        assert_eq!(model_gateway_estimated_input_tokens("abc"), 1);
        assert_eq!(model_gateway_estimated_input_tokens("abcd"), 1);
        assert_eq!(model_gateway_estimated_input_tokens("abcde"), 1);
        assert_eq!(model_gateway_estimated_input_tokens("abcdefgh"), 2);
        assert_eq!(model_gateway_estimated_input_tokens("数据平台问答"), 1);
        assert_eq!(model_gateway_estimated_input_tokens("数据平台问答测试"), 2);
    }

    #[test]
    fn model_gateway_lane_routing_mode_preserves_env_precedence_and_defaults() {
        let _guard = env_lock();
        let lane = "test-runtime-selection-lane";
        clear_lane_env(lane);

        assert_eq!(model_gateway_lane_routing_mode(lane), "observe_only");

        let prefix = model_gateway_lane_env_prefix(lane);
        std::env::set_var(format!("{prefix}_ROUTING_MODE"), " CANARY ");
        assert_eq!(model_gateway_lane_routing_mode(lane), "canary");

        std::env::set_var(format!("{prefix}_MODE"), " ACTIVE ");
        assert_eq!(model_gateway_lane_routing_mode(lane), "active");

        clear_lane_env(lane);
    }

    #[test]
    fn model_gateway_lane_canary_percent_trims_caps_and_ignores_invalid_values() {
        let _guard = env_lock();
        let lane = "test-runtime-canary-lane";
        clear_lane_env(lane);
        let prefix = model_gateway_lane_env_prefix(lane);

        assert_eq!(model_gateway_lane_canary_percent(lane), None);

        std::env::set_var(format!("{prefix}_CANARY_PERCENT"), " 35 ");
        assert_eq!(model_gateway_lane_canary_percent(lane), Some(35));

        std::env::set_var(format!("{prefix}_CANARY_PERCENT"), "150");
        assert_eq!(model_gateway_lane_canary_percent(lane), Some(100));

        std::env::set_var(format!("{prefix}_CANARY_PERCENT"), "bad");
        assert_eq!(model_gateway_lane_canary_percent(lane), None);

        clear_lane_env(lane);
    }
}
