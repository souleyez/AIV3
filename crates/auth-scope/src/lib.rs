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
    ExternalIntegration(TenantId),
    ExternalSource(TenantId),
    ExternalActionRun(TenantId),
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
            ScopeResource::ExternalIntegration(tenant_id) => ScopeDecision {
                allowed: principal.tenant_id == *tenant_id,
                reason: "external integration tenant scope".to_string(),
            },
            ScopeResource::ExternalSource(tenant_id) => ScopeDecision {
                allowed: principal.tenant_id == *tenant_id,
                reason: "external source tenant scope".to_string(),
            },
            ScopeResource::ExternalActionRun(tenant_id) => ScopeDecision {
                allowed: principal.tenant_id == *tenant_id,
                reason: "external action tenant scope".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn principal_for_tenant(tenant_id: TenantId) -> PrincipalContext {
        PrincipalContext {
            tenant_id,
            user_id: UserId::new(),
            roles: vec!["operator".to_string()],
            readable_dataset_ids: Vec::new(),
            readable_document_ids: Vec::new(),
            readable_report_ids: Vec::new(),
            unlocked_secret_grant_ids: Vec::new(),
        }
    }

    #[test]
    fn external_integration_scope_is_limited_to_principal_tenant() {
        let tenant_id = TenantId::new();
        let other_tenant_id = TenantId::new();
        let principal = principal_for_tenant(tenant_id);
        let resolver = ScopeResolver;

        let own_integration =
            resolver.can_access(&principal, &ScopeResource::ExternalIntegration(tenant_id));
        let other_integration = resolver.can_access(
            &principal,
            &ScopeResource::ExternalIntegration(other_tenant_id),
        );
        let own_source = resolver.can_access(&principal, &ScopeResource::ExternalSource(tenant_id));
        let own_action =
            resolver.can_access(&principal, &ScopeResource::ExternalActionRun(tenant_id));

        assert!(own_integration.allowed);
        assert!(!other_integration.allowed);
        assert!(own_source.allowed);
        assert!(own_action.allowed);
        assert_eq!(
            other_integration.reason,
            "external integration tenant scope"
        );
    }
}
