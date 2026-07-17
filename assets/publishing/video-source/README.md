# Video source layout

`zh/` and `en/` are self-contained locale snapshots for HyperFrames. They intentionally keep localized captions beside the matching storyboard so either public edition can be inspected without a build-time translation step.

Do not render from either locale directory directly: shared screenshots and the QIDU brand master are injected at build time and are not duplicated here. From the repository root, run:

```bash
pnpm publishing:render
```

The cross-platform renderer checks both 29-second timelines, stages their shared assets in the ignored `output/` directory, and creates compact H.264/AAC videos.
