# SleepLens sleep-stage evidence

Research date: 2026-11-09. Repository read-only; no Git used. Reviewed `crates/sleeplens-core/src/sleep_stages.rs`, estimator version 2. This report concerns 30-second epochs from patient airflow, excess leak and machine events, without cardiac, oxygen or movement measurements.

## What the evidence supports

Respiration carries sleep-state information, but sensor type, treatment and label granularity matter. Nasal pressure, respiratory effort and PAP patient flow are different measurements. Four classes mean Wake/Light=N1+N2/Deep=N3/REM; three mean Wake/NREM/REM. Below, class performance means sensitivity/recall, the fraction of PSG epochs of that class recovered. It is not one-versus-rest accuracy. NR means not recovered from the inspected primary evidence, not zero. Overall accuracy and Cohen's κ are separate metrics.

| Study and population | Input and evaluation | Overall accuracy; κ | Class recall, % |
|---|---|---|---|
| Long et al. 2014, 48 sleepers [1] | Respiratory effort, learned linear discriminant, 10-fold validation | Four: 63.8%; .38. Three: 76.2%; .45 | NR |
| Aggarwal et al. 2018, 400 sampled MESA sleep-apnea participants [2] | 32 Hz nasal-pressure signal; neural CRF, subject split 60/20/20% for train/validation/test | Four: 74.1%; .57 | NR |
| Zhuang et al. 2025 [3] | Respiratory belts, deep learning; internal held-out SHHS n=1,675 and MESA n=403, older community cohorts including SDB | Four, SHHS: 80.34%; .70. MESA: 83.09%; .73 | W/L/D/R: SHHS 87.92/78.19/68.97/82.98; MESA 89.08/82.02/53.57/83.35 |
| Ahsan et al. 2026, 100 OSA titration PSGs, 70 training/30 testing [4] | PAP flow **plus pressure and demographics**, boosted features | Binary: 81.6%; .511. Three: 71.3%; .489 | Binary W/S: 66.4/86.4. Three W/NREM/R: 68.2/75.0/60.5 |
| Chen et al. 2026, Duke titration, 36 enrolled/32 analyzed; four-stage figure restricted to 8 with N3 [5] | CPAP-only pretrained model, direct inference | Reported binary: 82.1%; .470; three: 80.7%; .452; four: 77.6%; .287 | Binary W/S: 55.5/89.9; three W/NREM/R: 55.1/85.9/42.8; four W/L/D/R: 59.7/73.1/17.0/33.5 |

The first four studies are peer-reviewed journal or conference publications. Their results establish feasibility, not SleepLens accuracy. Study [5] is peer-reviewed but preliminary: only eight participants had N3. Its reported overall accuracies cannot readily be reconciled with the displayed recalls and cohort proportions as a single pooled confusion matrix. Treat these as reported values with unresolved aggregation/reporting uncertainty, not a clean performance ceiling. [1–5]

## PAP-specific and manufacturer evidence

Aggarwal's CPAP motivation does **not** mean its training signal came from treated PAP users. Its actual data were MESA nasal-pressure PSG. Chen et al.'s DFMP-CNN is genuinely PAP-specific and now published in *Physiological Measurement* in 2026 [6], superseding its 2025 medRxiv preprint. The accessible final abstract supplies no numerical results; do not silently transfer preprint metrics into the final publication.

Ahsan used ResMed S9 VPAP and Respironics OmniLab titration equipment. Chen's Duke cohort used ResMed S9 VPAP Tx. Neither establishes validity on Aeonmed AS100 home recordings. Philips-affiliated Somnolyzer-CReSS achieved four-stage κ=.68 on 296 MESA PSGs and .64 on 296 SHHS PSGs, but used cardiac information with respiratory inputs [7]. Likewise Sun et al.'s frequently cited five-stage κ=.585 and three-stage κ=.760 use ECG plus abdominal effort across a development corpus of 8,682 PSGs [8]. These are not airflow-only benchmarks. No verified large ResMed manufacturer study establishing four-stage accuracy from home PAP flow alone was found. [2,4–8]

## Wearable comparison

| PSG validation | Device | Staging κ |
|---|---|---:|
| Robbins et al. 2024, 35 healthy adults aged 20–50, one laboratory night [9] | Oura Gen3 / Apple Watch S8 / Fitbit Sense 2 | .65 / .60 / .55 |
| Miller et al. 2022, 53 healthy adults, one laboratory night [10] | Oura Gen2 / WHOOP 3.0 | .43 / .44 |

