//! Task cards: finding the one that is meant, and what a card says.
//!
//! At work a task arrives as a line of text - `ACME-7310`, a ticket id with
//! a title after it, two customer numbers and a description, or just the
//! description. Tasks move between trackers and their ids change while
//! the title stays; the same task is named three different ways in a
//! week. So a card is looked for by everything the line holds: the ids,
//! the numbers, and the words - and the answer says how sure it is,
//! because a duplicate card is worse than one more question.
//!
//! The matching came from a script that ran this desk for a year: the
//! Dice coefficient over character bigrams (robust to typos and
//! inflection), the share of query tokens found in the title (a short
//! query against a long title is not penalised), and the thresholds that
//! said when to take, when to ask and when to start a new card.

use std::collections::HashMap;

use serde::Serialize;

/// A card as the record holds it.
#[derive(Debug, Clone, Serialize)]
pub struct Card {
    pub id: i64,
    pub key: String,
    pub title: String,
    pub status: String,
    pub aliases: Vec<String>,
    pub summary: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

/// A card's link to a repository.
#[derive(Debug, Clone, Serialize)]
pub struct Link {
    pub project: String,
    pub path: String,
    pub branch: Option<String>,
    pub role: Option<String>,
}

/// What git says about a card in one repository it is worked in.
#[derive(Debug, Clone, Serialize)]
pub struct Activity {
    pub project: String,
    /// The branches whose name carries the card's, local ones first.
    pub branches: Vec<Branch>,
    /// How many of the commits read name the card in their message.
    pub commits: u32,
    /// The newest movement: the newest of those commits and the branch tips.
    pub last_commit: Option<Commit>,
}

/// A branch a card is worked on - what a worktree for the card is made from.
#[derive(Debug, Clone, Serialize)]
pub struct Branch {
    pub name: String,
    /// The remote it is on, when there is no local branch of that name.
    pub remote: Option<String>,
    pub tip: String,
    pub tip_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Commit {
    pub hash: String,
    pub at: String,
    pub subject: String,
}

/// One candidate for a query, and why it scored.
#[derive(Debug, Clone, Serialize)]
pub struct Hit {
    pub score: u32,
    pub key: String,
    pub status: String,
    pub title: String,
    pub why: String,
}

/// What to do with the candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// One card is it; take it without asking.
    Take,
    /// Several could be it, or one is close: show them and ask.
    Ask,
    /// Nothing is close; the task is new.
    New,
}

/// What a query line breaks into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub ids: Vec<String>,
    pub numbers: Vec<String>,
    /// The line without its ids and numbers.
    pub text: String,
}

/// Lower case, `ё` as `е`, letters and digits only, single spaces.
pub fn normalise(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for c in text.chars().flat_map(char::to_lowercase) {
        let c = if c == 'ё' { 'е' } else { c };
        if c.is_alphanumeric() {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            out.push(c);
        } else {
            space = true;
        }
    }
    out
}

/// Words of three letters or more, normalised.
pub fn tokens(text: &str) -> Vec<String> {
    normalise(text).split(' ').filter(|w| w.chars().count() >= 3).map(str::to_string).collect()
}

fn bigrams(text: &str) -> HashMap<(char, char), u32> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = HashMap::new();
    for pair in chars.windows(2) {
        *out.entry((pair[0], pair[1])).or_insert(0) += 1;
    }
    out
}

/// Dice coefficient over character bigrams, on normalised text.
pub fn dice(a: &str, b: &str) -> f64 {
    if a.trim().is_empty() || b.trim().is_empty() {
        return 0.0;
    }
    let (ba, bb) = (bigrams(a), bigrams(b));
    let inter: u32 = ba.iter().map(|(k, n)| n.min(bb.get(k).unwrap_or(&0))).sum();
    let total: u32 = ba.values().sum::<u32>() + bb.values().sum::<u32>();
    if total == 0 { 0.0 } else { 2.0 * inter as f64 / total as f64 }
}

