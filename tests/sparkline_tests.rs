use ollama_scope::sparkline::SparklineData;
use std::collections::VecDeque;

#[test]
fn test_sparkline_push_within_capacity() {
    let mut data = SparklineData::new(10);
    for i in 0..5 {
        data.push(i as f64);
    }
    assert_eq!(data.samples.len(), 5);
    assert_eq!(data.samples, VecDeque::from(vec![0.0, 1.0, 2.0, 3.0, 4.0]));
}

#[test]
fn test_sparkline_push_exceeds_capacity() {
    let mut data = SparklineData::new(3);
    for i in 0..7 {
        data.push(i as f64);
    }
    assert_eq!(data.samples.len(), 3);
    assert_eq!(data.samples, VecDeque::from(vec![4.0, 5.0, 6.0]));
}

#[test]
fn test_sparkline_current_empty() {
    let data = SparklineData::new(10);
    assert_eq!(data.current(), 0.0);
}

#[test]
fn test_sparkline_current_returns_last() {
    let mut data = SparklineData::new(10);
    data.push(1.0);
    data.push(2.0);
    data.push(3.0);
    assert_eq!(data.current(), 3.0);
}

#[test]
fn test_sparkline_peak_empty() {
    let data = SparklineData::new(10);
    assert_eq!(data.peak(), 0.0);
}

#[test]
fn test_sparkline_peak_returns_max() {
    let mut data = SparklineData::new(10);
    data.push(5.0);
    data.push(15.0);
    data.push(3.0);
    data.push(10.0);
    assert_eq!(data.peak(), 15.0);
}

#[test]
fn test_sparkline_peak_after_trim() {
    let mut data = SparklineData::new(3);
    data.push(100.0); // will be trimmed
    data.push(1.0);
    data.push(2.0);
    data.push(3.0);
    // 100.0 was trimmed off
    assert_eq!(data.peak(), 3.0);
}

#[test]
fn test_sparkline_capacity_one() {
    let mut data = SparklineData::new(1);
    data.push(1.0);
    data.push(2.0);
    data.push(3.0);
    assert_eq!(data.samples.len(), 1);
    assert_eq!(data.current(), 3.0);
}

#[test]
fn test_sparkline_zero_values() {
    let mut data = SparklineData::new(5);
    for _ in 0..5 {
        data.push(0.0);
    }
    assert_eq!(data.peak(), 0.0);
    assert_eq!(data.current(), 0.0);
}
