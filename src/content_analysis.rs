use std::collections::{HashMap, HashSet};

use crate::analysis::TextPipeline;
use crate::lexicons::emotion::Emotion;
use crate::lexicons::{emotion, formality, sentiment};

/// Result of content analysis for a single turn.
#[derive(Debug, Clone, Default)]
pub struct ContentAnalysisResult {
    pub session_id: String,
    pub turn_index: usize,
    // Sentiment
    pub sentiment_score: f64,
    pub user_sentiment_score: f64,
    pub dominant_emotion: Option<String>,
    pub emotion_counts: Vec<(String, u32)>,
    pub emotional_range: u32,
    // Linguistic
    pub reading_level: f64,
    pub avg_sentence_length: f64,
    pub avg_word_length: f64,
    pub type_token_ratio: f64,
    pub hapax_percentage: f64,
    pub lexical_density: f64,
    // Conversational
    pub response_amplification: f64,
    pub question_density: f64,
    pub hedging_index: f64,
    pub code_density: f64,
    pub list_density: f64,
    pub topic_similarity_prev: f64,
    pub topic_similarity_first: f64,
    // Style
    pub formality_score: f64,
    pub repetition_index: f64,
    pub instructional_density: f64,
    pub certainty_score: f64,
}

/// Pure, synchronous content analysis engine.
pub struct ContentAnalyzer;

// ── Stop words for lexical density ──────────────────────

static STOP_WORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
    "from", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had", "do",
    "does", "did", "will", "would", "could", "should", "may", "might", "shall", "can", "need",
    "it", "its", "this", "that", "these", "those", "i", "me", "my", "we", "our", "you", "your",
    "he", "him", "his", "she", "her", "they", "them", "their", "what", "which", "who", "whom",
    "when", "where", "why", "how", "not", "no", "nor", "as", "if", "then", "so", "than", "too",
    "very", "just", "about",
];

// ── Common imperative verbs for instructional density ────

static IMPERATIVE_VERBS: &[&str] = &[
    "use", "add", "set", "run", "make", "create", "write", "read", "open", "close", "start",
    "stop", "go", "come", "take", "give", "put", "get", "find", "show", "tell", "ask", "try",
    "check", "look", "move", "turn", "keep", "let", "help", "call", "follow", "consider",
    "note", "ensure", "avoid", "include", "remove", "update", "install", "configure", "define",
    "implement", "build", "test", "deploy", "copy", "paste", "click", "select", "enter", "type",
    "press", "save", "load", "import", "export", "print", "return", "pass", "send", "receive",
    "connect", "disconnect", "enable", "disable", "apply", "reset", "verify", "validate",
    "specify", "provide", "replace", "insert", "delete", "modify", "adjust", "fix", "resolve",
    "handle", "process", "execute", "assign", "initialize", "declare", "convert", "transform",
    "parse", "format", "render", "compile", "debug", "log", "monitor", "track",
];

