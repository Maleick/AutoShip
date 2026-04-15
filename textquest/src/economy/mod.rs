//! Economy system — Krono farm, vendor automation, loot distribution, banking.
//!
//! This module contains the failure detection and recovery routing layer
//! for the economy loop (M10).

pub mod failure_handling;
pub mod price_monitor;

pub use failure_handling::{
    FailureHistory, FailureRouter, FailureState, FailureType, RecoveryAction,
};

#[cfg(test)]
mod tests {
    use textquest_common::chat::{ChatChannel, ChatEvent};

    #[test]
    fn parse_trade_observation_extracts_krono_listing() {
        let observation = crate::economy::price_monitor::parse_trade_observation(
            4242,
            "nexus",
            "Traderone",
            &ChatEvent {
                channel: ChatChannel::Auction,
                sender: "Traderone".into(),
                message: "WTS Fungi Tunic 3 krono firm".into(),
            },
            1_715_000_000_000,
        )
        .expect("expected Krono listing");

        assert_eq!(observation.source_pid, 4242);
        assert_eq!(observation.zone, "nexus");
        assert_eq!(
            observation.intent,
            crate::economy::price_monitor::TradeIntent::Sell
        );
        assert_eq!(observation.item_name, "Fungi Tunic");
        assert_eq!(observation.price_milli_krono, 3_000);
    }

    #[test]
    fn parse_trade_observation_handles_price_first_messages() {
        let observation = crate::economy::price_monitor::parse_trade_observation(
            4242,
            "poknowledge",
            "Buyerone",
            &ChatEvent {
                channel: ChatChannel::Ooc,
                sender: "Buyerone".into(),
                message: "2.5 kr for Cloak of Flames".into(),
            },
            1_715_000_000_500,
        )
        .expect("expected price-first listing");

        assert_eq!(observation.zone, "poknowledge");
        assert_eq!(observation.item_name, "Cloak of Flames");
        assert_eq!(observation.price_milli_krono, 2_500);
    }

    #[test]
    fn parse_trade_observation_rejects_non_krono_prices() {
        let observation = crate::economy::price_monitor::parse_trade_observation(
            4242,
            "nexus",
            "Platguy",
            &ChatEvent {
                channel: ChatChannel::Auction,
                sender: "Platguy".into(),
                message: "WTS Fungi Tunic 3000pp".into(),
            },
            1_715_000_001_000,
        );

        assert!(observation.is_none());
    }

    #[test]
    fn trade_price_store_roundtrips_recent_history() {
        let store = crate::economy::price_monitor::TradePriceStore::open_memory()
            .expect("in-memory trade price store");

        let observation = crate::economy::price_monitor::parse_trade_observation(
            77,
            "nexus",
            "Traderone",
            &ChatEvent {
                channel: ChatChannel::Auction,
                sender: "Traderone".into(),
                message: "WTS Circlet of Shadow 8 kr".into(),
            },
            1_715_000_002_000,
        )
        .expect("expected parsed trade observation");

        store
            .insert_observation(&observation)
            .expect("trade observation inserted");

        let history = store
            .recent_observations(10)
            .expect("recent trade observation query");

        assert_eq!(history.len(), 1);
        assert_eq!(history[0].item_name, "Circlet of Shadow");
        assert_eq!(history[0].price_milli_krono, 8_000);
        assert_eq!(history[0].raw_message, "WTS Circlet of Shadow 8 kr");
    }

    #[test]
    fn trade_price_monitor_filters_zones_and_deduplicates_observers() {
        let mut monitor = crate::economy::price_monitor::TradePriceMonitor::for_tests()
            .expect("trade price monitor");

        let chat = ChatEvent {
            channel: ChatChannel::Auction,
            sender: "Traderone".into(),
            message: "WTS Fungi Tunic 3 krono".into(),
        };

        assert!(
            monitor
                .record_chat(1001, "nexus", Some("WatcherA"), &chat, 1_715_000_003_000)
                .expect("nexus auction should record")
        );
        assert!(
            !monitor
                .record_chat(1002, "nexus", Some("WatcherB"), &chat, 1_715_000_003_100)
                .expect("duplicate observer should be suppressed")
        );
        assert!(
            !monitor
                .record_chat(
                    1003,
                    "eastcommons",
                    Some("WatcherC"),
                    &chat,
                    1_715_000_004_000,
                )
                .expect("non-hub zone should be ignored")
        );

        let history = monitor
            .store()
            .recent_observations(10)
            .expect("recent trade observation query");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].speaker, "Traderone");
    }
}
