//! Fixture host policy, intentionally outside Kit and platform adapters.
use gpui_kit::navigation::NavHistory;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tab {
    Library,
    Form,
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub pinned: bool,
}

/// Only durable fixture values cross background/recreation. Focus handles,
/// keyboard state, modal openness and gesture progress are deliberately absent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub version: u32,
    pub tab: Tab,
    pub visits: Vec<String>,
    pub cursor: usize,
    pub routes: BTreeMap<String, String>,
    pub next_visit: u64,
    pub notes: Vec<Note>,
    pub name: String,
    pub draft: String,
    pub category: String,
    pub dirty: bool,
}

pub struct FixtureState {
    pub tab: Tab,
    pub history: NavHistory,
    routes: BTreeMap<String, String>,
    next_visit: u64,
    pub notes: Vec<Note>,
    pub name: String,
    pub draft: String,
    pub category: String,
    pub dirty: bool,
    pub notice: String,
    pub refresh_failed: bool,
}

impl Default for FixtureState {
    fn default() -> Self {
        Self {
            tab: Tab::Library,
            history: NavHistory::new("library"),
            routes: BTreeMap::new(),
            next_visit: 1,
            notes: [
                ("notes", "项目笔记"),
                ("travel", "旅行清单"),
                ("ideas", "设计草稿"),
            ]
            .into_iter()
            .map(|(id, title)| Note {
                id: id.into(),
                title: title.into(),
                pinned: false,
            })
            .collect(),
            name: "林小满".into(),
            draft: "第一行：保留中文输入。\n第二行：多行草稿属于调用方。".into(),
            category: "personal".into(),
            dirty: false,
            notice: "Fixture only · no network or native success claims".into(),
            refresh_failed: false,
        }
    }
}

