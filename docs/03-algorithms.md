# 03 — Algorithms

## EWMA

Recurrence:

```text
y[0] = x[0]
y[k] = alpha * x[k] + (1 - alpha) * y[k-1]
```

Configuration:
- `0 < alpha <= 1`.
- `alpha = 1` is identity.
- Initialize from first observed sample by default.
- `reset()` returns the filter to uninitialized state.

Tests:
- alpha validation;
- exact known sequence;
- alpha=1 identity;
- constant sequence remains constant.

## Sliding-window median

Maintain the most recent odd `window` samples. For the startup prefix use all samples currently available, not zero-padding.

For an even startup count, define median as the arithmetic mean of the two middle sorted values. Once the configured odd window is full, the median is a single middle value.

The implementation sorts a small copied window on every update. The point is clarity, not asymptotic optimization.

Configuration:
- `window >= 1`;
- configured window must be odd.

Important behavior:
- a single extreme impulse should be rejected after enough neighboring normal samples exist;
- output length equals input length.

## Scalar Kalman filter

Use the random-walk state model:

```text
x_k = x_(k-1) + w_k
z_k = x_k + v_k

w ~ N(0, Q)
v ~ N(0, R)
```

Per measurement:

```text
Predict:
    x_prior = x
    P_prior = P + Q

Update:
    K = P_prior / (P_prior + R)
    x = x_prior + K * (z - x_prior)
    P = (1 - K) * P_prior
```

Initialization:
- the estimate is initialized from the first measurement;
- covariance starts at `P = p0`;
- the first measurement is emitted directly, and subsequent measurements run the predict/update equations.

Validation:
- `Q >= 0`;
- `R > 0`;
- `P0 >= 0`;
- all finite.

Numerical invariants:
- covariance must remain finite and non-negative;
- gain should remain within `[0, 1]` for valid scalar parameters;
- reject non-finite measurements at the domain boundary rather than poisoning state.

## Synthetic signal

Truth:

```text
truth[i] = amplitude * sin(2*pi*cycles_per_sample*i + phase)
```

Noise:

```text
noisy[i] = truth[i] + gaussian(0, sigma)
```

Impulse injection:
- independently draw one Bernoulli event per sample;
- when triggered, add a signed spike;
- simple deterministic rule: magnitude uniformly distributed from `0.5 * spike_amplitude` to `spike_amplitude`, sign chosen uniformly;
- record `spike_mask[i] = true`.

Parameter validation:
- samples > 0;
- amplitude finite;
- cycles/sample > 0 and finite;
- sigma >= 0 and finite;
- spike probability in `[0,1]`;
- spike amplitude >= 0 and finite.

## Metrics

For reference `r[i]` and estimate `y[i]`:

```text
error[i] = y[i] - r[i]
RMSE = sqrt(mean(error^2))
MAE = mean(abs(error))
MaxAbs = max(abs(error))
```

Signal/noise SNR when clean reference is known:

```text
signal_power = mean(reference^2)
error_power = mean((estimate-reference)^2)
SNR_dB = 10 * log10(signal_power/error_power)
```

Special cases:
- zero error power -> positive infinity SNR represented as an enum/status or formatted `inf`, never panic;
- zero signal power with nonzero error -> return `None` for SNR and print `n/a`.

SNR improvement:

```text
output_snr - input_snr
```

## Spike metrics

Only valid when a synthetic/injected spike mask exists.

`spike_rmse`:
- RMSE evaluated only at spike indices.

`recovery_samples`:
- after each spike, find first subsequent sample where absolute error is <= configurable tolerance for `stable_count` consecutive samples;
- default tolerance = `0.10 * max(1.0, truth amplitude)` for sensor mode;
- default stable count = 3;
- report mean and max recovery length.

Keep this metric isolated so its definition can evolve without changing core filtering.

## Pseudo-reference for CSV without ground truth

Purpose: obtain a stable offline target for relative Kalman parameter search. It is not ground truth.

Construction:

1. Hampel-style despiking
   - window radius = 3 (7 points where full);
   - local median;
   - MAD = median(|x - median|);
   - robust sigma = 1.4826 * MAD;
   - if robust sigma > 0 and `|x-median| > 3*robust_sigma`, replace with local median;
   - if MAD is zero, only replace values that differ from a constant local neighborhood by a meaningful epsilon.

2. Zero-phase EWMA approximation
   - run EWMA forward with `alpha_ref = 0.10`;
   - reverse that output;
   - run same EWMA again;
   - reverse back.

This offline reference intentionally uses future samples, so it must never be presented as an online filter.

## Kalman Q/R grid search

Use logarithmic candidate ranges.

Default candidate multipliers:

```text
1, 3
```

Default Q decades:

```text
1e-8 .. 1e-1
```

Default R decades:

```text
1e-6 .. 1e1
```

This produces 16 Q values x 16 R values = 256 candidates if both endpoints and multipliers fit each decade. Generate candidates in ascending numeric order and deduplicate exact repeats.

For each `(Q,R)`:
- instantiate a fresh filter;
- run the complete measurement sequence;
- compute RMSE vs selected reference;
- record candidate result.

Winner ordering:
1. finite RMSE before non-finite;
2. lower RMSE;
3. lower Q;
4. lower R.

Output top N (default 5) candidates and best values.

## Audio noise model

Normalize PCM16 sample `s`:

```text
x = s / 32768.0
```

Gaussian component:

```text
x' = x + N(0, sigma)
```

Impulse component:
- Bernoulli event per sample per channel;
- if event: add signed impulse with magnitude in `[0.5*A, A]`;
- clamp only when converting to PCM output.

Use one seeded RNG stream for deterministic interleaved sample processing. Document that changing channel count changes RNG consumption and therefore noise realization.
