#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use media::{
    MediaAvailability, MediaCommand, MediaCommandOutcome, MediaErrorKind, MediaKind, MediaPlayer,
    MediaSource, PlaybackState,
};

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn main() {}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn main() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("create native media test directory");
    let playable = directory.join("playable.wav");
    write_pcm_wave(&playable, 8_000);
    let empty = directory.join("empty.wav");
    write_pcm_wave(&empty, 0);

    main_thread_construction_and_native_view();
    zero_duration_is_not_ready(&empty);
    source_replacement_and_controls(&playable);
    callback_teardown(&playable);
    #[cfg(target_os = "windows")]
    com_and_media_foundation_lifetimes(&playable);

    fs::remove_dir_all(directory).expect("remove native media test directory");
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn zero_duration_is_not_ready(empty: &Path) {
    let player = MediaPlayer::new(MediaKind::Audio);
    player
        .load(MediaSource::file(empty))
        .expect("native backend accepts the empty PCM container for inspection");
    wait_for("empty PCM source to reach a terminal availability", || {
        !matches!(player.snapshot().availability, MediaAvailability::Loading)
    });
    let snapshot = player.snapshot();
    assert!(
        matches!(
            snapshot.availability,
            MediaAvailability::Failed(_) | MediaAvailability::NoBackend(_)
        ),
        "zero-duration media must not become ready: {:?}",
        snapshot.availability
    );
    assert_eq!(snapshot.duration, None);
    assert!(matches!(
        player.command(MediaCommand::Seek(0.0)),
        MediaCommandOutcome::Refused(_)
    ));
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn main_thread_construction_and_native_view() {
    let audio = MediaPlayer::new(MediaKind::Audio);
    assert!(matches!(
        audio.snapshot().availability,
        MediaAvailability::Idle
    ));
    assert!(audio.native_video_view().is_none());

    let video = MediaPlayer::new(MediaKind::Video);
    assert!(matches!(
        video.snapshot().availability,
        MediaAvailability::Idle
    ));
    assert!(video.native_video_view().is_some());
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn source_replacement_and_controls(playable: &Path) {
    let player = MediaPlayer::new(MediaKind::Audio);
    let changes = Arc::new(AtomicUsize::new(0));
    let _subscription = player.subscribe({
        let changes = Arc::clone(&changes);
        move |_| {
            changes.fetch_add(1, Ordering::Relaxed);
        }
    });

    player
        .load(MediaSource::file(playable))
        .expect("native backend accepts PCM wave source");
    wait_for("playable source to become ready", || {
        matches!(player.snapshot().availability, MediaAvailability::Ready)
    });
    let ready = player.snapshot();
    let duration = ready.duration.expect("PCM wave duration is known");
    assert!(duration > 0.9 && duration < 1.1, "duration was {duration}");

    assert_eq!(
        player.command(MediaCommand::SetVolume(0.25)),
        MediaCommandOutcome::Applied
    );
    assert_eq!(
        player.command(MediaCommand::SetMuted(true)),
        MediaCommandOutcome::Applied
    );
    assert_eq!(
        player.command(MediaCommand::SetRate(1.25)),
        MediaCommandOutcome::Applied
    );
    let settings = player.snapshot();
    assert!((settings.volume - 0.25).abs() < 0.01);
    assert!(settings.muted);
    assert!((settings.rate - 1.25).abs() < 0.01);

    assert_eq!(
        player.command(MediaCommand::Seek(0.5)),
        MediaCommandOutcome::Applied
    );
    wait_for("seek to be reflected by the native clock", || {
        player.snapshot().position >= 0.4
    });
    assert_eq!(
        player.command(MediaCommand::Play),
        MediaCommandOutcome::Applied
    );
    wait_for("native playback to start", || {
        matches!(
            player.snapshot().state,
            PlaybackState::Playing | PlaybackState::Ended
        )
    });
    wait_for("native playback to reach its terminal state", || {
        player.snapshot().state == PlaybackState::Ended
    });

    assert_eq!(
        player.command(MediaCommand::Play),
        MediaCommandOutcome::Applied
    );
    wait_for("ended playback to restart from the beginning", || {
        let snapshot = player.snapshot();
        snapshot.state != PlaybackState::Ended && snapshot.position < 0.4
    });
    assert_eq!(
        player.command(MediaCommand::Pause),
        MediaCommandOutcome::Applied
    );
    wait_for("native playback to pause", || {
        player.snapshot().state == PlaybackState::Paused
    });

    let invalid = player
        .load(MediaSource::url("invalid\0replacement"))
        .expect_err("a null-containing replacement is invalid");
    assert_eq!(invalid.kind(), MediaErrorKind::InvalidSource);
    assert!(matches!(
        player.snapshot().availability,
        MediaAvailability::Failed(_)
    ));
    pump_for(Duration::from_millis(500));
    assert!(matches!(
        player.snapshot().availability,
        MediaAvailability::Failed(_)
    ));
    assert!(changes.load(Ordering::Relaxed) > 0);
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn callback_teardown(playable: &Path) {
    let calls = Arc::new(AtomicUsize::new(0));
    let before_drop = {
        let player = MediaPlayer::new(MediaKind::Audio);
        let _subscription = player.subscribe({
            let calls = Arc::clone(&calls);
            move |_| {
                calls.fetch_add(1, Ordering::Relaxed);
            }
        });
        player
            .load(MediaSource::file(playable))
            .expect("native backend accepts teardown source");
        wait_for("teardown source callback", || {
            calls.load(Ordering::Relaxed) > 0
        });
        calls.load(Ordering::Relaxed)
    };
    pump_for(Duration::from_millis(500));
    assert_eq!(
        calls.load(Ordering::Relaxed),
        before_drop,
        "destroyed player delivered a queued callback"
    );
}

#[cfg(target_os = "windows")]
fn com_and_media_foundation_lifetimes(playable: &Path) {
    use windows::Win32::System::Com::{
        APTTYPE, APTTYPEQUALIFIER, COINIT_MULTITHREADED, CoGetApartmentType, CoInitializeEx,
        CoUninitialize,
    };

    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    assert!(initialized.is_ok(), "initialize outer COM apartment");
    let first = MediaPlayer::new(MediaKind::Audio);
    let second = MediaPlayer::new(MediaKind::Audio);
    drop(first);
    let mut apartment = APTTYPE::default();
    let mut qualifier = APTTYPEQUALIFIER::default();
    unsafe { CoGetApartmentType(&mut apartment, &mut qualifier) }
        .expect("player must not uninitialize the caller-owned COM apartment");

    second
        .load(MediaSource::file(playable))
        .expect("remaining player keeps Media Foundation alive");
    wait_for("second player to become ready after first drops", || {
        matches!(second.snapshot().availability, MediaAvailability::Ready)
    });
    drop(second);
    unsafe { CoUninitialize() };
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn wait_for(description: &str, mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if predicate() {
            return;
        }
        pump_for(Duration::from_millis(10));
    }
    panic!("timed out waiting for {description}");
}

#[cfg(target_os = "macos")]
fn pump_for(duration: Duration) {
    use objc::{class, msg_send, runtime::Object, sel, sel_impl};

    unsafe {
        let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
        let date: *mut Object =
            msg_send![class!(NSDate), dateWithTimeIntervalSinceNow: duration.as_secs_f64()];
        let run_loop: *mut Object = msg_send![class!(NSRunLoop), currentRunLoop];
        let _: () = msg_send![run_loop, runUntilDate: date];
        let _: () = msg_send![pool, drain];
    }
}

#[cfg(target_os = "windows")]
fn pump_for(duration: Duration) {
    std::thread::sleep(duration);
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn temporary_directory() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time follows Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "gpui-box-native-media-{}-{nonce}",
        std::process::id()
    ))
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn write_pcm_wave(path: &Path, sample_count: u32) {
    let channels = 1_u16;
    let sample_rate = 8_000_u32;
    let bits_per_sample = 16_u16;
    let block_align = channels * (bits_per_sample / 8);
    let byte_rate = sample_rate * u32::from(block_align);
    let data_size = sample_count * u32::from(block_align);
    let mut wave = Vec::with_capacity(44 + data_size as usize);
    wave.extend_from_slice(b"RIFF");
    wave.extend_from_slice(&(36 + data_size).to_le_bytes());
    wave.extend_from_slice(b"WAVEfmt ");
    wave.extend_from_slice(&16_u32.to_le_bytes());
    wave.extend_from_slice(&1_u16.to_le_bytes());
    wave.extend_from_slice(&channels.to_le_bytes());
    wave.extend_from_slice(&sample_rate.to_le_bytes());
    wave.extend_from_slice(&byte_rate.to_le_bytes());
    wave.extend_from_slice(&block_align.to_le_bytes());
    wave.extend_from_slice(&bits_per_sample.to_le_bytes());
    wave.extend_from_slice(b"data");
    wave.extend_from_slice(&data_size.to_le_bytes());
    wave.resize(44 + data_size as usize, 0);
    fs::write(path, wave).expect("write PCM wave fixture");
}
