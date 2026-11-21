//! The command usages shown in the player's tree and their legacy permission rules.

use std::sync::OnceLock;

use mod_api::Command;

fn usages() -> &'static [Command] {
    static USAGES: OnceLock<Vec<Command>> = OnceLock::new();
    USAGES.get_or_init(|| {
        let mut commands = Vec::new();
        crate::glue::commands(&mut commands);
        crate::invsee_glue::commands(&mut commands);
        crate::ping_glue::commands(&mut commands);
        commands
    })
}

pub fn registered(commands: &mut Vec<Command>) {
    commands.extend_from_slice(usages());
}

/// The matching usage with the most leading literals controls permission.
/// When the input fits no usage, an accessible root still lets SMP report
/// its own usage error.
pub fn player_can_dispatch(line: &str, permission: u8) -> bool {
    let words: Vec<_> = line.split(' ').collect();
    let Some(root) = words.first() else {
        return false;
    };
    let mut allowed_root = false;
    let mut best = None;
    for command in usages() {
        let usage: Vec<_> = command.usage.split_whitespace().collect();
        if usage.first() != Some(root) {
            continue;
        }
        allowed_root |= command.level <= permission;
        let literals = usage
            .iter()
            .take_while(|w| !w.starts_with(['<', '[']))
            .count();
        let required = usage.iter().filter(|w| !w.starts_with('[')).count();
        let greedy = usage
            .last()
            .is_some_and(|w| w.ends_with(":text>") || w.ends_with(":text]"));
        if words.len() >= required
            && (words.len() <= usage.len() || greedy)
            && usage.get(..literals) == words.get(..literals)
            && best.is_none_or(|(count, _)| literals > count)
        {
            best = Some((literals, command.level));
        }
    }
    best.map_or(allowed_root, |(_, level)| level <= permission)
}

#[cfg(test)]
mod tests {
    use super::player_can_dispatch;

    #[test]
    fn legacy_player_commands_keep_smp_parsing_and_subcommand_permissions() {
        assert!(player_can_dispatch("pay Alex 2147483648", 0));
        assert!(player_can_dispatch("pay Alex", 0));
        assert!(player_can_dispatch("eco stats", 0));
        assert!(!player_can_dispatch(
            "eco give Alex 2147483648 event prize",
            0
        ));
        assert!(player_can_dispatch(
            "eco give Alex 2147483648 event prize",
            3
        ));
        assert!(!player_can_dispatch("say hello", 4));
    }
}
