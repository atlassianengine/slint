// Rust guideline compliant 2026-02-21

#![expect(
    dead_code,
    reason = "Capability resolution API for workspace agent indexing"
)]

//! Resolves native `todoco://` capability URIs and builds workspace agent indexes.
//!
//! Provides URI parsing and agent index construction for Retrospect workspace
//! entities, documents, and agent manifests.

use std::collections::HashMap;

use document_retrieval::{
    build_workspace_agent_index, workspace_agent_index_uri, AgentIndexBuildResult, DocumentRecord,
    ManifestIndexMetadata, WorkspaceAgentIndexV1,
};

/// Represents a parsed `todoco://` capability URI target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityTarget {
    /// Workspace agent index: `todoco://workspaces/{workspace_id}`
    WorkspaceIndex { workspace_id: String },
    /// Document resource: `todoco://documents/{document_id}`
    Document { document_id: String },
    /// Document manifest resource: `todoco://workspaces/{workspace_id}/documents/{document_id}/manifest`
    DocumentManifest {
        workspace_id: String,
        document_id: String,
    },
}

/// Parses a `todoco://` URI string into a typed [`CapabilityTarget`].
///
/// # Errors
///
/// Returns an error string if the scheme is not `todoco://` or the path is invalid.
pub fn parse_capability_uri(uri: &str) -> Result<CapabilityTarget, String> {
    let stripped = uri
        .strip_prefix("todoco://")
        .ok_or_else(|| format!("Invalid URI scheme in '{uri}', expected 'todoco://'"))?;

    let parts: Vec<&str> = stripped
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();

    match parts.as_slice() {
        ["workspaces", workspace_id] => Ok(CapabilityTarget::WorkspaceIndex {
            workspace_id: (*workspace_id).to_string(),
        }),
        ["documents", document_id] => Ok(CapabilityTarget::Document {
            document_id: (*document_id).to_string(),
        }),
        ["workspaces", workspace_id, "documents", document_id] => {
            Ok(CapabilityTarget::DocumentManifest {
                workspace_id: (*workspace_id).to_string(),
                document_id: (*document_id).to_string(),
            })
        }
        ["workspaces", workspace_id, "documents", document_id, "manifest"] => {
            Ok(CapabilityTarget::DocumentManifest {
                workspace_id: (*workspace_id).to_string(),
                document_id: (*document_id).to_string(),
            })
        }
        _ => Err(format!("Unrecognized capability URI structure: '{uri}'")),
    }
}

/// Formats a workspace agent index URI for a given workspace.
pub fn format_workspace_uri(workspace_id: &str) -> String {
    workspace_agent_index_uri(workspace_id)
}

/// Formats a document agent manifest URI.
pub fn format_manifest_uri(workspace_id: &str, document_id: &str) -> String {
    format!("todoco://workspaces/{workspace_id}/documents/{document_id}/manifest")
}

/// Builds the workspace agent index for Retrospect documents.
pub fn build_native_workspace_index(
    workspace_id: &str,
    tenant_id: &str,
    title: &str,
    documents: &[DocumentRecord],
    manifest_hashes: &HashMap<String, String>,
    indexed_at: &str,
) -> AgentIndexBuildResult<WorkspaceAgentIndexV1> {
    let mut manifest_metadata = HashMap::new();
    for (doc_id, hash) in manifest_hashes {
        manifest_metadata.insert(
            doc_id.clone(),
            ManifestIndexMetadata {
                manifest_hash: hash.clone(),
                trust_label: "local".to_string(),
            },
        );
    }

    build_workspace_agent_index(
        workspace_id,
        tenant_id,
        title,
        &[],
        documents,
        &manifest_metadata,
        indexed_at,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_workspace_capability_uri() {
        let parsed = parse_capability_uri("todoco://workspaces/retrospect").unwrap();
        assert_eq!(
            parsed,
            CapabilityTarget::WorkspaceIndex {
                workspace_id: "retrospect".to_string(),
            }
        );
    }

    #[test]
    fn parses_document_manifest_capability_uri() {
        let parsed =
            parse_capability_uri("todoco://workspaces/retrospect/documents/doc-1/manifest")
                .unwrap();
        assert_eq!(
            parsed,
            CapabilityTarget::DocumentManifest {
                workspace_id: "retrospect".to_string(),
                document_id: "doc-1".to_string(),
            }
        );
    }

    #[test]
    fn formats_workspace_index_uris_consistently() {
        assert_eq!(
            format_workspace_uri("retrospect"),
            "todoco://workspace/retrospect/agent-index"
        );
    }
}
