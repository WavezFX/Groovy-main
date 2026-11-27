//! Round 4C items 2 and 6: when Groovy may look at the screen (pure).

/// The `screenLook` setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// An Allow button in the island for each look (the default).
    Ask,
    /// Looks when asked, with no button.
    Allowed,
    /// Never looks.
    Off,
}

impl Look {
    pub fn parse(s: &str) -> Look {
        match s {
            "allowed" => Look::Allowed,
            "off" => Look::Off,
            _ => Look::Ask,
        }
    }
}

/// Why a look is refused, in words he can say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Off,
    NotApproved,
    GameMode,
    Locked,
}

impl Refusal {
    pub fn words(self) -> &'static str {
        match self {
            Refusal::Off => "I can't see your screen: looking at the screen is off in Settings → Screen.",
            Refusal::NotApproved => "I can't see your screen right now: the look wasn't allowed.",
            Refusal::GameMode => "I can't see your screen during Game Mode.",
            Refusal::Locked => "I can't see your screen while it's locked.",
        }
    }
}

/// Whether a look may happen. `approved` is the user's click on Allow (only
/// needed in `Ask`). Game Mode and a locked screen always refuse.
pub fn may_look(look: Look, approved: bool, game_mode: bool, locked: bool) -> Result<(), Refusal> {
    if look == Look::Off {
        return Err(Refusal::Off);
    }
    if locked {
        return Err(Refusal::Locked);
    }
    if game_mode {
        return Err(Refusal::GameMode);
    }
    if look == Look::Ask && !approved {
        return Err(Refusal::NotApproved);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setting_decides_and_game_mode_and_the_lock_always_refuse() {
        assert_eq!(Look::parse(""), Look::Ask, "ask is the default");
        assert_eq!(Look::parse("allowed"), Look::Allowed);
        assert_eq!(may_look(Look::Ask, false, false, false), Err(Refusal::NotApproved));
        assert_eq!(may_look(Look::Ask, true, false, false), Ok(()));
        assert_eq!(may_look(Look::Allowed, false, false, false), Ok(()));
        assert_eq!(may_look(Look::Off, true, false, false), Err(Refusal::Off));
        assert_eq!(may_look(Look::Allowed, true, true, false), Err(Refusal::GameMode));
        assert_eq!(may_look(Look::Allowed, true, false, true), Err(Refusal::Locked));
    }
}
