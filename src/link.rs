//! The line as a graph: what a project is tied to, and when the two ends
//! have drifted apart.
//!
//! Eighteen projects were eighteen separate lists. Everything they share
//! lived in prose - that kasl and kasl-server are two halves of one thing,
//! that lyrid draws its primitives from dowel, that the HTTPS door rhapsod
//! built is what hilvan is waiting for - and prose is not checked by
//! anything. So the halves drifted, and both times it was a review that
//! noticed, weeks later, by reading two hubs side by side.
//!
//! A link is a fact of the record, so the drift is a query rather than a
//! reading. Three kinds, and they are not three names for one thing:
//!
//! - a **pair** is symmetric. Two products ship one capability between
//!   them, and neither is finished without the other.
//! - a **consumer** points at what it draws on: lyrid consumes dowel.
//! - a **donor** is the same tie written from the other end, so that a
//!   project can state either side of a relationship it knows about
//!   without having to open the neighbour's record to do it.
//!
//! Only a pair can drift, and that is the whole reason the kinds are told
//! apart. A consumer running ahead of what it consumes is ordinary - it is
//! what a consumer does between releases of the thing it uses. A pair
//! running ahead of itself is a half-built capability, and it is invisible
//! until someone asks.

use serde::Serialize;

/// What a link says about two ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Two halves of one capability; neither ships alone.
    Pair,
    /// This end draws on the other.
    Consumer,
    /// The other end draws on this one.
    Donor,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Pair => "pair",
            Kind::Consumer => "consumer",
            Kind::Donor => "donor",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "pair" => Some(Kind::Pair),
            "consumer" => Some(Kind::Consumer),
            "donor" => Some(Kind::Donor),
            _ => None,
        }
    }

    /// Whether the two ends stand as equals.
    ///
    /// A pair reads the same from either side, so `show` prints it under
    /// one heading whichever end it was recorded from; the directed kinds
    /// read as two different facts and get two headings.
    pub fn symmetric(self) -> bool {
        matches!(self, Kind::Pair)
    }

    /// The same tie seen from the other end.
    ///
    /// What one project calls a consumer the other calls a donor, and a
    /// link is recorded once: `show` on the far end has to turn it round
    /// rather than expect a second row that says the same thing.
    pub fn flipped(self) -> Self {
        match self {
            Kind::Pair => Kind::Pair,
            Kind::Consumer => Kind::Donor,
            Kind::Donor => Kind::Consumer,
        }
    }
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `pad` rather than `write_str`: a column of kinds is printed with
        // a width, and `write_str` ignores it silently - the list came out
        // ragged and nothing said why.
        f.pad(self.as_str())
    }
}

/// One end of a link, as the record holds it.
#[derive(Debug, Clone, Serialize)]
pub struct End {
    pub project: String,
    /// The version anchored on this side, when one is.
    pub version: Option<String>,
    /// Whether that version has a tag. `None` when no version is anchored.
    pub shipped: Option<bool>,
}

/// A link, read from the side of the project that asked for it.
///
/// `near` is always the asking project and `far` the other one, whichever
/// way round the row was written. A caller that had to know which column it
/// landed in would get it wrong the first time a pair was recorded from the
/// other end - which is exactly the mistake the table exists to catch.
#[derive(Debug, Clone, Serialize)]
pub struct Link {
    pub id: i64,
    pub kind: Kind,
    pub near: End,
    pub far: End,
    pub note: Option<String>,
}

/// The other links that state the same tie as `link`, anchored differently.
///
/// The version anchors are part of a link's identity on purpose: kasl v1.12
/// <-> kasl-server v0.22 and v1.13 <-> v0.23 are the history of a pair, and a
/// record that rewrote one into the other would lose it. But a tie recorded
/// bare and the same tie recorded with versions read to a person as one fact,
/// and the list printed both. So a link is alike when it joins the same two
/// projects in the same kind and nothing in it contradicts `link`: on each
/// side the versions are the same, or one of the two is not given, and so
/// with the notes. Two anchored links of different versions are history, not
/// a double; two links whose notes say different things are two ties, such as
/// a product drawn on for its sessions and, since a version, for its tickets.
///
/// `links` are read from the near end of `link`, as `Db::links_of` gives them,
/// so a directed kind recorded from the far end is already turned round.
pub fn alike(links: &[Link], link: &Link) -> Vec<Link> {
    let agree = |a: &Option<String>, b: &Option<String>| a.is_none() || b.is_none() || a == b;
    links
        .iter()
        .filter(|other| {
            other.id != link.id
                && other.kind == link.kind
                && other.near.project == link.near.project
                && other.far.project == link.far.project
                && agree(&other.near.version, &link.near.version)
                && agree(&other.far.version, &link.far.version)
                && agree(&other.note, &link.note)
        })
        .cloned()
        .collect()
}

