//! Slint host adapter for a scoped execution-workspace document.
//!
//! The UI owns projection and intent only. The shared gateway owns path
//! confinement, conflict detection, identity, and atomic persistence.

use std::{cell::RefCell, rc::Rc};

use execution_workspace::{RelativePath, SaveRequest, WorkspaceGateway};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use ui_kit_slint::ui_kit_slint_ui::{CodeEditorDocument, CodeEditorSaveState};

use crate::{services::SearchRuntimeHandle, AppWindow};

pub fn install(
    app: &AppWindow,
    gateway: WorkspaceGateway,
    document_path: RelativePath,
    search: SearchRuntimeHandle,
) {
    let session = Rc::new(RefCell::new(
        EditorSession::load(&gateway, document_path)
            .unwrap_or_else(|error| EditorSession::failed(error.to_string())),
    ));
    let line_numbers = Rc::new(VecModel::from(Vec::<i32>::new()));
    app.set_editor_line_numbers(ModelRc::from(line_numbers.clone()));
    publish(app, &session.borrow(), &line_numbers, true);

    {
        let session = session.clone();
        let line_numbers = line_numbers.clone();
        let callback_app = app.as_weak();
        app.on_editor_text_changed(move |text| {
            let mut session = session.borrow_mut();
            session.replace_text(text.as_str());
            if let Some(app) = callback_app.upgrade() {
                publish(&app, &session, &line_numbers, false);
            }
        });
    }
    {
        let session = session.clone();
        let line_numbers = line_numbers.clone();
        let callback_app = app.as_weak();
        let gateway = gateway.clone();
        let search = search.clone();
        app.on_editor_save_requested(move || {
            let mut session = session.borrow_mut();
            if let Ok(change) = session.save(&gateway) {
                search.enqueue_file_change(change);
            }
            if let Some(app) = callback_app.upgrade() {
                publish(&app, &session, &line_numbers, false);
            }
        });
    }
    {
        let session = session.clone();
        let line_numbers = line_numbers.clone();
        let callback_app = app.as_weak();
        let gateway = gateway.clone();
        app.on_editor_revert_requested(move || {
            let mut session = session.borrow_mut();
            session.revert(&gateway);
            if let Some(app) = callback_app.upgrade() {
                publish(&app, &session, &line_numbers, true);
            }
        });
    }
}

fn publish(
    app: &AppWindow,
    session: &EditorSession,
    line_numbers: &Rc<VecModel<i32>>,
    publish_text: bool,
) {
    line_numbers.set_vec((1..=session.line_count()).collect::<Vec<_>>());
    if publish_text {
        app.set_editor_text(SharedString::from(session.text.as_str()));
    }
    app.set_editor_document(CodeEditorDocument {
        id: SharedString::from(session.document_id.as_str()),
        title: SharedString::from(session.title.as_str()),
        language: SharedString::from("Rust"),
        read_only: false,
        save_state: session.save_state(),
        status_message: SharedString::from(session.status_message()),
    });
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SessionSaveState {
    Clean,
    Dirty,
    Error,
}

struct EditorSession {
    document_id: String,
    path: Option<RelativePath>,
    title: String,
    digest: Option<String>,
    saved_text: String,
    text: String,
    state: SessionSaveState,
    error: Option<String>,
}

impl EditorSession {
    fn load(gateway: &WorkspaceGateway, path: RelativePath) -> Result<Self, String> {
        let snapshot = gateway
            .read_text(path.clone())
            .map_err(|error| error.to_string())?;
        let saved_text = snapshot.text;
        let title = path
            .as_path()
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Document".into());
        Ok(Self {
            document_id: snapshot.id.as_str().into(),
            path: Some(path),
            title,
            digest: Some(snapshot.digest),
            text: saved_text.clone(),
            saved_text,
            state: SessionSaveState::Clean,
            error: None,
        })
    }

    fn failed(error: String) -> Self {
        Self {
            document_id: "unavailable".into(),
            path: None,
            title: "Document unavailable".into(),
            digest: None,
            saved_text: String::new(),
            text: String::new(),
            state: SessionSaveState::Error,
            error: Some(error),
        }
    }

    fn replace_text(&mut self, text: &str) {
        self.text = text.to_owned();
        self.state = if self.text == self.saved_text {
            SessionSaveState::Clean
        } else {
            SessionSaveState::Dirty
        };
        self.error = None;
    }

    fn save(&mut self, gateway: &WorkspaceGateway) -> Result<execution_workspace::FileChange, ()> {
        let Some(path) = self.path.clone() else {
            self.state = SessionSaveState::Error;
            return Err(());
        };
        match gateway.save_text(SaveRequest {
            path,
            text: self.text.clone(),
            expected_digest: self.digest.clone(),
        }) {
            Ok(result) => {
                self.document_id = result.snapshot.id.as_str().into();
                self.saved_text = result.snapshot.text;
                self.text.clone_from(&self.saved_text);
                self.digest = Some(result.snapshot.digest);
                self.state = SessionSaveState::Clean;
                self.error = None;
                Ok(result.change)
            }
            Err(error) => {
                self.state = SessionSaveState::Error;
                self.error = Some(error.to_string());
                Err(())
            }
        }
    }

    fn revert(&mut self, gateway: &WorkspaceGateway) {
        let Some(path) = self.path.clone() else {
            return;
        };
        match Self::load(gateway, path) {
            Ok(reloaded) => *self = reloaded,
            Err(error) => {
                self.state = SessionSaveState::Error;
                self.error = Some(error);
            }
        }
    }

    fn line_count(&self) -> i32 {
        self.text.lines().count().max(1) as i32
    }

    fn save_state(&self) -> CodeEditorSaveState {
        match self.state {
            SessionSaveState::Clean => CodeEditorSaveState::Clean,
            SessionSaveState::Dirty => CodeEditorSaveState::Dirty,
            SessionSaveState::Error => CodeEditorSaveState::Error,
        }
    }

    fn status_message(&self) -> &str {
        if let Some(error) = self.error.as_deref() {
            return error;
        }
        match self.state {
            SessionSaveState::Clean => "Execution workspace — saved",
            SessionSaveState::Dirty => "Execution workspace — modified",
            SessionSaveState::Error => "Execution workspace error",
        }
    }
}

#[cfg(test)]
mod tests {
    use execution_workspace::{
        CapabilityGrant, ExecutionWorkspaceId, RelativePath, WorkspaceGateway,
    };
    use tempfile::TempDir;

    use super::{EditorSession, SessionSaveState};

    #[test]
    fn reverting_restores_the_saved_fixture() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("document.rs"), "saved").unwrap();
        let gateway = WorkspaceGateway::open(
            ExecutionWorkspaceId::parse("editor-test").unwrap(),
            temp.path(),
            CapabilityGrant::editor(),
        )
        .unwrap();
        let mut session =
            EditorSession::load(&gateway, RelativePath::parse("document.rs").unwrap()).unwrap();
        session.replace_text("edited");
        session.revert(&gateway);

        assert_eq!(session.state, SessionSaveState::Clean);
        assert_eq!(session.text, session.saved_text);
    }
}