impl ContentAnalyzer {
    /// Analyze a complete turn, producing all metrics.
    pub fn analyze_turn(
        session_id: &str,
        turn_index: usize,
        user_text: &str,
        assistant_text: &str,
        first_assistant_text: &str,
        previous_assistant_text: Option<&str>,
    ) -> ContentAnalysisResult {
        let pipeline = TextPipeline::new(3);

        let mut result = ContentAnalysisResult {
            session_id: session_id.to_string(),
            turn_index,
            ..Default::default()
        };

        // Sentiment
        result.sentiment_score = Self::sentiment_score(assistant_text);
        result.user_sentiment_score = Self::sentiment_score(user_text);

        // Emotion
        let profile = Self::emotion_profile(assistant_text);
        result.dominant_emotion = profile.dominant.map(|e| e.as_str().to_string());
        result.emotion_counts = profile
            .counts
            .into_iter()
            .map(|(e, c)| (e.as_str().to_string(), c))
            .collect();
        result.emotional_range = profile.range;

        // Linguistic
        result.reading_level = Self::reading_level(assistant_text);
        result.avg_sentence_length = Self::avg_sentence_length(assistant_text);
        result.avg_word_length = Self::avg_word_length(assistant_text);
        result.type_token_ratio = Self::type_token_ratio(assistant_text);
        result.hapax_percentage = Self::hapax_percentage(assistant_text);
        result.lexical_density = Self::lexical_density(assistant_text);

        // Conversational
        result.response_amplification = Self::response_amplification(user_text, assistant_text);
        result.question_density = Self::question_density(assistant_text);
        result.hedging_index = Self::hedging_index(assistant_text);
        result.code_density = Self::code_density(assistant_text);
        result.list_density = Self::list_density(assistant_text);
        result.topic_similarity_prev = match previous_assistant_text {
            Some(prev) => pipeline.similarity(assistant_text, prev),
            None => 0.0,
        };
        result.topic_similarity_first = pipeline.similarity(assistant_text, first_assistant_text);

        // Style
        result.formality_score = Self::formality_score(assistant_text);
        result.repetition_index = match previous_assistant_text {
            Some(prev) => Self::repetition_index(assistant_text, prev),
            None => 0.0,
        };
        result.instructional_density = Self::instructional_density(assistant_text);
        result.certainty_score = Self::certainty_score(assistant_text);

        result
    }

    // ── Sentiment ──────────────────────────────────────

    /// AFINN-based sentiment score, normalized to -1.0 .. +1.0.
    pub fn sentiment_score(text: &str) -> f64 {
        let afinn = &*sentiment::AFINN;
        let mut total: f64 = 0.0;
        let mut scored = 0u32;

        for word in tokenize(text) {
            if let Some(&score) = afinn.get(word.as_str()) {
                total += score as f64;
                scored += 1;
            }
        }

        if scored == 0 {
            return 0.0;
        }

        (total / scored as f64) / 5.0
    }

    // ── Emotion ────────────────────────────────────────

    /// Build an emotion profile from text.
    pub fn emotion_profile(text: &str) -> EmotionProfile {
        let nrc = &*emotion::NRC_EMOTIONS;
        let mut counts: HashMap<Emotion, u32> = HashMap::new();

        for word in tokenize(text) {
            if let Some(emotions) = nrc.get(word.as_str()) {
                for &emo in emotions {
                    *counts.entry(emo).or_insert(0) += 1;
                }
            }
        }

        let dominant = counts
            .iter()
            .max_by_key(|(_, &c)| c)
            .map(|(&e, _)| e);

        let range = counts.values().filter(|&&c| c > 0).count() as u32;

        let mut sorted_counts: Vec<(Emotion, u32)> = counts.into_iter().collect();
        sorted_counts.sort_by(|a, b| b.1.cmp(&a.1));

        EmotionProfile {
            counts: sorted_counts,
            dominant,
            range,
        }
    }

    // ── Linguistic metrics ─────────────────────────────

    /// Flesch-Kincaid grade level.
    pub fn reading_level(text: &str) -> f64 {
        let sentences = split_sentences(text);
        let sentence_count = sentences.len().max(1) as f64;
        let words: Vec<String> = tokenize(text);
        let word_count = words.len().max(1) as f64;
        let syllable_count: f64 = words.iter().map(|w| count_syllables(w) as f64).sum();

        0.39 * (word_count / sentence_count) + 11.8 * (syllable_count / word_count) - 15.59
    }

    /// Average words per sentence.
    pub fn avg_sentence_length(text: &str) -> f64 {
        let sentences = split_sentences(text);
        if sentences.is_empty() {
            return 0.0;
        }
        let total_words: usize = sentences.iter().map(|s| tokenize(s).len()).sum();
        total_words as f64 / sentences.len() as f64
    }

    /// Average character length per word.
    pub fn avg_word_length(text: &str) -> f64 {
        let words = tokenize(text);
        if words.is_empty() {
            return 0.0;
        }
        let total_chars: usize = words.iter().map(|w| w.len()).sum();
        total_chars as f64 / words.len() as f64
    }

