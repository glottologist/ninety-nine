# Bayesian Detector API

The `BayesianDetector` is the core statistical engine that computes flakiness probabilities using Beta-Binomial conjugate inference.

## `BayesianDetector`

```rust
pub struct BayesianDetector {
    prior_alpha: f64,     // default: 1.0 (uniform prior)
    prior_beta: f64,      // default: 1.0 (uniform prior)
    confidence_threshold: f64,
}
```

### Constructor

```rust
pub const fn new(confidence_threshold: f64) -> Self
```

Creates a detector with a uniform Beta(1, 1) prior and the given confidence threshold.

| Parameter | Type | Description |
|-----------|------|-------------|
| `confidence_threshold` | `f64` | Minimum confidence required to classify a test as flaky (typically 0.95) |

### Methods

#### `calculate_flakiness_score`

```rust
pub fn calculate_flakiness_score(
    &self,
    test_name: &str,
    runs: &[TestRun],
) -> FlakinessScore
```

Computes a `FlakinessScore` from a set of test runs using Bayesian inference.

**Algorithm:**

1. Count passes and failures from `runs` (ignoring `Ignored` outcomes; `Failed`, `Panic`, and `Timeout` all count as failures)
2. Update the Beta prior: `alpha = prior_alpha + failures`, `beta = prior_beta + passes`
3. Compute posterior mean: `alpha / (alpha + beta)`
4. Compute posterior variance: `(alpha * beta) / ((alpha + beta)^2 * (alpha + beta + 1))`
5. Compute 95% credible interval via `Beta::inverse_cdf(0.025)` and `Beta::inverse_cdf(0.975)`
6. Confidence = `1.0 - (upper - lower)` (narrower interval = higher confidence)
7. Count consecutive trailing failures

**Returns:** A `FlakinessScore` with all computed fields populated.

#### `is_flaky`

```rust
pub fn is_flaky(&self, score: &FlakinessScore) -> bool
```

Determines whether a test should be classified as flaky.

**Criteria:** Returns `true` when **both** conditions are met:
- `score.probability_flaky > 0.01` — non-trivial failure probability
- `score.confidence >= confidence_threshold` — sufficient statistical certainty

## Internal Functions

These are not public but documented for understanding the algorithm:

| Function | Purpose |
|----------|---------|
| `count_outcomes(runs)` | Counts passes and failures, ignoring `Ignored` outcomes |
| `count_consecutive_trailing_failures(runs)` | Counts failures from the end of the run list until a pass is encountered |
| `credible_interval(alpha, beta)` | Computes 2.5th–97.5th percentile interval using the `statrs` Beta distribution |

## Usage Example

```rust
use cargo_ninety_nine::detector::BayesianDetector;

let detector = BayesianDetector::new(0.95);
let score = detector.calculate_flakiness_score("tests::my_test", &runs);

if detector.is_flaky(&score) {
    println!("{} is flaky (P={:.2})", score.test_name, score.probability_flaky);
}
```

## Statistical Properties

The detector guarantees the following properties (verified by property-based tests):

- `probability_flaky` is always in `[0.0, 1.0]`
- `confidence` is always in `[0.0, 1.0]`
- More failures relative to passes always produces a higher `probability_flaky`
- The credible interval is always valid: `lower >= 0.0`, `upper <= 1.0`, `lower <= upper`

See the [Bayesian Detection](../reference/bayesian.md) reference for the full mathematical model.
