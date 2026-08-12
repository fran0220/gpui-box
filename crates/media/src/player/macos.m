#import <AppKit/AppKit.h>
#import <AVFoundation/AVFoundation.h>
#import <CoreMedia/CoreMedia.h>
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>

typedef void (*GPBXMediaEventCallback)(void *context, int event);

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
@property(nonatomic, strong) id timeObserver;
@property(nonatomic, strong) NSError *immediateError;
@property(nonatomic) GPBXMediaEventCallback callback;
@property(nonatomic) void *callbackContext;
@property(nonatomic) double requestedRate;
@property(nonatomic) BOOL ended;
- (instancetype)initVideo:(BOOL)video
                  callback:(GPBXMediaEventCallback)callback
                   context:(void *)context;
- (BOOL)loadURL:(NSURL *)url;
- (void)emit:(int)event;
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
        [_player addObserver:self
                  forKeyPath:@"timeControlStatus"
                     options:NSKeyValueObservingOptionNew
                     context:NULL];
        __weak GPBXMediaPlayer *weakSelf = self;
        _timeObserver = [_player
            addPeriodicTimeObserverForInterval:CMTimeMake(1, 10)
                                      queue:dispatch_get_main_queue()
                                 usingBlock:^(CMTime time) {
                                   (void)time;
                                   [weakSelf emit:0];
                                 }];
        if (video) {
            _view = [[GPBXMediaPlayerView alloc] initWithFrame:NSZeroRect];
            _view.wantsLayer = YES;
            AVPlayerLayer *layer = [AVPlayerLayer playerLayerWithPlayer:_player];
            layer.videoGravity = AVLayerVideoGravityResizeAspect;
            _view.playerLayer = layer;
            [_view.layer addSublayer:layer];
        }
        [[NSNotificationCenter defaultCenter]
            addObserver:self
               selector:@selector(itemEnded:)
                   name:AVPlayerItemDidPlayToEndTimeNotification
                 object:nil];
    }
    return self;
}

- (void)dealloc {
    [[NSNotificationCenter defaultCenter] removeObserver:self];
    [self removeItemObservers];
    if (_timeObserver != nil) {
        [_player removeTimeObserver:_timeObserver];
    }
    [_player removeObserver:self forKeyPath:@"timeControlStatus"];
}

- (void)removeItemObservers {
    if (_observedItem != nil) {
        [_observedItem removeObserver:self forKeyPath:@"status"];
        [_observedItem removeObserver:self forKeyPath:@"loadedTimeRanges"];
        [_observedItem removeObserver:self forKeyPath:@"playbackBufferEmpty"];
        _observedItem = nil;
    }
}

- (BOOL)loadURL:(NSURL *)url {
    if (url == nil) {
        _immediateError = [NSError
            errorWithDomain:@"GPUIBoxMedia"
                       code:2
                   userInfo:@{NSLocalizedDescriptionKey : @"The media source is not a valid URL."}];
        [self emit:0];
        return NO;
    }
    [self removeItemObservers];
    AVPlayerItem *item = [AVPlayerItem playerItemWithURL:url];
    if (item == nil) {
        _immediateError = [NSError
            errorWithDomain:@"GPUIBoxMedia"
                       code:2
                   userInfo:@{NSLocalizedDescriptionKey : @"AVFoundation refused the media URL."}];
        [self emit:0];
        return NO;
    }
    _immediateError = nil;
    _ended = NO;
    _observedItem = item;
    [item addObserver:self
           forKeyPath:@"status"
              options:NSKeyValueObservingOptionNew
              context:NULL];
    [item addObserver:self
           forKeyPath:@"loadedTimeRanges"
              options:NSKeyValueObservingOptionNew
              context:NULL];
    [item addObserver:self
           forKeyPath:@"playbackBufferEmpty"
              options:NSKeyValueObservingOptionNew
              context:NULL];
    [_player replaceCurrentItemWithPlayerItem:item];
    [self emit:0];
    return YES;
}

- (void)itemEnded:(NSNotification *)notification {
    if (notification.object == _player.currentItem) {
        _ended = YES;
        [self emit:1];
    }
}

- (void)observeValueForKeyPath:(NSString *)keyPath
                      ofObject:(id)object
                        change:(NSDictionary<NSKeyValueChangeKey, id> *)change
                       context:(void *)context {
    (void)keyPath;
    (void)object;
    (void)change;
    (void)context;
    [self emit:0];
}

