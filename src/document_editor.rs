//! V5 native document surface wiring.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use document_core::{
    BlockId, BlockKind, CanonicalBlock, CanonicalDocumentV2, ChildLink, DocumentId, InlineContent,
    InlineId, SiblingPositionKey, Utf8ByteOffset,
};
use document_layout::PageProfileV1;
use execution_workspace::WorkspaceGateway;
use lattice_slint::{NativeDocumentSession, SnapshotProjection};
use slint::ComponentHandle;

use crate::{services::SearchRuntimeHandle, AppWindow};

const DOCUMENT_ID: &str = "retrospect-native-document";
const INITIAL_BLOCK_ID: &str = "retrospect-native-paragraph";
const INITIAL_INLINE_ID: &str = "retrospect-native-inline";

pub fn install(app: &AppWindow, _gateway: WorkspaceGateway, _search: SearchRuntimeHandle) {
    let session = Rc::new(RefCell::new(
        NativeDocumentSession::open(initial_document(), PageProfileV1::a4())
            .expect("the V5 seed document must satisfy canonical layout invariants"),
    ));

    sync(app, &session.borrow());

    app.on_document_text_replacement_requested({
        let weak = app.as_weak();
        let session = Rc::clone(&session);
        move |intent| {
            let Ok(block_id) = BlockId::parse(intent.block_id.to_string()) else {
                return;
            };
            let (Ok(start), Ok(end)) = (
                u64::try_from(intent.start.max(0)),
                u64::try_from(intent.end.max(0)),
            ) else {
                return;
            };
            if session
                .borrow_mut()
                .replace_text(
                    &block_id,
                    Utf8ByteOffset::new(start),
                    Utf8ByteOffset::new(end),
                    intent.text.as_str(),
                )
                .is_ok()
            {
                if let Some(app) = weak.upgrade() {
                    sync(&app, &session.borrow());
                }
            }
        }
    });

    app.on_document_selection_changed({
        let weak = app.as_weak();
        let session = Rc::clone(&session);
        move |block_id, anchor, focus, _source_text| {
            let Ok(block_id) = BlockId::parse(block_id.to_string()) else {
                return;
            };
            let (Ok(anchor), Ok(focus)) =
                (u64::try_from(anchor.max(0)), u64::try_from(focus.max(0)))
            else {
                return;
            };
            if session
                .borrow_mut()
                .set_selection(
                    block_id,
                    Utf8ByteOffset::new(anchor),
                    Utf8ByteOffset::new(focus),
                )
                .is_ok()
            {
                if let Some(app) = weak.upgrade() {
                    sync(&app, &session.borrow());
                }
            }
        }
    });

    app.on_document_undo_requested({
        let weak = app.as_weak();
        let session = Rc::clone(&session);
        move || {
            if session.borrow_mut().undo().is_ok() {
                if let Some(app) = weak.upgrade() {
                    sync(&app, &session.borrow());
                }
            }
        }
    });

    app.on_document_redo_requested({
        let weak = app.as_weak();
        let session = Rc::clone(&session);
        move || {
            if session.borrow_mut().redo().is_ok() {
                if let Some(app) = weak.upgrade() {
                    sync(&app, &session.borrow());
                }
            }
        }
    });
}

fn initial_document() -> CanonicalDocumentV2 {
    let document_id = DocumentId::parse(DOCUMENT_ID).expect("stable document id is valid");
    let block_id = BlockId::parse(INITIAL_BLOCK_ID).expect("stable block id is valid");
    let inline_id = InlineId::parse(INITIAL_INLINE_ID).expect("stable inline id is valid");
    let mut document = CanonicalDocumentV2::empty(document_id);
    document.blocks.insert(
        block_id.clone(),
        CanonicalBlock {
            id: block_id.clone(),
            block_kind: BlockKind::Paragraph,
            children: Vec::new(),
            content: vec![InlineContent::Text {
                id: inline_id,
                text: "This document is powered by the V5 native layout engine.".to_owned(),
                marks: BTreeSet::new(),
            }],
        },
    );
    document.roots.push(ChildLink {
        block_id,
        position: SiblingPositionKey::FIRST,
    });
    document
}

fn sync(app: &AppWindow, session: &NativeDocumentSession) {
    let projection = SnapshotProjection::from_snapshot(session.snapshot());
    app.set_document_pages(projection.to_slint_pages());
    app.set_document_selection(session.selection_for_surface());
    app.set_document_can_undo(session.can_undo());
    app.set_document_can_redo(session.can_redo());
}
