use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemDetails {
    #[serde(rename = "type")]
    pub error_type: String,
    pub title: String,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ValidationError>,
}

#[derive(Debug, Serialize)]
pub struct ValidationError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

impl ProblemDetails {
    pub fn bad_request(detail: impl Into<String>) -> Self {
        Self {
            error_type: "urn:burst:error:bad-request".into(),
            title: "Bad Request".into(),
            status: 400,
            detail: Some(detail.into()),
            instance: None,
            errors: Vec::new(),
        }
    }

    pub fn validation(errors: Vec<ValidationError>) -> Self {
        Self {
            error_type: "urn:burst:error:validation".into(),
            title: "Validation Error".into(),
            status: 400,
            detail: Some("One or more fields failed validation".into()),
            instance: None,
            errors,
        }
    }

    pub fn unauthorized() -> Self {
        Self {
            error_type: "urn:burst:error:unauthorized".into(),
            title: "Unauthorized".into(),
            status: 401,
            detail: Some("Missing or invalid authentication".into()),
            instance: None,
            errors: Vec::new(),
        }
    }

    pub fn forbidden() -> Self {
        Self {
            error_type: "urn:burst:error:forbidden".into(),
            title: "Forbidden".into(),
            status: 403,
            detail: Some("Insufficient permissions".into()),
            instance: None,
            errors: Vec::new(),
        }
    }

    pub fn not_found(resource: &str) -> Self {
        Self {
            error_type: "urn:burst:error:not-found".into(),
            title: "Not Found".into(),
            status: 404,
            detail: Some(format!("{resource} not found")),
            instance: None,
            errors: Vec::new(),
        }
    }

    pub fn conflict(detail: impl Into<String>) -> Self {
        Self {
            error_type: "urn:burst:error:conflict".into(),
            title: "Conflict".into(),
            status: 409,
            detail: Some(detail.into()),
            instance: None,
            errors: Vec::new(),
        }
    }

    pub fn internal_error() -> Self {
        Self {
            error_type: "urn:burst:error:internal-error".into(),
            title: "Internal Server Error".into(),
            status: 500,
            detail: None,
            instance: None,
            errors: Vec::new(),
        }
    }
}
