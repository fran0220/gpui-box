// Copyright 2026 GPUI Box contributors. SPDX-License-Identifier: Apache-2.0
#pragma once
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

// All callbacks execute synchronously on the main thread. The context and
// callback table must remain alive until gpui_ios_destroy returns. Callbacks
// must not unwind through Objective-C. Strings are UTF-8, ranges are UTF-16.
typedef struct { double x, y, width, height; } GpuiIosRect;
typedef struct { size_t location, length; bool valid; } GpuiIosRange;
typedef struct { double top, right, bottom, left; } GpuiIosEdges;
typedef struct { size_t offset; bool upstream, valid; } GpuiIosPosition;
typedef struct { GpuiIosPosition anchor, head; } GpuiIosSelection;
typedef struct {
    GpuiIosRect bounds;
    bool rtl, contains_start, contains_end, vertical;
} GpuiIosSelectionRect;
typedef struct {
    void *context;
    void (*launched)(void *);
    // 0 active, 1 inactive, 2 background, 3 foreground; inactive arrives
    // after touch cancellation and before the display link is paused.
    void (*lifecycle)(void *, uint32_t);
    void (*memory_warning)(void *);
    void (*frame)(void *, double);
    void (*geometry)(void *, GpuiIosRect, double, GpuiIosEdges, GpuiIosEdges);
    void (*touch)(void *, uint64_t, uint32_t, double, double, double);
    void (*cancel_input)(void *);
    // Copy into caller storage, return required byte count (without NUL).
    size_t (*text)(void *, GpuiIosRange, char *, size_t);
    size_t (*text_length)(void *);
    GpuiIosRange (*selection)(void *);
    GpuiIosRange (*marked)(void *);
    void (*select)(void *, GpuiIosRange);
    void (*replace)(void *, GpuiIosRange, const char *, size_t);
    void (*mark)(void *, const char *, size_t, GpuiIosRange);
    void (*unmark)(void *);
    GpuiIosRect (*text_bounds)(void *, GpuiIosRange);
    size_t (*text_hit_test)(void *, double, double);
    bool (*accessibility_action)(void *, uint64_t, uint32_t);
    // 0 URL, 1 thermal, 2 input mode, 3 appearance, 4 native refusal.
    void (*system_event)(void *, uint32_t, const char *);
    bool (*text_action)(void *, uint32_t);
    GpuiIosSelection (*native_selection)(void *);
    bool (*set_native_selection)(void *, GpuiIosSelection);
    // Directions: left=0, right=1, up=2, down=3.
    GpuiIosPosition (*move_position)(void *, GpuiIosPosition, uint32_t, size_t);
    GpuiIosPosition (*farthest_position)(void *, GpuiIosRange, uint32_t);
    bool (*position_bounds)(void *, GpuiIosPosition, GpuiIosRect *);
    size_t (*selection_rects)(void *, GpuiIosRange, GpuiIosSelectionRect *, size_t);
    GpuiIosRange (*grapheme)(void *, size_t);
    int32_t (*writing_direction)(void *, size_t);
    bool (*set_writing_direction)(void *, GpuiIosRange, bool);
    GpuiIosPosition (*position_for_point)(void *, double, double, GpuiIosRange);
} GpuiIosCallbacks;

// Process entry, called once from main. UIKit owns the application loop.
int gpui_ios_run(GpuiIosCallbacks callbacks);
// Single-scene host; create only inside launched. Returns retained host or NULL.
void *gpui_ios_create(GpuiIosCallbacks callbacks);
void gpui_ios_destroy(void *host);
void *gpui_ios_view(void *host);
void *gpui_ios_controller(void *host);
void gpui_ios_keyboard(void *host, bool visible);
void gpui_ios_text_changed(void *host, bool selection_only);
void gpui_ios_set_title(void *host, const char *title);
// Owned UTF-8 app-sandbox path, or NULL on native refusal; release with free_string.
char *gpui_ios_copy_application_support_path(void);
void gpui_ios_free_string(char *text);
// Replace one complete, ordered accessibility snapshot. Stable identities
// preserve UIKit element instances and therefore VoiceOver focus.
typedef struct {
    uint64_t id;
    GpuiIosRect bounds;
    const char *label;
    const char *value;
    uint32_t role; // 0 text, 1 button, 2 adjustable, 3 text field, 4 image
    bool disabled, selected;
} GpuiIosAccessibilityNode;
void gpui_ios_accessibility(void *host, const GpuiIosAccessibilityNode *, size_t);
