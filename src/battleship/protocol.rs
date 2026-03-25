// battleship/protocol.rs — Network message definitions for the Battleship game.
// All messages are serialized as newline-delimited JSON over a TCP stream.
// Each variant maps to a distinct event in the game flow:
//   Handshake → Placement → Turns → End.

use serde::{Deserialize, Serialize};

use super::game::ShipKind;

// ---------------------------------------------------------------------------
// Message — every possible network message between two peers
// ---------------------------------------------------------------------------

/// A single message exchanged between host and client over TCP.
/// Serialized to one line of JSON, terminated by '\n'.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Message {
    // -- Handshake ----------------------------------------------------------

    /// First message after TCP connect. Both sides send their display name.
    Hello { name: String },

    /// Sent after a player finishes placing all ships.
    /// The game begins once both sides have sent `Ready`.
    Ready,

    // -- Gameplay -----------------------------------------------------------

    /// Fire a shot at the opponent's grid at column `x`, row `y` (zero-indexed).
    Fire { x: u8, y: u8 },

    /// Response to a `Fire` message.
    /// - `hit`: whether the shot struck a ship.
    /// - `sunk`: if the hit just sunk a ship, which kind it was.
    FireResult {
        x: u8,
        y: u8,
        hit: bool,
        sunk: Option<ShipKind>,
    },

    // -- End ----------------------------------------------------------------

    /// Announces the game is over. `winner` is the winning player's name.
    GameOver { winner: String },

    /// One player requests a rematch after the game ends.
    Rematch,

    /// Clean disconnect — the peer is leaving.
    Disconnect,

    // -- Stretch ------------------------------------------------------------

    /// In-game chat message (stretch goal).
    Chat { text: String },
}

// ===========================================================================
// Unit tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_hello() {
        let msg = Message::Hello {
            name: "Alice".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("Alice"));

        // Round-trip
        let decoded: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn serialize_fire_result_with_sunk() {
        let msg = Message::FireResult {
            x: 3,
            y: 7,
            hit: true,
            sunk: Some(ShipKind::Destroyer),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn serialize_fire_result_miss() {
        let msg = Message::FireResult {
            x: 0,
            y: 0,
            hit: false,
            sunk: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn serialize_all_variants() {
        // Make sure every variant can be serialized without panicking
        let messages = vec![
            Message::Hello {
                name: "Bob".into(),
            },
            Message::Ready,
            Message::Fire { x: 5, y: 9 },
            Message::FireResult {
                x: 5,
                y: 9,
                hit: true,
                sunk: Some(ShipKind::Carrier),
            },
            Message::GameOver {
                winner: "Bob".into(),
            },
            Message::Rematch,
            Message::Disconnect,
            Message::Chat {
                text: "gg".into(),
            },
        ];

        for msg in &messages {
            let json = serde_json::to_string(msg).unwrap();
            let decoded: Message = serde_json::from_str(&json).unwrap();
            assert_eq!(&decoded, msg);
        }
    }
}