- (void)emit:(int)event {
    if (_callback != NULL) {
        _callback(_callbackContext, event);
    }
}
@end

void *gpui_media_player_create(bool video,
                               GPBXMediaEventCallback callback,
                               void *context) {
    GPBXMediaPlayer *player = [[GPBXMediaPlayer alloc] initVideo:video
                                                       callback:callback
                                                        context:context];
    return (__bridge_retained void *)player;
}

void gpui_media_player_destroy(void *opaque) {
    if (opaque != NULL) {
        CFBridgingRelease(opaque);
    }
}

static GPBXMediaPlayer *GPBXPlayer(void *opaque) {
    return (__bridge GPBXMediaPlayer *)opaque;
}

void *gpui_media_player_view(void *opaque) {
    return (__bridge void *)GPBXPlayer(opaque).view;
}

bool gpui_media_player_load_file(void *opaque, const char *path) {
    if (path == NULL) {
        return false;
    }
    NSString *string = [NSString stringWithUTF8String:path];
    return [GPBXPlayer(opaque) loadURL:[NSURL fileURLWithPath:string]];
}

bool gpui_media_player_load_url(void *opaque, const char *url) {
    if (url == NULL) {
        return false;
    }
    NSString *string = [NSString stringWithUTF8String:url];
    return [GPBXPlayer(opaque) loadURL:[NSURL URLWithString:string]];
}

void gpui_media_player_play(void *opaque) {
    GPBXMediaPlayer *controller = GPBXPlayer(opaque);
    if (controller.ended) {
        [controller.player seekToTime:kCMTimeZero];
        controller.ended = NO;
    }
    [controller.player playImmediatelyAtRate:(float)controller.requestedRate];
    [controller emit:0];
}

void gpui_media_player_pause(void *opaque) {
    GPBXMediaPlayer *controller = GPBXPlayer(opaque);
    [controller.player pause];
    [controller emit:0];
}

void gpui_media_player_seek(void *opaque, double seconds) {
    GPBXMediaPlayer *controller = GPBXPlayer(opaque);
    CMTime time = CMTimeMakeWithSeconds(seconds, NSEC_PER_SEC);
    [controller.player seekToTime:time];
    controller.ended = NO;
    [controller emit:0];
}

void gpui_media_player_set_volume(void *opaque, double volume) {
    GPBXMediaPlayer *controller = GPBXPlayer(opaque);
    controller.player.volume = (float)volume;
    [controller emit:0];
}

void gpui_media_player_set_muted(void *opaque, bool muted) {
    GPBXMediaPlayer *controller = GPBXPlayer(opaque);
    controller.player.muted = muted;
    [controller emit:0];
}

void gpui_media_player_set_rate(void *opaque, double rate) {
    GPBXMediaPlayer *controller = GPBXPlayer(opaque);
    controller.requestedRate = rate;
    if (controller.player.rate != 0.0f) {
        [controller.player setRate:(float)rate];
    }
    [controller emit:0];
}

static double GPBXSeconds(CMTime time) {
    if (!CMTIME_IS_NUMERIC(time)) {
        return NAN;
    }
    return CMTimeGetSeconds(time);
}

void gpui_media_player_snapshot(void *opaque, GPBXMediaSnapshot *snapshot) {
    GPBXMediaPlayer *controller = GPBXPlayer(opaque);
    AVPlayerItem *item = controller.player.currentItem;
    if (item == nil) {
        snapshot->availability = controller.immediateError == nil ? 0 : 3;
    } else if (item.status == AVPlayerItemStatusUnknown) {
        snapshot->availability = 1;
    } else if (item.status == AVPlayerItemStatusReadyToPlay) {
        snapshot->availability = 2;
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
    NSError *error = controller.immediateError ?: controller.player.currentItem.error;
    if (capacity > 0) {
        const char *message = error.localizedDescription.UTF8String;
        snprintf(buffer, capacity, "%s", message == NULL ? "" : message);
    }
    if ([error.domain isEqualToString:@"GPUIBoxMedia"] && error.code == 2) {
        return 2;
    }
    if (error.code == AVErrorDecoderNotFound) {
        return 1;
    }
    return 0;
}