impl FixtureState {
    pub fn open_note(&mut self, id: &str) {
        if !self.notes.iter().any(|note| note.id == id) {
            return;
        }
        let visit = format!("visit-{}", self.next_visit);
        self.next_visit += 1;
        if self.history.push(visit.clone()) {
            self.routes.insert(visit, id.into());
        }
    }
    pub fn current_note(&self) -> Option<&Note> {
        let id = self.routes.get(self.history.current().as_ref())?;
        self.notes.iter().find(|note| &note.id == id)
    }
    pub fn select_tab(&mut self, tab: Tab) -> bool {
        if tab != self.tab && self.dirty {
            self.notice = "Back refused: accept the fixture draft before leaving.".into();
            return false;
        }
        self.tab = tab;
        true
    }
    pub fn request_back(&mut self) -> bool {
        if self.dirty {
            self.notice = "Back refused: unsaved draft remains visible.".into();
            return false;
        }
        if self.tab != Tab::Library {
            self.tab = Tab::Library;
            return true;
        }
        if self.history.pop() {
            return true;
        }
        self.notice = "At root: platform host decides whether to leave the app.".into();
        false
    }
    pub fn fail_refresh(&mut self) {
        self.refresh_failed = true;
        self.notice = "Fixture refresh failed · last verified rows retained".into();
    }
    pub fn row_action(&mut self, note_id: &str, action: &str) {
        if let Some(note) = self.notes.iter_mut().find(|note| note.id == note_id) {
            match action {
                "pin" => {
                    note.pinned = !note.pinned;
                    self.notice = format!("Fixture pin changed: {}", note.title);
                }
                "remove" => self.notice = "Removal refused by fixture host · row retained".into(),
                _ => self.notice = "Unavailable action refused".into(),
            }
        }
    }
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            version: 1,
            tab: self.tab,
            visits: self
                .history
                .entries()
                .iter()
                .map(ToString::to_string)
                .collect(),
            cursor: self.history.cursor(),
            routes: self.routes.clone(),
            next_visit: self.next_visit,
            notes: self.notes.clone(),
            name: self.name.clone(),
            draft: self.draft.clone(),
            category: self.category.clone(),
            dirty: self.dirty,
        }
    }
    pub fn restore(saved: Checkpoint) -> Result<Self, &'static str> {
        if saved.version != 1 {
            return Err("Unsupported checkpoint version");
        }
        if saved.next_visit == 0
            || saved.routes.keys().any(|visit| {
                visit
                    .strip_prefix("visit-")
                    .and_then(|suffix| suffix.parse::<u64>().ok())
                    .is_none_or(|visit| visit >= saved.next_visit)
            })
        {
            return Err("Invalid visit sequence");
        }
        let history = NavHistory::restore(
            saved.visits.into_iter().map(Into::into).collect(),
            saved.cursor,
        )
        .ok_or("Invalid visit history")?;
        if history.entries().first().map(|id| id.as_ref()) != Some("library")
            || history.entries().iter().skip(1).any(|visit| {
                saved
                    .routes
                    .get(visit.as_ref())
                    .is_none_or(|id| !saved.notes.iter().any(|note| &note.id == id))
            })
        {
            return Err("Unresolved fixture route");
        }
        Ok(Self {
            tab: saved.tab,
            history,
            routes: saved.routes,
            next_visit: saved.next_visit,
            notes: saved.notes,
            name: saved.name,
            draft: saved.draft,
            category: saved.category,
            dirty: saved.dirty,
            notice: "Restored fixture checkpoint · overlays closed; refresh not replayed".into(),
            refresh_failed: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failure_and_refused_delete_retain_verified_rows_but_explicit_pin_changes_one() {
        let mut state = FixtureState::default();
        let before = state.notes.clone();
        state.fail_refresh();
        assert_eq!(state.notes, before);
        assert!(state.refresh_failed);
        state.row_action("travel", "remove");
        assert_eq!(state.notes, before);
        state.row_action("travel", "pin");
        assert!(state.notes[1].pinned);
        assert!(!state.notes[0].pinned);
    }
    #[test]
    fn draft_refuses_back_and_tab_change_until_host_accepts() {
        let mut state = FixtureState {
            tab: Tab::Form,
            dirty: true,
            ..Default::default()
        };
        assert!(!state.request_back());
        assert!(!state.select_tab(Tab::Image));
        assert_eq!(state.tab, Tab::Form);
        state.dirty = false;
        assert!(state.request_back());
        assert_eq!(state.tab, Tab::Library);
        assert!(!state.request_back());
    }
    #[test]
    fn checkpoint_restores_chinese_draft_route_and_verified_data_not_refresh_activity() {
        let mut state = FixtureState::default();
        state.open_note("travel");
        state.dirty = true;
        state.fail_refresh();
        let bytes = serde_json::to_vec(&state.checkpoint()).expect("serializable checkpoint");
        let saved = serde_json::from_slice(&bytes).expect("valid checkpoint");
        let restored = FixtureState::restore(saved).expect("known fixture routes");
        assert_eq!(
            restored.current_note().expect("detail restored").id,
            "travel"
        );
        assert_eq!(restored.draft, state.draft);
        assert!(restored.dirty);
        assert!(!restored.refresh_failed);
        assert!(restored.notice.contains("overlays closed"));
    }
    #[test]
    fn restoration_rejects_unresolvable_routes_and_visit_ids_are_not_record_ids() {
        let mut state = FixtureState::default();
        state.open_note("notes");
        let first = state.history.current().clone();
        state.request_back();
        state.open_note("notes");
        assert_ne!(first, *state.history.current());
        let mut saved = state.checkpoint();
        saved.routes.clear();
        assert!(FixtureState::restore(saved).is_err());
        let mut saved = state.checkpoint();
        saved.version = 2;
        assert!(FixtureState::restore(saved).is_err());
        let mut saved = state.checkpoint();
        saved.next_visit = 1;
        assert!(FixtureState::restore(saved).is_err());
    }
}
