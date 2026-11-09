# SleepLens browser demo

The demo uses the current React interface from `app/src/` and its synthetic mock API. It starts with sample nights loaded. It cannot read a CPAP card or access the desktop database.

From the repository root:

```bash
npm --prefix app ci
npm --prefix app run build:demo
python3 -m http.server 8080
```

Open `http://localhost:8080/demo/`. Serve over HTTP; opening `index.html` as a local file does not load JavaScript modules in all browsers.

The build replaces generated files in `demo/`. Review and include those files when publishing an interface update. The source is shared with the desktop app, so the public demo does not need a separate screen implementation.
