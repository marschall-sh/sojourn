use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

use crate::inventory::Host;

pub struct SearchEngine {
    matcher: SkimMatcherV2,
}

impl SearchEngine {
    pub fn new() -> Self {
        Self {
            matcher: SkimMatcherV2::default(),
        }
    }

    pub fn search(&self, query: &str, hosts: &[Host]) -> Vec<(usize, i64)> {
        let mut results: Vec<(usize, i64)> = hosts
            .iter()
            .enumerate()
            .filter_map(|(i, host)| {
                let search_str = host.search_string();
                self.matcher
                    .fuzzy_match(&search_str, query)
                    .map(|score| (i, score))
            })
            .collect();

        results.sort_by(|a, b| b.1.cmp(&a.1));
        results
    }

    pub fn highlight_indices(&self, text: &str, query: &str) -> Vec<usize> {
        self.matcher
            .fuzzy_indices(text, query)
            .map(|(_, indices)| indices)
            .unwrap_or_default()
    }
}