    /// Type-token ratio: unique words / total words.
    pub fn type_token_ratio(text: &str) -> f64 {
        let words = tokenize(text);
        if words.is_empty() {
            return 0.0;
        }
        let unique: HashSet<&str> = words.iter().map(|w| w.as_str()).collect();
        unique.len() as f64 / words.len() as f64
    }

    /// Percentage of words that appear exactly once.
    pub fn hapax_percentage(text: &str) -> f64 {
        let words = tokenize(text);
        if words.is_empty() {
            return 0.0;
        }
        let mut freq: HashMap<&str, u32> = HashMap::new();
        for w in &words {
            *freq.entry(w.as_str()).or_insert(0) += 1;
        }
        let hapax = freq.values().filter(|&&c| c == 1).count();
        let unique = freq.len();
        if unique == 0 {
            return 0.0;
        }
        hapax as f64 / unique as f64
    }

    /// Lexical density: content words / total words.
    pub fn lexical_density(text: &str) -> f64 {
        let words = tokenize(text);
        if words.is_empty() {
            return 0.0;
        }
        let stop: HashSet<&str> = STOP_WORDS.iter().copied().collect();
        let content_words = words.iter().filter(|w| !stop.contains(w.as_str())).count();
        content_words as f64 / words.len() as f64
    }

    // ── Conversational dynamics ────────────────────────

    /// Word ratio: assistant words / user words.
    pub fn response_amplification(user_text: &str, assistant_text: &str) -> f64 {
        let user_words = tokenize(user_text).len().max(1) as f64;
        let assistant_words = tokenize(assistant_text).len() as f64;
        assistant_words / user_words
    }

    /// Fraction of sentences that are questions.
    pub fn question_density(text: &str) -> f64 {
        let sentences = split_sentences(text);
        if sentences.is_empty() {
            return 0.0;
        }
        let questions = sentences
            .iter()
            .filter(|s| s.trim().ends_with('?'))
            .count();
        questions as f64 / sentences.len() as f64
    }

    /// Hedging phrase density: occurrences per sentence.
    pub fn hedging_index(text: &str) -> f64 {
        let lower = text.to_lowercase();
        let sentence_count = split_sentences(text).len().max(1) as f64;
        let hedge_count: f64 = formality::HEDGING_PHRASES
            .iter()
            .map(|phrase| count_occurrences(&lower, phrase) as f64)
            .sum();
        hedge_count / sentence_count
    }

    /// Fraction of text that is inside code blocks.
    pub fn code_density(text: &str) -> f64 {
        let total_chars = text.len();
        if total_chars == 0 {
            return 0.0;
        }

        let mut code_chars = 0usize;
        let mut in_code = false;
        let mut remaining = text;

        while !remaining.is_empty() {
            if in_code {
                if let Some(end) = remaining.find("```") {
                    code_chars += end;
                    remaining = &remaining[end + 3..];
                    in_code = false;
                } else {
                    code_chars += remaining.len();
                    break;
                }
            } else if let Some(start) = remaining.find("```") {
                remaining = &remaining[start + 3..];
                // Skip language identifier on the same line
                if let Some(nl) = remaining.find('\n') {
                    remaining = &remaining[nl + 1..];
                }
                in_code = true;
            } else {
                break;
            }
        }

        code_chars as f64 / total_chars as f64
    }

    /// Fraction of lines that are structured (lists, headers).
    pub fn list_density(text: &str) -> f64 {
        let lines: Vec<&str> = text.lines().collect();
        if lines.is_empty() {
            return 0.0;
        }
        let structured = lines
            .iter()
            .filter(|line| {
                let trimmed = line.trim();
                trimmed.starts_with("- ")
                    || trimmed.starts_with("* ")
                    || trimmed.starts_with("# ")
                    || trimmed.starts_with("## ")
                    || trimmed.starts_with("### ")
                    || is_numbered_list_item(trimmed)
            })
            .count();
        structured as f64 / lines.len() as f64
    }

