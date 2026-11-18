# Retrospect Slint fork patch ledger

Base upstream release: `v1.19.0` (`e436f546f71481d786cbc2a74316b1be1ef678c1`).

This fork carries only framework changes that downstream consumers cannot yet get from upstream.

| Patch | Reason | Upstream issue | Affected crates | Remove when | Last rebased upstream tag |
| --- | --- | --- | --- | --- | --- |
| AccessKit `text_state` keyed by accessible wrapper item | Nested accessible wrappers can refer to the same inner `TextInput`; each wrapper must own its `TextRun` node IDs and selection map. | Not filed. | `i-slint-backend-winit` | Upstream keys `NodeCollection::text_state` by the wrapper `ItemRc`. | `v1.19.0` |
| Winit/Windows Ink pointer-pressure bridge | Expose per-window normalized native force through `TouchArea.pressure`; Windows fallback queries `GetPointerPenInfo` only when the live pen ID, target `HWND`, client-pixel position, and pressure bit agree. Mouse and unknown devices remain `1.0`. | Not filed; review upstream after hardware verification. | `i-slint-core`, `i-slint-compiler`, `i-slint-backend-winit` | Upstream exposes equivalent Winit/Win32 pointer force including missing-force pen events. | `v1.19.0` |

## Retired patches (subsumed by upstream `v1.19.0`)

- Custom `Rectangle.background-blur` FemtoVG-WGPU implementation → replaced by native `Rectangle.backdrop-blur` across Skia and FemtoVG (WGPU and OpenGL).
- `BoxShadowCache::clear_all()` → upstream `1.19.0` provides `BoxShadowCache::clear()` and calls it in FemtoVG context teardown.
- FemtoVG WGPU swapchain surface format fallback & `Queue::present(frame)` → upstream `1.19.0` includes both.
