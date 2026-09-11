// Copyright 2026 GPUI Box contributors. SPDX-License-Identifier: Apache-2.0
#pragma once
#include "host.h"

// A floating or split keyboard cannot be represented by edge occlusion.
// Evaluate against the current viewport, never the screen's pre-resize bounds.
static inline double gpui_ios_bottom_occlusion(GpuiIosRect viewport, GpuiIosRect keyboard) {
    double bottom = viewport.y + viewport.height;
    if (keyboard.x > viewport.x || keyboard.x + keyboard.width < viewport.x + viewport.width ||
        keyboard.y >= bottom || keyboard.y + keyboard.height < bottom) return 0;
    double overlap = bottom - keyboard.y;
    return overlap < viewport.height ? overlap : viewport.height;
}
