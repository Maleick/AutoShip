use std::path::{Path, PathBuf};

use crate::config::VoiceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceArchetype {
    pub name: &'static str,
    pub file_name: &'static str,
}

impl VoiceArchetype {
    pub fn asset_path(&self) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("voices")
            .join(self.file_name)
    }
}

pub const STARTER_VOICE_ARCHETYPES: &[VoiceArchetype] = &[
    VoiceArchetype {
        name: "gruff_warrior",
        file_name: "gruff_warrior.wav",
    },
    VoiceArchetype {
        name: "sardonic_wizard",
        file_name: "sardonic_wizard.wav",
    },
    VoiceArchetype {
        name: "motherly_cleric",
        file_name: "motherly_cleric.wav",
    },
    VoiceArchetype {
        name: "methodical_paladin",
        file_name: "methodical_paladin.wav",
    },
    VoiceArchetype {
        name: "theatrical_bard",
        file_name: "theatrical_bard.wav",
    },
    VoiceArchetype {
        name: "terse_rogue",
        file_name: "terse_rogue.wav",
    },
    VoiceArchetype {
        name: "gentle_druid",
        file_name: "gentle_druid.wav",
    },
    VoiceArchetype {
        name: "dry_enchanter",
        file_name: "dry_enchanter.wav",
    },
];

#[derive(Debug, Clone)]
pub struct VoiceLibrary {
    home_dir: PathBuf,
}

impl VoiceLibrary {
    pub fn new(home_dir: impl Into<PathBuf>) -> Self {
        Self {
            home_dir: home_dir.into(),
        }
    }

    pub fn starter_archetypes(&self) -> &'static [VoiceArchetype] {
        STARTER_VOICE_ARCHETYPES
    }

    pub fn resolve_voice_clip(&self, voice_id: &VoiceId) -> Option<PathBuf> {
        let expanded = self.expand_home(voice_id.as_str());
        if expanded.exists() {
            return Some(expanded);
        }

        let file_name = Path::new(voice_id.as_str()).file_name()?.to_str()?;
        let archetype = STARTER_VOICE_ARCHETYPES
            .iter()
            .find(|candidate| candidate.file_name == file_name)?;
        let bundled = archetype.asset_path();
        if bundled.exists() {
            Some(bundled)
        } else {
            None
        }
    }

    fn expand_home(&self, raw: &str) -> PathBuf {
        if let Some(rest) = raw.strip_prefix("~/") {
            return self.home_dir.join(rest);
        }

        PathBuf::from(raw)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::{STARTER_VOICE_ARCHETYPES, VoiceLibrary};
    use crate::config::VoiceId;

    #[test]
    fn ships_eight_starter_voice_archetypes() {
        assert_eq!(STARTER_VOICE_ARCHETYPES.len(), 8);
        for archetype in STARTER_VOICE_ARCHETYPES {
            assert!(
                archetype.asset_path().exists(),
                "missing bundled starter voice: {}",
                archetype.asset_path().display()
            );
        }
    }

    #[test]
    fn operator_voice_clip_overrides_the_bundled_starter_pack() {
        let temp_home = TempDir::new().expect("temp home");
        let override_path = temp_home
            .path()
            .join(".textquest")
            .join("voices")
            .join("motherly_cleric.wav");
        fs::create_dir_all(override_path.parent().expect("override parent")).unwrap();
        fs::write(&override_path, b"override").unwrap();

        let library = VoiceLibrary::new(temp_home.path());
        let resolved = library
            .resolve_voice_clip(&VoiceId::new("~/.textquest/voices/motherly_cleric.wav"))
            .expect("resolved path");

        assert_eq!(resolved, override_path);
    }
}