/// The share of query tokens found in the candidate's, by prefix either
/// way - containment, not Jaccard, so a short query against a long title
/// is not penalised.
pub fn containment(query: &[String], candidate: &[String]) -> f64 {
    if query.is_empty() || candidate.is_empty() {
        return 0.0;
    }
    let hit = query
        .iter()
        .filter(|q| candidate.iter().any(|c| c == *q || c.starts_with(q.as_str()) || q.starts_with(c.as_str())))
        .count();
    hit as f64 / query.len() as f64
}

/// Ticket ids in a line: `ACME-7310`, `ops-512` - two to eight letters, a
/// dash, up to six digits - upper-cased.
pub fn ticket_ids(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')) {
        let Some((letters, digits)) = word.split_once('-') else { continue };
        let ok = (2..=8).contains(&letters.len())
            && letters.bytes().all(|b| b.is_ascii_alphabetic())
            && (1..=6).contains(&digits.len())
            && digits.bytes().all(|b| b.is_ascii_digit());
        if ok {
            let id = word.to_ascii_uppercase();
            if !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
}

/// Long numbers are, as a rule, customer case numbers: five to eight digits.
pub fn customer_numbers(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in text.split(|c: char| !c.is_ascii_digit()) {
        if (5..=8).contains(&word.len()) && !out.contains(&word.to_string()) {
            out.push(word.to_string());
        }
    }
    out
}

/// Breaks a line into ids, numbers and the text that remains.
pub fn parse(line: &str) -> Query {
    let flat = line.split_whitespace().collect::<Vec<_>>().join(" ");
    let ids = ticket_ids(&flat);
    let numbers = customer_numbers(&flat);
    let mut text = String::new();
    for word in flat.split(' ') {
        let upper = word.to_ascii_uppercase();
        let is_id = ids.iter().any(|id| upper.trim_matches(|c: char| !c.is_ascii_alphanumeric()) == *id);
        let is_number = numbers.iter().any(|n| word.trim_matches(|c: char| !c.is_ascii_digit()) == n);
        if !is_id && !is_number {
            text.push_str(word);
            text.push(' ');
        }
    }
    Query {
        ids,
        numbers,
        text: text.trim().to_string(),
    }
}

/// Scores one card against a query. `None` below the floor.
pub fn score(query: &Query, card: &Card) -> Option<Hit> {
    let mut score = 0.0f64;
    let mut why = Vec::new();

    let mut card_ids: Vec<String> = ticket_ids(&card.key);
    for alias in &card.aliases {
        card_ids.extend(ticket_ids(alias));
    }
    for id in &query.ids {
        if card_ids.contains(id) {
            score = score.max(100.0);
            why.push(format!("id={id}"));
        }
    }

    let text = normalise(&query.text);
    let query_tokens = tokens(&query.text);
    let mut best = 0.0f64;
    if !text.is_empty() {
        for hay in std::iter::once(&card.title).chain(card.aliases.iter()) {
            let norm = normalise(hay);
            if norm.is_empty() {
                continue;
            }
            let v = 100.0 * (0.45 * dice(&text, &norm) + 0.55 * containment(&query_tokens, &tokens(hay)));
            best = best.max(v);
        }
    }
    if best > 0.0 {
        if best > score {
            why.push(format!("title ~{}%", best.round() as u32));
        }
        score = score.max(best);
    }

    let card_numbers = {
        let mut all = customer_numbers(&card.title);
        for alias in &card.aliases {
            all.extend(customer_numbers(alias));
        }
        all
    };
    for n in &query.numbers {
        if card_numbers.contains(n) {
            score += 55.0;
            why.push(format!("case {n}"));
        }
    }

    (score >= 15.0).then(|| Hit {
        score: score.min(130.0).round() as u32,
        key: card.key.clone(),
        status: card.status.clone(),
        title: card.title.clone(),
        why: why.join(", "),
    })
}

/// The candidates for a line, best first, and what to do with them.
pub fn find(query: &Query, cards: &[Card], top: usize) -> (Vec<Hit>, Verdict) {
    let mut hits: Vec<Hit> = cards.iter().filter_map(|c| score(query, c)).collect();
    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.key.cmp(&b.key)));
    hits.truncate(top);
    // "One candidate" means one that could be it: a second card scraping
    // the floor on a shared word does not turn a sure match into a question.
    let contenders = hits.iter().filter(|h| h.score >= 60).count();
    let verdict = match hits.first().map(|h| h.score) {
        None => Verdict::New,
        Some(s) if s >= 100 => Verdict::Take,
        Some(s) if s >= 85 && contenders == 1 => Verdict::Take,
        Some(s) if s >= 40 => Verdict::Ask,
        Some(_) if hits.iter().filter(|h| h.score >= 60).count() > 1 => Verdict::Ask,
        Some(_) => Verdict::New,
    };
    (hits, verdict)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(key: &str, title: &str, aliases: &[&str]) -> Card {
        Card {
            id: 1,
            key: key.to_string(),
            title: title.to_string(),
            status: "new".to_string(),
            aliases: aliases.iter().map(|a| a.to_string()).collect(),
            summary: None,
            created_at: String::new(),
            updated_at: None,
        }
    }

    #[test]
    fn a_line_breaks_into_ids_numbers_and_text() {
        let q = parse("OPS-512 [CUSTOMER] 561207, 561208: GUI for showing files");
        assert_eq!(q.ids, vec!["OPS-512"]);
        assert_eq!(q.numbers, vec!["561207", "561208"]);
        assert_eq!(q.text, "[CUSTOMER] GUI for showing files");
        let q = parse("acme-7310\nrtf files are not supported");
        assert_eq!(q.ids, vec!["ACME-7310"]);
        assert_eq!(q.text, "rtf files are not supported");
    }

    #[test]
    fn an_id_is_a_certain_match_even_when_the_title_moved() {
        let cards = [card("ACME-7310", "rtf files", &[]), card("ACME-4200", "something else", &["ACME-7310"])];
        let (hits, verdict) = find(&parse("ACME-7310"), &cards, 8);
        assert_eq!(verdict, Verdict::Take);
        assert_eq!(hits.len(), 2, "the alias counts: {hits:?}");
        assert!(hits.iter().all(|h| h.score == 100));
    }

    #[test]
    fn a_title_close_enough_is_taken_and_a_far_one_is_new() {
        let cards = [card("ACME-1", "Не поддерживаются файлы rtf в родном формате", &[])];
        let (hits, verdict) = find(&parse("не поддерживаются файлы rtf"), &cards, 8);
        assert_eq!(verdict, Verdict::Take, "{hits:?}");
        let (hits, verdict) = find(&parse("добавить новый фильтр в отчёт"), &cards, 8);
        assert_eq!(verdict, Verdict::New, "{hits:?}");
    }

    #[test]
    fn a_middling_match_asks() {
        let cards = [card("ACME-1", "Экспорт отчёта в файл rtf", &[])];
        let (hits, verdict) = find(&parse("экспорт файла"), &cards, 8);
        assert_eq!(verdict, Verdict::Ask, "{hits:?}");
    }

    #[test]
    fn a_customer_number_lifts_a_card() {
        let cards = [card("ACME-1", "Заказ 561207: не открывается вложение", &[]), card("ACME-2", "Другое", &[])];
        let (hits, _) = find(&parse("обращение 561207"), &cards, 8);
        assert_eq!(hits[0].key, "ACME-1");
        assert!(hits[0].why.contains("case 561207"), "{hits:?}");
    }

    #[test]
    fn normalisation_folds_case_yo_and_punctuation() {
        assert_eq!(normalise("Ёлка, Ель — ёж!"), "елка ель еж");
        assert_eq!(tokens("не в рф"), Vec::<String>::new());
        assert!(dice("файлы", "файл") > 0.7);
    }
}
