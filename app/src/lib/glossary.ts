const LEGACY: { id: string; term: string; body: string[]; also?: string }[] = [
  { id: "hours", term: "Hours used", body: [
    "The time the machine ran with the mask on, added up across the night. Breaks with the mask off are not counted.",
    "A night with more than 4 hours is a \"valid night\". Many insurers look for valid nights on at least 70% of days (compliance)."
  ], also: "General guide: 7 hours or more is good, 4 to 7 is okay, under 4 is short." },
  { id: "ahi", term: "Events per hour (AHI)", body: [
    "AHI stands for apnea-hypopnea index: the number of apneas and hypopneas per hour of use.",
    "On therapy, under 5 an hour is generally considered the normal range. 5 to 15 is worth watching, and over 15 is worth raising with your clinician."
  ], also: "General guide only. Your clinician may set a different goal for you." },
  { id: "obstructive", term: "Obstructive apnea", body: [
    "Breathing stopped for 10 seconds or more because the airway narrowed or closed. CPAP pressure is meant to hold the airway open, so an auto machine usually raises pressure after these."
  ] },
  { id: "central", term: "Central apnea", body: [
    "Breathing stopped for 10 seconds or more while the airway was open: the body briefly did not take a breath. Raising pressure does not help these, so the machine does not respond the same way."
  ] },
  { id: "hypopnea", term: "Hypopnea", body: [
    "Breathing became much shallower for 10 seconds or more. These events are included in AHI, along with complete pauses called apneas."
  ] },
  { id: "leak", term: "Mask leak", body: [
    "Every CPAP mask has a vent that lets out the air you breathe out. That is intended leak. Extra leak is air escaping beyond the vent, usually around the mask edge or through an open mouth.",
    "The 95th percentile extra leak is the level stayed at or below for 95% of recorded time. Large leak minutes show time with over 90 L/min of escaping air."
  ], also: "Large leaks make event counts less reliable, and can dry the mouth or wake you." },
  { id: "pressure", term: "Pressure", body: [
    "The air pressure the machine delivered. An auto CPAP moves within the minimum and maximum your clinician set, rising when it senses the airway narrowing.",
    "\"Average\" is the mean across the night. \"High (90%)\" is the level pressure stayed under 90% of the time. If it often sits at the top of the range, mention it at your next visit."
  ] },
  { id: "units", term: "hPa and cmH₂O", body: [
    "Pressure units. Some machines show hectopascals (hPa), others centimetres of water (cmH₂O). 1 hPa is about 1.02 cmH₂O, so the numbers are almost the same. SleepLens uses the unit your machine is set to."
  ] },
  { id: "ramp", term: "Ramp", body: [
    "The machine starts at a lower pressure and rises slowly over a set time while you fall asleep. This can make it easier to settle into therapy."
  ] },
  { id: "comfort", term: "Comfort level (exhale relief)", body: [
    "Lowers the pressure a little each time you breathe out, so exhaling feels easier. ResMed calls this EPR. Higher levels mean more relief."
  ] },
  { id: "humidifier", term: "Humidifier", body: [
    "Warms and moistens the air to prevent a dry mouth or nose. The level is a comfort setting."
  ] },
  { id: "computed", term: "Computed by SleepLens", body: [
    "SleepLens prefers the values your machine stored in its own summary, because they match the machine's screen. When the machine did not store one (for example nights older than its summary log keeps), SleepLens works it out from the night file and labels it."
  ] }
];


