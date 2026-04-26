//! Shared command metadata for TUI help, hints, completion, and suggestions.

/// Major help/reference sections in the help overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HelpSection {
    Workflows,
    Targeting,
    Combat,
    Navigation,
    ChChain,
    Lifecycle,
    Troubleshooting,
}

/// Shared metadata for one canonical command or subcommand phrase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandEntry {
    /// Canonical command phrase shown in help and suggestions.
    pub phrase: &'static str,
    /// Accepted aliases for this exact phrase.
    pub aliases: &'static [&'static str],
    /// Help section used by the overlay and command jumps.
    pub section: HelpSection,
    /// Usage text shown inline and in help.
    pub usage: &'static str,
    /// Short operator-facing summary.
    pub summary: &'static str,
    /// One concrete example shown in hints/errors.
    pub example: &'static str,
}

/// Result of a command hint lookup for the current buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandHintMatch {
    pub canonical: &'static str,
    pub usage: &'static str,
    pub example: &'static str,
}

/// Result of a did-you-mean lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSuggestion {
    pub phrase: &'static str,
    pub alias: Option<&'static str>,
}

pub const COMMAND_ENTRIES: &[CommandEntry] = &[
    CommandEntry {
        phrase: "help",
        aliases: &["h"],
        section: HelpSection::Lifecycle,
        usage: "help [command]",
        summary: "Open the help overlay or jump to one command section.",
        example: "help nav",
    },
    CommandEntry {
        phrase: "commands",
        aliases: &["cmds"],
        section: HelpSection::Lifecycle,
        usage: "commands",
        summary: "Jump straight to the command reference inside help.",
        example: "commands",
    },
    CommandEntry {
        phrase: "status",
        aliases: &["s"],
        section: HelpSection::Lifecycle,
        usage: "status [overview]",
        summary: "Show connected-client counts or a compact runtime summary.",
        example: "status overview",
    },
    CommandEntry {
        phrase: "status overview",
        aliases: &["overview"],
        section: HelpSection::Lifecycle,
        usage: "status overview",
        summary: "Show zone, mode, screen, and other live operator context.",
        example: "status overview",
    },
    CommandEntry {
        phrase: "mode",
        aliases: &[],
        section: HelpSection::Combat,
        usage: "mode <camp|hunt>",
        summary: "Switch the operating mode for live control.",
        example: "mode hunt",
    },
    CommandEntry {
        phrase: "ma",
        aliases: &["assist"],
        section: HelpSection::Combat,
        usage: "ma [character_name]",
        summary: "Show or set Main Assist and send /assist when setting it.",
        example: "ma Warrior",
    },
    CommandEntry {
        phrase: "mt",
        aliases: &["tank"],
        section: HelpSection::Combat,
        usage: "mt [character_name]",
        summary: "Show or set Main Tank.",
        example: "mt Paladin",
    },
    CommandEntry {
        phrase: "combat",
        aliases: &["fight"],
        section: HelpSection::Combat,
        usage: "combat [status|scope]",
        summary: "Show a scoped combat summary for the current focus.",
        example: "combat status",
    },
    CommandEntry {
        phrase: "engage",
        aliases: &["pull"],
        section: HelpSection::Combat,
        usage: "engage [target_id]",
        summary: "Engage combat for the focused scope.",
        example: "engage 3472",
    },
    CommandEntry {
        phrase: "disengage",
        aliases: &[],
        section: HelpSection::Combat,
        usage: "disengage",
        summary: "Stop combat for the focused scope.",
        example: "disengage",
    },
    CommandEntry {
        phrase: "loot",
        aliases: &[],
        section: HelpSection::Combat,
        usage: "loot",
        summary: "Loot nearby corpses for the focused scope.",
        example: "loot",
    },
    CommandEntry {
        phrase: "door",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "door",
        summary: "Target and open the nearest door or switch (MQ2 /click door equivalent).",
        example: "door",
    },
    CommandEntry {
        phrase: "click",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "click [door|item]",
        summary: "Click nearest ground item (default) or door. Equivalent to MQ2 /click.",
        example: "click door",
    },
    CommandEntry {
        phrase: "heal",
        aliases: &[],
        section: HelpSection::Combat,
        usage: "heal cancel",
        summary: "Toggle healer quality-of-life options like heal-cancel.",
        example: "heal cancel",
    },
    CommandEntry {
        phrase: "heal cancel",
        aliases: &[],
        section: HelpSection::Combat,
        usage: "heal cancel",
        summary: "Toggle heal-cancel optimization.",
        example: "heal cancel",
    },
    CommandEntry {
        phrase: "invite",
        aliases: &[],
        section: HelpSection::Combat,
        usage: "invite <character_name>",
        summary: "Send a group invite from the active client.",
        example: "invite Cleric01",
    },
    CommandEntry {
        phrase: "accept",
        aliases: &[],
        section: HelpSection::Combat,
        usage: "accept",
        summary: "Accept a pending group invite on the active client.",
        example: "accept",
    },
    CommandEntry {
        phrase: "all",
        aliases: &[],
        section: HelpSection::Targeting,
        usage: "all /<slash command>",
        summary: "Broadcast a slash command to every connected client.",
        example: "all /sit",
    },
    CommandEntry {
        phrase: "bc",
        aliases: &[],
        section: HelpSection::Targeting,
        usage: "bc /<slash command>",
        summary: "Relay a slash command to local clients and connected local box-chat peers.",
        example: "bc /assist MainTank",
    },
    CommandEntry {
        phrase: "bca",
        aliases: &[],
        section: HelpSection::Targeting,
        usage: "bca //<slash command>",
        summary: "MQ2EQBC-style alias for a relay command using double-slash syntax.",
        example: "bca //follow MainTank",
    },
    CommandEntry {
        phrase: "bcaa",
        aliases: &[],
        section: HelpSection::Targeting,
        usage: "bcaa //<slash command>",
        summary: "MQ2EQBC-style raid-wide alias using the same network relay path.",
        example: "bcaa //sit",
    },
    CommandEntry {
        phrase: "bct",
        aliases: &[],
        section: HelpSection::Targeting,
        usage: "bct <character_name> //<slash command>",
        summary: "Relay a slash command to one named local or remote character.",
        example: "bct Cleric01 //cast 1",
    },
    CommandEntry {
        phrase: "scope",
        aliases: &[],
        section: HelpSection::Workflows,
        usage: "scope [all|G<n>|<character_name>]",
        summary: "Show or change the active routing scope (all, group, or one toon).",
        example: "scope G2",
    },
    CommandEntry {
        phrase: "session",
        aliases: &[],
        section: HelpSection::Lifecycle,
        usage: "session [list|status]",
        summary: "Show session slot lifecycle states and health.",
        example: "session list",
    },
    CommandEntry {
        phrase: "session list",
        aliases: &[],
        section: HelpSection::Lifecycle,
        usage: "session list",
        summary: "List all session slots with lifecycle state and hook status.",
        example: "session list",
    },
    CommandEntry {
        phrase: "session status",
        aliases: &[],
        section: HelpSection::Lifecycle,
        usage: "session status",
        summary: "Show a compact summary of live, recovering, and blocked slots.",
        example: "session status",
    },
    CommandEntry {
        phrase: "nav",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "nav <camp_name|x y z|zone|reload>",
        summary: "Navigate the focused scope or reload the active zone navmesh.",
        example: "nav gfay",
    },
    CommandEntry {
        phrase: "find",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "find <poi> [zone]",
        summary: "Route to a named point-of-interest in the current or target zone.",
        example: "find banker ro",
    },
    CommandEntry {
        phrase: "nav ui",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "nav ui",
        summary: "Toggle the nav debug diagnostics overlay on the Navigation screen.",
        example: "nav ui",
    },
    CommandEntry {
        phrase: "mapfilter",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "mapfilter <npc|pc|corpse|ground|pet|named|untargetable> [on|off]",
        summary: "Toggle map visibility for MQ2Map-style categories.",
        example: "mapfilter npc off",
    },
    CommandEntry {
        phrase: "track",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "track <spawn_name>",
        summary: "Track a spawn on the map and tactical panels.",
        example: "track \"Fippy Darkpaw\"",
    },
    CommandEntry {
        phrase: "track list",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "track list",
        summary: "List tracked spawns and their current status.",
        example: "track list",
    },
    CommandEntry {
        phrase: "untrack",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "untrack <spawn_name>",
        summary: "Stop tracking a spawn.",
        example: "untrack \"Fippy Darkpaw\"",
    },
    CommandEntry {
        phrase: "mapclick",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "mapclick [none|loc|nav]",
        summary: "Set map Enter key action.",
        example: "mapclick loc",
    },
    CommandEntry {
        phrase: "maploc",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "maploc [recall|clear]",
        summary: "Place/recall/clear location marker.",
        example: "maploc recall",
    },
    CommandEntry {
        phrase: "mapmarker",
        aliases: &["mm"],
        section: HelpSection::Navigation,
        usage: "mapmarker <set|recall|clear|list|save|load> [name] [x y [z]]",
        summary: "Persistent named map markers — place, recall, clear, list, and persist.",
        example: "mapmarker set camp",
    },
    CommandEntry {
        phrase: "mapshow",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "mapshow <preset>",
        summary: "Apply a visibility preset.",
        example: "mapshow tactical",
    },
    CommandEntry {
        phrase: "maphide",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "maphide <layer>",
        summary: "Hide a map layer.",
        example: "maphide labels",
    },
    CommandEntry {
        phrase: "mapnames",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "mapnames [style]",
        summary: "Set spawn label style.",
        example: "mapnames namelevel",
    },
    CommandEntry {
        phrase: "map",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "map <layer> [on|off]",
        summary: "Toggle map layer.",
        example: "map spawns off",
    },
    CommandEntry {
        phrase: "highlight",
        aliases: &["hl"],
        section: HelpSection::Navigation,
        usage: "highlight <sub> [opts]",
        summary: "Manage spawn highlights.",
        example: "highlight add Fippy color=red",
    },
    CommandEntry {
        phrase: "mapfilter castradius",
        aliases: &["mapfilter cr"],
        section: HelpSection::Navigation,
        usage: "mapfilter castradius <r> [color]",
        summary: "Cast radius circle overlay.",
        example: "mapfilter castradius 200 cyan",
    },
    CommandEntry {
        phrase: "mapfilter spellradius",
        aliases: &["mapfilter sr"],
        section: HelpSection::Navigation,
        usage: "mapfilter spellradius <r> [color]",
        summary: "Spell radius circle overlay.",
        example: "mapfilter spellradius 150",
    },
    CommandEntry {
        phrase: "mapfilter targetpath",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "mapfilter targetpath [on|off]",
        summary: "Toggle target path overlay.",
        example: "mapfilter targetpath off",
    },
    CommandEntry {
        phrase: "mapfilter targetline",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "mapfilter targetline [on|off]",
        summary: "Toggle target line overlay.",
        example: "mapfilter targetline off",
    },
    CommandEntry {
        phrase: "mapfilter save",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "mapfilter save <name>",
        summary: "Save filter preset.",
        example: "mapfilter save hunting",
    },
    CommandEntry {
        phrase: "mapfilter load",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "mapfilter load <name>",
        summary: "Load filter preset.",
        example: "mapfilter load hunting",
    },
    CommandEntry {
        phrase: "mapfilter delete",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "mapfilter delete <name>",
        summary: "Delete filter preset.",
        example: "mapfilter delete hunting",
    },
    CommandEntry {
        phrase: "mapfilter list",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "mapfilter list",
        summary: "List filter presets.",
        example: "mapfilter list",
    },
    CommandEntry {
        phrase: "watch",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "watch <pattern> | watch list | watch named [on|off]",
        summary: "Add a spawn watch pattern or toggle named alerts.",
        example: "watch *moss*",
    },
    CommandEntry {
        phrase: "watch list",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "watch list",
        summary: "List active spawn watch patterns.",
        example: "watch list",
    },
    CommandEntry {
        phrase: "unwatch",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "unwatch <pattern>",
        summary: "Remove a spawn watch pattern.",
        example: "unwatch *moss*",
    },
    CommandEntry {
        phrase: "alerts",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "alerts [clear|count]",
        summary: "Show recent spawn alerts, clear the feed, or show count.",
        example: "alerts",
    },
    CommandEntry {
        phrase: "pf",
        aliases: &["playerfilter", "player_filter"],
        section: HelpSection::Navigation,
        usage: "pf [all|strangers|friends]",
        summary: "Set player zone notification filter (all/strangers/friends).",
        example: "pf friends",
    },
    CommandEntry {
        phrase: "sound",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "sound [on|off]",
        summary: "Toggle terminal bell sound on player zone-in.",
        example: "sound on",
    },
    CommandEntry {
        phrase: "friends",
        aliases: &["friend"],
        section: HelpSection::Navigation,
        usage: "friends [add|remove|list|clear] [name]",
        summary: "Manage friends list for player notification filtering.",
        example: "friends add Guildie",
    },
    CommandEntry {
        phrase: "camp",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "camp <start|stop|status|list|add|remove|next|prev> [name]",
        summary: "Manage saved camps and the active camp loop.",
        example: "camp start orc",
    },
    CommandEntry {
        phrase: "camp start",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "camp start <name>",
        summary: "Load a saved camp config and start the camp loop.",
        example: "camp start orc",
    },
    CommandEntry {
        phrase: "camp add",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "camp add <name>",
        summary: "Save the current position as a named camp.",
        example: "camp add frenzy",
    },
    CommandEntry {
        phrase: "camp remove",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "camp remove <name>",
        summary: "Delete a saved camp config.",
        example: "camp remove frenzy",
    },
    CommandEntry {
        phrase: "camp list",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "camp list",
        summary: "List saved camp configs.",
        example: "camp list",
    },
    CommandEntry {
        phrase: "camp next",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "camp next",
        summary: "Advance to the next linked camp.",
        example: "camp next",
    },
    CommandEntry {
        phrase: "camp prev",
        aliases: &[],
        section: HelpSection::Navigation,
        usage: "camp prev",
        summary: "Fall back to the previous linked camp.",
        example: "camp prev",
    },
    CommandEntry {
        phrase: "ch",
        aliases: &[],
        section: HelpSection::ChChain,
        usage: "ch <start|stop|add|rm|interval|adaptive|status> ...",
        summary: "Control the Complete Heal chain.",
        example: "ch status",
    },
    CommandEntry {
        phrase: "ch status",
        aliases: &[],
        section: HelpSection::ChChain,
        usage: "ch status",
        summary: "Show the current CH chain state.",
        example: "ch status",
    },
    CommandEntry {
        phrase: "ch start",
        aliases: &[],
        section: HelpSection::ChChain,
        usage: "ch start <pid1,pid2,...> <interval_secs> <target_id> [spell_slot]",
        summary: "Start a CH chain for the given cleric PIDs.",
        example: "ch start 1001,1002 2.5 3472 1",
    },
    CommandEntry {
        phrase: "ch stop",
        aliases: &[],
        section: HelpSection::ChChain,
        usage: "ch stop",
        summary: "Stop the running CH chain.",
        example: "ch stop",
    },
    CommandEntry {
        phrase: "ch add",
        aliases: &[],
        section: HelpSection::ChChain,
        usage: "ch add <pid>",
        summary: "Add one cleric PID to the running CH chain.",
        example: "ch add 1003",
    },
    CommandEntry {
        phrase: "ch remove",
        aliases: &["ch rm"],
        section: HelpSection::ChChain,
        usage: "ch remove <pid>",
        summary: "Remove one cleric PID from the running CH chain.",
        example: "ch remove 1003",
    },
    CommandEntry {
        phrase: "ch interval",
        aliases: &[],
        section: HelpSection::ChChain,
        usage: "ch interval <seconds>",
        summary: "Set the delay between CH casts.",
        example: "ch interval 2.5",
    },
    CommandEntry {
        phrase: "ch adaptive",
        aliases: &[],
        section: HelpSection::ChChain,
        usage: "ch adaptive <on|off>",
        summary: "Toggle adaptive CH timing.",
        example: "ch adaptive on",
    },
    CommandEntry {
        phrase: "chui",
        aliases: &[],
        section: HelpSection::ChChain,
        usage: "chui [open|close|toggle|status]",
        summary: "Open, close, or inspect the CH chain panel.",
        example: "chui open",
    },
    CommandEntry {
        phrase: "login",
        aliases: &["launch"],
        section: HelpSection::Lifecycle,
        usage: "login [all|G<n>|name]",
        summary: "Show account status or queue launches.",
        example: "login G2",
    },
    CommandEntry {
        phrase: "login all",
        aliases: &["launch all"],
        section: HelpSection::Lifecycle,
        usage: "login all",
        summary: "Queue launches for every configured account.",
        example: "login all",
    },
    CommandEntry {
        phrase: "profile",
        aliases: &[],
        section: HelpSection::Lifecycle,
        usage: "profile [list|launch <name>]",
        summary: "Manage and launch named MQ2-style profile groups.",
        example: "profile launch MainRaid",
    },
    CommandEntry {
        phrase: "profile list",
        aliases: &[],
        section: HelpSection::Lifecycle,
        usage: "profile list",
        summary: "List all configured profile groups and their hotkeys.",
        example: "profile list",
    },
    CommandEntry {
        phrase: "profile launch",
        aliases: &[],
        section: HelpSection::Lifecycle,
        usage: "profile launch <name>",
        summary: "Queue all accounts in the named profile group for launch.",
        example: "profile launch MainRaid",
    },
    CommandEntry {
        phrase: "stop",
        aliases: &[],
        section: HelpSection::Lifecycle,
        usage: "stop <name|all>",
        summary: "Eject clients without relaunching them.",
        example: "stop all",
    },
    CommandEntry {
        phrase: "restart",
        aliases: &[],
        section: HelpSection::Lifecycle,
        usage: "restart <name|all>",
        summary: "Eject clients, then queue a relaunch.",
        example: "restart Dmft01",
    },
    CommandEntry {
        phrase: "theme",
        aliases: &[],
        section: HelpSection::Lifecycle,
        usage: "theme [list|<theme_name>]",
        summary: "Switch color theme, list available themes, or cycle to next.",
        example: "theme dark",
    },
    CommandEntry {
        phrase: "theme list",
        aliases: &[],
        section: HelpSection::Lifecycle,
        usage: "theme list",
        summary: "Display all available color themes.",
        example: "theme list",
    },
    CommandEntry {
        phrase: "privacy",
        aliases: &[],
        section: HelpSection::Lifecycle,
        usage: "privacy",
        summary: "Toggle privacy mode for names and server.",
        example: "privacy",
    },
    CommandEntry {
        phrase: "inject",
        aliases: &[],
        section: HelpSection::Lifecycle,
        usage: "inject",
        summary: "Request DLL injection for the current client.",
        example: "inject",
    },
    CommandEntry {
        phrase: "quit",
        aliases: &["q"],
        section: HelpSection::Lifecycle,
        usage: "quit",
        summary: "Exit the TextQuest TUI immediately.",
        example: "quit",
    },
    CommandEntry {
        phrase: "addr",
        aliases: &[],
        section: HelpSection::Troubleshooting,
        usage: "addr <hex_address|global[+offset]>",
        summary: "Set the Debug panel hex dump address and trigger an immediate memory poll.",
        example: "addr pinstLocalPlayer+0x78",
    },
    CommandEntry {
        phrase: "camera",
        aliases: &["cam"],
        section: HelpSection::Navigation,
        usage: "camera [preset|<distance>|list]",
        summary: "Apply a camera preset or set camera distance.",
        example: "camera Far",
    },
    CommandEntry {
        phrase: "camera list",
        aliases: &["cam list"],
        section: HelpSection::Navigation,
        usage: "camera list",
        summary: "List all configured camera presets and hotkeys.",
        example: "camera list",
    },
];

