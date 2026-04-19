//! Configurable audio alert system for terminal UI event notifications.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use rodio::{Decoder, OutputStream, Sink};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertType {
    LowMana,
    CharacterDead,
    LootDrop,
    GroupWipe,
    NamedSpawn,
    StuckDetection,
    CampPhaseChange,
    CharmBreak,
    SlowCast,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioAlert {
    pub alert_type: AlertType,
    pub sound_file: PathBuf,
    pub volume: f32,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct AudioConfig {
    alerts: Vec<AudioAlert>,
}

#[derive(Debug)]
pub struct AudioPlayer {
    pub alerts: HashMap<AlertType, AudioAlert>,
    pub config_path: PathBuf,
    pub active_channels: HashMap<AlertType, usize>,

    #[cfg(windows)]
    output: Option<(OutputStream, rodio::OutputStreamHandle)>,
}

impl AudioPlayer {
    pub fn new(config_path: &Path) -> Result<Self> {
        let mut player = Self {
            alerts: Self::default_alerts(),
            config_path: config_path.to_path_buf(),
            active_channels: HashMap::new(),
            #[cfg(windows)]
            output: OutputStream::try_default().map_or_else(
                |error| {
                    tracing::warn!(%error, "No audio output device available");
                    None
                },
                |pair| Some(pair),
            ),
        };

        player.load_config()?;
        Ok(player)
    }

    pub fn play_alert(&mut self, alert_type: AlertType) -> Result<()> {
        let Some(alert) = self.alerts.get(&alert_type) else {
            tracing::debug!(?alert_type, "No matching audio alert configured");
            return Ok(());
        };

        if !alert.enabled {
            tracing::debug!(?alert_type, "Audio alert disabled");
            return Ok(());
        }

        self.active_channels
            .insert(alert_type.clone(), 1);

        let result = self.play_alert_impl(&alert_type);
        match result {
            Ok(_) => {
                self.active_channels.remove(&alert_type);
                Ok(())
            }
            Err(error) => {
                self.active_channels.remove(&alert_type);
                Err(error)
            }
        }
    }

    pub fn set_volume(&mut self, alert_type: AlertType, volume: f32) {
        if let Some(alert) = self.alerts.get_mut(&alert_type) {
            alert.volume = volume.clamp(0.0, 1.0);
        }
    }

    pub fn enable_alert(&mut self, alert_type: AlertType, enabled: bool) {
        if let Some(alert) = self.alerts.get_mut(&alert_type) {
            alert.enabled = enabled;
        }
    }

    pub fn load_config(&mut self) -> Result<()> {
        let mut contents = String::new();

        let read_result = fs::read_to_string(&self.config_path);
        match read_result {
            Ok(contents_data) => {
                contents = contents_data;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.save_config()?;
                return Ok(());
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "Failed to read audio config at {}",
                        self.config_path.display()
                    )
                });
            }
        }

        let config = toml::from_str::<AudioConfig>(&contents).context("Failed to parse audio config")?;
        self.alerts = config
            .alerts
            .into_iter()
            .map(|mut alert| {
                alert.volume = alert.volume.clamp(0.0, 1.0);
                (alert.alert_type.clone(), alert)
            })
            .collect();

        Ok(())
    }

    pub fn save_config(&self) -> Result<()> {
        let config = AudioConfig {
            alerts: self.alerts.values().cloned().collect(),
        };
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory {}", parent.display()))?;
        }
        let serialized =
            toml::to_string_pretty(&config).context("Failed to serialize audio config")?;
        fs::write(&self.config_path, serialized).with_context(|| {
            format!(
                "Failed to write audio config to {}",
                self.config_path.display()
            )
        })?;
        Ok(())
    }

    fn default_alerts() -> HashMap<AlertType, AudioAlert> {
        let mut alerts = HashMap::new();

        alerts.insert(
            AlertType::LowMana,
            AudioAlert {
                alert_type: AlertType::LowMana,
                sound_file: PathBuf::from("audio/low_mana.wav"),
                volume: 0.75,
                enabled: true,
            },
        );
        alerts.insert(
            AlertType::CharacterDead,
            AudioAlert {
                alert_type: AlertType::CharacterDead,
                sound_file: PathBuf::from("audio/character_dead.wav"),
                volume: 0.85,
                enabled: true,
            },
        );
        alerts.insert(
            AlertType::LootDrop,
            AudioAlert {
                alert_type: AlertType::LootDrop,
                sound_file: PathBuf::from("audio/loot_drop.wav"),
                volume: 0.60,
                enabled: true,
            },
        );
        alerts.insert(
            AlertType::GroupWipe,
            AudioAlert {
                alert_type: AlertType::GroupWipe,
                sound_file: PathBuf::from("audio/group_wipe.wav"),
                volume: 0.80,
                enabled: true,
            },
        );
        alerts.insert(
            AlertType::NamedSpawn,
            AudioAlert {
                alert_type: AlertType::NamedSpawn,
                sound_file: PathBuf::from("audio/named_spawn.wav"),
                volume: 0.80,
                enabled: true,
            },
        );
        alerts.insert(
            AlertType::StuckDetection,
            AudioAlert {
                alert_type: AlertType::StuckDetection,
                sound_file: PathBuf::from("audio/stuck_detection.wav"),
                volume: 0.80,
                enabled: false,
            },
        );
        alerts.insert(
            AlertType::CampPhaseChange,
            AudioAlert {
                alert_type: AlertType::CampPhaseChange,
                sound_file: PathBuf::from("audio/camp_phase_change.wav"),
                volume: 0.70,
                enabled: true,
            },
        );
        alerts.insert(
            AlertType::CharmBreak,
            AudioAlert {
                alert_type: AlertType::CharmBreak,
                sound_file: PathBuf::from("audio/charm_break.wav"),
                volume: 0.70,
                enabled: true,
            },
        );
        alerts.insert(
            AlertType::SlowCast,
            AudioAlert {
                alert_type: AlertType::SlowCast,
                sound_file: PathBuf::from("audio/slow_cast.wav"),
                volume: 0.75,
                enabled: true,
            },
        );
        alerts.insert(
            AlertType::Custom("general".to_string()),
            AudioAlert {
                alert_type: AlertType::Custom("general".to_string()),
                sound_file: PathBuf::from("audio/custom.wav"),
                volume: 0.75,
                enabled: true,
            },
        );

        alerts
    }

    #[cfg(windows)]
    fn play_alert_impl(&mut self, alert_type: &AlertType) -> Result<()> {
        let alert = self
            .alerts
            .get(alert_type)
            .with_context(|| format!("Missing alert configuration for {:?}", alert_type))?;

        let Some((_stream, stream_handle)) = self.output.as_ref() else {
            tracing::warn!("No audio output stream available; skipping alert");
            return Ok(());
        };

        let file = fs::File::open(&alert.sound_file).with_context(|| {
            format!(
                "Failed to open audio file {}",
                alert.sound_file.display()
            )
        })?;
        let reader = BufReader::new(file);
        let source = Decoder::new(reader).context("Failed to decode audio alert")?;

        let sink = Sink::try_new(stream_handle).context("Failed to create audio sink")?;
        sink.set_volume(alert.volume.clamp(0.0, 1.0));
        sink.append(source);
        sink.detach();
        Ok(())
    }

    #[cfg(not(windows))]
    fn play_alert_impl(&mut self, alert_type: &AlertType) -> Result<()> {
        tracing::info!("play_alert: {:?}", alert_type);
        Ok(())
    }
}
