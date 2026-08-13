#import <AppKit/AppKit.h>
#import <AVFoundation/AVFoundation.h>
#import <CoreMedia/CoreMedia.h>
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>

typedef void (*GPBXMediaEventCallback)(void *context, int event);

static NSString *const GPBXMediaErrorDomain = @"GPUIBoxMedia";

typedef NS_ENUM(NSInteger, GPBXMediaErrorCode) {
    GPBXMediaErrorInvalidSource = 2,
    GPBXMediaErrorPlayback = 3,
    GPBXMediaErrorOpen = 4,
};

typedef struct GPBXMediaSnapshot {
    int availability;
    int playback;
    double position;
    double duration;
    double volume;
    double rate;
    bool muted;
} GPBXMediaSnapshot;

@interface GPBXMediaPlayerView : NSView
@property(nonatomic, strong) AVPlayerLayer *playerLayer;
@end

@implementation GPBXMediaPlayerView
- (void)layout {
    [super layout];
    self.playerLayer.frame = self.bounds;
}
@end

@interface GPBXMediaPlayer : NSObject
@property(nonatomic, strong) AVPlayer *player;
@property(nonatomic, strong) GPBXMediaPlayerView *view;
@property(nonatomic, strong) AVPlayerItem *observedItem;
@property(nonatomic, strong) id endObserver;
@property(nonatomic, strong) id timeObserver;
@property(nonatomic, strong) NSError *loadError;
@property(nonatomic, strong) NSError *commandError;
@property(nonatomic) GPBXMediaEventCallback callback;
@property(nonatomic) void *callbackContext;
@property(nonatomic) double requestedRate;
@property(nonatomic) uint64_t generation;
@property(nonatomic) uint64_t seekGeneration;
@property(nonatomic) BOOL ended;
@property(nonatomic) BOOL invalidated;
- (instancetype)initVideo:(BOOL)video
                  callback:(GPBXMediaEventCallback)callback
                   context:(void *)context;
- (BOOL)loadURL:(NSURL *)url;
- (NSError *)errorWithCode:(GPBXMediaErrorCode)code message:(NSString *)message;
- (void)failReplacementWithCode:(GPBXMediaErrorCode)code message:(NSString *)message;
- (void)pollStatusForGeneration:(uint64_t)generation;
- (void)emitForGeneration:(uint64_t)generation event:(int)event;
- (void)invalidate;
@end

@implementation GPBXMediaPlayer
- (instancetype)initVideo:(BOOL)video
                  callback:(GPBXMediaEventCallback)callback
                   context:(void *)context {
    self = [super init];
    if (self) {
        _player = [[AVPlayer alloc] init];
        _callback = callback;
        _callbackContext = context;
        _requestedRate = 1.0;
        __weak GPBXMediaPlayer *weakSelf = self;
        _timeObserver = [_player
            addPeriodicTimeObserverForInterval:CMTimeMake(1, 10)
                                      queue:dispatch_get_main_queue()
                                 usingBlock:^(CMTime time) {
                                   (void)time;
                                   GPBXMediaPlayer *strongSelf = weakSelf;
                                   if (strongSelf != nil) {
                                       [strongSelf emitForGeneration:strongSelf.generation event:0];
                                   }
                                 }];
        if (video) {
            _view = [[GPBXMediaPlayerView alloc] initWithFrame:NSZeroRect];
            _view.wantsLayer = YES;
            AVPlayerLayer *layer = [AVPlayerLayer playerLayerWithPlayer:_player];
            layer.videoGravity = AVLayerVideoGravityResizeAspect;
            _view.playerLayer = layer;
            [_view.layer addSublayer:layer];
        }
    }
    return self;
}

- (void)dealloc {
    [self invalidate];
}

- (void)invalidate {
    NSCAssert([NSThread isMainThread], @"native media teardown must run on the main thread");
    if (_invalidated) {
        return;
    }
    _invalidated = YES;
    _generation += 1;
    _seekGeneration += 1;
    _callback = NULL;
    _callbackContext = NULL;
    [self removeItemObservers];
    if (_timeObserver != nil) {
        [_player removeTimeObserver:_timeObserver];
        _timeObserver = nil;
    }
    [_player pause];
    [_player replaceCurrentItemWithPlayerItem:nil];
    if (_view.playerLayer != nil) {
        _view.playerLayer.player = nil;
    }
}

- (void)removeItemObservers {
    if (_endObserver != nil) {
        [[NSNotificationCenter defaultCenter] removeObserver:_endObserver];
        _endObserver = nil;
    }
    _observedItem = nil;
}

- (void)beginReplacement {
    NSCAssert([NSThread isMainThread], @"native media loads must run on the main thread");
    _generation += 1;
    _seekGeneration += 1;
    [_player pause];
    [self removeItemObservers];
    [_player replaceCurrentItemWithPlayerItem:nil];
    _loadError = nil;
    _commandError = nil;
    _ended = NO;
}

