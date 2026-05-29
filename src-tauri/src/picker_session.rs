// ABOUTME: Picker-mode runtime state holder. Enforces mutual exclusion across
// ABOUTME: live/frozen/picking/annotating modes via a single RunMode enum.

use std::sync::Mutex;

use crate::window_picker::{DisplayBounds, PickableWindow};

#[derive(Debug, Clone, PartialEq)]
pub enum RunMode {
    Idle,
    Live,
    Frozen,
    Picking { session: PickerSession },
    Annotating { session: AnnotatingSession },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PickerSession {
    pub windows: Vec<PickableWindow>,
    pub displays: Vec<DisplayBounds>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnnotatingSession {
    pub window_id: u32,
    pub window: PickableWindow,
    pub display: DisplayBounds,
}

pub struct PickerStateHolder {
    inner: Mutex<RunMode>,
}

impl PickerStateHolder {
    pub fn new() -> Self {
        Self { inner: Mutex::new(RunMode::Idle) }
    }

    pub fn current(&self) -> RunMode {
        self.inner.lock().unwrap().clone()
    }

    pub fn try_enter_live(&self) -> Result<(), String> {
        let mut g = self.inner.lock().unwrap();
        if !matches!(*g, RunMode::Idle) {
            return Err(format!("cannot enter live from {:?}", *g));
        }
        *g = RunMode::Live;
        Ok(())
    }

    pub fn try_enter_frozen(&self) -> Result<(), String> {
        let mut g = self.inner.lock().unwrap();
        if !matches!(*g, RunMode::Idle) {
            return Err(format!("cannot enter frozen from {:?}", *g));
        }
        *g = RunMode::Frozen;
        Ok(())
    }

    pub fn try_enter_picking(&self, session: PickerSession) -> Result<(), String> {
        let mut g = self.inner.lock().unwrap();
        if !matches!(*g, RunMode::Idle) {
            return Err(format!("cannot enter picking from {:?}", *g));
        }
        *g = RunMode::Picking { session };
        Ok(())
    }

    pub fn try_select_window(&self, session: AnnotatingSession) -> Result<(), String> {
        let mut g = self.inner.lock().unwrap();
        if !matches!(*g, RunMode::Picking { .. }) {
            return Err(format!("cannot select window from {:?}", *g));
        }
        *g = RunMode::Annotating { session };
        Ok(())
    }

    pub fn reset_to_idle(&self) {
        let mut g = self.inner.lock().unwrap();
        *g = RunMode::Idle;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window_picker::WindowBounds;

    fn session() -> PickerSession {
        PickerSession {
            windows: vec![],
            displays: vec![DisplayBounds {
                display_index: 0, x: 0, y: 0, w: 2560, h: 1414, scale_factor: 2.0,
            }],
        }
    }

    fn pw() -> PickableWindow {
        PickableWindow {
            window_id: 7, app_name: "Safari".into(), title: "x".into(),
            bounds: WindowBounds { x: 0, y: 0, w: 1, h: 1 },
            display_index: 0, layer: 0,
            visible_regions: vec![],
        }
    }

    fn ann() -> AnnotatingSession {
        AnnotatingSession {
            window_id: 7,
            window: pw(),
            display: DisplayBounds { display_index: 0, x: 0, y: 0, w: 1, h: 1, scale_factor: 1.0 },
        }
    }

    #[test]
    fn starts_idle() {
        let h = PickerStateHolder::new();
        assert!(matches!(h.current(), RunMode::Idle));
    }

    #[test]
    fn enters_picking_from_idle() {
        let h = PickerStateHolder::new();
        h.try_enter_picking(session()).unwrap();
        assert!(matches!(h.current(), RunMode::Picking { .. }));
    }

    #[test]
    fn rejects_picking_from_live() {
        let h = PickerStateHolder::new();
        h.try_enter_live().unwrap();
        let err = h.try_enter_picking(session()).unwrap_err();
        assert!(err.contains("Live"), "got: {err}");
    }

    #[test]
    fn picker_to_annotating_succeeds() {
        let h = PickerStateHolder::new();
        h.try_enter_picking(session()).unwrap();
        h.try_select_window(ann()).unwrap();
        assert!(matches!(h.current(), RunMode::Annotating { .. }));
    }

    #[test]
    fn select_window_rejected_from_idle() {
        let h = PickerStateHolder::new();
        let err = h.try_select_window(ann()).unwrap_err();
        assert!(err.contains("Idle"));
    }

    #[test]
    fn reset_to_idle_from_annotating() {
        let h = PickerStateHolder::new();
        h.try_enter_picking(session()).unwrap();
        h.try_select_window(ann()).unwrap();
        h.reset_to_idle();
        assert!(matches!(h.current(), RunMode::Idle));
    }
}
