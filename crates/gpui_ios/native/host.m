// Copyright 2026 GPUI Box contributors. SPDX-License-Identifier: Apache-2.0
#import "host.h"
#import "geometry.h"
#import <UIKit/UIKit.h>
#import <QuartzCore/CAMetalLayer.h>
#include <string.h>

static GpuiIosCallbacks applicationCallbacks;
static GpuiIosRect rectValue(CGRect r) {
    return (GpuiIosRect){r.origin.x, r.origin.y, r.size.width, r.size.height};
}
static CGRect cgRect(GpuiIosRect r) { return CGRectMake(r.x, r.y, r.width, r.height); }
static GpuiIosRange rangeValue(NSRange r) {
    return (GpuiIosRange){r.location, r.length, r.location != NSNotFound};
}

@interface GpuiPosition : UITextPosition
@property(nonatomic) NSUInteger offset;
@property(nonatomic) BOOL upstream;
+ (instancetype)at:(NSUInteger)offset;
+ (instancetype)native:(GpuiIosPosition)position;
- (GpuiIosPosition)native;
@end
@implementation GpuiPosition
+ (instancetype)at:(NSUInteger)offset {
    GpuiPosition *p = [self new]; p.offset = offset; return p;
}
+ (instancetype)native:(GpuiIosPosition)position {
    if (!position.valid) return nil;
    GpuiPosition *p = [self at:position.offset]; p.upstream = position.upstream; return p;
}
- (GpuiIosPosition)native { return (GpuiIosPosition){self.offset, self.upstream, true}; }
@end

@interface GpuiRange : UITextRange
@property(nonatomic) NSRange range;
@property(nonatomic, strong) GpuiPosition *anchor;
@property(nonatomic, strong) GpuiPosition *head;
+ (instancetype)from:(GpuiIosRange)range;
@end
@implementation GpuiRange
+ (instancetype)from:(GpuiIosRange)r {
    if (!r.valid) return nil;
    GpuiRange *result = [self new];
    result.range = NSMakeRange(r.location, r.length);
    result.anchor = [GpuiPosition at:r.location]; result.head = [GpuiPosition at:r.location + r.length];
    return result;
}
- (UITextPosition *)start { return self.anchor.offset <= self.head.offset ? self.anchor : self.head; }
- (UITextPosition *)end { return self.anchor.offset <= self.head.offset ? self.head : self.anchor; }
- (BOOL)isEmpty { return self.range.length == 0; }
@end

@interface GpuiSelectionRect : UITextSelectionRect
@property(nonatomic) GpuiIosSelectionRect fragment;
@end
@implementation GpuiSelectionRect
- (CGRect)rect { return cgRect(self.fragment.bounds); }
- (UITextWritingDirection)writingDirection { return self.fragment.rtl ? UITextWritingDirectionRightToLeft : UITextWritingDirectionLeftToRight; }
- (BOOL)containsStart { return self.fragment.contains_start; }
- (BOOL)containsEnd { return self.fragment.contains_end; }
- (BOOL)isVertical { return self.fragment.vertical; }
@end
static uint32_t nativeDirection(UITextLayoutDirection direction) {
    switch (direction) {
        case UITextLayoutDirectionLeft: return 0;
        case UITextLayoutDirectionRight: return 1;
        case UITextLayoutDirectionUp: return 2;
        case UITextLayoutDirectionDown: return 3;
    }
    return UINT32_MAX;
}

@interface GpuiView : UIView <UITextInput>
@property(nonatomic) GpuiIosCallbacks callbacks;
@property(nonatomic, weak) id<UITextInputDelegate> inputDelegate;
@property(nonatomic, copy) NSDictionary *markedTextStyle;
@property(nonatomic, strong) UITextInputStringTokenizer *tokenizer;
@property(nonatomic, strong) NSMapTable<UITouch *, NSNumber *> *contacts;
@property(nonatomic) uint64_t nextContact;
@property(nonatomic, strong) NSMutableDictionary<NSNumber *, UIAccessibilityElement *> *elements;
@property(nonatomic, strong) UIView *keyboardProbe;
@property(nonatomic) CGRect lastBounds;
@property(nonatomic) CGFloat lastScale;
@property(nonatomic) UIEdgeInsets lastSafe;
@property(nonatomic) CGFloat lastKeyboard;
@property(nonatomic) UIKeyboardType keyboardType;
@property(nonatomic) UIReturnKeyType returnKeyType;
@property(nonatomic, copy) UITextContentType textContentType;
@property(nonatomic, getter=isSecureTextEntry) BOOL secureTextEntry;
@property(nonatomic) uint32_t textAction;
@property(nonatomic) BOOL multiline;
- (void)publishGeometry;
- (void)cancelInput;
@end

