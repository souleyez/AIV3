use domain_model::{
    DatasetId, DocumentId, PublishedReportId, SecretBindingId, SecretGrantId, TenantId, UserId,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrincipalContext {
    pub tenant_id: TenantId,
    pub user_id: UserId,
    pub roles: Vec<String>,
    pub readable_dataset_ids: Vec<DatasetId>,
    pub readable_document_ids: Vec<DocumentId>,
    pub readable_report_ids: Vec<PublishedReportId>,
    pub unlocked_secret_grant_ids: Vec<SecretGrantId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ScopeResource {
    Dataset(DatasetId),
    Document(DocumentId),
    Secret(SecretBindingId),
    PublishedReport(PublishedReportId),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScopeDecision {
    pub allowed: bool,
    pub reason: String,
}

#[derive(Default)]
pub struct ScopeResolver;

impl ScopeResolver {
    pub fn can_access(
        &self,
        principal: &PrincipalContext,
        resource: &ScopeResource,
    ) -> ScopeDecision {
        match resource {
            ScopeResource::Dataset(dataset_id) => ScopeDecision {
                allowed: principal.readable_dataset_ids.contains(dataset_id),
                reason: "dataset scope".to_string(),
            },
            ScopeResource::Document(document_id) => ScopeDecision {
                allowed: principal.readable_document_ids.contains(document_id),
                reason: "document scope".to_string(),
            },
            ScopeResource::Secret(_) => ScopeDecision {
                allowed: !principal.unlocked_secret_grant_ids.is_empty(),
                reason: "secret grant scope".to_string(),
            },
            ScopeResource::PublishedReport(report_id) => ScopeDecision {
                allowed: principal.readable_report_ids.contains(report_id),
                reason: "published asset scope".to_string(),
            },
        }
    }
}
