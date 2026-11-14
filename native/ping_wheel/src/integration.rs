//! Upstream: common/integration/ (1:1). ModContext, TeamContext, TeamContextHandler
//! and the VoiceChat/FTBTeams wrappers. The optional third-party APIs (Simple Voice
//! Chat, FTB Teams) are host surfaces, so the wrappers read them through the
//! integration service seam; the guard checks and Optional handling are exact.

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::global::log_info;
use crate::seams;

// ---------------------------------------------------------------------------
// ModContext.java
// ---------------------------------------------------------------------------

pub static HAS_DISTANT_HORIZONS: AtomicBool = AtomicBool::new(false);
pub static HAS_VOICE_CHAT: AtomicBool = AtomicBool::new(false);
pub static HAS_FTB_TEAMS: AtomicBool = AtomicBool::new(false);

/// ModContext.indexMods(): the three isModLoaded checks plus the Distant
/// Horizons API version log ("%s.%s.%s"). DhApi is a host API, so the version
/// triple comes from the integration seam.
pub fn index_mods() {
    let ctx = seams::context_service();

    HAS_DISTANT_HORIZONS.store(ctx.is_mod_loaded("distanthorizons"), Ordering::SeqCst);
    HAS_VOICE_CHAT.store(ctx.is_mod_loaded("voicechat"), Ordering::SeqCst);
    HAS_FTB_TEAMS.store(ctx.is_mod_loaded("ftbteams"), Ordering::SeqCst);

    if HAS_DISTANT_HORIZONS.load(Ordering::SeqCst) {
        log_info(&format!(
            "Distant Horizons API Version: {}",
            ctx.dh_api_version()
                .map(|(maj, min, pat)| format!("{maj}.{min}.{pat}"))
                .unwrap_or_else(|| "0.0.0".to_string()),
        ));
    }
}

pub fn has_distant_horizons() -> bool {
    HAS_DISTANT_HORIZONS.load(Ordering::SeqCst)
}

pub fn has_voice_chat() -> bool {
    HAS_VOICE_CHAT.load(Ordering::SeqCst)
}

pub fn has_ftb_teams() -> bool {
    HAS_FTB_TEAMS.load(Ordering::SeqCst)
}

// ---------------------------------------------------------------------------
// TeamContext.java: toString() is super.toString().toLowerCase().
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TeamContext {
    None,
    VanillaTeam,
    FtbTeams,
    VoiceChat,
}

impl TeamContext {
    pub fn to_string(self) -> String {
        match self {
            TeamContext::None => "none".to_string(),
            TeamContext::VanillaTeam => "vanilla_team".to_string(),
            TeamContext::FtbTeams => "ftb_teams".to_string(),
            TeamContext::VoiceChat => "voice_chat".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// VoiceChatWrapper.java / FTBTeamsWrapper.java. Both return Optional.empty()
// when the mod is absent, the API is null, the connection/group is null, or
// the FTB team is a player team; those conditions live in the seam's answers.
// ---------------------------------------------------------------------------

/// VoiceChatWrapper.getGroupId(player): Optional&lt;UUID&gt;.
pub fn voice_chat_group_id(player_uuid: u128) -> Option<u128> {
    if !has_voice_chat() {
        return None;
    }
    seams::integration_service().voice_chat_group_id(player_uuid)
}

/// VoiceChatWrapper.getSelfGroupId(): Optional&lt;UUID&gt;.
pub fn voice_chat_self_group_id() -> Option<u128> {
    if !has_voice_chat() {
        return None;
    }
    seams::integration_service().voice_chat_self_group_id()
}

/// FTBTeamsWrapper.getTeamId(player): empty when the manager is not loaded,
/// the team is missing, or the team is a player team.
pub fn ftb_team_id(player_uuid: u128) -> Option<u128> {
    if !has_ftb_teams() {
        return None;
    }
    seams::integration_service().ftb_team_id(player_uuid)
}

/// FTBTeamsWrapper.getSelfTeamId().
pub fn ftb_self_team_id() -> Option<u128> {
    if !has_ftb_teams() {
        return None;
    }
    seams::integration_service().ftb_self_team_id()
}

// ---------------------------------------------------------------------------
// TeamContextHandler.java
// ---------------------------------------------------------------------------

/// hasTeam(player): player == null -> false; getContext(player) != NONE.
pub fn has_team(player_uuid: Option<u128>) -> bool {
    let Some(player_uuid) = player_uuid else {
        return false;
    };

    get_context(player_uuid) != TeamContext::None
}

/// getContext(player): voice chat group first, then FTB team, then the
/// player's vanilla team.
pub fn get_context(player_uuid: u128) -> TeamContext {
    if voice_chat_group_id(player_uuid).is_some() {
        return TeamContext::VoiceChat;
    }

    if ftb_team_id(player_uuid).is_some() {
        return TeamContext::FtbTeams;
    }

    if seams::integration_service().player_team_present(player_uuid) {
        return TeamContext::VanillaTeam;
    }

    TeamContext::None
}

/// getSelfContext(): the same ladder against the local player.
pub fn get_self_context() -> TeamContext {
    if voice_chat_self_group_id().is_some() {
        return TeamContext::VoiceChat;
    }

    if ftb_self_team_id().is_some() {
        return TeamContext::FtbTeams;
    }

    if let Some(player) = seams::game().and_then(|g| g.player()) {
        if player.team_present() {
            return TeamContext::VanillaTeam;
        }
    }

    TeamContext::None
}

/// inSameContext(p1, p2): the first non-empty of (voice chat, FTB) ids on each
/// side is compared (p1 id with p2 id; a null p2 id fails equals); when only p2
/// has an id the contexts differ; otherwise the vanilla teams are compared.
pub fn in_same_context(p1: u128, p2: u128) -> bool {
    let svc = seams::integration_service();
    let p1_team_id = voice_chat_group_id(p1).or_else(|| ftb_team_id(p1));
    let p2_team_id = voice_chat_group_id(p2).or_else(|| ftb_team_id(p2));

    if let Some(p1_team_id) = p1_team_id {
        return Some(p1_team_id) == p2_team_id;
    }
    if let Some(_) = p2_team_id {
        return false;
    }

    let p1_team = svc.player_team_uuid(p1);
    let p2_team = svc.player_team_uuid(p2);
    p1_team == p2_team
}

