use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

#[derive(Debug, Default)]
pub(crate) struct ExternalConversationInFlightGuard {
    active: Mutex<HashSet<String>>,
}

#[derive(Debug)]
pub(crate) struct ExternalConversationInFlightPermit {
    guard: Arc<ExternalConversationInFlightGuard>,
    key: String,
}

impl ExternalConversationInFlightGuard {
    pub(crate) fn try_acquire(
        self: &Arc<Self>,
        key: String,
    ) -> Option<ExternalConversationInFlightPermit> {
        let mut active = self.active.lock().expect("external conversation guard");
        if !active.insert(key.clone()) {
            return None;
        }
        Some(ExternalConversationInFlightPermit {
            guard: Arc::clone(self),
            key,
        })
    }

    pub(crate) fn active_count(&self) -> usize {
        self.active
            .lock()
            .expect("external conversation guard")
            .len()
    }
}

impl Drop for ExternalConversationInFlightPermit {
    fn drop(&mut self) {
        let mut active = self
            .guard
            .active
            .lock()
            .expect("external conversation guard");
        active.remove(&self.key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_guard_allows_different_conversations() {
        let guard = Arc::new(ExternalConversationInFlightGuard::default());
        let first = guard
            .try_acquire("tenant|channel|generic_chat|conversation-a".to_string())
            .expect("first conversation should acquire");
        assert_eq!(guard.active_count(), 1);
        assert!(guard
            .try_acquire("tenant|channel|generic_chat|conversation-a".to_string())
            .is_none());
        let second = guard
            .try_acquire("tenant|channel|generic_chat|conversation-b".to_string())
            .expect("different conversation should acquire");
        assert_eq!(guard.active_count(), 2);
        drop(second);
        assert_eq!(guard.active_count(), 1);
        drop(first);
        assert_eq!(guard.active_count(), 0);
        assert!(guard
            .try_acquire("tenant|channel|generic_chat|conversation-a".to_string())
            .is_some());
    }
}