The Oura Gen3 four-stage accuracy was 76.3%, versus binary accuracy 92.0% and binary κ=.60. Miller's Oura/WHOOP four-stage accuracies were 61%/60%, versus binary 89%/86%. Versions, populations and aggregation differ. These are comparison points, not a ranking across studies or a prediction for treated OSA. [9,10]

## Useful respiratory features and failure modes

Timing and amplitude variability, inspiratory duration, inspiratory duty cycle, ventilation and waveform morphology are reasonable candidates. Douglas measured 19 healthy adults and found lower ventilation and tidal volume during REM; rate alone cannot identify it. Long showed added effort-amplitude features improved deep-sleep discrimination. PAP studies support irregular breathing as evidence for wake or arousal, but quiet wake can remain regular. Ayappa's 74-subject study found visual irregularity PPV .89 for wake and .98 for wake/arousal; automated irregularity PPV for wake was .66, not 98% staging accuracy. [1,4,11,12]

Obstruction, recovery breaths and arousals can create REM-like variability; successful CPAP can remove informative variability. Pressure adjustments and mask leak alter morphology and integrated volume. Periodic crescendo-decrescendo breathing should trigger a confounder flag, not REM or Deep; airflow alone cannot confirm Cheyne–Stokes physiology without corroboration. The current rules' equation of artifact/leak with Wake and event-context exclusion from REM/Deep can therefore bias staging. This last assessment is code-based inference informed by [4–6,12], not a validated correction.

## Expected accuracy of our heuristic

**Actual PSG accuracy is unknown.** No publication justifies assigning this code a validated accuracy interval. For engineering planning only, I would budget roughly **45–65% four-stage agreement and κ≈.10–.35**, with failures outside that range entirely possible. This is judgment anchored to the learned respiratory baselines and weak PAP deep-sleep results, not a confidence interval or publishable performance claim. Class-specific estimates would be invented. [1–5]

The code uses a 150-second context, fixed onset delays, a 90-minute REM schedule, no Deep after 60% of observed time, and deletes REM/Deep bouts under five minutes. It can fill an entire REM window when only one fifth has irregularity evidence. These choices can produce plausible architecture without recovering the correct epochs. Per-night percentiles rank regularity even when N3 is absent. Its `signal_quality` correctly describes usable breathing, not stage confidence. Source: inspected implementation.

## Prioritized improvement plan and offline feasibility

1. **Establish PSG validation first.** Freeze version 2; compare with always-Light and time-only baselines. Split participants before windowing, keeping repeat nights together. Report κ, macro-F1, every class's recall/precision, participant-bootstrap intervals and performance by residual AHI/leak/device. Expected immediate accuracy gain: zero; it makes subsequent gains measurable. [2–5]
2. **Fix uncertainty and confounders.** Separate unusable data from Wake; retain event duration/type where available, flag periodic breathing, and use robust breath statistics plus morphology at short and long contexts. Replace hard cycle/onset/bout rules with learned soft transition probabilities. Ablate each change on held-out PSG. For scale, Aggarwal's learned sequence/regularization changes raised 71.5%/.49 to 74.1%/.57 within one experiment. That is +2.6 percentage points and +.08 κ, not a transferable gain estimate. Our gain is unknown and may be negative. [1,2,4–6,12]
3. **Train a compact model offline.** Start with boosted respiratory features and a learned sequence decoder; compare a small CNN with temporal context. MESA nasal-pressure EDF plus PSG labels is the closest public pretraining candidate, not PAP ground truth. Its cohort spans ages 54–95 and enrolled 2,237 in the sleep exam; usable PSG counts depend on exclusions. SHHS supplies thousands of labeled nights but predominantly thermocouple airflow and respiratory belts. Resampling cannot erase that domain difference. Use an independent cohort, then seek synchronized PAP-flow/PSG nights for tuning and device-held-out testing. [2–8,13]
4. **Set conditional targets.** Learned non-PAP respiration studies suggest four-stage 64–83%, κ=.38–.73; direct PAP evidence is weaker, especially for N3. A practical first PAP milestone is three-state κ around .45–.60, not an assured gain over our unmeasured baseline. Offer sleep/wake or Wake/NREM/REM when four-stage evidence remains inadequate. [1–6]

Offline inference is technically feasible with bundled weights and local processing; training can happen separately. NSRR requires reviewed, dataset-specific Data Access and Use Agreements, not an unrestricted open-data license. Verify permission for product development, derived-weight redistribution and intended commercial use before shipping. ResSleepNet code availability is separately restricted to noncommercial academic use by agreement. PAP datasets in [4,5] are private/request-based. Access, licenses and paired PAP validation remain the practical blockers. [3–5,13]

## References

