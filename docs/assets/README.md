# Documentation media

All committed product screenshots and animations use the original **fictional** dataset in [examples/demo](../../examples/demo/README.md). They do not contain real account history, owner quotations, keys or dashboard tokens. The screenshots show the shipped Remnary dashboard; illustrations are explicitly conceptual artwork.

| Asset | Content and origin |
| --- | --- |
| [hero.svg](hero.svg) | Original vector illustration: conversation fragments stitched into a journal. |
| [workflow.svg](workflow.svg) | Original five-stage overview of collection, evidence, analysis, review and reading. |
| [dashboard-demo.gif](dashboard-demo.gif) | Six actual browser captures: discoveries, source revision, synthetic confirmation, profile, search and timeline. A guided sequence, not real-time video. |
| [cli-demo.gif](cli-demo.gif) | Seven terminal cards rendered from actual synthetic CLI output. Commands abbreviate IDs; output is excerpted to fit. |
| [cli-transcript.txt](cli-transcript.txt) | Accessible text alternative to the terminal GIF. Temporary paths replaced with `<demo>`; no secret content. |
| [discoveries.png](discoveries.png) | Full discoveries page before demo feedback. |
| [source-evidence.png](source-evidence.png) | Exact source-revision dialog, with original role attribution. |
| [profile.png](profile.png) | The fictional Rust preference after explicit demo confirmation. |
| [search.png](search.png) | Five exact-message results for `SQLite`. |
| [timeline.png](timeline.png) | Separate consideration and choice events. |

[Asset hashes and capture identity](manifest.json) record the reviewed files.

## Rebuild the illustrations

Only Python's standard library is needed:

```sh
python3 scripts/render-doc-art.py
```

The SVGs contain their own backgrounds, accessible titles/descriptions, vector shapes and text. They have no remote fonts, linked images, scripts or other external resources. System font fallbacks may change the text rendering slightly.

## Recreate screenshots and animations

1. Build or install Remnary and run `python3 scripts/demo.py` into a new directory.
2. Open the printed dashboard command and capture the six steps described in [the demo guide](../../examples/demo/README.md). Use page-only captures, not browser chrome with account tabs or a token-bearing address bar. Preserve the real UI and its state; do not alter claims or review labels in the captures.
3. Save equal-sized viewport captures as `demo-01.png` through `demo-06.png` in an ignored private directory. Capture discoveries/profile full-page PNGs separately for the static gallery. No real-history archive should be selected.
4. With the optional **Pillow** documentation dependency available, render the guided GIFs:

```sh
python3 scripts/render-doc-media.py --binary ./target/release/remnary \
  --screenshots .remnary/readme-captures
```

Omit `--screenshots` to regenerate only the CLI animation/transcript. The CLI renderer creates and removes its own synthetic temporary encrypted archive, without reading user histories or calling a model. PNG screenshots themselves are created through browser capture, not drawn by the renderer. Screenshot frame durations are chosen for readability and do not measure performance. Animations loop; use the static PNGs or transcript for a motion-free alternative.

The initial captures were taken on 2026-10-22 from the renamed v0.1 app at its available browser viewport. All original artwork, fictional input, media and rendering scripts are covered by the root **MIT OR Apache-2.0** choice. System fonts are not bundled. Third-party notices and separately licensed compatibility fixtures retain their own terms. No assets were copied from the reference project.