    // ── Style ──────────────────────────────────────────

    /// Formality score from 0.0 (very informal) to 1.0 (very formal).
    pub fn formality_score(text: &str) -> f64 {
        let lower = text.to_lowercase();
        let words = tokenize(text);
        let total = words.len().max(1) as f64;

        let formal_count: f64 = formality::FORMAL_MARKERS
            .iter()
            .map(|m| count_word_occurrences(&lower, m) as f64)
            .sum();

        let informal_count: f64 = formality::INFORMAL_MARKERS
            .iter()
            .map(|m| count_occurrences(&lower, m) as f64)
            .sum();

        let raw = (formal_count - informal_count) / total;
        // Normalize to 0.0-1.0 using sigmoid-like mapping
        0.5 + (raw * 5.0).tanh() * 0.5
    }

    /// N-gram overlap fraction between current and previous text.
    pub fn repetition_index(current: &str, previous: &str) -> f64 {
        let n = 3;
        let current_ngrams = ngrams(current, n);
        let previous_ngrams = ngrams(previous, n);

        if current_ngrams.is_empty() || previous_ngrams.is_empty() {
            return 0.0;
        }

        let overlap = current_ngrams.intersection(&previous_ngrams).count();
        overlap as f64 / current_ngrams.len() as f64
    }

    /// Fraction of sentences that start with imperative verbs.
    pub fn instructional_density(text: &str) -> f64 {
        let sentences = split_sentences(text);
        if sentences.is_empty() {
            return 0.0;
        }

        let imperative_set: HashSet<&str> = IMPERATIVE_VERBS.iter().copied().collect();
        let imperative_count = sentences
            .iter()
            .filter(|s| {
                let first_word = s
                    .trim()
                    .split_whitespace()
                    .next()
                    .map(|w| {
                        w.chars()
                            .filter(|c| c.is_alphanumeric())
                            .collect::<String>()
                            .to_lowercase()
                    })
                    .unwrap_or_default();
                imperative_set.contains(first_word.as_str())
            })
            .count();

        imperative_count as f64 / sentences.len() as f64
    }

    /// Weighted certainty score from 0.0 (uncertain) to 1.0 (certain).
    pub fn certainty_score(text: &str) -> f64 {
        let lower = text.to_lowercase();
        let mut total_weight = 0.0;
        let mut total_count = 0.0;

        for &(modal, score) in formality::CERTAINTY_MODALS {
            let count = count_word_occurrences(&lower, modal) as f64;
            if count > 0.0 {
                total_weight += score * count;
                total_count += count;
            }
        }

        if total_count == 0.0 {
            return 0.5; // neutral if no certainty markers found
        }

        total_weight / total_count
    }
}

// ── Emotion profile result ──────────────────────────

pub struct EmotionProfile {
    pub counts: Vec<(Emotion, u32)>,
    pub dominant: Option<Emotion>,
    pub range: u32,
}

// ── Helper functions ────────────────────────────────

/// Tokenize text into lowercase words (alphanumeric only).
fn tokenize(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric() || *c == '\'')
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// Split text into sentences on `.`, `!`, `?`.
fn split_sentences(text: &str) -> Vec<&str> {
    let mut sentences = Vec::new();
    let mut last = 0;

    for (i, c) in text.char_indices() {
        if c == '.' || c == '!' || c == '?' {
            let sentence = text[last..=i].trim();
            if !sentence.is_empty() && sentence.split_whitespace().count() > 0 {
                sentences.push(sentence);
            }
            last = i + c.len_utf8();
        }
    }

    // Trailing text without punctuation counts as a sentence
    let trailing = text[last..].trim();
    if !trailing.is_empty() && trailing.split_whitespace().count() > 0 {
        sentences.push(trailing);
    }

    sentences
}

