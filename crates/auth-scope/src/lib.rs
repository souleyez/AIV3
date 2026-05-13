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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalPrincipalTrustLevel {
    Unresolved,
    Guest,
    ExternalUser,
    Employee,
    Manager,
    Admin,
    SystemOperator,
}

impl ExternalPrincipalTrustLevel {
    pub fn from_str(value: &str) -> Self {
        match value {
            "guest" => Self::Guest,
            "external_user" => Self::ExternalUser,
            "employee" => Self::Employee,
            "manager" => Self::Manager,
            "admin" => Self::Admin,
            "system_operator" => Self::SystemOperator,
            _ => Self::Unresolved,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalPrincipalContext {
    pub tenant_id: TenantId,
    pub platform: String,
    pub external_user_id: String,
    pub external_department_ids: Vec<String>,
    pub external_group_ids: Vec<String>,
    pub external_role_ids: Vec<String>,
    pub v3_user_id: Option<UserId>,
    pub trust_level: ExternalPrincipalTrustLevel,
    pub is_disabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalDocumentAclSnapshot {
    pub source_id: String,
    pub document_external_id: String,
    pub revision_external_id: Option<String>,
    pub allowed_user_external_ids: Vec<String>,
    pub allowed_department_external_ids: Vec<String>,
    pub allowed_group_external_ids: Vec<String>,
    pub allowed_role_external_ids: Vec<String>,
    pub denied_user_external_ids: Vec<String>,
    pub denied_department_external_ids: Vec<String>,
    pub denied_group_external_ids: Vec<String>,
    pub denied_role_external_ids: Vec<String>,
    pub acl_hash: Option<String>,
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

    pub fn can_access_external_document(
        &self,
        principal: &ExternalPrincipalContext,
        acl: &ExternalDocumentAclSnapshot,
    ) -> ScopeDecision {
        if principal.is_disabled {
            return ScopeDecision {
                allowed: false,
                reason: "external principal disabled".to_string(),
            };
        }
        if principal.trust_level == ExternalPrincipalTrustLevel::Unresolved {
            return ScopeDecision {
                allowed: false,
                reason: "external principal unresolved".to_string(),
            };
        }
        if acl
            .denied_user_external_ids
            .contains(&principal.external_user_id)
        {
            return ScopeDecision {
                allowed: false,
                reason: "external source acl denied user".to_string(),
            };
        }
        if intersects(
            &principal.external_department_ids,
            &acl.denied_department_external_ids,
        ) {
            return ScopeDecision {
                allowed: false,
                reason: "external source acl denied department".to_string(),
            };
        }
        if intersects(
            &principal.external_group_ids,
            &acl.denied_group_external_ids,
        ) {
            return ScopeDecision {
                allowed: false,
                reason: "external source acl denied group".to_string(),
            };
        }
        if intersects(&principal.external_role_ids, &acl.denied_role_external_ids) {
            return ScopeDecision {
                allowed: false,
                reason: "external source acl denied role".to_string(),
            };
        }
        if acl
            .allowed_user_external_ids
            .contains(&principal.external_user_id)
        {
            return ScopeDecision {
                allowed: true,
                reason: "external source acl allowed user".to_string(),
            };
        }
        if intersects(
            &principal.external_department_ids,
            &acl.allowed_department_external_ids,
        ) {
            return ScopeDecision {
                allowed: true,
                reason: "external source acl allowed department".to_string(),
            };
        }
        if intersects(
            &principal.external_group_ids,
            &acl.allowed_group_external_ids,
        ) {
            return ScopeDecision {
                allowed: true,
                reason: "external source acl allowed group".to_string(),
            };
        }
        if intersects(&principal.external_role_ids, &acl.allowed_role_external_ids) {
            return ScopeDecision {
                allowed: true,
                reason: "external source acl allowed role".to_string(),
            };
        }

        ScopeDecision {
            allowed: false,
            reason: "external source acl no matching allow".to_string(),
        }
    }
}

fn intersects(left: &[String], right: &[String]) -> bool {
    left.iter().any(|item| right.contains(item))
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

    fn external_principal(external_user_id: &str) -> ExternalPrincipalContext {
        ExternalPrincipalContext {
            tenant_id: TenantId::new(),
            platform: "generic_chat".to_string(),
            external_user_id: external_user_id.to_string(),
            external_department_ids: vec!["dept-risk".to_string()],
            external_group_ids: vec!["group-procurement".to_string()],
            external_role_ids: vec!["role-requester".to_string()],
            v3_user_id: None,
            trust_level: ExternalPrincipalTrustLevel::Employee,
            is_disabled: false,
        }
    }

    fn acl_snapshot() -> ExternalDocumentAclSnapshot {
        ExternalDocumentAclSnapshot {
            source_id: "src-docs".to_string(),
            document_external_id: "doc-risk".to_string(),
            revision_external_id: Some("rev-1".to_string()),
            allowed_user_external_ids: vec!["user-direct".to_string()],
            allowed_department_external_ids: vec!["dept-risk".to_string()],
            allowed_group_external_ids: vec!["group-finance".to_string()],
            allowed_role_external_ids: vec!["role-admin".to_string()],
            denied_user_external_ids: vec!["user-denied".to_string()],
            denied_department_external_ids: Vec::new(),
            denied_group_external_ids: vec!["group-denied".to_string()],
            denied_role_external_ids: Vec::new(),
            acl_hash: Some("acl-hash-1".to_string()),
        }
    }

    #[test]
    fn external_acl_resolver_allows_direct_or_membership_subjects() {
        let resolver = ScopeResolver;
        let acl = acl_snapshot();
        let direct = external_principal("user-direct");
        let department_member = external_principal("user-department");

        let direct_decision = resolver.can_access_external_document(&direct, &acl);
        let department_decision = resolver.can_access_external_document(&department_member, &acl);

        assert!(direct_decision.allowed);
        assert_eq!(direct_decision.reason, "external source acl allowed user");
        assert!(department_decision.allowed);
        assert_eq!(
            department_decision.reason,
            "external source acl allowed department"
        );
    }

    #[test]
    fn external_acl_resolver_denies_overrides_and_unresolved_principals() {
        let resolver = ScopeResolver;
        let mut acl = acl_snapshot();
        let denied = external_principal("user-denied");
        let mut group_denied = external_principal("user-group-denied");
        group_denied
            .external_group_ids
            .push("group-denied".to_string());
        let mut unresolved = external_principal("user-unresolved");
        unresolved.trust_level = ExternalPrincipalTrustLevel::Unresolved;
        let mut disabled = external_principal("user-disabled");
        disabled.is_disabled = true;
        acl.allowed_user_external_ids
            .push("user-denied".to_string());
        acl.allowed_group_external_ids
            .push("group-denied".to_string());
        acl.allowed_user_external_ids
            .push("user-unresolved".to_string());
        acl.allowed_user_external_ids
            .push("user-disabled".to_string());

        let denied_decision = resolver.can_access_external_document(&denied, &acl);
        let group_denied_decision = resolver.can_access_external_document(&group_denied, &acl);
        let unresolved_decision = resolver.can_access_external_document(&unresolved, &acl);
        let disabled_decision = resolver.can_access_external_document(&disabled, &acl);

        assert!(!denied_decision.allowed);
        assert_eq!(denied_decision.reason, "external source acl denied user");
        assert!(!group_denied_decision.allowed);
        assert_eq!(
            group_denied_decision.reason,
            "external source acl denied group"
        );
        assert!(!unresolved_decision.allowed);
        assert_eq!(unresolved_decision.reason, "external principal unresolved");
        assert!(!disabled_decision.allowed);
        assert_eq!(disabled_decision.reason, "external principal disabled");
    }
}
