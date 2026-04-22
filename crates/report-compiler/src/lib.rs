use domain_model::{PublishedSurface, ReportPlanId};

#[derive(Clone, Debug)]
pub struct CompileRequest {
    pub plan_id: ReportPlanId,
    pub surface: PublishedSurface,
    pub theme_key: String,
}

#[derive(Clone, Debug)]
pub struct CompileResult {
    pub stylesheet_key: String,
    pub entrypoint_asset: String,
}

pub trait ThemeCompiler {
    fn compile(&self, request: &CompileRequest) -> CompileResult;
}
