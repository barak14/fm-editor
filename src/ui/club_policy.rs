//! Explicit, non-optimistic policy trial actions. Rendering never emits a write.
use crate::club_policy::{ClubIdentity, PolicyProposal};
use crate::undo::UndoEntry;
use eframe::egui;

pub(crate) enum Action {
    Prepare {
        request_id: u64,
        identity: ClubIdentity,
    },
    Apply(UndoEntry),
}

#[derive(Default)]
pub(crate) struct PolicyUi {
    pub enabled: bool,
    identity: Option<ClubIdentity>,
    request_id: u64,
    pending: Option<u64>,
    proposal: Option<PolicyProposal>,
    confirmed_copy: bool,
    message: Option<String>,
}

impl PolicyUi {
    pub fn reset(&mut self) {
        self.identity = None;
        self.pending = None;
        self.proposal = None;
        self.confirmed_copy = false;
        self.message = None;
    }

    pub fn receive(&mut self, request_id: u64, result: Result<PolicyProposal, String>) {
        if self.pending != Some(request_id) {
            return;
        }
        self.pending = None;
        self.confirmed_copy = false;
        match result {
            Ok(proposal) if Some(proposal.identity) == self.identity => {
                self.proposal = Some(proposal);
                self.message = None;
            }
            Ok(_) => {
                self.proposal = None;
                self.message = Some("Club identity changed; prepare again.".into());
            }
            Err(e) => {
                self.proposal = None;
                self.message = Some(e);
            }
        }
    }