export const HELP_KEYS = ["ahi", "obstructiveApnea", "centralApnea", "hypopnea", "apneaIndex", "excessLeak", "largeLeak", "pressureP90", "pressure", "ramp", "comfort", "validNight", "compliance", "sleepScore", "sleepStages", "deepSleep", "remSleep", "lightSleep", "wake", "interruptions"] as const;
export type GlossaryKey = typeof HELP_KEYS[number];
const additions: { id: string; term: string; body: string[]; also?: string }[] = [
  { id: "uncertainStage", term: "Uncertain stage", body: ["The mask was on, but leak, an event or another breathing disturbance made the signal hard to interpret. SleepLens leaves this time uncertain rather than claiming a sleep stage; no data means a recording is missing."] },
  { id: "nonRemSleep", term: "Non-REM sleep", body: ["Non-REM sleep combines light and deep sleep. The Simple view groups these together because breathing alone is much less reliable at telling deep sleep apart."] },
  { id: "stageAccuracy", term: "How accurate are sleep stages?", body: ["SleepLens now measures itself against sleep-lab recordings, and the first results are modest: telling sleep from wake is clearly better than chance, but splitting sleep into light, deep and REM is not yet reliable, and deep sleep is rarely detected. Treat the four-stage view as an experiment. Signal quality describes usable breathing, not accuracy."] },
  { id: "sleepWakeConfidence", term: "Asleep versus awake confidence", body: ["Separating asleep from awake is the part breathing does best: measured against sleep-lab recordings, SleepLens agrees clearly better than chance on this, though quiet wake can still look like sleep. It has not been checked against your own sleep study."] },
  { id: "remConfidence", term: "REM confidence", body: ["REM is often missed or confused with wake, because both look irregular. Measured against sleep-lab recordings, SleepLens finds only about a quarter of REM and is wrong often enough that this stage should be treated as an experiment."] },
  { id: "deepConfidence", term: "Deep sleep confidence", body: ["Deep sleep is the least reliable part of this estimate. In the sleep-lab recordings SleepLens has been checked against, it essentially never labelled an epoch deep, so treat any deep time shown as very uncertain."] },
  { id: "remSleepScore", term: "REM sleep score", body: ["Only estimated REM contributes to this part of the score, with low confidence. Deep sleep is not scored because it is the least reliable part of the estimate. Uncertain time is excluded, and this component is omitted when more than 40% of mask-on time is uncertain."] },
  { id: "pressureInsights", term: "Pressure insights", body: ["These observations compare recorded pressure, breathing events and leak with the range set on the machine. They can help you discuss whether your prescription still suits you with your clinician. SleepLens never changes settings."] },
  { id: "pressureRange", term: "Pressure range / prescription", body: ["The minimum and maximum pressures recorded in the machine settings describe its prescribed range. In a period view, SleepLens shows the most recent recorded range, which may differ from earlier nights. Only your clinician should change your pressure."] },
  { id: "pressureNearMax", term: "Near the maximum pressure", body: ["A night counts as near the maximum when its P90 is within 0.5 pressure units of the recorded maximum setting. This is an observation for review, not a reason to change a setting yourself."] },
  { id: "pressureTimeNearMax", term: "Time near maximum pressure", body: ["The share of recorded mask-on time within 0.5 pressure units of the maximum setting. For a period, SleepLens averages the nightly percentages; missing pressure recordings are not treated as zero."] },
  { id: "pressureEventsAtMin", term: "Events at the minimum pressure", body: ["The percentage of recorded obstructive apneas and hypopneas whose event pressure was within 0.5 pressure units of the minimum setting, including ramp. Central apneas are excluded from this percentage."] },
  { id: "treatmentEmergentCentralApnea", term: "Treatment-emergent central apnea", body: ["Central breathing pauses can appear or persist after CPAP starts treating an obstructed airway. Machine flags alone cannot confirm this condition, so discuss frequent central events with your clinician rather than changing pressure yourself."] },
  { id: "machineSummary", term: "Machine summary", body: ["The machine’s stored overnight measurements are shown here. When a stored summary is unavailable, SleepLens calculates available values from the night file. Missing measurements stay marked as not recorded."] },
  { id: "nightSettings", term: "Settings this night", body: ["The mode, pressure range and comfort settings recorded by your machine for this night. A change marker shows a recorded setting change. Discuss treatment changes with your clinician."] },
  { id: "nightCharts", term: "Through the night", body: ["These charts share one time axis. Zooming or choosing an event keeps the breathing, leak, pressure and estimated stage views aligned. Gaps mean no recorded measurement. Sleep stages are estimates from breathing, not a sleep study."] },
  { id: "apneaIndex", term: "Apnea index", body: ["The number of complete breathing pauses per hour of machine use. Unlike AHI, this count does not include shallow breathing, called hypopneas."] },
  { id: "excessLeak", term: "Extra leak", body: ["Air escaping beyond the mask's intended vent, often around the cushion or through an open mouth. The 95th percentile is the level extra leak stayed at or below for 95% of recorded time; it helps show leaks that a median can hide."] },
  { id: "largeLeak", term: "Large leak", body: ["Time when the machine recorded a large amount of escaping air, over 90 L/min on this device. Breathing event counts may be less reliable during these periods."] },
  { id: "pressureP90", term: "90th-percentile pressure (P90)", body: ["The pressure the machine stayed at or below for 90% of recorded time. It describes the higher pressures used overnight, rather than the single highest reading."] },
  { id: "validNight", term: "Valid night", body: ["A night with more than 4 hours of machine use, following this machine's rule. Exactly 4 hours does not qualify; this is a counting rule, not a full night's sleep goal."] },
  { id: "compliance", term: "Compliance", body: ["The percentage of calendar days with a valid night of more than 4 hours of use. Days without recorded use still count in the denominator; the 70% guide is a common administrative guide, not a personal treatment goal."] },
  { id: "sleepScore", term: "Sleep score", body: ["An estimated score out of 100 summarising the time you used therapy, breathing events, mask seal, interruptions and estimated sleep stages. It is a general guide to your recorded night.", "The five parts and their maximum points are Hours used 30, Breathing events 30, Mask seal 15, Unbroken sleep 15, and REM sleep 10. Deep sleep is not scored because it is the least reliable estimate. Uncertain time is excluded from stage shares, and the REM component is omitted when over 40% of mask-on time is uncertain. Missing parts are excluded and available points are scaled to 100. The ring keeps each part’s original weight; its colour matches the labelled contributor below.", "Contributor words reflect points earned: Optimal at 90% or more, Good at 70%, Fair at 45%, and Pay attention below 45%. The three-step meter groups these as Optimal, Sufficient and Poor.", "Stages are estimated from breathing, not brain activity or a sleep study. The total is estimated too, and cannot diagnose sleep quality. A score needs at least 30 minutes of use."] },
  { id: "sleepStages", term: "Estimated sleep stages", body: ["SleepLens estimates stages from breathing patterns in 30-second intervals while the mask is on. A CPAP does not measure brain activity, so these estimates can differ from a sleep study and cannot diagnose sleep stages."] },
  { id: "deepSleep", term: "Deep sleep", body: ["A stage of non-REM sleep usually more common earlier in the night. SleepLens infers it from breathing, so the amount is an estimate rather than a measurement of brain activity."] },
  { id: "remSleep", term: "REM sleep", body: ["Rapid eye movement sleep is a stage often associated with dreaming and tends to last longer toward morning. SleepLens estimates it from breathing patterns and does not measure eye movements."] },
  { id: "lightSleep", term: "Light sleep", body: ["Lighter non-REM sleep usually makes up much of the night and links other sleep stages. SleepLens infers it from breathing, which cannot confirm that you were asleep."] },
  { id: "wake", term: "Wake", body: ["Intervals when breathing patterns suggest you may have been awake while wearing the mask. Quiet wakefulness and light sleep can look similar, so this estimate may miss or overcount awake time."] },
  { id: "interruptions", term: "Interruptions", body: ["Mask-off breaks longer than one minute between sessions within a night. They show breaks in therapy, but do not count every awakening or explain why you took the mask off."] }
];
const aliases: Record<string, string> = { obstructive: "obstructiveApnea", central: "centralApnea", leak: "excessLeak" };
export const resolveGlossaryKey = (key: string): string => aliases[key] ?? key;
const revised: Record<string, { term?: string; body: string[]; also?: string }> = {
  hours: { body: ["Time the machine ran with the mask on, added up across the night and excluding breaks. More than 4 hours counts as a valid night; 7 hours or more is a general guide for a full night's therapy, rather than a personal prescription."] },
  ahi: { body: ["AHI means apnea-hypopnea index, the number of complete breathing pauses and shallow breathing events per hour of use. On therapy, under 5 an hour is a general guide; 5 to 15 is worth watching, and over 15 is worth discussing with your clinician. Your clinician may set a different goal."] },
  excessLeak: { term: "Extra leak", body: ["Air escaping beyond the mask's intended vent, often around the cushion or through an open mouth. The 95th percentile is the level extra leak stayed at or below for 95% of recorded time; it helps reveal leaks that a median can hide. Large leaks may make breathing event counts less reliable."] },
  pressure: { body: ["The air pressure delivered by the machine, within the range your clinician set. An auto CPAP raises pressure when it senses airway narrowing; the average is the mean overnight, while P90 is the level it stayed at or below for 90% of recorded time. If pressure often reaches the top of the range, mention it to your clinician."] }
};
export const GLOSSARY = [...LEGACY.map((entry) => {
  const id = aliases[entry.id] ?? entry.id;
  const revision = revised[id];
  return revision ? { id, ...revision, term: revision.term ?? entry.term } : { ...entry, id };
}), ...additions].filter((entry, i, all) => all.findIndex((x) => x.id === entry.id) === i);
// Keep historic Learn links working while all visible explanations share one source.
export const glossaryEntry = (key: string) => GLOSSARY.find((entry) => entry.id === resolveGlossaryKey(key));


/** Every glossary entry belongs to exactly one Learn section. */
export const GLOSSARY_GROUPS = [
  { id: "therapy", title: "Your therapy numbers", keys: ["hours", "validNight", "compliance", "computed", "interruptions", "machineSummary", "nightCharts"] },
  { id: "breathing", title: "Breathing events", keys: ["ahi", "apneaIndex", "obstructiveApnea", "centralApnea", "hypopnea", "treatmentEmergentCentralApnea"] },
  { id: "mask", title: "Mask & leak", keys: ["excessLeak", "largeLeak"] },
  { id: "sleep", title: "Sleep stages & score", keys: ["sleepScore", "sleepStages", "wake", "lightSleep", "deepSleep", "remSleep", "nonRemSleep", "uncertainStage", "stageAccuracy", "sleepWakeConfidence", "remConfidence", "deepConfidence", "remSleepScore"] },
  { id: "settings", title: "Settings", keys: ["pressure", "pressureP90", "units", "ramp", "comfort", "humidifier", "nightSettings", "pressureInsights", "pressureRange", "pressureNearMax", "pressureTimeNearMax", "pressureEventsAtMin"] }
];
