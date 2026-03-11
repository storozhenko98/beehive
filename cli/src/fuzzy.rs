fn is_boundary(ch: char) -> bool {
    !ch.is_ascii_alphanumeric()
}

pub fn fuzzy_match_score(query: &str, candidate: &str) -> Option<i64> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Some(0);
    }

    let candidate = candidate.to_lowercase();
    let candidate_chars: Vec<char> = candidate.chars().collect();
    let query_chars: Vec<char> = query.chars().collect();
    let mut score = 0i64;
    let mut query_index = 0usize;
    let mut prev_match_index: Option<usize> = None;

    for (candidate_index, candidate_char) in candidate_chars.iter().enumerate() {
        if query_index == query_chars.len() {
            break;
        }
        if *candidate_char != query_chars[query_index] {
            continue;
        }

        score += 10;

        if candidate_index == 0 {
            score += 20;
        } else if is_boundary(candidate_chars[candidate_index - 1]) {
            score += 12;
        }

        if let Some(prev_index) = prev_match_index {
            if candidate_index == prev_index + 1 {
                score += 18;
            } else {
                score -= (candidate_index - prev_index - 1).min(6) as i64;
            }
        } else {
            score -= candidate_index.min(8) as i64;
        }

        prev_match_index = Some(candidate_index);
        query_index += 1;
    }

    if query_index != query_chars.len() {
        return None;
    }

    if candidate == query {
        score += 100;
    } else if candidate.starts_with(&query) {
        score += 60;
    } else if candidate.contains(&query) {
        score += 35;
    }

    score -= candidate_chars.len().min(24) as i64;
    Some(score)
}

pub fn fuzzy_filter_strings<'a>(items: &'a [String], query: &str) -> Vec<&'a String> {
    let query = query.trim();
    if query.is_empty() {
        return items.iter().collect();
    }

    let mut scored: Vec<(i64, &String)> = items
        .iter()
        .filter_map(|item| fuzzy_match_score(query, item).map(|score| (score, item)))
        .collect();

    scored.sort_by(|(score_a, item_a), (score_b, item_b)| {
        score_b
            .cmp(score_a)
            .then_with(|| item_a.len().cmp(&item_b.len()))
            .then_with(|| item_a.to_lowercase().cmp(&item_b.to_lowercase()))
    });

    scored.into_iter().map(|(_, item)| item).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_match_score_prefers_prefix_over_later_match() {
        let prefix = fuzzy_match_score("feat", "feature-branch").unwrap();
        let later = fuzzy_match_score("feat", "my-feature-branch").unwrap();

        assert!(prefix > later);
    }

    #[test]
    fn fuzzy_match_score_rejects_non_subsequence() {
        assert_eq!(fuzzy_match_score("fz", "feature-branch"), None);
    }

    #[test]
    fn fuzzy_filter_strings_sorts_best_match_first() {
        let items = vec![
            "release/main".to_string(),
            "feature-branch".to_string(),
            "main".to_string(),
        ];

        let filtered = fuzzy_filter_strings(&items, "main");

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0], "main");
    }
}