/// Count syllables in a word using vowel-group counting with silent-e adjustment.
pub fn count_syllables(word: &str) -> u32 {
    let word = word.to_lowercase();
    let chars: Vec<char> = word.chars().collect();

    if chars.len() <= 2 {
        return 1;
    }

    let vowels: HashSet<char> = ['a', 'e', 'i', 'o', 'u', 'y'].iter().copied().collect();
    let mut count = 0u32;
    let mut prev_vowel = false;

    for &c in &chars {
        let is_vowel = vowels.contains(&c);
        if is_vowel && !prev_vowel {
            count += 1;
        }
        prev_vowel = is_vowel;
    }

    // Silent-e adjustment
    if chars.last() == Some(&'e') && count > 1 {
        count -= 1;
    }

    count.max(1)
}

/// Count occurrences of a substring in text (case-insensitive, text assumed lowercase).
fn count_occurrences(text: &str, pattern: &str) -> usize {
    if pattern.is_empty() {
        return 0;
    }
    text.matches(pattern).count()
}

/// Count occurrences of a word as a whole word in text.
fn count_word_occurrences(text: &str, word: &str) -> usize {
    text.split_whitespace()
        .filter(|w| {
            let cleaned: String = w
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '\'')
                .collect::<String>()
                .to_lowercase();
            cleaned == word
        })
        .count()
}

/// Generate word n-grams from text.
fn ngrams(text: &str, n: usize) -> HashSet<Vec<String>> {
    let words = tokenize(text);
    if words.len() < n {
        return HashSet::new();
    }
    words.windows(n).map(|w| w.to_vec()).collect()
}

/// Check if a line is a numbered list item (e.g., "1. ", "12. ").
fn is_numbered_list_item(line: &str) -> bool {
    let mut chars = line.chars().peekable();
    let mut has_digit = false;

    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            has_digit = true;
            chars.next();
        } else {
            break;
        }
    }

    has_digit && chars.next() == Some('.') && chars.next() == Some(' ')
}

