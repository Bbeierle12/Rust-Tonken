use rust_stemmers::{Algorithm, Stemmer};
use std::collections::HashSet;

/// Text analysis pipeline: tokenize → lowercase → stop words → stem → shingle.
pub struct TextPipeline {
    stemmer: Stemmer,
    stop_words: HashSet<String>,
    shingle_size: usize,
}

impl TextPipeline {
    /// Create a new pipeline with the given shingle size.
    /// Uses English stemmer and a hardcoded English stop word list.
    pub fn new(shingle_size: usize) -> Self {
        let stop_words: HashSet<String> = [
            "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with",
            "by", "from", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
            "do", "does", "did", "will", "would", "could", "should", "may", "might", "shall",
            "can", "need", "dare", "ought", "used", "it", "its", "this", "that", "these", "those",
            "i", "me", "my", "we", "our", "you", "your", "he", "him", "his", "she", "her", "they",
            "them", "their", "what", "which", "who", "whom", "when", "where", "why", "how", "not",
            "no", "nor", "as", "if", "then", "so", "than", "too", "very", "just", "about", "above",
            "after", "again", "all", "also", "am", "any", "because", "before", "between", "both",
            "each", "few", "further", "get", "got", "here", "into", "more", "most", "must",
            "only", "other", "own", "same", "some", "such", "there", "through", "up", "down",
            "out", "off", "over", "under", "until", "while", "during",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        Self {
            stemmer: Stemmer::create(Algorithm::English),
            stop_words,
            shingle_size,
        }
    }

    /// Process text into a set of stemmed shingles.
    pub fn process(&self, text: &str) -> HashSet<String> {
        let tokens: Vec<String> = text
            .split_whitespace()
            .map(|w| {
                w.chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect::<String>()
                    .to_lowercase()
            })
            .filter(|w| !w.is_empty() && !self.stop_words.contains(w))
            .map(|w| self.stemmer.stem(&w).to_string())
            .collect();

        if tokens.is_empty() {
            return HashSet::new();
        }

        if self.shingle_size <= 1 || tokens.len() < self.shingle_size {
            return tokens.into_iter().collect();
        }

        tokens
            .windows(self.shingle_size)
            .map(|window| window.join(" "))
            .collect()
    }

    /// Jaccard similarity between two texts: |A∩B| / |A∪B|.
    /// Returns 1.0 if both sets are empty (identical empty texts).
    pub fn similarity(&self, a: &str, b: &str) -> f64 {
        let set_a = self.process(a);
        let set_b = self.process(b);

        if set_a.is_empty() && set_b.is_empty() {
            return 1.0;
        }

        let intersection = set_a.intersection(&set_b).count();
        let union = set_a.union(&set_b).count();

        if union == 0 {
            return 1.0;
        }

        intersection as f64 / union as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_stemming() {
        let pipeline = TextPipeline::new(1);
        let result = pipeline.process("running runners run");
        assert!(result.contains("run"), "Should stem to 'run': {result:?}");
    }

    #[test]
    fn test_stop_word_removal() {
        let pipeline = TextPipeline::new(1);
        let result = pipeline.process("the quick brown fox");
        assert!(!result.contains("the"), "Stop word 'the' should be removed");
        assert!(result.contains("quick"), "'quick' should remain");
    }

    #[test]
    fn test_shingle_generation() {
        let pipeline = TextPipeline::new(2);
        // "quick brown fox" after stop word removal and stemming
        let result = pipeline.process("quick brown fox");
        // Should produce bigrams of stemmed tokens
        assert!(!result.is_empty(), "Should produce shingles");
        // With shingle_size=2, we should get pairs
        for shingle in &result {
            let parts: Vec<&str> = shingle.split(' ').collect();
            assert_eq!(parts.len(), 2, "Each shingle should have 2 tokens");
        }
    }

    #[test]
    fn test_similarity_identical() {
        let pipeline = TextPipeline::new(1);
        let score = pipeline.similarity("hello world", "hello world");
        assert!(
            (score - 1.0).abs() < f64::EPSILON,
            "Identical texts should have similarity 1.0, got {score}"
        );
    }

    #[test]
    fn test_similarity_disjoint() {
        let pipeline = TextPipeline::new(1);
        let score = pipeline.similarity("apple banana cherry", "dog elephant frog");
        assert!(
            score.abs() < f64::EPSILON,
            "Disjoint texts should have similarity 0.0, got {score}"
        );
    }

    #[test]
    fn test_similarity_empty_empty() {
        let pipeline = TextPipeline::new(1);
        let score = pipeline.similarity("", "");
        assert!(
            (score - 1.0).abs() < f64::EPSILON,
            "Two empty texts should have similarity 1.0, got {score}"
        );
    }
}