@interface GpuiElement : UIAccessibilityElement
@property(nonatomic, weak) GpuiView *owner;
@property(nonatomic) uint64_t nodeId;
@end
@implementation GpuiElement
- (BOOL)accessibilityActivate {
    GpuiIosCallbacks c = self.owner.callbacks;
    return c.accessibility_action && c.accessibility_action(c.context, self.nodeId, 0);
}
- (void)accessibilityIncrement {
    GpuiIosCallbacks c = self.owner.callbacks;
    if (c.accessibility_action) c.accessibility_action(c.context, self.nodeId, 1);
}
- (void)accessibilityDecrement {
    GpuiIosCallbacks c = self.owner.callbacks;
    if (c.accessibility_action) c.accessibility_action(c.context, self.nodeId, 2);
}
@end

@implementation GpuiView
@synthesize inputDelegate, markedTextStyle, tokenizer;
+ (Class)layerClass { return CAMetalLayer.class; }
- (instancetype)initWithFrame:(CGRect)frame {
    if ((self = [super initWithFrame:frame])) {
        self.multipleTouchEnabled = YES;
        self.opaque = YES;
        self.nextContact = 1;
        self.contacts = [NSMapTable strongToStrongObjectsMapTable];
        self.elements = [NSMutableDictionary new];
        self.tokenizer = [[UITextInputStringTokenizer alloc] initWithTextInput:self];
        // A constrained probe has a presentation layer during keyboard animation.
        // Unlike a target-frame notification, sampling it follows interactive
        // dismissal as well as UIKit's actual animation timing.
        self.keyboardProbe = [UIView new];
        self.keyboardProbe.userInteractionEnabled = NO;
        self.keyboardProbe.isAccessibilityElement = NO;
        self.keyboardProbe.translatesAutoresizingMaskIntoConstraints = NO;
        [self addSubview:self.keyboardProbe];
        self.keyboardLayoutGuide.followsUndockedKeyboard = YES;
        [NSLayoutConstraint activateConstraints:@[
            [self.keyboardProbe.leadingAnchor constraintEqualToAnchor:self.keyboardLayoutGuide.leadingAnchor],
            [self.keyboardProbe.trailingAnchor constraintEqualToAnchor:self.keyboardLayoutGuide.trailingAnchor],
            [self.keyboardProbe.topAnchor constraintEqualToAnchor:self.keyboardLayoutGuide.topAnchor],
            [self.keyboardProbe.bottomAnchor constraintEqualToAnchor:self.keyboardLayoutGuide.bottomAnchor]
        ]];
    }
    return self;
}
- (void)layoutSubviews { [super layoutSubviews]; [self publishGeometry]; }
- (void)safeAreaInsetsDidChange { [super safeAreaInsetsDidChange]; [self publishGeometry]; }
- (void)publishGeometry {
    CGFloat scale = self.window.screen.scale ?: self.contentScaleFactor;
    self.contentScaleFactor = scale;
    CAMetalLayer *metal = (CAMetalLayer *)self.layer;
    metal.contentsScale = scale;
    metal.drawableSize = CGSizeMake(self.bounds.size.width * scale, self.bounds.size.height * scale);
    CALayer *probe = self.keyboardProbe.layer.presentationLayer ?: self.keyboardProbe.layer;
    CGRect keyboard = probe.frame;
    // Layout guide includes bottom safe area while keyboard is hidden. Keep
    // safe area and IME distinct; only excess occlusion is a keyboard.
    CGFloat overlap = gpui_ios_bottom_occlusion(rectValue(self.bounds), rectValue(keyboard));
    UIEdgeInsets safe = self.safeAreaInsets;
    if (overlap <= safe.bottom) overlap = 0;
    if (CGRectEqualToRect(self.lastBounds, self.bounds) && self.lastScale == scale &&
        UIEdgeInsetsEqualToEdgeInsets(self.lastSafe, safe) && self.lastKeyboard == overlap) return;
    self.lastBounds = self.bounds; self.lastScale = scale;
    self.lastSafe = safe; self.lastKeyboard = overlap;
    GpuiIosCallbacks c = self.callbacks;
    if (c.geometry) c.geometry(c.context, rectValue(self.bounds), scale,
        (GpuiIosEdges){safe.top, safe.right, safe.bottom, safe.left},
        (GpuiIosEdges){0, 0, overlap, 0});
}
- (void)sendTouches:(NSSet<UITouch *> *)touches phase:(uint32_t)phase event:(UIEvent *)event {
    GpuiIosCallbacks c = self.callbacks;
    for (UITouch *touch in touches) {
        NSNumber *identity = [self.contacts objectForKey:touch];
        if (phase == 0) {
            identity = @(self.nextContact++);
            [self.contacts setObject:identity forKey:touch];
        }
        if (!identity) continue;
        NSArray<UITouch *> *samples = phase == 1 ? [event coalescedTouchesForTouch:touch] : nil;
        if (!samples.count) samples = @[touch];
        for (UITouch *sample in samples) {
            CGPoint p = [sample locationInView:self];
            double force = sample.maximumPossibleForce > 0 ? sample.force / sample.maximumPossibleForce : -1;
            if (c.touch) c.touch(c.context, identity.unsignedLongLongValue, phase, p.x, p.y, force);
        }
        if (phase == 2 || phase == 3) [self.contacts removeObjectForKey:touch];
    }
}
- (void)touchesBegan:(NSSet<UITouch *> *)t withEvent:(UIEvent *)e { [self sendTouches:t phase:0 event:e]; }
- (void)touchesMoved:(NSSet<UITouch *> *)t withEvent:(UIEvent *)e { [self sendTouches:t phase:1 event:e]; }
- (void)touchesEnded:(NSSet<UITouch *> *)t withEvent:(UIEvent *)e { [self sendTouches:t phase:2 event:e]; }
- (void)touchesCancelled:(NSSet<UITouch *> *)t withEvent:(UIEvent *)e { [self sendTouches:t phase:3 event:e]; }
- (void)cancelInput {
    // Also cancels GPUI momentum/recognizer timers when no contacts remain.
    GpuiIosCallbacks c = self.callbacks;
    if (c.cancel_input) c.cancel_input(c.context);
    [self.contacts removeAllObjects];
}
- (NSUInteger)documentLength {
    return self.callbacks.text_length ? self.callbacks.text_length(self.callbacks.context) : 0;
}
- (BOOL)canBecomeFirstResponder { return self.callbacks.text_length != NULL; }
- (BOOL)hasText { return [self documentLength] != 0; }
- (UITextPosition *)beginningOfDocument { return [GpuiPosition at:0]; }
- (UITextPosition *)endOfDocument { return [GpuiPosition at:[self documentLength]]; }
- (UITextRange *)selectedTextRange {
    if (self.callbacks.native_selection) {
        GpuiIosSelection s = self.callbacks.native_selection(self.callbacks.context);
        if (!s.anchor.valid || !s.head.valid) return nil;
        GpuiRange *range = [GpuiRange from:(GpuiIosRange){MIN(s.anchor.offset,s.head.offset),
            MAX(s.anchor.offset,s.head.offset)-MIN(s.anchor.offset,s.head.offset), true}];
        range.anchor = [GpuiPosition native:s.anchor]; range.head = [GpuiPosition native:s.head];
        return range;
    }
    return self.callbacks.selection ? [GpuiRange from:self.callbacks.selection(self.callbacks.context)] : nil;
}
- (void)setSelectedTextRange:(UITextRange *)range {
    if (range && self.callbacks.set_native_selection) {
        GpuiRange *r = (GpuiRange *)range;
        if (!self.callbacks.set_native_selection(self.callbacks.context, (GpuiIosSelection){r.anchor.native, r.head.native}))
            NSLog(@"GPUI iOS: native selection refused");
        return;
    }
    if (range && self.callbacks.select) self.callbacks.select(self.callbacks.context, rangeValue(((GpuiRange *)range).range));
}
- (UITextStorageDirection)selectionAffinity {
    GpuiRange *range = (GpuiRange *)self.selectedTextRange;
    return range.head.upstream ? UITextStorageDirectionBackward : UITextStorageDirectionForward;
}
- (void)setSelectionAffinity:(UITextStorageDirection)affinity {
    GpuiIosCallbacks c = self.callbacks;
    if (!c.native_selection || !c.set_native_selection) return;
    GpuiIosSelection s = c.native_selection(c.context);
    if (!s.anchor.valid || !s.head.valid) return;
    s.head.upstream = affinity == UITextStorageDirectionBackward;
    if (!c.set_native_selection(c.context, s)) NSLog(@"GPUI iOS: selection affinity refused");
}
- (UITextRange *)markedTextRange {
    return self.callbacks.marked ? [GpuiRange from:self.callbacks.marked(self.callbacks.context)] : nil;
}
- (NSString *)textInRange:(UITextRange *)range {
    if (!self.callbacks.text || !range) return nil;
    GpuiIosRange r = rangeValue(((GpuiRange *)range).range);
    GpuiIosCallbacks c = self.callbacks;
    size_t length = c.text(c.context, r, NULL, 0);
    NSMutableData *data = [NSMutableData dataWithLength:length];
    if (c.text(c.context, r, data.mutableBytes, length) != length) return nil;
    return [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
}
- (void)replaceRange:(UITextRange *)range withText:(NSString *)text {
    if (!self.callbacks.replace || !range) return;
    NSData *bytes = [text dataUsingEncoding:NSUTF8StringEncoding];
    self.callbacks.replace(self.callbacks.context, rangeValue(((GpuiRange *)range).range), bytes.bytes, bytes.length);
}
- (void)insertText:(NSString *)text {
    if ([text isEqualToString:@"\n"]) {
        GpuiIosCallbacks c = self.callbacks;
        if (c.text_action && c.text_action(c.context, self.textAction)) return;
        if (!self.multiline) return;
    }
    UITextRange *range = self.markedTextRange ?: self.selectedTextRange;
    if (range) [self replaceRange:range withText:text];
}
- (void)deleteBackward {
    GpuiRange *selected = (GpuiRange *)(self.markedTextRange ?: self.selectedTextRange);
    if (!selected) return;
    NSRange range = selected.range;
    if (range.length == 0 && range.location > 0) {
        // NSString operates in UTF-16 but deletes a complete grapheme (emoji,
        // combining marks, flags), never one surrogate code unit.
        NSString *prefix = [self textInRange:[GpuiRange from:(GpuiIosRange){0, range.location, true}]];
        if (!prefix.length) return;
        range = [prefix rangeOfComposedCharacterSequenceAtIndex:prefix.length - 1];
    }
    [self replaceRange:[GpuiRange from:rangeValue(range)] withText:@""];
}
- (void)setMarkedText:(NSString *)text selectedRange:(NSRange)selected {
    if (!self.callbacks.mark) return;
    NSData *bytes = [(text ?: @"") dataUsingEncoding:NSUTF8StringEncoding];
    self.callbacks.mark(self.callbacks.context, bytes.bytes, bytes.length, rangeValue(selected));
}
- (void)unmarkText { if (self.callbacks.unmark) self.callbacks.unmark(self.callbacks.context); }
- (UITextRange *)textRangeFromPosition:(UITextPosition *)from toPosition:(UITextPosition *)to {
    if (!from || !to) return nil;
    NSUInteger a = ((GpuiPosition *)from).offset, b = ((GpuiPosition *)to).offset;
    GpuiRange *range = [GpuiRange from:(GpuiIosRange){MIN(a,b), MAX(a,b)-MIN(a,b), true}];
    range.anchor = (GpuiPosition *)from; range.head = (GpuiPosition *)to; return range;
}
- (UITextPosition *)positionFromPosition:(UITextPosition *)position offset:(NSInteger)offset {
    NSUInteger length = [self documentLength];
    NSInteger start = (NSInteger)((GpuiPosition *)position).offset;
    if (offset == NSIntegerMin) return nil;
    if ((offset < 0 && start < -offset) || (offset > 0 && (NSUInteger)offset > length - MIN((NSUInteger)start, length))) return nil;
    NSInteger end = start + offset;
    return end >= 0 && (NSUInteger)end <= length ? [GpuiPosition at:(NSUInteger)end] : nil;
}
- (UITextPosition *)positionFromPosition:(UITextPosition *)p inDirection:(UITextLayoutDirection)d offset:(NSInteger)o {
    if (!p || !self.callbacks.move_position || o == NSIntegerMin) return nil;
    uint32_t direction = nativeDirection(d);
    if (direction > 3) return nil;
    if (o < 0) direction ^= 1;
    return [GpuiPosition native:self.callbacks.move_position(self.callbacks.context,
        ((GpuiPosition *)p).native, direction, (size_t)(o < 0 ? -o : o))];
}
- (NSComparisonResult)comparePosition:(UITextPosition *)a toPosition:(UITextPosition *)b {
    NSUInteger x = ((GpuiPosition *)a).offset, y = ((GpuiPosition *)b).offset;
    return x < y ? NSOrderedAscending : x > y ? NSOrderedDescending : NSOrderedSame;
}
- (NSInteger)offsetFromPosition:(UITextPosition *)a toPosition:(UITextPosition *)b {
    return (NSInteger)((GpuiPosition *)b).offset - (NSInteger)((GpuiPosition *)a).offset;
}
- (UITextPosition *)positionWithinRange:(UITextRange *)r farthestInDirection:(UITextLayoutDirection)d {
    if (!r || !self.callbacks.farthest_position) return nil;
    return [GpuiPosition native:self.callbacks.farthest_position(self.callbacks.context,
        rangeValue(((GpuiRange *)r).range), nativeDirection(d))];
}
- (UITextRange *)characterRangeByExtendingPosition:(UITextPosition *)p inDirection:(UITextLayoutDirection)d {
    UITextPosition *end = [self positionFromPosition:p inDirection:d offset:1];
    return end ? [self textRangeFromPosition:p toPosition:end] : nil;
}
- (UITextWritingDirection)baseWritingDirectionForPosition:(UITextPosition *)p inDirection:(UITextStorageDirection)d {
    if (!p || !self.callbacks.writing_direction) return UITextWritingDirectionNatural;
    NSUInteger offset = ((GpuiPosition *)p).offset;
    if (d == UITextStorageDirectionBackward && offset > 0) offset--;
    int32_t direction = self.callbacks.writing_direction(self.callbacks.context, offset);
    return direction == 0 ? UITextWritingDirectionLeftToRight : direction == 1 ? UITextWritingDirectionRightToLeft : UITextWritingDirectionNatural;
}
- (void)setBaseWritingDirection:(UITextWritingDirection)d forRange:(UITextRange *)r {
    if (!r || d == UITextWritingDirectionNatural || !self.callbacks.set_writing_direction ||
        !self.callbacks.set_writing_direction(self.callbacks.context, rangeValue(((GpuiRange *)r).range), d == UITextWritingDirectionRightToLeft))
        NSLog(@"GPUI iOS: document writing-direction mutation refused");
}
- (CGRect)firstRectForRange:(UITextRange *)r {
    if (!r) return CGRectNull;
    if (r.isEmpty) return [self caretRectForPosition:r.start];
    NSArray<UITextSelectionRect *> *rects = [self selectionRectsForRange:r];
    for (UITextSelectionRect *rect in rects) if (rect.containsStart) return rect.rect;
    return rects.count ? rects.firstObject.rect : CGRectNull;
}
- (CGRect)caretRectForPosition:(UITextPosition *)p {
    GpuiIosRect bounds;
    if (!p || !self.callbacks.position_bounds || !self.callbacks.position_bounds(self.callbacks.context, ((GpuiPosition *)p).native, &bounds)) return CGRectNull;
    return cgRect(bounds);
}
- (NSArray<UITextSelectionRect *> *)selectionRectsForRange:(UITextRange *)range {
    GpuiIosCallbacks c = self.callbacks;
    if (!range || !c.selection_rects) return @[];
    GpuiIosRange r = rangeValue(((GpuiRange *)range).range);
    size_t count = c.selection_rects(c.context, r, NULL, 0);
    if (!count || count > SIZE_MAX / sizeof(GpuiIosSelectionRect)) return @[];
    NSMutableData *data = [NSMutableData dataWithLength:count * sizeof(GpuiIosSelectionRect)];
    if (c.selection_rects(c.context, r, data.mutableBytes, count) != count) return @[];
    const GpuiIosSelectionRect *fragments = data.bytes;
    NSMutableArray *result = [NSMutableArray arrayWithCapacity:count];
    for (size_t index = 0; index < count; index++) {
        GpuiSelectionRect *rect = [GpuiSelectionRect new]; rect.fragment = fragments[index]; [result addObject:rect];
    }
    return result;
}
- (UITextPosition *)closestPositionToPoint:(CGPoint)p {
    if (self.callbacks.position_for_point)
        return [GpuiPosition native:self.callbacks.position_for_point(self.callbacks.context, p.x, p.y, (GpuiIosRange){0})];
    if (!self.callbacks.text_hit_test) return nil;
    size_t offset = self.callbacks.text_hit_test(self.callbacks.context,p.x,p.y);
    return offset <= [self documentLength] ? [GpuiPosition at:offset] : nil;
}
- (UITextPosition *)closestPositionToPoint:(CGPoint)p withinRange:(UITextRange *)r {
    if (r && self.callbacks.position_for_point)
        return [GpuiPosition native:self.callbacks.position_for_point(self.callbacks.context, p.x, p.y, rangeValue(((GpuiRange *)r).range))];
    if (!self.callbacks.text_hit_test || !r) return nil;
    GpuiPosition *position = (GpuiPosition *)[self closestPositionToPoint:p];
    if (!position) return nil;
    NSUInteger offset = position.offset;
    NSRange range = ((GpuiRange *)r).range;
    // Legacy fixture callback has no range-aware geometry. Never clamp it.
    return offset >= range.location && offset <= NSMaxRange(range) ? position : nil;
}
- (UITextRange *)characterRangeAtPoint:(CGPoint)p {
    GpuiPosition *position = (GpuiPosition *)[self closestPositionToPoint:p];
    if (!position || !self.callbacks.grapheme) return nil;
    return [GpuiRange from:self.callbacks.grapheme(self.callbacks.context, position.offset)];
}
@end

@interface GpuiController : UIViewController
@property(nonatomic) GpuiIosCallbacks callbacks;
@end
@implementation GpuiController
- (void)loadView {
    GpuiView *view = [[GpuiView alloc] initWithFrame:CGRectZero];
    view.callbacks = self.callbacks; self.view = view;
}
- (void)didReceiveMemoryWarning {
    [super didReceiveMemoryWarning];
    if (self.callbacks.memory_warning) self.callbacks.memory_warning(self.callbacks.context);
}
- (void)traitCollectionDidChange:(UITraitCollection *)previous {
    [super traitCollectionDidChange:previous];
    if ([self.traitCollection hasDifferentColorAppearanceComparedToTraitCollection:previous] && self.callbacks.system_event)
        self.callbacks.system_event(self.callbacks.context, 3, NULL);
}
@end

@interface GpuiHost : NSObject
@property(nonatomic, strong) UIWindow *window;
@property(nonatomic, strong) GpuiController *controller;
@property(nonatomic, strong) CADisplayLink *displayLink;
- (void)tick:(CADisplayLink *)link;
- (void)invalidate;
@end
static __weak GpuiHost *activeHost;
@implementation GpuiHost
- (void)tick:(CADisplayLink *)link {
    GpuiView *view = (GpuiView *)self.controller.view;
    [view publishGeometry];
    GpuiIosCallbacks c = view.callbacks;
    if (c.frame) c.frame(c.context, link.targetTimestamp);
}
- (void)invalidate {
    [self.displayLink invalidate]; self.displayLink = nil;
    GpuiView *view = (GpuiView *)self.controller.view;
    [view cancelInput];
    [view resignFirstResponder];
    view.callbacks = (GpuiIosCallbacks){0};
    self.controller.callbacks = (GpuiIosCallbacks){0};
    view.accessibilityElements = @[];
    [view.elements removeAllObjects];
    self.window.hidden = YES;
    self.window.rootViewController = nil;
}
@end

@interface GpuiAppDelegate : UIResponder <UIApplicationDelegate>
@end
@implementation GpuiAppDelegate
- (BOOL)application:(UIApplication *)application didFinishLaunchingWithOptions:(NSDictionary *)options {
    (void)application; (void)options;
    [NSNotificationCenter.defaultCenter addObserver:self selector:@selector(thermalChanged:) name:NSProcessInfoThermalStateDidChangeNotification object:nil];
    [NSNotificationCenter.defaultCenter addObserver:self selector:@selector(inputModeChanged:) name:UITextInputCurrentInputModeDidChangeNotification object:nil];
    applicationCallbacks.launched(applicationCallbacks.context); return YES;
}
- (void)thermalChanged:(NSNotification *)notification {
    (void)notification;
    if (applicationCallbacks.system_event) applicationCallbacks.system_event(applicationCallbacks.context, 1, NULL);
}
- (void)inputModeChanged:(NSNotification *)notification {
    (void)notification;
    if (applicationCallbacks.system_event) applicationCallbacks.system_event(applicationCallbacks.context, 2, NULL);
}
- (BOOL)application:(UIApplication *)app openURL:(NSURL *)url options:(NSDictionary<UIApplicationOpenURLOptionsKey,id> *)options {
    (void)app; (void)options;
    if (!applicationCallbacks.system_event) return NO;
    applicationCallbacks.system_event(applicationCallbacks.context, 0, url.absoluteString.UTF8String); return YES;
}
- (void)applicationWillResignActive:(UIApplication *)application {
    (void)application;
    GpuiHost *host = activeHost;
    [(GpuiView *)host.controller.view cancelInput];
    host.displayLink.paused = YES;
    if (applicationCallbacks.lifecycle) applicationCallbacks.lifecycle(applicationCallbacks.context, 1);
}
- (void)applicationDidEnterBackground:(UIApplication *)application {
    (void)application;
    if (applicationCallbacks.lifecycle) applicationCallbacks.lifecycle(applicationCallbacks.context, 2);
}
- (void)applicationWillEnterForeground:(UIApplication *)application {
    (void)application;
    if (applicationCallbacks.lifecycle) applicationCallbacks.lifecycle(applicationCallbacks.context, 3);
}
- (void)applicationDidBecomeActive:(UIApplication *)application {
    (void)application;
    if (applicationCallbacks.lifecycle) applicationCallbacks.lifecycle(applicationCallbacks.context, 0);
    activeHost.displayLink.paused = NO;
}
@end

int gpui_ios_run(GpuiIosCallbacks callbacks) {
    NSCAssert(NSThread.isMainThread, @"UIKit must run on the main thread");
    if (!callbacks.launched) return -1;
    applicationCallbacks = callbacks;
    @autoreleasepool { return UIApplicationMain(0, NULL, nil, NSStringFromClass(GpuiAppDelegate.class)); }
}
void *gpui_ios_create(GpuiIosCallbacks callbacks) {
    NSCAssert(NSThread.isMainThread, @"UIKit window creation must be on the main thread");
    if (activeHost || !callbacks.text || !callbacks.text_length || !callbacks.selection ||
        !callbacks.marked || !callbacks.select || !callbacks.replace || !callbacks.mark ||
        !callbacks.unmark || !callbacks.text_bounds || !callbacks.text_hit_test) return NULL;
    GpuiHost *host = [GpuiHost new];
    host.controller = [GpuiController new]; host.controller.callbacks = callbacks;
    host.window = [[UIWindow alloc] initWithFrame:UIScreen.mainScreen.bounds];
    host.window.rootViewController = host.controller;
    activeHost = host;
    [host.window makeKeyAndVisible];
    host.displayLink = [CADisplayLink displayLinkWithTarget:host selector:@selector(tick:)];
    host.displayLink.paused = UIApplication.sharedApplication.applicationState != UIApplicationStateActive;
    [host.displayLink addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
    return (__bridge_retained void *)host;
}
void gpui_ios_destroy(void *pointer) {
    NSCAssert(NSThread.isMainThread, @"UIKit destruction must be on the main thread");
    GpuiHost *host = (__bridge_transfer GpuiHost *)pointer;
    [host invalidate];
}
void *gpui_ios_view(void *h) { return (__bridge void *)((__bridge GpuiHost *)h).controller.view; }
void *gpui_ios_controller(void *h) { return (__bridge void *)((__bridge GpuiHost *)h).controller; }
void gpui_ios_keyboard(void *h, bool visible) {
    UIView *view = ((__bridge GpuiHost *)h).controller.view;
    if (visible) [view becomeFirstResponder]; else [view resignFirstResponder];
}
void gpui_ios_text_options(void *h, uint32_t purpose, uint32_t action, uint32_t autofill, bool secure, bool multiline) {
    GpuiView *view = (GpuiView *)((__bridge GpuiHost *)h).controller.view;
    UIKeyboardType keyboards[] = {UIKeyboardTypeDefault, UIKeyboardTypeNumberPad,
        UIKeyboardTypeDecimalPad, UIKeyboardTypePhonePad, UIKeyboardTypeEmailAddress,
        UIKeyboardTypeURL, UIKeyboardTypeWebSearch};
    UIReturnKeyType returns[] = {UIReturnKeyDefault, UIReturnKeyDefault,
        UIReturnKeyDone, UIReturnKeyGo, UIReturnKeyNext, UIReturnKeyDefault,
        UIReturnKeySearch, UIReturnKeySend};
    NSArray<UITextContentType> *contents = @[@"", UITextContentTypeName,
        UITextContentTypeUsername, UITextContentTypePassword, UITextContentTypeNewPassword,
        UITextContentTypeEmailAddress, UITextContentTypeTelephoneNumber, UITextContentTypeOneTimeCode];
    UIKeyboardType keyboard = purpose < 7 ? keyboards[purpose] : UIKeyboardTypeDefault;
    UIReturnKeyType key = action < 8 ? returns[action] : UIReturnKeyDefault;
    UITextContentType content = autofill < contents.count ? contents[autofill] : @"";
    BOOL changed = view.keyboardType != keyboard || view.returnKeyType != key ||
        view.secureTextEntry != secure || ![view.textContentType isEqualToString:content];
    view.keyboardType = keyboard; view.returnKeyType = key;
    view.textContentType = content; view.secureTextEntry = secure;
    view.textAction = action; view.multiline = multiline;
    if (changed && view.isFirstResponder) [view reloadInputViews];
}
void gpui_ios_text_changed(void *h, bool selectionOnly) {
    GpuiView *view = (GpuiView *)((__bridge GpuiHost *)h).controller.view;
    if (!selectionOnly) [view.inputDelegate textWillChange:view];
    [view.inputDelegate selectionWillChange:view];
    [view.inputDelegate selectionDidChange:view];
    if (!selectionOnly) [view.inputDelegate textDidChange:view];
}
void gpui_ios_set_title(void *h, const char *title) {
    ((__bridge GpuiHost *)h).controller.title = [NSString stringWithUTF8String:title];
}
void gpui_ios_accessibility(void *h, const GpuiIosAccessibilityNode *nodes, size_t count) {
    GpuiView *view = (GpuiView *)((__bridge GpuiHost *)h).controller.view;
    NSMutableArray *ordered = [NSMutableArray arrayWithCapacity:count];
    NSMutableDictionary *next = [NSMutableDictionary dictionaryWithCapacity:count];
    for (size_t i = 0; i < count; i++) {
        const GpuiIosAccessibilityNode *node = &nodes[i];
        NSNumber *key = @(node->id);
        GpuiElement *element = (GpuiElement *)view.elements[key];
        if (!element) element = [[GpuiElement alloc] initWithAccessibilityContainer:view];
        element.owner = view; element.nodeId = node->id;
        element.accessibilityLabel = node->label ? [NSString stringWithUTF8String:node->label] : nil;
        element.accessibilityValue = node->value ? [NSString stringWithUTF8String:node->value] : nil;
        element.accessibilityFrameInContainerSpace = cgRect(node->bounds);
        UIAccessibilityTraits traits = UIAccessibilityTraitStaticText;
        if (node->role == 1) traits = UIAccessibilityTraitButton;
        if (node->role == 2) traits = UIAccessibilityTraitAdjustable;
        if (node->role == 3) traits = UIAccessibilityTraitNone;
        if (node->role == 4) traits = UIAccessibilityTraitImage;
        if (node->disabled) traits |= UIAccessibilityTraitNotEnabled;
        if (node->selected) traits |= UIAccessibilityTraitSelected;
        element.accessibilityTraits = traits;
        next[key] = element; [ordered addObject:element];
    }
    BOOL structureChanged = ![view.accessibilityElements isEqualToArray:ordered];
    view.elements = next; view.accessibilityElements = ordered;
    if (structureChanged) UIAccessibilityPostNotification(UIAccessibilityLayoutChangedNotification, nil);
}

bool gpui_ios_is_main_thread(void) { return NSThread.isMainThread; }
GpuiIosRect gpui_ios_screen_bounds(void) { return rectValue(UIScreen.mainScreen.bounds); }
bool gpui_ios_dark(void) {
    return (activeHost ? activeHost.controller.traitCollection : UIScreen.mainScreen.traitCollection).userInterfaceStyle == UIUserInterfaceStyleDark;
}
uint32_t gpui_ios_thermal_state(void) { return (uint32_t)NSProcessInfo.processInfo.thermalState; }
void gpui_ios_activate_window(void *host) { [((__bridge GpuiHost *)host).window makeKeyAndVisible]; }
void gpui_ios_open_url(const char *text) {
    NSURL *url = [NSURL URLWithString:[NSString stringWithUTF8String:text]];
    if (!url) {
        if (applicationCallbacks.system_event) applicationCallbacks.system_event(applicationCallbacks.context, 4, "Invalid URL");
        return;
    }
    [UIApplication.sharedApplication openURL:url options:@{} completionHandler:^(BOOL success) {
        if (!success && applicationCallbacks.system_event) applicationCallbacks.system_event(applicationCallbacks.context, 4, "iOS refused to open URL");
    }];
}
char *gpui_ios_copy_string(uint32_t which) {
    NSString *value = nil;
    switch (which) {
        case 0: value = NSBundle.mainBundle.bundlePath; break;
        case 1: value = UIPasteboard.generalPasteboard.string; break;
        case 2: value = activeHost.controller.view.textInputMode.primaryLanguage; break;
    }
    return value ? strdup(value.UTF8String) : NULL;
}
void gpui_ios_free_string(char *text) { free(text); }
char *gpui_ios_copy_application_support_path(void) {
    NSURL *directory = [NSFileManager.defaultManager URLForDirectory:NSApplicationSupportDirectory
        inDomain:NSUserDomainMask appropriateForURL:nil create:YES error:nil];
    return directory.path ? strdup(directory.path.UTF8String) : NULL;
}
void gpui_ios_set_clipboard(const char *text) { UIPasteboard.generalPasteboard.string = [NSString stringWithUTF8String:text]; }
void gpui_ios_dispatch(void *context, void (*callback)(void *), uint64_t delay) {
    dispatch_after_f(dispatch_time(DISPATCH_TIME_NOW, (int64_t)delay), dispatch_get_main_queue(), context, callback);
}
