use domain_model::{DatasetId, PublishedSurface, ReportPlanId};

#[derive(Clone, Debug)]
pub struct PublishRequest {
    pub dataset_id: DatasetId,
    pub plan_id: ReportPlanId,
    pub slug: String,
    pub surface: PublishedSurface,
}

#[derive(Clone, Debug)]
pub struct PublishResult {
    pub version_no: i32,
    pub asset_manifest_key: String,
}

pub trait ReportPublisher {
    fn publish(&self, request: &PublishRequest) -> PublishResult;
}
