use crate::cli::errors::{CliError, CliResult};
use url::Url;

/// Validates URL format
pub fn validate_url(url: &str) -> CliResult<()> {
    if url.trim().is_empty() {
        return Err(CliError::validation("url", "URL cannot be empty"));
    }

    match Url::parse(url) {
        Ok(parsed_url) => {
            if parsed_url.scheme().is_empty() {
                return Err(CliError::validation("url", "URL must have a scheme (http://, https://, etc.)"));
            }
            Ok(())
        }
        Err(_) => Err(CliError::validation("url", "Invalid URL format")),
    }
}

/// Validates tag format
pub fn validate_tags(tags: &str) -> CliResult<()> {
    if tags.trim().is_empty() {
        return Ok(());
    }

    let tag_list: Vec<&str> = tags.split(',').collect();
    for tag in tag_list {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            continue;
        }
        
        if trimmed.contains(' ') {
            return Err(CliError::validation("tags", "Tags cannot contain spaces"));
        }
        
        if trimmed.len() > 50 {
            return Err(CliError::validation("tags", "Individual tags cannot exceed 50 characters"));
        }
    }
    
    Ok(())
}

/// Validates title length
pub fn validate_title(title: &str) -> CliResult<()> {
    if title.len() > 500 {
        return Err(CliError::validation("title", "Title cannot exceed 500 characters"));
    }
    Ok(())
}

/// Validates description length
pub fn validate_description(description: &str) -> CliResult<()> {
    if description.len() > 2000 {
        return Err(CliError::validation("description", "Description cannot exceed 2000 characters"));
    }
    Ok(())
}

/// Validates bookmark creation input
pub fn validate_bookmark_create(
    url: &Option<String>,
    title: &Option<String>,
    description: &Option<String>,
    tags: &Option<String>,
) -> CliResult<()> {
    if let Some(url) = url {
        validate_url(url)?;
    }
    
    if let Some(title) = title {
        validate_title(title)?;
    }
    
    if let Some(description) = description {
        validate_description(description)?;
    }
    
    if let Some(tags) = tags {
        validate_tags(tags)?;
    }
    
    Ok(())
}

/// Validates search query input
pub fn validate_search_query(
    url: &Option<String>,
    title: &Option<String>,
    description: &Option<String>,
    tags: &Option<String>,
) -> CliResult<()> {
    if let Some(url) = url {
        if !url.trim().is_empty() {
            validate_url(url)?;
        }
    }
    
    if let Some(title) = title {
        if !title.trim().is_empty() {
            validate_title(title)?;
        }
    }
    
    if let Some(description) = description {
        if !description.trim().is_empty() {
            validate_description(description)?;
        }
    }
    
    if let Some(tags) = tags {
        if !tags.trim().is_empty() {
            validate_tags(tags)?;
        }
    }
    
    Ok(())
}

/// Validates semantic search parameters
pub fn validate_semantic_params(
    semantic: &Option<String>,
    threshold: &Option<f32>,
) -> CliResult<()> {
    // Validate threshold is in valid range [0.0, 1.0]
    if let Some(threshold) = threshold {
        if !(*threshold >= 0.0 && *threshold <= 1.0) {
            return Err(CliError::validation(
                "threshold",
                "Threshold must be between 0.0 and 1.0",
            ));
        }
    }

    // Threshold without semantic query is allowed (uses config default_threshold)
    // but warn if it seems unintentional
    if threshold.is_some() && semantic.is_none() {
        // This is valid - threshold will be used with any future semantic search
        // No error, just proceed
    }

    Ok(())
}

/// Validates rule input
pub fn validate_rule_input(
    url: &Option<String>,
    title: &Option<String>,
    description: &Option<String>,
    tags: &Option<String>,
) -> CliResult<()> {
    if let Some(url) = url {
        if !url.trim().is_empty() {
            // For rules, we allow regex patterns, so just check if it's not empty
            if url.trim().is_empty() {
                return Err(CliError::validation("url", "URL pattern cannot be empty"));
            }
        }
    }

    if let Some(title) = title {
        if !title.trim().is_empty() && title.len() > 200 {
            return Err(CliError::validation("title", "Title pattern cannot exceed 200 characters"));
        }
    }

    if let Some(description) = description {
        if !description.trim().is_empty() && description.len() > 500 {
            return Err(CliError::validation("description", "Description pattern cannot exceed 500 characters"));
        }
    }

    if let Some(tags) = tags {
        if !tags.trim().is_empty() {
            validate_tags(tags)?;
        }
    }

    // At least one field must be specified
    if url.is_none() && title.is_none() && description.is_none() && tags.is_none() {
        return Err(CliError::validation("rule", "At least one field (url, title, description, or tags) must be specified"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Semantic parameter validation tests (E.4)
    // =========================================================================

    #[test]
    fn test_validate_semantic_params_valid_threshold() {
        // Valid thresholds
        assert!(validate_semantic_params(&Some("query".to_string()), &Some(0.0)).is_ok());
        assert!(validate_semantic_params(&Some("query".to_string()), &Some(0.5)).is_ok());
        assert!(validate_semantic_params(&Some("query".to_string()), &Some(1.0)).is_ok());
        assert!(validate_semantic_params(&Some("query".to_string()), &Some(0.35)).is_ok());
    }

    #[test]
    fn test_validate_semantic_params_invalid_threshold_below_zero() {
        let result = validate_semantic_params(&Some("query".to_string()), &Some(-0.1));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, CliError::Validation { field, .. } if field == "threshold"));
    }

    #[test]
    fn test_validate_semantic_params_invalid_threshold_above_one() {
        let result = validate_semantic_params(&Some("query".to_string()), &Some(1.1));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, CliError::Validation { field, .. } if field == "threshold"));
    }

    #[test]
    fn test_validate_semantic_params_threshold_without_semantic() {
        // Threshold without semantic query is allowed (uses config default)
        assert!(validate_semantic_params(&None, &Some(0.5)).is_ok());
    }

    #[test]
    fn test_validate_semantic_params_semantic_without_threshold() {
        // Semantic without threshold uses default from config
        assert!(validate_semantic_params(&Some("query".to_string()), &None).is_ok());
    }

    #[test]
    fn test_validate_semantic_params_neither_provided() {
        // Neither provided - valid (no semantic search)
        assert!(validate_semantic_params(&None, &None).is_ok());
    }

    #[test]
    fn test_validate_semantic_params_empty_query() {
        // Empty string for semantic query - let backend decide
        assert!(validate_semantic_params(&Some("".to_string()), &Some(0.5)).is_ok());
    }
}
