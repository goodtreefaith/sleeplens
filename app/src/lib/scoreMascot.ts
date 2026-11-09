export type OwlState = "curious" | "rest" | "gentle" | "steady" | "bright" | "happy" | "celebrate";

/** Artwork only. This does not change the score or its interpretation. */
export function scoreOwlState(score: number | null | undefined): OwlState {
  if (score == null || !Number.isFinite(score) || score < 0 || score > 100) return "curious";
  if (score < 40) return "rest";
  if (score < 50) return "gentle";
  if (score < 60) return "steady";
  if (score < 70) return "bright";
  if (score < 85) return "happy";
  return "celebrate";
}