/// Return the full shared command metadata.
#[must_use]
pub fn command_entries() -> &'static [CommandEntry] {
    COMMAND_ENTRIES
}

/// Find metadata for an exact canonical command or alias phrase.
#[must_use]
pub fn command_entry(input: &str) -> Option<&'static CommandEntry> {
    let normalized = input.trim().to_ascii_lowercase();
    COMMAND_ENTRIES.iter().find(|entry| {
        entry.phrase.eq_ignore_ascii_case(&normalized)
            || entry
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(&normalized))
    })
}

/// Normalize a typed command by expanding exact aliases and first-token
/// aliases.
#[must_use]
pub fn normalize_command_alias(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if let Some(entry) = command_entry(trimmed) {
        return entry.phrase.to_string();
    }

    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let first = parts.next().unwrap_or_default();
    let rest = parts.next().map(str::trim_start);
    if let Some(entry) = COMMAND_ENTRIES.iter().find(|entry| {
        entry.phrase.split(' ').count() == 1
            && entry
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(first))
    }) {
        if let Some(rest) = rest {
            format!("{} {}", entry.phrase, rest)
        } else {
            entry.phrase.to_string()
        }
    } else {
        trimmed.to_string()
    }
}

/// Find the best hint for the current command buffer.
#[must_use]
pub fn find_command_hint(input: &str) -> Option<CommandHintMatch> {
    let normalized = normalize_command_alias(input);
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return None;
    }

    COMMAND_ENTRIES
        .iter()
        .filter(|entry| {
            trimmed.starts_with(entry.phrase)
                && (trimmed.len() == entry.phrase.len()
                    || trimmed.as_bytes().get(entry.phrase.len()) == Some(&b' '))
        })
        .max_by_key(|entry| entry.phrase.len())
        .map(|entry| CommandHintMatch {
            canonical: entry.phrase,
            usage: entry.usage,
            example: entry.example,
        })
}

