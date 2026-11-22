//! `selection_announce` (UX spec "Selection", AC-SLT-9): a list or container whose chosen set
//! changed announces how many items are chosen, politely, once the changes have stopped for
//! `selection_announce_ms` -- so ten changes in a burst are one announcement. The wording is the
//! app's own template (`{count}` is the number chosen), so it can be translated; the engine
//! only counts and waits.

use super::*;

impl Tree {
    /// A change of `host`'s chosen set was reported: its announcement, if it has a template, is
    /// due after the delay, counted from now.
    pub(super) fn note_selection_reported(&mut self, host: NodeId) {
        let now = self.clock;
        let Some((sel, _)) = self.selection_mut(host) else {
            return;
        };
        sel.note_reported(now);
        if sel.announce_waiting() && !self.announce_waiting.contains(&host) {
            self.announce_waiting.push(host);
        }
    }

    /// Makes the announcements whose time has come; whether any is still waiting (the window
    /// keeps drawing frames until it is made).
    pub(super) fn tick_selection_announcements(&mut self, now: Instant) -> bool {
        if self.announce_waiting.is_empty() {
            return false;
        }
        let mut waiting = std::mem::take(&mut self.announce_waiting);
        let mut texts = Vec::new();
        waiting.retain(|&host| {
            let Some((sel, _)) = self.selection_mut(host) else {
                return false;
            };
            texts.extend(sel.take_announcement(now));
            sel.announce_waiting()
        });
        self.announce_waiting = waiting;
        for text in texts.iter().filter(|t| !t.trim().is_empty()) {
            self.announce(text, false);
        }
        !self.announce_waiting.is_empty()
    }
}
