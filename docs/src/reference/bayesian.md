# Bayesian Detection

## Overview

`cargo ninety-nine` uses Bayesian inference to estimate the probability that a test is flaky. This approach provides calibrated uncertainty estimates rather than simple pass/fail ratios.

## The Model

### Prior

The model starts with a uniform (uninformative) prior: Beta(1, 1). This represents no prior knowledge — any flakiness probability from 0 to 1 is equally likely before observing data.

### Posterior Update

After observing test runs, the posterior distribution is:

```
Beta(alpha, beta)

where:
  alpha = prior_alpha + failures
  beta  = prior_beta  + passes
```

The **posterior mean** is used as the flakiness probability:

```
P(flaky) = alpha / (alpha + beta)
```

### Credible Interval

A 95% credible interval is computed from the Beta distribution's inverse CDF:

```
CI = [Beta.inverse_cdf(0.025), Beta.inverse_cdf(0.975)]
```

This interval narrows as more data is collected.

### Confidence

Confidence is derived from the width of the credible interval:

```
confidence = 1.0 - (CI_upper - CI_lower)
```

Narrow intervals yield high confidence; wide intervals yield low confidence.

## Classification

A test is classified as **flaky** when:

```
P(flaky) > 0.01 AND confidence >= confidence_threshold
```

The `confidence_threshold` is configurable (default: 0.95).

### Flakiness Categories

| Category | P(flaky) Range |
|----------|---------------|
| Stable | < 1% |
| Occasional | 1% — 5% |
| Moderate | 5% — 15% |
| Frequent | 15% — 30% |
| Critical | > 30% |

## Practical Implications

### Number of Runs

With the Beta(1,1) prior:

- **10 runs, 1 failure**: P(flaky) = 2/12 = 16.7%, wide CI → low confidence
- **100 runs, 1 failure**: P(flaky) = 2/102 = 2.0%, narrow CI → high confidence
- **100 runs, 0 failures**: P(flaky) = 1/102 = 1.0%, classified as Stable

More runs yield narrower credible intervals and more reliable classifications.

### Prior Effect

The uniform prior has minimal effect when there are many observations. With 100+ runs, the prior contributes < 2% to the posterior. For small sample sizes (< 10 runs), the prior pulls estimates toward 50%, which is conservative.

## Stored Parameters

Each `FlakinessScore` record includes the full Bayesian parameters:

| Field | Description |
|-------|-------------|
| `alpha` | Posterior alpha (prior + failures) |
| `beta` | Posterior beta (prior + passes) |
| `posterior_mean` | alpha / (alpha + beta) |
| `posterior_variance` | (alpha * beta) / (total^2 * (total + 1)) |
| `credible_interval_lower` | 2.5th percentile of posterior |
| `credible_interval_upper` | 97.5th percentile of posterior |
