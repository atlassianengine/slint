<!--
Copyright © SixtyFPS GmbH <info@slint.dev>
SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
-->

# Backdrop Blur Renderer Spike

This headless harness measures an isolated FemtoVG-WGPU backdrop blur pipeline.
It does not add a public Slint property.

FemtoVG 0.25.1's WGPU filter shader accumulates and normalizes full `vec4` samples, so its Gaussian math preserves premultiplied color and alpha.
Its Gaussian command allocates a new horizontal texture on every invocation, however, so this spike mirrors the same separable full-RGBA math in renderer-local passes backed by pooled capture, horizontal, and result textures.

The four-panel profile represents three header rails plus the navigator:

```powershell
$env:SLINT_BLUR_SPIKE_PANEL_COUNT = '4'
cargo run --release -p i-slint-renderer-femtovg --example backdrop_blur_spike --features wgpu-30
```

The six-panel profile represents all five header rails plus the navigator:

```powershell
$env:SLINT_BLUR_SPIKE_PANEL_COUNT = '6'
cargo run --release -p i-slint-renderer-femtovg --example backdrop_blur_spike --features wgpu-30
```

Both profiles default to 2560×1440, a 10-second warm-up, a 60-second sample, an 18-pixel blur radius, half-resolution blur, and a 60 FPS frame budget.
Override those defaults with `SLINT_BLUR_SPIKE_WIDTH`, `SLINT_BLUR_SPIKE_HEIGHT`, `SLINT_BLUR_SPIKE_WARMUP_SECONDS`, `SLINT_BLUR_SPIKE_SAMPLE_SECONDS`, `SLINT_BLUR_SPIKE_RADIUS`, `SLINT_BLUR_SPIKE_DOWNSAMPLE`, and `SLINT_BLUR_SPIKE_TARGET_FPS`.

The result line reports missed frames, synchronized frame percentiles, summed GPU time for the capture and two blur passes, and whether pooled texture allocation stayed stable during the sample window.
Treat a profile as passing when missed frames stay below 1%, the summed blur work is at most 4 ms p95 when timestamp queries are available, and texture allocations stay stable.
