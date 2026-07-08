use crate::slug::Slug;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionName {
    pub team: Slug,
    pub agent: Slug,
    pub n: u32,
}

impl SessionName {
    pub fn encode(&self) -> String {
        format!("damon_{}_{}_{}", self.team, self.agent, self.n)
    }

    pub fn parse(name: &str) -> Option<Self> {
        let mut parts = name.split('_');
        if parts.next()? != "damon" {
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
        assert_eq!(n.encode(), "damon_newsletter_scout_3");
        assert_eq!(SessionName::parse("damon_newsletter_scout_3").unwrap(), n);
    }

    #[test]
    fn rejects_foreign_names() {
        for bad in [
            "other_newsletter_scout_1",
            "damon_a_b",
            "damon_a_b_c_d",
            "damon_A_b_1",
            "damon_a_b_x",
        ] {
            assert!(SessionName::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn next_free_picks_lowest_gap() {
        let live = vec![
            "damon_newsletter_scout_1".to_string(),
            "damon_newsletter_scout_3".to_string(),
            "damon_newsletter_other_2".to_string(),
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