- (NSError *)errorWithCode:(GPBXMediaErrorCode)code message:(NSString *)message {
    return [NSError errorWithDomain:GPBXMediaErrorDomain
                               code:code
                           userInfo:@{NSLocalizedDescriptionKey : message}];
}

- (void)failReplacementWithCode:(GPBXMediaErrorCode)code message:(NSString *)message {
    [self beginReplacement];
    _loadError = [self errorWithCode:code message:message];
    [self emitForGeneration:_generation event:0];
}

- (BOOL)loadURL:(NSURL *)url {
    [self beginReplacement];
    if (url == nil) {
        _loadError = [self errorWithCode:GPBXMediaErrorInvalidSource
                                 message:@"The media source is not a valid URL."];
        [self emitForGeneration:_generation event:0];
        return NO;
    }
    AVPlayerItem *item = [AVPlayerItem playerItemWithURL:url];
    if (item == nil) {
        _loadError = [self errorWithCode:GPBXMediaErrorInvalidSource
                                 message:@"AVFoundation refused the media URL."];
        [self emitForGeneration:_generation event:0];
        return NO;
    }
    uint64_t generation = _generation;
    _observedItem = item;
    __weak GPBXMediaPlayer *weakSelf = self;
    _endObserver = [[NSNotificationCenter defaultCenter]
        addObserverForName:AVPlayerItemDidPlayToEndTimeNotification
                    object:item
                     queue:[NSOperationQueue mainQueue]
                usingBlock:^(NSNotification *notification) {
                  (void)notification;
                  GPBXMediaPlayer *strongSelf = weakSelf;
                  if (strongSelf != nil && generation == strongSelf.generation &&
                      item == strongSelf.observedItem) {
                      strongSelf.ended = YES;
                      [strongSelf emitForGeneration:generation event:1];
                  }
                }];
    [_player replaceCurrentItemWithPlayerItem:item];
    [self pollStatusForGeneration:generation];
    return YES;
}

- (void)pollStatusForGeneration:(uint64_t)generation {
    NSCAssert([NSThread isMainThread], @"native media status polling must run on the main thread");
    if (_invalidated || generation != _generation || _observedItem == nil) {
        return;
    }
    if (_observedItem.status != AVPlayerItemStatusUnknown) {
        [self emitForGeneration:generation event:0];
        return;
    }
    __weak GPBXMediaPlayer *weakSelf = self;
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 20 * NSEC_PER_MSEC),
                   dispatch_get_main_queue(), ^{
                     GPBXMediaPlayer *strongSelf = weakSelf;
                     if (strongSelf != nil) {
                         [strongSelf pollStatusForGeneration:generation];
                     }
                   });
}

- (void)emitForGeneration:(uint64_t)generation event:(int)event {
    if (![NSThread isMainThread]) {
        __weak GPBXMediaPlayer *weakSelf = self;
        dispatch_async(dispatch_get_main_queue(), ^{
          GPBXMediaPlayer *strongSelf = weakSelf;
          if (strongSelf != nil) {
              [strongSelf emitForGeneration:generation event:event];
          }
        });
        return;
    }
    if (!_invalidated && generation == _generation && _callback != NULL) {
        _callback(_callbackContext, event);
    }
}
@end

bool gpui_media_player_is_main_thread(void) {
    return [NSThread isMainThread];
}

void *gpui_media_player_create(bool video,
                               GPBXMediaEventCallback callback,
                               void *context) {
    if (![NSThread isMainThread]) {
        return NULL;
    }
    GPBXMediaPlayer *player = [[GPBXMediaPlayer alloc] initVideo:video
                                                       callback:callback
                                                        context:context];
    return (__bridge_retained void *)player;
}

static GPBXMediaPlayer *GPBXPlayer(void *opaque) {
    return (__bridge GPBXMediaPlayer *)opaque;
}

void gpui_media_player_destroy(void *opaque) {
    if (opaque != NULL) {
        [GPBXPlayer(opaque) invalidate];
        CFBridgingRelease(opaque);
    }
}

void *gpui_media_player_view(void *opaque) {
    return (__bridge void *)GPBXPlayer(opaque).view;
}

bool gpui_media_player_load_file(void *opaque, const char *path) {
    if (path == NULL) {
        return false;
    }
    NSString *string = [NSString stringWithUTF8String:path];
    if (string == nil) {
        [GPBXPlayer(opaque) failReplacementWithCode:GPBXMediaErrorInvalidSource
                                            message:@"The local media path is not valid UTF-8."];
        return false;
    }
    return [GPBXPlayer(opaque) loadURL:[NSURL fileURLWithPath:string]];
}

