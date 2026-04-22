use domain_model::DatasetId;
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct AnalyticsQuery {
    pub dataset_id: DatasetId,
    pub logical_name: String,
    pub parameters: Value,
}

#[derive(Clone, Debug)]
pub struct AnalyticsResult {
    pub row_count: usize,
    pub payload: Value,
}

pub trait AnalyticsService {
    fn execute(&self, query: &AnalyticsQuery) -> AnalyticsResult;
}