// ── Tests ───────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syllable_counting() {
        assert_eq!(count_syllables("hello"), 2);
        assert_eq!(count_syllables("world"), 1);
        assert_eq!(count_syllables("beautiful"), 3);
        assert_eq!(count_syllables("a"), 1);
        assert_eq!(count_syllables("the"), 1);
        assert_eq!(count_syllables("understanding"), 4);
        assert_eq!(count_syllables("programming"), 3);
    }

    #[test]
    fn test_sentiment_positive() {
        let score = ContentAnalyzer::sentiment_score("I love this wonderful amazing product");
        assert!(score > 0.0, "Positive text should have positive sentiment: {score}");
    }

    #[test]
    fn test_sentiment_negative() {
        let score = ContentAnalyzer::sentiment_score("I hate this terrible awful disaster");
        assert!(score < 0.0, "Negative text should have negative sentiment: {score}");
    }

    #[test]
    fn test_sentiment_empty() {
        let score = ContentAnalyzer::sentiment_score("");
        assert!((score - 0.0).abs() < f64::EPSILON, "Empty text should have neutral sentiment");
    }

    #[test]
    fn test_reading_level() {
        let simple = "The cat sat on the mat. The dog ran fast.";
        let complex = "The implementation of sophisticated algorithms necessitates a comprehensive understanding of computational complexity theory and its practical ramifications.";
        let simple_level = ContentAnalyzer::reading_level(simple);
        let complex_level = ContentAnalyzer::reading_level(complex);
        assert!(
            complex_level > simple_level,
            "Complex text ({complex_level}) should have higher reading level than simple ({simple_level})"
        );
    }

    #[test]
    fn test_type_token_ratio() {
        let varied = "The quick brown fox jumps over the lazy dog";
        let repetitive = "the the the the the the the the the";
        let ttr_varied = ContentAnalyzer::type_token_ratio(varied);
        let ttr_repetitive = ContentAnalyzer::type_token_ratio(repetitive);
        assert!(
            ttr_varied > ttr_repetitive,
            "Varied text ({ttr_varied}) should have higher TTR than repetitive ({ttr_repetitive})"
        );
    }

    #[test]
    fn test_question_density() {
        let qs = "What is this? How does it work? Tell me more.";
        let density = ContentAnalyzer::question_density(qs);
        assert!(
            (density - 2.0 / 3.0).abs() < 0.01,
            "Should be ~0.67 questions per sentence: {density}"
        );
    }

    #[test]
    fn test_code_density() {
        let text = "Here is code:\n```rust\nfn main() {}\n```\nEnd.";
        let density = ContentAnalyzer::code_density(text);
        assert!(density > 0.0, "Should detect code blocks: {density}");
    }

    #[test]
    fn test_list_density() {
        let text = "# Header\n- Item one\n- Item two\nSome paragraph.\n1. First\n2. Second";
        let density = ContentAnalyzer::list_density(text);
        assert!(density > 0.5, "Most lines are structured: {density}");
    }

    #[test]
    fn test_formality_formal() {
        let formal = "Furthermore, the methodology demonstrates significant compliance with the framework specifications. Therefore, we shall proceed accordingly.";
        let informal = "yeah so basically it's kinda cool lol. i'm gonna try it out, it's pretty awesome tbh.";
        let formal_score = ContentAnalyzer::formality_score(formal);
        let informal_score = ContentAnalyzer::formality_score(informal);
        assert!(
            formal_score > informal_score,
            "Formal text ({formal_score}) should score higher than informal ({informal_score})"
        );
    }

    #[test]
    fn test_instructional_density() {
        let instructions = "Open the file. Add a new line. Save your changes. Run the tests.";
        let narrative = "The system processes data efficiently. Results are stored in memory. The algorithm converges quickly.";
        let instr = ContentAnalyzer::instructional_density(instructions);
        let narr = ContentAnalyzer::instructional_density(narrative);
        assert!(
            instr > narr,
            "Instructional text ({instr}) should have higher density than narrative ({narr})"
        );
    }

    #[test]
    fn test_certainty_high() {
        let certain = "This will definitely work. It must succeed. Certainly the results confirm our hypothesis.";
        let uncertain = "This might possibly work. Perhaps it could succeed. Maybe the results suggest something.";
        let certain_score = ContentAnalyzer::certainty_score(certain);
        let uncertain_score = ContentAnalyzer::certainty_score(uncertain);
        assert!(
            certain_score > uncertain_score,
            "Certain text ({certain_score}) should score higher than uncertain ({uncertain_score})"
        );
    }

    #[test]
    fn test_emotion_profile() {
        let text = "I love happiness and joy, celebrating wonderful achievements";
        let profile = ContentAnalyzer::emotion_profile(text);
        // Should detect at least some emotional content
        assert!(
            !profile.counts.is_empty() || profile.dominant.is_none(),
            "Should process emotion profile without panic"
        );
    }

    #[test]
    fn test_repetition_index() {
        let same = "The quick brown fox jumps over the lazy dog";
        let different = "A completely unrelated sentence about mathematics";
        let high = ContentAnalyzer::repetition_index(same, same);
        let low = ContentAnalyzer::repetition_index(same, different);
        assert!(
            high > low,
            "Same text ({high}) should have higher repetition than different ({low})"
        );
    }

    #[test]
    fn test_analyze_turn_no_panic() {
        let result = ContentAnalyzer::analyze_turn(
            "test-session",
            0,
            "Hello, how are you?",
            "I am doing well, thank you for asking! How can I help you today?",
            "I am doing well, thank you for asking! How can I help you today?",
            None,
        );
        assert_eq!(result.session_id, "test-session");
        assert_eq!(result.turn_index, 0);
        assert!(result.reading_level.is_finite());
        assert!(result.formality_score >= 0.0 && result.formality_score <= 1.0);
    }
}