bool gpui_media_player_load_url(void *opaque, const char *url) {
    if (url == NULL) {
        return false;
    }
    NSString *string = [NSString stringWithUTF8String:url];
    if (string == nil) {
        [GPBXPlayer(opaque) failReplacementWithCode:GPBXMediaErrorInvalidSource
                                            message:@"The media URL is not valid UTF-8."];
        return false;
    }
    return [GPBXPlayer(opaque) loadURL:[NSURL URLWithString:string]];
}

void gpui_media_player_fail_load(void *opaque, int kind, const char *message) {
    NSString *detail = message == NULL ? @"The media source is invalid."
                                        : [NSString stringWithUTF8String:message];
    [GPBXPlayer(opaque) failReplacementWithCode:(GPBXMediaErrorCode)kind
                                        message:detail];
}

static double GPBXSeconds(CMTime time) {
    if (!CMTIME_IS_NUMERIC(time)) {
        return NAN;
    }
    return CMTimeGetSeconds(time);
}

static BOOL GPBXReady(GPBXMediaPlayer *controller) {
    if (controller.loadError != nil ||
        controller.player.currentItem.status != AVPlayerItemStatusReadyToPlay) {
        return NO;
    }
    double duration = GPBXSeconds(controller.player.currentItem.duration);
    return !isfinite(duration) || duration > 0.0;
}

static BOOL GPBXSeekable(GPBXMediaPlayer *controller, double *duration) {
    if (!GPBXReady(controller)) {
        return NO;
    }
    double seconds = GPBXSeconds(controller.player.currentItem.duration);
    if (!isfinite(seconds) || seconds <= 0.0) {
        return NO;
    }
    if (duration != NULL) {
        *duration = seconds;
    }
    return YES;
}

static void GPBXRefuseCommand(GPBXMediaPlayer *controller, NSString *message) {
    controller.commandError = [controller errorWithCode:GPBXMediaErrorPlayback message:message];
}

bool gpui_media_player_play(void *opaque) {
    GPBXMediaPlayer *controller = GPBXPlayer(opaque);
    controller.commandError = nil;
    if (!GPBXReady(controller)) {
        GPBXRefuseCommand(controller, @"The media source is not ready for playback.");
        return false;
    }
    if (controller.ended) {
        uint64_t generation = controller.generation;
        uint64_t seekGeneration = ++controller.seekGeneration;
        AVPlayerItem *item = controller.observedItem;
        __weak GPBXMediaPlayer *weakController = controller;
        [controller.player seekToTime:kCMTimeZero
                      toleranceBefore:kCMTimeZero
                       toleranceAfter:kCMTimeZero
                    completionHandler:^(BOOL finished) {
                      dispatch_async(dispatch_get_main_queue(), ^{
                        GPBXMediaPlayer *strongController = weakController;
                        if (strongController == nil ||
                            generation != strongController.generation ||
                            seekGeneration != strongController.seekGeneration ||
                            item != strongController.observedItem) {
                            return;
                        }
                        if (!finished) {
                            GPBXRefuseCommand(strongController,
                                             @"AVFoundation could not restart playback.");
                            [strongController emitForGeneration:generation event:0];
                            return;
                        }
                        strongController.ended = NO;
                        [strongController.player
                            playImmediatelyAtRate:(float)strongController.requestedRate];
                        [strongController emitForGeneration:generation event:0];
                      });
                    }];
        return true;
    }
    [controller.player playImmediatelyAtRate:(float)controller.requestedRate];
    [controller emitForGeneration:controller.generation event:0];
    return true;
}

bool gpui_media_player_pause(void *opaque) {
    GPBXMediaPlayer *controller = GPBXPlayer(opaque);
    controller.commandError = nil;
    if (!GPBXReady(controller)) {
        GPBXRefuseCommand(controller, @"The media source is not ready for playback.");
        return false;
    }
    [controller.player pause];
    [controller emitForGeneration:controller.generation event:0];
    return true;
}

bool gpui_media_player_seek(void *opaque, double seconds) {
    GPBXMediaPlayer *controller = GPBXPlayer(opaque);
    controller.commandError = nil;
    double duration = 0.0;
    if (!GPBXSeekable(controller, &duration) || seconds < 0.0 || seconds > duration) {
        GPBXRefuseCommand(controller,
                         @"The active media source does not have a seekable duration.");
        return false;
    }
    uint64_t generation = controller.generation;
    uint64_t seekGeneration = ++controller.seekGeneration;
    AVPlayerItem *item = controller.observedItem;
    __weak GPBXMediaPlayer *weakController = controller;
    CMTime time = CMTimeMakeWithSeconds(seconds, NSEC_PER_SEC);
    [controller.player seekToTime:time
                  toleranceBefore:kCMTimeZero
                   toleranceAfter:kCMTimeZero
                completionHandler:^(BOOL finished) {
                  dispatch_async(dispatch_get_main_queue(), ^{
                    GPBXMediaPlayer *strongController = weakController;
                    if (strongController == nil || generation != strongController.generation ||
                        seekGeneration != strongController.seekGeneration ||
                        item != strongController.observedItem) {
                        return;
                    }
                    if (finished) {
                        strongController.ended = NO;
                    } else {
                        GPBXRefuseCommand(strongController,
                                         @"AVFoundation could not seek the media source.");
                    }
                    [strongController emitForGeneration:generation event:0];
                  });
                }];
    return true;
}

