//! Engine-bound seams: the Minecraft client surfaces the Java mixin injects
//! into, mirrored for the port and the tests.

use crate::mixins::max_history;

/// java: the CONSTANT intValue=100 in ChatComponent's three call sites.
pub const VANILLA_MAX_HISTORY: i32 = 100;

/// java: ChatComponent.addMessageToDisplayQueue(...) trims to the wrapped
/// constant; the seam mirrors the call site's trim for the tests.
pub fn add_message_to_display_queue(history: &mut Vec<String>, message: String) {
    history.push(message);
    if history.len() as i32 > max_history() {
        let overflow = history.len() as i32 - max_history();
        history.drain(..overflow as usize);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_queue_trims_to_wrapped_cap() {
        let mut history: Vec<String> = (0..16_400).map(|i| format!("msg{i}")).collect();
        add_message_to_display_queue(&mut history, "msg16400".to_string());
        assert_eq!(history.len() as i32, crate::mixins::max_history());
        assert_eq!(history.first(), Some(&"msg17".to_string()));
        assert_eq!(history.last(), Some(&"msg16400".to_string()));
    }

    #[test]
    fn display_queue_keeps_short_histories() {
        let mut history = vec!["hi".to_string()];
        add_message_to_display_queue(&mut history, "again".to_string());
        assert_eq!(history, vec!["hi".to_string(), "again".to_string()]);
    }
}