/// Get one command's primary help/reference section.
#[must_use]
pub fn help_section_for_command(input: &str) -> Option<HelpSection> {
    let normalized = normalize_command_alias(input);
    COMMAND_ENTRIES
        .iter()
        .find(|entry| {
            normalized == entry.phrase
                || normalized.starts_with(entry.phrase)
                    && normalized
                        .as_bytes()
                        .get(entry.phrase.len())
                        .is_some_and(|b| *b == b' ')
        })
        .map(|entry| entry.section)
}

/// Get completion candidates for top-level command names plus aliases.
#[must_use]
pub fn top_level_completion_candidates() -> Vec<String> {
    let mut candidates = Vec::new();
    for entry in COMMAND_ENTRIES
        .iter()
        .filter(|entry| !entry.phrase.contains(' '))
    {
        candidates.push(entry.phrase.to_string());
        for alias in entry.aliases {
            candidates.push((*alias).to_string());
        }
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();
    let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

    for (i, row) in matrix.iter_mut().enumerate().take(a_len + 1) {
        row[0] = i;
    }
    for (j, cell) in matrix[0].iter_mut().enumerate().take(b_len + 1) {
        *cell = j;
    }

    for (i, ca) in a.chars().enumerate() {
        for (j, cb) in b.chars().enumerate() {
            let cost = usize::from(ca != cb);
            matrix[i + 1][j + 1] = (matrix[i][j + 1] + 1)
                .min(matrix[i + 1][j] + 1)
                .min(matrix[i][j] + cost);
        }
    }

    matrix[a_len][b_len]
}

/// Find the closest matching command phrase or alias.
#[must_use]
pub fn did_you_mean(input: &str) -> Option<CommandSuggestion> {
    let trimmed = input.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return None;
    }

    let mut exact_prefix: Option<CommandSuggestion> = None;
    for entry in COMMAND_ENTRIES {
        if entry.phrase.starts_with(&trimmed) {
            return Some(CommandSuggestion {
                phrase: entry.phrase,
                alias: None,
            });
        }
        if let Some(alias) = entry
            .aliases
            .iter()
            .find(|alias| alias.starts_with(&trimmed))
        {
            exact_prefix = Some(CommandSuggestion {
                phrase: entry.phrase,
                alias: Some(*alias),
            });
        }
    }
    if exact_prefix.is_some() {
        return exact_prefix;
    }

    let mut best: Option<(usize, CommandSuggestion)> = None;
    for entry in COMMAND_ENTRIES {
        for candidate in std::iter::once((entry.phrase, None))
            .chain(entry.aliases.iter().map(|alias| (*alias, Some(*alias))))
        {
            let dist = edit_distance(&trimmed, candidate.0);
            let max_dist = if candidate.0.len() > 8 { 4 } else { 3 };
            if dist <= max_dist && best.as_ref().is_none_or(|(best_dist, _)| dist < *best_dist) {
                best = Some((
                    dist,
                    CommandSuggestion {
                        phrase: entry.phrase,
                        alias: candidate.1,
                    },
                ));
            }
        }
    }

    best.map(|(_, suggestion)| suggestion)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_aliases_exact_and_prefixed() {
        assert_eq!(normalize_command_alias("h"), "help");
        assert_eq!(normalize_command_alias("cmds"), "commands");
        assert_eq!(normalize_command_alias("launch all"), "login all");
        assert_eq!(normalize_command_alias("launch Dmft01"), "login Dmft01");
        assert_eq!(normalize_command_alias("overview"), "status overview");
        assert_eq!(normalize_command_alias("assist Warrior"), "ma Warrior");
        assert_eq!(normalize_command_alias("tank Bob"), "mt Bob");
        assert_eq!(normalize_command_alias("pull 1234"), "engage 1234");
        assert_eq!(normalize_command_alias("combat status"), "combat status");
    }

    #[test]
    fn hints_prefer_longest_match() {
        let hint = find_command_hint("ch adaptive on").expect("hint");
        assert_eq!(hint.canonical, "ch adaptive");
        assert_eq!(hint.usage, "ch adaptive <on|off>");
    }

    #[test]
    fn help_section_lookup_handles_alias_phrase() {
        assert_eq!(
            help_section_for_command("launch all"),
            Some(HelpSection::Lifecycle)
        );
        assert_eq!(
            help_section_for_command("ch start"),
            Some(HelpSection::ChChain)
        );
    }

    #[test]
    fn did_you_mean_prefers_canonical_phrase_with_alias_note() {
        let suggestion = did_you_mean("lauch all").expect("suggestion");
        assert_eq!(suggestion.phrase, "login all");
        assert_eq!(suggestion.alias, Some("launch all"));
    }

    #[test]
    fn top_level_completion_contains_aliases_once() {
        let candidates = top_level_completion_candidates();
        assert!(candidates.contains(&String::from("help")));
        assert!(candidates.contains(&String::from("h")));
        assert!(candidates.contains(&String::from("login")));
        assert!(candidates.contains(&String::from("launch")));
        assert!(candidates.contains(&String::from("find")));
        assert!(candidates.contains(&String::from("assist")));
        assert!(candidates.contains(&String::from("tank")));
        assert!(candidates.contains(&String::from("pull")));
    }
}
