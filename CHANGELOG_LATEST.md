## 0.3.1 - 2026-11-27

- **Backdrop no longer hides shell chrome**: `paint_backdrop` paints via the central panel Ui (panel rect + panel painter, behind page content) instead of the full-window background layer. Animated styles stay inside the content area; menu bar, nav rail, status bar, and detail dock are untouched.
- **Freeze toggle**: `SkinState.animate_backdrop` (default on, backfilled for existing `skin.json`) + settings checkbox. Uncheck to still animated styles on one frame and stop the 120ms repaint loop.
- **Regression test**: `backdrop_never_paints_outside_central_rect` renders all six styles headless and asserts every emitted shape stays clipped to the central rect.
