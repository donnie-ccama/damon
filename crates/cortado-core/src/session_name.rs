use crate::slug::Slug;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionName {
    pub team: Slug,
    pub agent: Slug,
    pub n: u32,
}

impl SessionName {
    pub fn encode(&self) -> String {
        format!("cortado_{}_{}_{}", self.team, self.agent, self.n)
    }

    pub fn parse(name: &str) -> Option<Self> {
        let mut parts = name.split('_');
        if parts.next()? != "cortado" {
            return None;
        }
        let team = Slug::parse(parts.next()?).ok()?;
        let agent = Slug::parse(parts.next()?).ok()?;
        let n: u32 = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(SessionName { team, agent, n })
    }

    /// Lowest n >= 1 not taken by a live session of this agent.
    pub fn next_free(team: &Slug, agent: &Slug, live: &[String]) -> SessionName {
        let taken: std::collections::BTreeSet<u32> = live
            .iter()
            .filter_map(|s| SessionName::parse(s))
            .filter(|s| &s.team == team && &s.agent == agent)
            .map(|s| s.n)
            .collect();
        let n = (1..).find(|i| !taken.contains(i)).unwrap();
        SessionName {
            team: team.clone(),
            agent: agent.clone(),
            n,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: &str) -> Slug {
        Slug::parse(x).unwrap()
    }

    #[test]
    fn round_trips() {
        let n = SessionName {
            team: s("newsletter"),
            agent: s("scout"),
            n: 3,
        };
        assert_eq!(n.encode(), "cortado_newsletter_scout_3");
        assert_eq!(SessionName::parse("cortado_newsletter_scout_3").unwrap(), n);
    }

    #[test]
    fn rejects_foreign_names() {
        for bad in [
            "other_newsletter_scout_1",
            "cortado_a_b",
            "cortado_a_b_c_d",
            "cortado_A_b_1",
            "cortado_a_b_x",
        ] {
            assert!(SessionName::parse(bad).is_none(), "{bad}");
        }
    }

    /// Pins numeric (not lexical) ordering for the "most recent session"
    /// picker in `commands::open::open_session`, which reattaches to
    /// `live.iter().max_by_key(|a| SessionName::parse(&a.name).map(|n| n.n))`.
    /// A lexical/string comparison of "_9" vs "_10" would rank "_9" higher
    /// ('9' > '1' byte-wise); parsing `n` into a `u32` first prevents that.
    #[test]
    fn parse_orders_numerically_not_lexically() {
        let names = ["cortado_demo_scout_9", "cortado_demo_scout_10"];
        let parsed: Vec<SessionName> = names
            .iter()
            .map(|n| SessionName::parse(n).unwrap())
            .collect();
        let most_recent = parsed.iter().max_by_key(|s| s.n).unwrap();
        assert_eq!(most_recent.n, 10);
        assert_eq!(most_recent.encode(), "cortado_demo_scout_10");
    }

    #[test]
    fn next_free_picks_lowest_gap() {
        let live = vec![
            "cortado_newsletter_scout_1".to_string(),
            "cortado_newsletter_scout_3".to_string(),
            "cortado_newsletter_other_2".to_string(),
        ];
        assert_eq!(
            SessionName::next_free(&s("newsletter"), &s("scout"), &live).n,
            2
        );
        assert_eq!(
            SessionName::next_free(&s("newsletter"), &s("fresh"), &live).n,
            1
        );
    }
}