1. Long X, Foussier J, Fonseca P, Haakma R, Aarts RM. 2014. Analyzing respiratory effort amplitude for automated sleep stage classification. *Biomedical Signal Processing and Control* 14:197–205. [doi:10.1016/j.bspc.2014.08.001](https://doi.org/10.1016/j.bspc.2014.08.001).
2. Aggarwal K, Khadanga S, Joty S, Kazaglis L, Srivastava J. 2018. A structured learning approach with neural conditional random fields for sleep staging. *IEEE Big Data*, 1318–1327. [doi:10.1109/BigData.2018.8622286](https://doi.org/10.1109/BigData.2018.8622286). [Accessible author manuscript](https://arxiv.org/html/1807.09119v1), differently titled, inspected for methods/results.
3. Zhuang Z, Xue B, An Q, et al. 2025. Advancing sleep health equity through deep learning on large-scale nocturnal respiratory signals. *Nature Communications* 16:9334. [doi:10.1038/s41467-025-64340-y](https://doi.org/10.1038/s41467-025-64340-y), Table 2 and availability statements.
4. Ahsan M, Chen HY, Chen CYJ, et al. 2026. Predicting sleep state from continuous positive airway pressure flow in patients with obstructive sleep apnea. *Sleep Medicine* 145:109044. [doi:10.1016/j.sleep.2026.109044](https://doi.org/10.1016/j.sleep.2026.109044). [Full text](https://pmc.ncbi.nlm.nih.gov/articles/PMC13436844/), Table 2.
5. Chen HY, Husain A, Zinchuk AV, et al. 2026. Sleep stage classification during CPAP therapy from CPAP-airflow and wearable fingertip signals. *Sensors* 26:3720. [doi:10.3390/s26123720](https://doi.org/10.3390/s26123720). [Full text](https://pmc.ncbi.nlm.nih.gov/articles/PMC13306538/), Tables 4–7 and Figure 2 caption.
6. Chen HY, Ahsan M, Zinchuk AV, et al. 2026. Automatic sleep staging from CPAP airflow using a dual fusion multi-period convolutional neural network. *Physiological Measurement* 47:045011. [doi:10.1088/1361-6579/ae60df](https://doi.org/10.1088/1361-6579/ae60df). Final abstract verified; numeric final results not independently extracted. Earlier [medRxiv preprint](https://doi.org/10.1101/2025.10.15.25337643).
7. Bakker JP, Ross M, Vasko R, et al. 2021. Estimating sleep stages using cardiorespiratory signals. *Journal of Clinical Sleep Medicine* 17:1343–1354. [doi:10.5664/jcsm.9192](https://doi.org/10.5664/jcsm.9192).
8. Sun H, Ganglberger W, Panneerselvam E, et al. 2020. Sleep staging from electrocardiography and respiration with deep learning. *Sleep* 43:zsz306. [Final journal abstract](https://pubmed.ncbi.nlm.nih.gov/31863111/). Preprint reports slightly different values; final abstract used here.
9. Robbins R, Weaver MD, Sullivan JP, et al. 2024. Accuracy of three commercial wearable devices for sleep tracking in healthy adults. *Sensors* 24:6532. [doi:10.3390/s24206532](https://doi.org/10.3390/s24206532). [Full text](https://pmc.ncbi.nlm.nih.gov/articles/PMC11511193/).
10. Miller DJ, Sargent C, Roach GD. 2022. A validation of six wearable devices for estimating sleep, heart rate and heart rate variability in healthy adults. *Sensors* 22:6317. [doi:10.3390/s22166317](https://doi.org/10.3390/s22166317).
11. Douglas NJ, White DP, Pickett CK, Weil JV, Zwillich CW. 1982. Respiration during sleep in normal man. *Thorax* 37:840–844. [doi:10.1136/thx.37.11.840](https://doi.org/10.1136/thx.37.11.840).
12. Ayappa I, Norman RG, Whiting D, et al. 2009. Irregular respiration as a marker of wakefulness during titration of CPAP. *Sleep* 32:99–104. [Full text](https://pmc.ncbi.nlm.nih.gov/articles/PMC2625330/).
13. National Sleep Research Resource, official dataset/access documentation, accessed 2026-11-09. [MESA](https://sleepdata.org/datasets/mesa), [SHHS](https://sleepdata.org/datasets/shhs), [data security and reviewed agreements](https://sleepdata.org/about/data-security), [official cohort inventory](https://sleepdata.org/datasets?data=polysomnography&order=alphabetical). Documentation, not peer-reviewed performance evidence. MESA 32 Hz nasal-pressure channel verified in [2]; SHHS sensor limitations corroborated in [7].
