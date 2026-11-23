# Known incidents

One line per surprise, with the fix. New entries go on top.

- Dashboard log spam (`Connection reset by peer`, `Broken pipe`): browsers cancel media and poll requests on navigation. Client disconnects are now silent in `src/dashboard.rs`; real errors still log.
- Judge 400 from OpenCode Go: the gateway needs `x-opencode-session` and a non-generic User-Agent.
- Image size warning (`png_size` mismatch): sd-server returned a different size; the log warns and the run continues, but check `meta.json` for the real size.
- ACE-Step drops top-level `duration_sec`, `reference_audio`, `sampler_mode` and `negative_prompt`: only the nested `options` object is read.
- Remotion render needs `VIDEO_PROJECT/node_modules` plus the overlays selected by `VIDEO_PARTICLES`; the stage fails with the fix command instead of falling back silently.
- The run is not resumable and a failed stage aborts it. `output/` and `target/` are gitignored.
