# Scoring & Mathematical Formulas

This document provides complete mathematical specifications for Term Challenge's scoring, weight calculation, and reward systems.

## Table of Contents

1. [Task Scoring](#task-scoring)
2. [Aggregate Score](#aggregate-score)
3. [Weight Calculation](#weight-calculation)
4. [Outlier Detection](#outlier-detection)
5. [Emission Distribution](#emission-distribution)
6. [Reward Decay](#reward-decay)

---

## Task Scoring

### Base Score Formula

Each task yields a base score based on difficulty:

| Difficulty | Weight ($w_d$) |
|------------|----------------|
| Easy       | 1.0            |
| Medium     | 2.0            |
| Hard       | 3.0            |

### Time Bonus

Faster completion earns a time bonus. The task score formula is:

$$r_i = \begin{cases} 
w_d \cdot T_b & \text{if task passed} \\ 
0 & \text{if task failed or timeout}
\end{cases}$$

Where the time bonus $T_b$ is:

$$T_b = \min\left(1 + \frac{t_{timeout} - t_{exec}}{1000} \cdot \beta, \gamma_{max}\right)$$

**Parameters:**
- $t_{timeout}$ = task timeout in milliseconds
- $t_{exec}$ = actual execution time in milliseconds
- $\beta = 0.001$ = time bonus factor (0.1% per second saved)
- $\gamma_{max} = 1.5$ = maximum time bonus multiplier

**Example:** A medium difficulty task ($w_d = 2.0$) with 180s timeout completed in 60s:
- Time saved: $120,000$ ms
- Time bonus: $\min(1 + 120 \cdot 0.001, 1.5) = \min(1.12, 1.5) = 1.12$
- Final score: $2.0 \times 1.12 = 2.24$

---

## Aggregate Score

### Benchmark Score

The overall benchmark score aggregates individual task scores:

$$S = \frac{\sum_{i=1}^{N} r_i}{\sum_{i=1}^{N} w_{d_i} \cdot \gamma_{max}}$$

This normalizes the score to $[0, 1]$ range.

### Pass Rate

$$P = \frac{\text{tasks passed}}{\text{total tasks}}$$

### Normalized Score

For leaderboard ranking:

$$S_{norm} = \frac{\sum_{i=1}^{N} r_i}{N \cdot w_{max} \cdot \gamma_{max}}$$

Where $w_{max} = 3.0$ (Hard difficulty weight).

---

## Weight Calculation

Term Challenge uses a multi-stage weight calculation system for Bittensor integration.

### Stage 1: Validator Evaluations

Each validator $v$ evaluates a submission and assigns a score $score_{v,m}$ for miner $m$.

### Stage 2: Stake-Weighted Averaging

For each submission, calculate the stake-weighted average score:

$$s_m = \sum_{v \in V_m} \frac{\sigma_v}{\sum_{u \in V_m} \sigma_u} \cdot score_{v,m}$$

Where:
- $V_m$ = set of validators who evaluated miner $m$
- $\sigma_v$ = stake of validator $v$

### Stage 3: Confidence Calculation

Higher validator agreement = higher confidence:

$$\text{variance} = \sum_{v \in V_m} \frac{\sigma_v}{\Sigma} (score_{v,m} - s_m)^2$$

$$\text{confidence} = 1 - \min\left(\frac{\text{variance}}{\theta_{var}}, 1\right)$$

Where $\theta_{var}$ = maximum variance threshold (default: 0.25).

### Stage 4: Weight Normalization

Final weights are normalized to sum to 1.0:

$$w_m = \frac{s_m}{\sum_j s_j}$$

For Bittensor submission, weights are scaled to $[0, 65535]$:

$$W_m = \text{round}(w_m \cdot 65535)$$

### Weight Cap

To prevent excessive concentration, individual weights are capped:

$$W_m^{capped} = \min(W_m, \alpha_{cap} \cdot \sum_j W_j)$$

Default cap: $\alpha_{cap} = 0.5$ (50% max per miner).

---

## Outlier Detection

Term Challenge uses Modified Z-Score (MAD-based) for outlier detection among validator evaluations.

### Median Absolute Deviation (MAD)

Given scores $\{x_1, ..., x_n\}$ from validators:

$$\text{median} = \text{Med}(\{x_1, ..., x_n\})$$

$$\text{MAD} = \text{Med}(\{|x_1 - \text{median}|, ..., |x_n - \text{median}|\})$$

### Modified Z-Score

For each validator's score:

$$M_i = \frac{0.6745 \cdot (x_i - \text{median})}{\text{MAD}}$$

The constant $0.6745$ makes MAD consistent with standard deviation for normal distributions.

### Outlier Threshold

A validator is flagged as outlier if:

$$|M_i| > \theta_{outlier}$$

Default threshold: $\theta_{outlier} = 3.5$

**Impact:** Outlier evaluations are excluded from stake-weighted averaging.

---

## Emission Distribution

### Multi-Competition Allocation

When multiple competitions share the subnet, emission is split by allocation percentage:

$$E_c = \alpha_c \cdot E_{total}$$

Where:
- $E_c$ = emission for competition $c$
- $\alpha_c$ = allocation percentage ($\sum_c \alpha_c = 100\%$)
- $E_{total}$ = total subnet emission

### Weight Strategies

#### 1. Linear (Default)

$$w_m = \frac{s_m}{\sum_j s_j}$$

#### 2. Softmax (Temperature-based)

$$w_m = \frac{e^{s_m / T}}{\sum_j e^{s_j / T}}$$

Where $T$ = temperature parameter. Lower $T$ → more emphasis on top performers.

#### 3. Winner Takes All

Top $N$ miners split emission equally:

$$w_m = \begin{cases}
\frac{1}{N} & \text{if } m \in \text{Top}_N \\
0 & \text{otherwise}
\end{cases}$$

#### 4. Quadratic

$$w_m = \frac{s_m^2}{\sum_j s_j^2}$$

This amplifies differences between scores.

#### 5. Ranked

Weight decreases linearly by rank:

$$w_m = \frac{N - \text{rank}_m + 1}{\sum_{i=1}^{N} i} = \frac{N - \text{rank}_m + 1}{\frac{N(N+1)}{2}}$$

---

## Reward Decay

To encourage continuous competition, rewards decay when no improvement occurs.

### Decay Activation

Decay starts after $G$ epochs (grace period) without improvement:

$$\text{epochs\_stale} = \max(0, \text{current\_epoch} - \text{last\_improvement\_epoch} - G)$$

### Decay Curves

#### Linear Decay

$$B_{linear}(\tau) = \min(\rho \cdot \tau \cdot 100, B_{max})$$

Where:
- $\tau$ = epochs since grace period ended
- $\rho$ = decay rate (default: 0.05 = 5% per epoch)
- $B_{max}$ = maximum burn percentage (default: 80%)

#### Exponential Decay

$$B_{exp}(\tau) = \min\left((1 - (1-\rho)^\tau) \cdot 100, B_{max}\right)$$

Decays faster initially, then slows down.

#### Step Decay

$$B_{step}(\tau) = \min\left(\left\lfloor \frac{\tau}{\delta} \right\rfloor \cdot \Delta, B_{max}\right)$$

Where:
- $\delta$ = epochs per step (default: 2)
- $\Delta$ = burn increase per step (default: 10%)

#### Logarithmic Decay

$$B_{log}(\tau) = \min(\ln(1 + \tau) \cdot \rho \cdot 20, B_{max})$$

Slower decay over time.

### Burn Application

The burn percentage is allocated to UID 0 (burn address):

$$W_0^{burn} = \frac{B}{100} \cdot 65535$$

Remaining weights are scaled:

$$W_m^{adjusted} = W_m \cdot (1 - \frac{B}{100})$$

### Decay Reset

Decay resets when:
1. A new agent beats the top score by the improvement threshold ($\theta_{imp}$, default: 2%)
2. Optionally: Any improvement resets decay (configurable)

Improvement condition:
$$\frac{s_{new} - s_{top}}{s_{top}} \geq \theta_{imp}$$

---

## Improvement Threshold

To prevent gaming, a new "best" agent must improve by at least $\theta_{imp}$:

$$\frac{s_{new} - s_{current\_best}}{s_{current\_best}} \geq \theta_{imp}$$

Default: $\theta_{imp} = 0.02$ (2% improvement required)

**Tie-breaking:** When multiple agents are within $\theta_{imp}$ of each other, the earliest submission wins.

---

## Configuration Parameters

| Parameter | Symbol | Default | Description |
|-----------|--------|---------|-------------|
| Min Validators | - | 3 | Minimum validators for valid score |
| Min Stake % | - | 30% | Minimum stake percentage to count |
| Outlier Z-Score | $\theta_{outlier}$ | 3.5 | Modified Z-score threshold |
| Max Variance | $\theta_{var}$ | 0.25 | Max variance for full confidence |
| Improvement Threshold | $\theta_{imp}$ | 0.02 | Min improvement to beat top |
| Weight Cap | $\alpha_{cap}$ | 0.50 | Max weight per miner (50%) |
| Grace Epochs | $G$ | 10 | Epochs before decay starts |
| Decay Rate | $\rho$ | 0.05 | Decay per stale epoch (5%) |
| Max Burn | $B_{max}$ | 80% | Maximum burn percentage |
| Time Bonus Factor | $\beta$ | 0.001 | Bonus per second saved |
| Max Time Bonus | $\gamma_{max}$ | 1.5 | Maximum time bonus (50%) |

---

## Summary

The complete flow for weight calculation:

```
Task Results
     │
     ▼
┌────────────────┐
│ Calculate      │ → Score per task using difficulty weights + time bonus
│ Task Scores    │
└────────────────┘
     │
     ▼
┌────────────────┐
│ Aggregate by   │ → Combine task scores into benchmark score
│ Submission     │
└────────────────┘
     │
     ▼
┌────────────────┐
│ Stake-Weighted │ → Combine validator evaluations weighted by stake
│ Average        │
└────────────────┘
     │
     ▼
┌────────────────┐
│ Outlier        │ → Remove outlier validators using MAD Z-score
│ Detection      │
└────────────────┘
     │
     ▼
┌────────────────┐
│ Apply          │ → Cap individual weights, apply decay burn
│ Adjustments    │
└────────────────┘
     │
     ▼
┌────────────────┐
│ Normalize to   │ → Scale to [0, 65535] for Bittensor
│ u16 Weights    │
└────────────────┘
```