    pub fn finished(
        &mut self,
        success: bool,
        error: Option<&str>,
        was_undo: bool,
        write_attempted: Option<bool>,
    ) {
        self.proposal = None;
        self.confirmed_copy = false;
        self.message = Some(if success {
            error.map(str::to_owned).unwrap_or_else(|| if was_undo {
                "Readback confirmed: the original club policy byte is restored. Existing offers and gameplay actions are not undone. Prepare a new preview to reapply the trial.".into()
            } else {
                "Readback confirmed. Use Undo before advancing time to restore this byte. Reopen negotiations in FM and verify the available types; no player contract has been repaired yet.".into()
            })
        } else if write_attempted == Some(false) {
            format!(
                "Refused before writing; no memory write was attempted. {}",
                error.unwrap_or("Validation failed.")
            )
        } else {
            error.unwrap_or("Policy write was not confirmed. Inspect memory before retrying; use the untouched backup if recovery is uncertain.").to_owned()
        });
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        identity: ClubIdentity,
        club_name: &str,
        writable: bool,
    ) -> Option<Action> {
        if self.identity != Some(identity) {
            self.reset();
            self.identity = Some(identity);
        }
        let mut action = None;
        egui::CollapsingHeader::new("Part Time offer policy trial")
            .default_open(true)
            .show(ui, |ui| {
                ui.label(format!("Club: {club_name} (UID {})", identity.uid));
                ui.label(concat!(
                    "This trial adds Part Time to the club's ordinary permitted player-contract types. ",
                    "It affects future offers to other players too."
                ));
                ui.label("Requires a verified build and a human manager at this club.");
                ui.label(concat!(
                    "FM must still create and agree a genuine contract. ",
                    "This action chooses no wages, dates, fees or bonuses."
                ));
                if let Some(message) = &self.message {
                    ui.colored_label(ui.visuals().warn_fg_color, message);
                }
                if !self.enabled {
                    ui.label(concat!(
                        "Writes disabled. A separate-copy trial requires an explicit launch ",
                        "with --enable-policy-trial."
                    ));
                }
                let ready = writable && self.pending.is_none();
                if ui.add_enabled(ready, egui::Button::new("Prepare read-only policy preview")).clicked() {
                    self.request_id = self.request_id.checked_add(1).expect("policy request IDs exhausted");
                    self.pending = Some(self.request_id);
                    self.proposal = None;
                    self.confirmed_copy = false;
                    self.message = None;
                    action = Some(Action::Prepare { request_id: self.request_id, identity });
                }
                if self.pending.is_some() {
                    ui.label("Checking build, identity, ownership and original bytes?");
                }
                if self.proposal.is_some() {
                    ui.label("Before: Non-Contract offers only.");
                    ui.label(concat!(
                        "Proposed: Part Time alongside Non-Contract. ",
                        "Other club rules and board limits still apply."
                    ));
                    ui.label(concat!(
                        "One byte is proposed. Contract conversion and payroll behaviour ",
                        "remain unverified until tested in FM."
                    ));
                    ui.add_enabled_ui(self.enabled && ready, |ui| {
                        ui.label("Confirm that you loaded a separate test copy and verified its untouched backup.");
                        ui.checkbox(&mut self.confirmed_copy, "Test copy and backup confirmed");
                    });
                    if ui.add_enabled(
                        self.enabled && ready && self.confirmed_copy,
                        egui::Button::new("Apply club policy trial"),
                    ).clicked() {
                        let proposal = self.proposal.take().expect("preview exists");
                        self.confirmed_copy = false;
                        action = Some(Action::Apply(UndoEntry {
                            description: format!("{club_name}: trial Part Time offer policy (club-wide)"),
                            addr: proposal.address(),
                            old_raw: vec![proposal.before],
                            new_raw: vec![proposal.after],
                            timestamp: std::time::Instant::now(),
                            policy_guard: Some(proposal.guard),
                        }));
                    }
                }
            });
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_distinguishes_restored_policy_preflight_refusal_and_uncertain_write() {
        let mut state = PolicyUi::default();
        state.finished(true, None, true, None);
        let message = state.message.as_ref().unwrap();
        assert!(message.contains("original club policy byte is restored"));
        assert!(!message.contains("Use Undo"));
        state.finished(false, Some("Identity changed"), true, Some(false));
        let message = state.message.as_ref().unwrap();
        assert!(message.contains("no memory write was attempted"));
        assert!(!message.contains("may have changed"));
        state.finished(false, None, true, Some(true));
        let message = state.message.as_ref().unwrap();
        assert!(message.contains("not confirmed"));
        assert!(message.contains("backup"));
    }

    #[test]
    fn asynchronous_results_cannot_bind_to_another_club_or_an_old_request() {
        let p = crate::club_policy::test_proposal();
        let mut state = PolicyUi {
            identity: Some(p.identity),
            pending: Some(7),
            ..Default::default()
        };
        state.receive(6, Ok(p.clone()));
        assert!(state.proposal.is_none());
        assert_eq!(state.pending, Some(7));
        state.identity = Some(ClubIdentity {
            uid: 18,
            ..p.identity
        });
        state.receive(7, Ok(p));
        assert!(state.proposal.is_none());
        assert!(state.message.as_ref().unwrap().contains("identity changed"));
    }

    fn apply_center(shape: &egui::Shape) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(t) if t.galley.text() == "Apply club policy trial" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            egui::Shape::Vec(v) => v.iter().find_map(apply_center),
            _ => None,
        }
    }

    #[test]
    fn rendering_and_disabled_apply_emit_no_write_at_desktop_and_narrow_widths() {
        for width in [1200.0, 280.0] {
            for enabled in [false, true] {
                let p = crate::club_policy::test_proposal();
                let identity = p.identity;
                let mut state = PolicyUi {
                    enabled,
                    identity: Some(identity),
                    proposal: Some(p),
                    ..Default::default()
                };
                let ctx = egui::Context::default();
                let mut center = None;
                for frame in 0..4 {
                    let mut input = egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 800.0),
                        )),
                        ..Default::default()
                    };
                    if frame == 1 || frame == 2 {
                        let pos = center.expect("Apply is visible");
                        input.events = vec![
                            egui::Event::PointerMoved(pos),
                            egui::Event::PointerButton {
                                pos,
                                button: egui::PointerButton::Primary,
                                pressed: frame == 1,
                                modifiers: Default::default(),
                            },
                        ];
                    }
                    let mut action = None;
                    let out = ctx.run(input, |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            action = state.show(ui, identity, "Test Club", true);
                        });
                    });
                    center = out.shapes.iter().find_map(|s| apply_center(&s.shape));
                    let position = center.expect("Apply remains visible");
                    assert!(position.x < width && position.y < 800.0);
                    assert!(action.is_none());
                    assert_eq!(state.proposal.as_ref().unwrap().current_codes, vec![4; 7]);
                }
            }
        }
    }

    #[test]
    fn only_explicit_apply_after_test_copy_confirmation_emits_a_guarded_entry() {
        let p = crate::club_policy::test_proposal();
        let identity = p.identity;
        let mut state = PolicyUi {
            enabled: true,
            identity: Some(identity),
            proposal: Some(p),
            confirmed_copy: true,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let mut center = None;
        let mut count = 0;
        for frame in 0..4 {
            let mut input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 1000.0),
                )),
                ..Default::default()
            };
            if frame == 1 || frame == 2 {
                let pos = center.unwrap();
                input.events = vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: frame == 1,
                        modifiers: Default::default(),
                    },
                ];
            }
            let mut action = None;
            let out = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    action = state.show(ui, identity, "Test Club", true);
                });
            });
            if center.is_none() {
                center = out.shapes.iter().find_map(|s| apply_center(&s.shape));
            }
            if let Some(Action::Apply(entry)) = action {
                count += 1;
                assert_eq!(entry.old_raw, vec![4]);
                assert_eq!(entry.new_raw, vec![0]);
                assert_eq!(entry.policy_guard.unwrap().identity(), identity);
            }
        }
        assert_eq!(count, 1);
    }
}