/// Why a pair is being reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Drift {
    /// One half is tagged and the other is not: the capability went out
    /// with a piece missing, and nothing else in the record says so.
    ShippedAlone,
    /// Both halves are still to come, but one end has moved on by several
    /// versions while the other stood still.
    RunAhead,
}

impl Drift {
    fn rank(self) -> u8 {
        match self {
            Drift::ShippedAlone => 0,
            Drift::RunAhead => 1,
        }
    }
}

impl Ord for Drift {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank().cmp(&other.rank())
    }
}

impl PartialOrd for Drift {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// A pair whose halves have parted company.
#[derive(Debug, Clone, Serialize)]
pub struct Parted {
    pub drift: Drift,
    /// The end that went on ahead, or shipped.
    pub ahead: End,
    /// The end left behind.
    pub behind: End,
    /// Versions between the two, for the drift measured in versions.
    pub versions: Option<u32>,
    pub note: Option<String>,
}

/// How far one half of a pair may run ahead before the record says so.
///
/// Two, not one. A pair is built one end at a time and one version apart is
/// the normal shape of the work in progress; two is where it stops being a
/// handover and starts being a half that was forgotten.
pub const AHEAD_VERSIONS: u32 = 2;

/// A pair as the check sees it: the link, and how far each end has moved
/// on past the version anchored to it.
#[derive(Debug, Clone)]
pub struct Pair {
    pub link: Link,
    /// Versions the near end has shipped since the one anchored here.
    pub near_ahead: u32,
    pub far_ahead: u32,
}

/// Reports the pairs whose halves have parted company.
///
/// Only pairs: a consumer that has moved on past the version of its donor
/// is a consumer doing what consumers do between releases, and reporting it
/// would put a line in the brief every week for every tie in the line, which
/// is how a warning stops being read.
pub fn parted(pairs: &[Pair]) -> Vec<Parted> {
    let mut out = Vec::new();
    for pair in pairs {
        if pair.link.kind != Kind::Pair {
            continue;
        }
        let near = &pair.link.near;
        let far = &pair.link.far;
        match (near.shipped, far.shipped) {
            // One tagged, one not. This is the loud one: the halves are
            // not merely out of step, one of them is out in the world.
            (Some(true), Some(false)) => out.push(Parted {
                drift: Drift::ShippedAlone,
                ahead: near.clone(),
                behind: far.clone(),
                versions: None,
                note: pair.link.note.clone(),
            }),
            (Some(false), Some(true)) => out.push(Parted {
                drift: Drift::ShippedAlone,
                ahead: far.clone(),
                behind: near.clone(),
                versions: None,
                note: pair.link.note.clone(),
            }),
            // Neither out yet: the question is whether one end has been
            // left where it was while the other kept releasing.
            (Some(false), Some(false)) => {
                let (ahead, behind, by) = if pair.near_ahead >= pair.far_ahead {
                    (near, far, pair.near_ahead - pair.far_ahead)
                } else {
                    (far, near, pair.far_ahead - pair.near_ahead)
                };
                if by >= AHEAD_VERSIONS {
                    out.push(Parted {
                        drift: Drift::RunAhead,
                        ahead: ahead.clone(),
                        behind: behind.clone(),
                        versions: Some(by),
                        note: pair.link.note.clone(),
                    });
                }
            }
            // Both shipped is the pair working. A pair with no version
            // anchored on one side says nothing about versions at all: it
            // is a tie between products, and there is nothing to compare.
            _ => {}
        }
    }
    // The loud drift first, then the widest gap: a capability that is half
    // out in the world outranks one that is half built.
    out.sort_by(|a, b| {
        a.drift
            .cmp(&b.drift)
            .then_with(|| b.versions.cmp(&a.versions))
            .then_with(|| a.ahead.project.cmp(&b.ahead.project))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn end(project: &str, version: &str, shipped: bool) -> End {
        End {
            project: project.into(),
            version: Some(version.into()),
            shipped: Some(shipped),
        }
    }

    fn bare(project: &str) -> End {
        End {
            project: project.into(),
            version: None,
            shipped: None,
        }
    }

    fn pair(near: End, far: End, near_ahead: u32, far_ahead: u32) -> Pair {
        Pair {
            link: Link {
                id: 1,
                kind: Kind::Pair,
                near,
                far,
                note: None,
            },
            near_ahead,
            far_ahead,
        }
    }

    #[test]
    fn a_half_that_shipped_without_the_other_is_the_loudest_signal() {
        let drifted = parted(&[pair(end("kasl", "v1.12.0", true), end("kasl-server", "v0.22.0", false), 0, 0)]);
        assert_eq!(drifted.len(), 1);
        assert_eq!(drifted[0].drift, Drift::ShippedAlone);
        assert_eq!(drifted[0].ahead.project, "kasl");
        assert_eq!(drifted[0].behind.project, "kasl-server");
    }

    /// Recorded from either end, the report reads the same. The row has a
    /// `from` and a `to` column and nothing stops a pair being written in
    /// whichever order it was noticed in.
    #[test]
    fn the_side_a_pair_was_recorded_from_does_not_change_what_it_says() {
        let drifted = parted(&[pair(end("kasl-server", "v0.22.0", false), end("kasl", "v1.12.0", true), 0, 0)]);
        assert_eq!(drifted[0].ahead.project, "kasl");
        assert_eq!(drifted[0].behind.project, "kasl-server");
    }

    #[test]
    fn a_pair_that_shipped_both_halves_is_the_pair_working() {
        assert!(parted(&[pair(end("kasl", "v1.12.0", true), end("kasl-server", "v0.22.0", true), 0, 0)]).is_empty());
    }

    /// One version apart is the shape of the work, not a fault: a pair is
    /// built one end at a time.
    #[test]
    fn one_version_ahead_is_not_yet_drift() {
        assert!(parted(&[pair(end("a", "v0.3.0", false), end("b", "v0.1.0", false), 1, 0)]).is_empty());
    }

    #[test]
    fn two_versions_ahead_is_reported_with_the_number() {
        let drifted = parted(&[pair(end("a", "v0.3.0", false), end("b", "v0.1.0", false), 2, 0)]);
        assert_eq!(drifted.len(), 1);
        assert_eq!(drifted[0].drift, Drift::RunAhead);
        assert_eq!(drifted[0].ahead.project, "a");
        assert_eq!(drifted[0].versions, Some(2));
    }

    /// The end that is behind is named whichever column it sits in.
    #[test]
    fn the_far_end_can_be_the_one_that_ran_ahead() {
        let drifted = parted(&[pair(end("a", "v0.1.0", false), end("b", "v0.4.0", false), 0, 3)]);
        assert_eq!(drifted[0].ahead.project, "b");
        assert_eq!(drifted[0].versions, Some(3));
    }

    /// A consumer is not a pair. lyrid releasing four times between two
    /// releases of dowel is lyrid working, and a brief that said so every
    /// week would be a brief nobody reads.
    #[test]
    fn a_consumer_running_ahead_is_not_reported() {
        let mut p = pair(end("lyrid", "v0.12.0", false), end("dowel", "v0.26.0", false), 5, 0);
        p.link.kind = Kind::Consumer;
        assert!(parted(&[p]).is_empty());
        // Nor does a consumer half out in the world: that is a release.
        let mut p = pair(end("lyrid", "v0.12.0", true), end("dowel", "v0.26.0", false), 0, 0);
        p.link.kind = Kind::Consumer;
        assert!(parted(&[p]).is_empty());
    }

    /// A tie between products with no version on either side is a fact
    /// about the line, not a claim about any release.
    #[test]
    fn a_link_without_versions_says_nothing_about_drift() {
        assert!(parted(&[pair(bare("a"), bare("b"), 0, 0)]).is_empty());
    }

    /// Half a pair anchored and half not cannot be compared either: the
    /// unanchored end has no version to be ahead or behind of.
    #[test]
    fn one_anchored_end_is_not_a_comparison() {
        assert!(parted(&[pair(end("a", "v0.3.0", true), bare("b"), 0, 0)]).is_empty());
    }

    #[test]
    fn the_shipped_half_outranks_the_widest_gap() {
        let drifted = parted(&[
            pair(end("a", "v0.9.0", false), end("b", "v0.1.0", false), 8, 0),
            pair(end("c", "v1.0.0", true), end("d", "v0.1.0", false), 0, 0),
        ]);
        assert_eq!(drifted[0].ahead.project, "c");
        assert_eq!(drifted[1].ahead.project, "a");
    }

    fn tie(id: i64, kind: Kind, near: End, far: End) -> Link {
        Link {
            id,
            kind,
            near,
            far,
            note: None,
        }
    }

    /// The double the list printed: the same pair, once bare and once with
    /// versions. Found from either of the two, since either can come second.
    #[test]
    fn a_bare_tie_and_the_same_tie_with_versions_are_alike() {
        let bare_one = tie(1, Kind::Pair, bare("kasl"), bare("kasl-server"));
        let anchored = tie(2, Kind::Pair, end("kasl", "v1.13", false), end("kasl-server", "v0.23.0", false));
        let links = vec![bare_one.clone(), anchored.clone()];
        assert_eq!(alike(&links, &anchored).iter().map(|l| l.id).collect::<Vec<_>>(), vec![1]);
        assert_eq!(alike(&links, &bare_one).iter().map(|l| l.id).collect::<Vec<_>>(), vec![2]);
    }

    /// Half anchored agrees with both the bare tie and the full one: neither
    /// says anything the other contradicts.
    #[test]
    fn one_side_anchored_is_alike_where_the_other_side_is_silent() {
        let half = tie(3, Kind::Pair, end("kasl", "v1.13", false), bare("kasl-server"));
        let full = tie(2, Kind::Pair, end("kasl", "v1.13", false), end("kasl-server", "v0.23.0", false));
        let other_half = tie(4, Kind::Pair, end("kasl", "v1.12", true), bare("kasl-server"));
        let links = vec![half.clone(), full, other_half];
        assert_eq!(alike(&links, &half).iter().map(|l| l.id).collect::<Vec<_>>(), vec![2]);
    }

    /// The history of a pair is not a double: each step anchors other
    /// versions, and the record keeps every one.
    #[test]
    fn two_ties_anchored_to_different_versions_are_history() {
        let then = tie(1, Kind::Pair, end("kasl", "v1.12", true), end("kasl-server", "v0.22.0", true));
        let now = tie(2, Kind::Pair, end("kasl", "v1.13", false), end("kasl-server", "v0.23.0", false));
        assert!(alike(&[then, now.clone()], &now).is_empty());
    }

    /// Two notes that differ are two ties, however the anchors agree: what a
    /// product is drawn on for, said twice in different words, is two things.
    /// A note on one of them only says more about the same tie.
    #[test]
    fn two_ties_told_apart_by_their_notes_are_not_alike() {
        let mut sessions = tie(1, Kind::Donor, bare("tracker"), bare("record"));
        sessions.note = Some("a session opens an interval".into());
        let mut tickets = tie(2, Kind::Donor, bare("tracker"), end("record", "v0.25.0", true));
        tickets.note = Some("the record reads the tickets".into());
        let silent = tie(3, Kind::Donor, end("tracker", "v1.0.0", true), bare("record"));
        let links = vec![sessions.clone(), tickets.clone(), silent.clone()];
        assert_eq!(alike(&links, &tickets).iter().map(|l| l.id).collect::<Vec<_>>(), vec![3]);
        assert_eq!(alike(&links, &silent).iter().map(|l| l.id).collect::<Vec<_>>(), vec![1, 2]);
    }

    /// Another kind between the same two, or the same kind with another
    /// project, says something else.
    #[test]
    fn another_kind_or_another_project_is_not_alike() {
        let pair = tie(1, Kind::Pair, bare("a"), bare("b"));
        let uses = tie(2, Kind::Consumer, bare("a"), bare("b"));
        let elsewhere = tie(3, Kind::Pair, bare("a"), bare("c"));
        let links = vec![pair.clone(), uses, elsewhere];
        assert!(alike(&links, &pair).is_empty());
    }

    /// A list of links is printed as a column, so the kind has to honour a
    /// width. `write_str` ignores one without complaint.
    #[test]
    fn a_kind_pads_to_the_width_it_is_given() {
        assert_eq!(format!("{:<9}|", Kind::Pair), "pair     |");
        assert_eq!(format!("{:<9}|", Kind::Consumer), "consumer |");
    }

    /// What one end calls a consumer the other calls a donor. The row is
    /// written once and read from both sides.
    #[test]
    fn a_directed_kind_reads_as_its_opposite_from_the_far_end() {
        assert_eq!(Kind::Consumer.flipped(), Kind::Donor);
        assert_eq!(Kind::Donor.flipped(), Kind::Consumer);
        assert_eq!(Kind::Pair.flipped(), Kind::Pair);
        assert!(Kind::Pair.symmetric());
        assert!(!Kind::Consumer.symmetric());
    }
}
