use ollama_scope::analysis::TextPipeline;
use proptest::prelude::*;

#[test]
fn test_stemming_snapshot() {
    let pipeline = TextPipeline::new(1);
    let result = pipeline.process("running jumps easily connected");
    let mut sorted: Vec<String> = result.into_iter().collect();
    sorted.sort();
    insta::assert_yaml_snapshot!(sorted);
}

#[test]
fn test_unicode_input_no_panic() {
    let pipeline = TextPipeline::new(1);
    let result = pipeline.process("日本語 テスト 🎉 café résumé");
    // Should not panic — may or may not produce useful stems for non-Latin text
    let _ = result;
}

#[test]
fn test_similarity_partial_overlap() {
    let pipeline = TextPipeline::new(1);
    let score = pipeline.similarity("quick brown fox", "quick red fox");
    assert!(score > 0.0, "Partial overlap should be > 0");
    assert!(score < 1.0, "Partial overlap should be < 1");
}

proptest! {
    #[test]
    fn prop_similarity_bounded(a in "[a-z ]{0,50}", b in "[a-z ]{0,50}") {
        let pipeline = TextPipeline::new(1);
        let score = pipeline.similarity(&a, &b);
        prop_assert!(score >= 0.0 && score <= 1.0, "Score must be in [0,1], got {score}");
    }

    #[test]
    fn prop_similarity_symmetric(a in "[a-z ]{1,30}", b in "[a-z ]{1,30}") {
        let pipeline = TextPipeline::new(1);
        let ab = pipeline.similarity(&a, &b);
        let ba = pipeline.similarity(&b, &a);
        prop_assert!((ab - ba).abs() < f64::EPSILON, "Similarity must be symmetric: {ab} vs {ba}");
    }

    #[test]
    fn prop_self_similarity_is_one(text in "[a-z]{3,20}( [a-z]{3,20}){1,5}") {
        let pipeline = TextPipeline::new(1);
        let score = pipeline.similarity(&text, &text);
        prop_assert!((score - 1.0).abs() < f64::EPSILON, "Self-similarity should be 1.0, got {score}");
    }
}
