# SleepLens smoke paths

Use only the static browser demo or the app's synthetic sample mode. Do not choose a real card or open the installed app's database. The demo uses the mock API and cannot read local therapy records.

Check the shared navigation and these journeys at desktop and phone widths. Also inspect 1920 and 1280 layouts before changing desktop grids.

1. Open `/demo/` with no saved state. Confirm sample nights load and the synthetic-data notice stays visible.
2. Open Overview. Change night, week, month, six-month and year views. Open a contributing score explanation and return.
3. Open Night from Overview. Select a date, inspect event details, switch simple/detailed stage views and reset chart zoom.
4. Open Trends. Change the period, choose a calendar night and return using browser Back.
5. Open Import. Confirm browser mode offers sample data only. Load it, cancel once, then load again and open the result.
6. Open Learn from a help tooltip. Close the tooltip with Escape and follow the glossary links.
7. Open Settings. Switch system, light and dark themes. Open the sample-clear confirmation, cancel, then clear the synthetic data and reload it from Import.
8. Refresh a Night deep link and use browser Back and Forward. Confirm the route and selected night stay consistent.

Use keyboard Tab, Enter and Escape on the changed controls. At phone width, check for horizontal overflow, clipped help text and hidden navigation. Read browser errors after each journey. Test sample clearing only after confirming the browser mock API and synthetic-data banner.

9. Open Your rhythm without therapy data. Skip, restart, complete all three steps and edit the result. Verify equal times cannot continue and overnight windows show the next day. Confirm schedule/alarm limitations appear. Clear answers, then check that desktop Delete all data also clears a saved profile. In browser demo mode, refresh and confirm answers disappear.
10. Inspect score owl variants for unavailable, invalid and boundary scores. They must change only the artwork, with no score or verdict changes.