bool gpui_media_player_set_volume(void *opaque, double volume) {
    GPBXMediaPlayer *controller = GPBXPlayer(opaque);
    controller.commandError = nil;
    controller.player.volume = (float)volume;
    [controller emitForGeneration:controller.generation event:0];
    return true;
}

bool gpui_media_player_set_muted(void *opaque, bool muted) {
    GPBXMediaPlayer *controller = GPBXPlayer(opaque);
    controller.commandError = nil;
    controller.player.muted = muted;
    [controller emitForGeneration:controller.generation event:0];
    return true;
}

bool gpui_media_player_set_rate(void *opaque, double rate) {
    GPBXMediaPlayer *controller = GPBXPlayer(opaque);
    controller.commandError = nil;
    controller.requestedRate = rate;
    if (controller.player.rate != 0.0f) {
        [controller.player setRate:(float)rate];
    }
    [controller emitForGeneration:controller.generation event:0];
    return true;
}

void gpui_media_player_snapshot(void *opaque, GPBXMediaSnapshot *snapshot) {
    GPBXMediaPlayer *controller = GPBXPlayer(opaque);
    AVPlayerItem *item = controller.player.currentItem;
    if (item == nil) {
        snapshot->availability = controller.loadError == nil ? 0 : 3;
    } else if (item.status == AVPlayerItemStatusUnknown) {
        snapshot->availability = 1;
    } else if (item.status == AVPlayerItemStatusReadyToPlay) {
        double duration = GPBXSeconds(item.duration);
        snapshot->availability = isfinite(duration) && duration <= 0.0 ? 3 : 2;
    } else {
        NSInteger code = item.error.code;
        snapshot->availability = code == AVErrorDecoderNotFound ? 4 : 3;
    }

    if (controller.ended) {
        snapshot->playback = 3;
    } else if (controller.player.timeControlStatus ==
               AVPlayerTimeControlStatusWaitingToPlayAtSpecifiedRate) {
        snapshot->playback = 2;
    } else if (controller.player.timeControlStatus == AVPlayerTimeControlStatusPlaying) {
        snapshot->playback = 1;
    } else {
        snapshot->playback = 0;
    }
    snapshot->position = GPBXSeconds(controller.player.currentTime);
    snapshot->duration = item == nil ? NAN : GPBXSeconds(item.duration);
    snapshot->volume = controller.player.volume;
    snapshot->muted = controller.player.muted;
    snapshot->rate = controller.requestedRate;
}

size_t gpui_media_player_buffered_count(void *opaque) {
    return GPBXPlayer(opaque).player.currentItem.loadedTimeRanges.count;
}

bool gpui_media_player_buffered_range(void *opaque,
                                      size_t index,
                                      double *start,
                                      double *end) {
    NSArray<NSValue *> *ranges = GPBXPlayer(opaque).player.currentItem.loadedTimeRanges;
    if (index >= ranges.count) {
        return false;
    }
    CMTimeRange range = [ranges[index] CMTimeRangeValue];
    *start = GPBXSeconds(range.start);
    *end = GPBXSeconds(CMTimeRangeGetEnd(range));
    return true;
}

int gpui_media_player_copy_error(void *opaque, char *buffer, size_t capacity) {
    GPBXMediaPlayer *controller = GPBXPlayer(opaque);
    AVPlayerItem *item = controller.player.currentItem;
    NSError *error = controller.loadError ?: item.error;
    if (error == nil && item.status == AVPlayerItemStatusReadyToPlay) {
        double duration = GPBXSeconds(item.duration);
        if (isfinite(duration) && duration <= 0.0) {
            error = [controller errorWithCode:GPBXMediaErrorOpen
                                      message:@"The media source has no playable duration."];
        }
    }
    error = error ?: controller.commandError;
    if (capacity > 0) {
        const char *message = error.localizedDescription.UTF8String;
        snprintf(buffer, capacity, "%s", message == NULL ? "" : message);
    }
    if ([error.domain isEqualToString:GPBXMediaErrorDomain]) {
        return (int)error.code;
    }
    if (error.code == AVErrorDecoderNotFound) {
        return 1;
    }
    return 0;
}
