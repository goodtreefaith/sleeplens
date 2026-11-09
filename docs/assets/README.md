# Repository visuals

## Transparent owl derivative

The approved mascot was isolated with the built-in imagegen tool. Its source has
an alpha channel, confirmed with `sips -g hasAlpha`. The app and demo use bundled
copies; Tauri generated the platform icon sizes. No installed app was replaced.

Exact edit prompt:

```text
Use case: precise-object-edit
Asset type: transparent mascot-only brand asset and app icon source.
Input image role: edit target, accepted SleepLens sleepy owl logo.
Primary request: Isolate ONLY the exact sleepy owl character from the supplied accepted logo. Remove the SleepLens text and remove the off-white outer background. Center the complete owl at large size on a square transparent canvas with generous 10% clear padding. Keep the owl's exact proportions, teal body, pale belly, closed charcoal eyes, orange beak, feet and pale teal circular lens around its right eye. Preserve white/off-white face areas as opaque within the character.
Style: preserve existing accepted flat illustration exactly.
Constraints: actual transparent alpha background, no text, no new symbols, no shadows, no pedestal, no clipped ears or feet, no redesign.
```

`sleeplens-banner.png` is the README brand banner. It follows the app's teal,
warm off-white and charcoal palette and its circular lens/waveform motif.
It is an earlier repository artwork version. The accepted owl mascot below now supplies the app branding.

Generated on 2026-11-09 with the built-in imagegen tool, in `logo-brand` mode.
The prompt was:

```text
Use case: logo-brand
Asset type: GitHub README brand banner for SleepLens, an open-source CPAP data viewer for macOS.
Primary request: Create a refined wide landscape brand banner, approximately 3:1. Use the project's existing circular lens with a short breathing waveform through its center as the logo motif, with the word "SleepLens" in large crisp sans-serif typography alongside it. A quiet abstract breathing trace can extend across the right side. Match the existing app palette: warm off-white #F6F5F1 background, deep teal #1D6870 logo and wordmark, charcoal #1B242B, pale teal #E1EFEF details.
Style/medium: Flat graphic brand design, restrained and precise, generous whitespace, sharp geometry. No mockup, no 3D.
Text (verbatim): "SleepLens" and below it "Your CPAP data, in plain language."
Composition/framing: wide horizontal banner, centered content with generous outer margins, readable at README width.
Constraints: Only the exact text requested, no extra words, no badges, no clinical claims, no charts with numbers, no medical cross, no stock photography, no watermark. Preserve the lens-and-waveform identity.
```

`overview.png` is an unedited screenshot captured on 2026-11-09 from the React
app with the uncommitted owl and rhythm changes based on commit `ec10d49`,
running in browser sample mode at 1920 × 1080. The preview used only the built-in mock API
and synthetic nights. It contains no imported card records. Refresh it when
the interface changes; continue to use sample mode for public screenshots.

## Owl mascot, approved 2026-11-09

Faith chose the sleepy owl concept on 2026-11-09 and asked to use it throughout the project. `sleeplens-owl-banner.png` is the accepted full logo lockup, generated with built-in imagegen in `logo-brand` mode. The previous `sleeplens-banner.png` is retained as an earlier version.

The full-logo prompt was:

```text
Use case: logo-brand
Asset type: mascot logo concept for SleepLens, a CPAP data desktop app.
Primary request: Design a distinctive, lovable sleepy owl mascot beside the exact wordmark "SleepLens". The owl should feel calm, reassuring and suitable for an adult health-data tool. Its rounded face has gently closed eyes, tiny ear tufts, small folded wings and one subtle circular lens-like facial shape. Use a compact memorable silhouette with few bold shapes that would remain recognizable as a small app icon. Avoid medical equipment on the character.
Style/medium: Professionally drawn flat vector-style brand artwork, crisp edges, tasteful character design, no 3D, no photorealism. Friendly without looking like a children's toy. Custom rounded sans-serif wordmark with good legibility.
Color palette: Existing SleepLens deep teal #1D6870, pale teal #E1EFEF, charcoal #1B242B and warm off-white #F6F5F1. At most one muted warm accent on the owl's small beak.
Composition: Wide horizontal logo lockup, approximately 2:1 image, mascot left, wordmark right, balanced spacing, generous clean margins. Warm off-white solid background. Show just one finished logo, no mockups or multiple concepts.
Text (verbatim): "SleepLens"
Constraints: No tagline, no extra text, no watermark, no medical cross, no heartbeat trace, no crescent moon replacing letters, no complex feather texture. Create a new mascot identity rather than the prior abstract waveform logo.
```

`app/src-tauri/icons/source-owl.png` is the transparent mascot-only source. The app, favicon and native icon sizes derive from it using Tauri's `icon` command. `app/public/sleeplens-owl.png` is the 256-pixel web asset; the full-resolution source keeps its alpha channel.

The public demo now builds from `app/src/` with the same screens and synthetic mock API. Run `npm --prefix app run build:demo` to refresh it. No real card records belong in these assets.

## Score expressions and rhythm companion

The full-resolution transparent sources and exact imagegen prompts live in [mascots/manifest.json](mascots/manifest.json). The web variants in `app/public/mascots/` are 256-pixel PNGs with transparency preserved. The static demo copies the same assets.

The score artwork map is decorative: 0 to 39 uses Rest, 40 to 49 Gentle, 50 to 59 Steady, 60 to 69 Bright, 70 to 84 Happy and 85 to 100 Celebrate. A missing, non-finite or out-of-range score uses Curious. The original owl remains the brand logo. This map changes no score formula, verdict or therapy recommendation.

The apricot fox accompanies the optional Your rhythm screen. It does not identify a biological chronotype or carry a validated productivity claim.
