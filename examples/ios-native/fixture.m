// Copyright 2026 GPUI Box contributors. SPDX-License-Identifier: Apache-2.0
// Native HOST fixture, not a GPUI application or full-platform acceptance.
#import <UIKit/UIKit.h>
#import <Metal/Metal.h>
#import <QuartzCore/CAMetalLayer.h>
#import "host.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>

static void *host;
static NSMutableString *document;
static NSRange selected, marked;
static id<MTLCommandQueue> queue;
static id<MTLCommandBuffer> lastSubmission;
static NSUInteger frames;
static BOOL suspended = YES;

static GpuiIosRange nativeRange(NSRange r) { return (GpuiIosRange){r.location, r.length, r.location != NSNotFound}; }
static size_t text(void *context, GpuiIosRange range, char *out, size_t capacity) {
    (void)context;
    NSData *data = [[document substringWithRange:NSMakeRange(range.location, range.length)] dataUsingEncoding:NSUTF8StringEncoding];
    if (capacity >= data.length && data.length) memcpy(out, data.bytes, data.length);
    return data.length;
}
static size_t length(void *context) { (void)context; return document.length; }
static GpuiIosRange selection(void *context) { (void)context; return nativeRange(selected); }
static GpuiIosRange composition(void *context) { (void)context; return nativeRange(marked); }
static void selectRange(void *context, GpuiIosRange range) {
    (void)context; selected = NSMakeRange(range.location, range.length);
}
static void replace(void *context, GpuiIosRange range, const char *bytes, size_t count) {
    (void)context;
    NSString *value = [[NSString alloc] initWithBytes:bytes length:count encoding:NSUTF8StringEncoding];
    [document replaceCharactersInRange:NSMakeRange(range.location, range.length) withString:value];
    selected = NSMakeRange(range.location + value.length, 0); marked = NSMakeRange(NSNotFound, 0);
}
static void mark(void *context, const char *bytes, size_t count, GpuiIosRange localSelection) {
    NSRange replacement = marked.location == NSNotFound ? selected : marked;
    replace(context, nativeRange(replacement), bytes, count);
    NSString *value = [[NSString alloc] initWithBytes:bytes length:count encoding:NSUTF8StringEncoding];
    marked = NSMakeRange(replacement.location, value.length);
    selected = NSMakeRange(marked.location + localSelection.location, localSelection.length);
}
static void unmark(void *context) { (void)context; marked = NSMakeRange(NSNotFound, 0); }
static GpuiIosRect textBounds(void *context, GpuiIosRange range) {
    (void)context;
    // Fixture geometry is explicitly monospace; not the production text bridge.
    return (GpuiIosRect){24 + range.location * 12, 160, MAX(2, range.length * 12), 24};
}
static size_t hitTest(void *context, double x, double y) {
    (void)context; (void)y; return MIN(document.length, (size_t)MAX(0, (x - 24) / 12));
}
static void cancelInput(void *context) { (void)context; puts("IOS_CANCEL_INPUT"); }
static void lifecycle(void *context, uint32_t phase) {
    (void)context;
    if (phase == 1 || phase == 2) {
        suspended = YES;
        [lastSubmission waitUntilCompleted];
        assert(lastSubmission.status != MTLCommandBufferStatusError);
        lastSubmission = nil;
    } else if (phase == 0) suspended = NO;
    printf("IOS_LIFECYCLE %u\n", phase); fflush(stdout);
}
static void frame(void *context, double time) {
    (void)context; (void)time;
    assert(!suspended);
    CAMetalLayer *layer = (CAMetalLayer *)((__bridge UIView *)gpui_ios_view(host)).layer;
    id<CAMetalDrawable> drawable = [layer nextDrawable];
    if (!drawable) return;
    MTLRenderPassDescriptor *pass = [MTLRenderPassDescriptor renderPassDescriptor];
    pass.colorAttachments[0].texture = drawable.texture;
    pass.colorAttachments[0].loadAction = MTLLoadActionClear;
    pass.colorAttachments[0].storeAction = MTLStoreActionStore;
    pass.colorAttachments[0].clearColor = MTLClearColorMake(0.08, 0.12, 0.20, 1);
    id<MTLCommandBuffer> command = [queue commandBuffer];
    id<MTLRenderCommandEncoder> encoder = [command renderCommandEncoderWithDescriptor:pass];
    [encoder endEncoding]; [command presentDrawable:drawable]; [command commit];
    lastSubmission = command;
    if (++frames == 3) {
        [command waitUntilCompleted];
        assert(command.status == MTLCommandBufferStatusCompleted);
        puts("IOS_METAL_PRESENT_PASS frames=3"); fflush(stdout);
    }
}
static void geometry(void *context, GpuiIosRect bounds, double scale, GpuiIosEdges safe, GpuiIosEdges ime) {
    (void)context;
    printf("IOS_GEOMETRY %.0fx%.0f scale=%.1f safe=%.1f,%.1f,%.1f,%.1f ime_bottom=%.1f\n",
        bounds.width, bounds.height, scale, safe.top, safe.right, safe.bottom, safe.left, ime.bottom);
    fflush(stdout);
}
static void touch(void *context, uint64_t identity, uint32_t phase, double x, double y, double force) {
    (void)context;
    printf("IOS_TOUCH %llu phase=%u %.1f,%.1f force=%.3f\n", (unsigned long long)identity, phase, x, y, force);
    if (phase == 0) gpui_ios_keyboard(host, true);
    fflush(stdout);
}
static bool accessibilityAction(void *context, uint64_t identity, uint32_t action) {
    (void)context;
    printf("IOS_ACCESSIBILITY %llu action=%u\n", (unsigned long long)identity, action);
    fflush(stdout); return identity == 42 && action == 0;
}
static GpuiIosCallbacks callbacks;
static void launched(void *context) {
    (void)context;
    document = [@"A😀中" mutableCopy]; selected = NSMakeRange(3, 0); marked = NSMakeRange(NSNotFound, 0);
    host = gpui_ios_create(callbacks); assert(host);
    UIView<UITextInput> *view = (__bridge UIView<UITextInput> *)gpui_ios_view(host);
    CAMetalLayer *layer = (CAMetalLayer *)view.layer;
    layer.device = MTLCreateSystemDefaultDevice(); assert(layer.device);
    layer.pixelFormat = MTLPixelFormatBGRA8Unorm; queue = [layer.device newCommandQueue]; assert(queue);
    [view deleteBackward]; assert([document isEqualToString:@"A中"]);
    [view setMarkedText:@"ni" selectedRange:NSMakeRange(2, 0)];
    assert(NSEqualRanges(marked, NSMakeRange(1, 2)) && selected.location == 3);
    [view setMarkedText:@"你" selectedRange:NSMakeRange(1, 0)];
    assert([document isEqualToString:@"A你中"] && marked.length == 1);
    [view unmarkText]; assert(marked.location == NSNotFound);
    UITextPosition *start = view.beginningOfDocument;
    UITextPosition *end = [view positionFromPosition:start offset:3];
    assert([view offsetFromPosition:start toPosition:end] == 3);
    assert([view positionFromPosition:start offset:-1] == nil);
    assert([[view textInRange:[view textRangeFromPosition:start toPosition:end]] isEqualToString:@"A你中"]);
    puts("IOS_TEXT_PROTOCOL_PASS emoji-delete composition-replace utf16-range");
    GpuiIosAccessibilityNode node = {42, {24, 100, 300, 50}, "UIKit host fixture; not GPUI", NULL, 1, false, false};
    gpui_ios_accessibility(host, &node, 1);
    id first = view.accessibilityElements.firstObject;
    gpui_ios_accessibility(host, &node, 1);
    assert(first == view.accessibilityElements.firstObject);
    assert([first accessibilityActivate]);
    puts("IOS_ACCESSIBILITY_BRIDGE_PASS identity action"); fflush(stdout);
    UILabel *label = [[UILabel alloc] initWithFrame:CGRectMake(24, 100, 340, 80)];
    label.text = @"UIKit / Metal host fixture\nNot full GPUI acceptance\nTap to open keyboard";
    label.numberOfLines = 3; label.textColor = UIColor.whiteColor;
    label.isAccessibilityElement = NO; [view addSubview:label];
}
int main(void) {
    callbacks = (GpuiIosCallbacks){
        .launched = launched, .lifecycle = lifecycle, .frame = frame,
        .geometry = geometry, .touch = touch, .cancel_input = cancelInput,
        .text = text, .text_length = length, .selection = selection, .marked = composition,
        .select = selectRange, .replace = replace, .mark = mark, .unmark = unmark,
        .text_bounds = textBounds, .text_hit_test = hitTest, .accessibility_action = accessibilityAction
    };
    return gpui_ios_run(callbacks);
}
