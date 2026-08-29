//! Motion primitives exercised against a live clock, plus described motion at rest.

use super::support::*;
use crate::motion::{
    CubicBezier, Easing, Keyframe, Keyframes, MotionSpec, Presence, Spring, Stagger, Transition,
};
use web_time::Instant;

const CLOCK_START: u16 = 8 * 60;
const CLOCK_END: u16 = 20 * 60;
const CLOCK_STEP: u16 = 30;
const CLOCK_TICK: Duration = Duration::from_millis(500);
const CLOCK_TRANSITION_MS: u64 = 620;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MotionDemo {
    SlidingTime,
    Spring,
    Keyframes,
    Presence,
    Stagger,
}

impl MotionDemo {
    fn id(self) -> &'static str {
        match self {
            Self::SlidingTime => "sliding-time",
            Self::Spring => "spring",
            Self::Keyframes => "keyframes",
            Self::Presence => "presence",
            Self::Stagger => "stagger",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        match id {
            "sliding-time" => Some(Self::SlidingTime),
            "spring" => Some(Self::Spring),
            "keyframes" => Some(Self::Keyframes),
            "presence" => Some(Self::Presence),
            "stagger" => Some(Self::Stagger),
            _ => None,
        }
    }
}

/// Caller-owned state for the five demonstrations.
///
/// The catalog rebuilds a scene every frame, just as an ordinary host rebuilds
/// component builders. The state therefore lives outside the element tree;
/// every animated channel beneath it still has a stable semantic identity.
#[derive(Debug)]
struct SceneMotionPrimitives {
    demo: MotionDemo,
    clock_minutes: u16,
    clock_targets: [f32; 4],
    clock_digits: [Transition<f32>; 4],
    clock_playing: bool,
    clock_started_at: Option<Instant>,
    clock_ticks: u16,
    spring_selected: usize,
    spring_indicator: Transition<f32>,
    present: bool,
    presence: Presence,
    keyframes_started_at: Option<Instant>,
    stagger_generation: u64,
    stagger_started_at: Option<Instant>,
}

impl Global for SceneMotionPrimitives {}

impl SceneMotionPrimitives {
    fn new(theme: &Theme) -> Self {
        let clock_spec = clock_spec(theme);
        let spring_spec = spring_spec();
        let presence_spec = presence_spec(theme);
        let targets = clock_digits(CLOCK_START).map(f32::from);
        Self {
            demo: MotionDemo::SlidingTime,
            clock_minutes: CLOCK_START,
            clock_targets: targets,
            clock_digits: targets.map(|digit| Transition::new(digit, clock_spec)),
            clock_playing: false,
            clock_started_at: None,
            clock_ticks: 0,
            spring_selected: 0,
            spring_indicator: Transition::new(0.0, spring_spec),
            present: true,
            presence: Presence::visible(presence_spec, presence_spec),
            keyframes_started_at: None,
            stagger_generation: 0,
            stagger_started_at: None,
        }
    }

    fn select(&mut self, demo: MotionDemo, theme: &Theme) {
        self.demo = demo;
        // Element-retained animation state disappears when its tab leaves the
        // tree. Recreate that boundary here even though the scene state itself
        // is longer lived, so returning to a tab never resumes an invisible
        // half-frame from the previous visit.
        match demo {
            MotionDemo::SlidingTime => {
                for (digit, target) in self
                    .clock_digits
                    .iter_mut()
                    .zip(self.clock_targets.iter().copied())
                {
                    *digit = Transition::new(target, clock_spec(theme));
                }
            }
            MotionDemo::Spring => self.spring_indicator.snap(if self.spring_selected == 0 {
                0.0
            } else {
                120.0
            }),
            MotionDemo::Keyframes => self.keyframes_started_at = None,
            MotionDemo::Presence => {
                let spec = presence_spec(theme);
                self.presence = if self.present {
                    Presence::visible(spec, spec)
                } else {
                    Presence::hidden(spec, spec)
                };
            }
            MotionDemo::Stagger => self.stagger_started_at = None,
        }
    }

    fn toggle_clock(&mut self, now: Instant, theme: &Theme) {
        if self.clock_playing {
            self.clock_playing = false;
            self.clock_started_at = None;
            self.clock_ticks = 0;
            return;
        }
        if self.clock_minutes >= CLOCK_END {
            self.clock_minutes = CLOCK_START;
            self.clock_targets = clock_digits(CLOCK_START).map(f32::from);
            for (digit, target) in self
                .clock_digits
                .iter_mut()
                .zip(self.clock_targets.iter().copied())
            {
                *digit = Transition::new(target, clock_spec(theme));
            }
        }
        self.clock_playing = true;
        self.clock_started_at = Some(now);
        self.clock_ticks = 0;
    }

    fn update_clock(&mut self, now: Instant) {
        let Some(started) = self.clock_started_at else {
            return;
        };
        let due = now
            .saturating_duration_since(started)
            .as_millis()
            .checked_div(CLOCK_TICK.as_millis())
            .unwrap_or(0) as u16;
        while self.clock_ticks < due && self.clock_minutes < CLOCK_END {
            self.clock_ticks += 1;
            self.clock_minutes = (self.clock_minutes + CLOCK_STEP).min(CLOCK_END);
            for (target, next) in self
                .clock_targets
                .iter_mut()
                .zip(clock_digits(self.clock_minutes))
            {
                *target = advance_digit(*target, next);
            }
        }
        if self.clock_minutes >= CLOCK_END {
            self.clock_playing = false;
            self.clock_started_at = None;
            self.clock_ticks = 0;
        }
    }

    fn sliding_time(&mut self, theme: &Theme, window: &mut Window, cx: &mut App) -> AnyElement {
        let spec = clock_spec(theme);
        let values: [f32; 4] = std::array::from_fn(|index| {
            let mut digit = self.clock_digits[index].spec(spec);
            digit.set(self.clock_targets[index]);
            let value = digit.animate(window, cx);
            self.clock_digits[index] = digit;
            value
        });
        let announced = format_time(self.clock_minutes);
        let play_label = if self.clock_playing { "Pause" } else { "Play" };

        demo_panel(theme, "Four interrupted transitions on one clock")
            .child(
                div()
                    .row()
                    .items_center()
                    .gap(px(theme.space(Space::Xs)))
                    .child(rolling_digit(theme, values[0]))
                    .child(rolling_digit(theme, values[1]))
                    .child(
                        div()
                            .h(px(52.0))
                            .flex()
                            .items_center()
                            .font_family(theme.typography.mono.clone())
                            .text_size(px(36.0))
                            .child(":"),
                    )
                    .child(rolling_digit(theme, values[2]))
                    .child(rolling_digit(theme, values[3]))
                    .semantic_in(
                        cx,
                        NodeSpec::new("scene.motion.clock", Role::Status)
                            .text("Simulated time")
                            .value(announced),
                    ),
            )
            .child(
                Button::new("scene.motion.clock.play")
                    .label(play_label)
                    .secondary()
                    .on_click(|_, cx| {
                        let now = cx.background_executor().now();
                        let theme = cx.theme().clone();
                        cx.update_global::<SceneMotionPrimitives, ()>(|scene, _| {
                            scene.toggle_clock(now, &theme)
                        });
                        cx.refresh_windows();
                    }),
            )
            .into_any_element()
    }

    fn spring(&mut self, theme: &Theme, window: &mut Window, cx: &mut App) -> AnyElement {
        let mut indicator = self.spring_indicator.spec(spring_spec());
        indicator.set(if self.spring_selected == 0 {
            0.0
        } else {
            120.0
        });
        let left = indicator.animate(window, cx);
        self.spring_indicator = indicator;
        let selected = self.spring_selected;

        let segment = |id: &'static str, label: &'static str, index: usize| {
            div().w(px(120.0)).child(
                Button::new(id)
                    .label(label)
                    .ghost()
                    .selected(selected == index)
                    .full_width(true)
                    .on_click(move |_, cx| {
                        cx.update_global::<SceneMotionPrimitives, ()>(|scene, _| {
                            scene.spring_selected = index
                        });
                        cx.refresh_windows();
                    }),
            )
        };

        demo_panel(theme, "Velocity survives rapid retargeting")
            .child(
                div()
                    .relative()
                    .w(px(240.0))
                    .radius(theme, Radius::Control)
                    .well(theme)
                    .child(
                        div()
                            .row()
                            .child(segment("scene.motion.spring.queue", "Queue", 0))
                            .child(segment("scene.motion.spring.timeline", "Timeline", 1)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(left))
                            .bottom_0()
                            .w(px(120.0))
                            .h(px(theme.effects.selection_rail_width))
                            .rounded(px(theme.effects.selection_rail_width / 2.0))
                            .bg(theme.colors.accent)
                            .semantic_in(
                                cx,
                                NodeSpec::new("scene.motion.spring.indicator", Role::Status)
                                    .text("Selected segment indicator")
                                    .value(if selected == 0 { "Queue" } else { "Timeline" }),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn keyframes(&mut self, theme: &Theme, window: &mut Window, cx: &mut App) -> AnyElement {
        let now = cx.background_executor().now();
        let started = *self.keyframes_started_at.get_or_insert(now);
        let elapsed = now.saturating_duration_since(started);
        let period = Duration::from_millis(1_200);
        let still = cx.reduce_motion();
        if !still {
            window.request_animation_frame();
        }
        let path = activity_keyframes(theme);
        let bars = [
            "clone", "resolve", "compile", "test", "lint", "package", "publish",
        ];

        demo_panel(theme, "One validated path, seven delayed playheads")
            .child(
                div()
                    .h(px(72.0))
                    .row()
                    .items_end()
                    .gap(px(theme.space(Space::Sm)))
                    .children(bars.into_iter().enumerate().map(|(index, name)| {
                        let delay = Duration::from_millis(index as u64 * 80);
                        let phase = if still || elapsed < delay {
                            0.0
                        } else {
                            elapsed.saturating_sub(delay).as_secs_f32() / period.as_secs_f32()
                        };
                        let value = path.sample(phase.rem_euclid(1.0));
                        div()
                            .w(px(18.0))
                            .h(px(18.0 + 38.0 * value))
                            .radius(theme, Radius::Small)
                            .bg(theme.colors.accent)
                            .opacity(0.35 + 0.65 * value)
                            .semantic_in(
                                cx,
                                NodeSpec::new(
                                    format!("scene.motion.keyframes.{name}"),
                                    Role::Status,
                                )
                                .text(format!("{name} activity"))
                                .value(format!("{value:.3}")),
                            )
                    })),
            )
            .into_any_element()
    }

    fn presence(&mut self, theme: &Theme, window: &mut Window, cx: &mut App) -> AnyElement {
        let progress = self.presence.animate(window, cx);
        let visible = self.presence.is_rendered();
        let button = if self.present {
            "Hide notice"
        } else {
            "Show notice"
        };

        demo_panel(theme, "Logical absence waits for the exit to finish")
            .child(
                div()
                    .h(px(76.0))
                    .flex()
                    .items_center()
                    .children(visible.then(|| {
                        div()
                            .w(px(360.0))
                            .p(px(theme.space(Space::Md)))
                            .radius(theme, Radius::Card)
                            .bg(theme.colors.accent.opacity(0.14))
                            .opacity(progress)
                            .child(crate::foundation::text(
                                theme,
                                TypeScale::Body,
                                "The verified result is ready to review.",
                            ))
                            .semantic_in(
                                cx,
                                NodeSpec::new("scene.motion.presence.notice", Role::Status)
                                    .text("The verified result is ready to review."),
                            )
                    })),
            )
            .child(
                Button::new("scene.motion.presence.toggle")
                    .label(button)
                    .secondary()
                    .on_click(|_, cx| {
                        cx.update_global::<SceneMotionPrimitives, ()>(|scene, _| {
                            scene.present = !scene.present;
                            if scene.present {
                                scene.presence.show();
                            } else {
                                scene.presence.hide();
                            }
                        });
                        cx.refresh_windows();
                    }),
            )
            .into_any_element()
    }

    fn stagger(&mut self, theme: &Theme, window: &mut Window, cx: &mut App) -> AnyElement {
        let now = cx.background_executor().now();
        let started = *self.stagger_started_at.get_or_insert(now);
        let elapsed = now.saturating_duration_since(started);
        let spec = MotionSpec::new(360, Easing::EaseOut.curve(theme));
        let stagger = Stagger::from_millis(90);
        let total = stagger.total(3, spec);
        let still = cx.reduce_motion();
        if !still && elapsed < total {
            window.request_animation_frame();
        }
        let rows = [
            ("plan", "Plan the release"),
            ("build", "Build the artifacts"),
            ("publish", "Publish the result"),
        ];

        demo_panel(theme, "A shared clock spreads one entrance across a list")
            .child(div().column().gap(px(theme.space(Space::Sm))).children(
                rows.into_iter().enumerate().map(|(index, (id, label))| {
                    let progress = if still {
                        1.0
                    } else {
                        stagger.progress_at(elapsed, index, 3, spec)
                    };
                    div()
                        .relative()
                        .left(px(24.0 * (1.0 - progress)))
                        .w(px(360.0))
                        .p(px(theme.space(Space::Sm)))
                        .radius(theme, Radius::Control)
                        .well(theme)
                        .opacity(progress)
                        .child(crate::foundation::text(theme, TypeScale::Body, label))
                        .semantic_in(
                            cx,
                            NodeSpec::new(format!("scene.motion.stagger.{id}"), Role::Status)
                                .text(label)
                                .value(format!("replay {}", self.stagger_generation)),
                        )
                }),
            ))
            .child(
                Button::new("scene.motion.stagger.replay")
                    .label("Replay")
                    .secondary()
                    .on_click(|_, cx| {
                        let now = cx.background_executor().now();
                        cx.update_global::<SceneMotionPrimitives, ()>(|scene, _| {
                            scene.stagger_generation += 1;
                            scene.stagger_started_at = Some(now);
                        });
                        cx.refresh_windows();
                    }),
            )
            .into_any_element()
    }
}

/// Five interactive demonstrations of interruption, physical motion,
/// authored timelines, delayed unmount and list choreography.
pub(super) fn motion_primitives(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneMotionPrimitives>() {
        cx.set_global(SceneMotionPrimitives::new(cx.theme()));
    }
    let now = cx.background_executor().now();
    let theme = cx.theme().clone();
    let selected = cx.global::<SceneMotionPrimitives>().demo;
    let tabs = Tabs::new("scene.motion.tabs")
        .tabs([
            TabItem::new("sliding-time", "Sliding time"),
            TabItem::new("spring", "Spring"),
            TabItem::new("keyframes", "Keyframes"),
            TabItem::new("presence", "Presence"),
            TabItem::new("stagger", "Stagger"),
        ])
        .selected(selected.id())
        .on_select(|id, _, cx| {
            let Some(demo) = MotionDemo::from_id(id.as_ref()) else {
                return;
            };
            let theme = cx.theme().clone();
            cx.update_global::<SceneMotionPrimitives, ()>(|scene, _| scene.select(demo, &theme));
            cx.refresh_windows();
        });

    let body = cx.update_global::<SceneMotionPrimitives, AnyElement>(|scene, cx| {
        scene.update_clock(now);
        if scene.clock_playing {
            window.request_animation_frame();
        }
        match scene.demo {
            MotionDemo::SlidingTime => scene.sliding_time(&theme, window, cx),
            MotionDemo::Spring => scene.spring(&theme, window, cx),
            MotionDemo::Keyframes => scene.keyframes(&theme, window, cx),
            MotionDemo::Presence => scene.presence(&theme, window, cx),
            MotionDemo::Stagger => scene.stagger(&theme, window, cx),
        }
    });

    stack(&theme)
        .w(px(640.0))
        .child(tabs)
        .child(body)
        .into_any_element()
}

fn clock_spec(theme: &Theme) -> MotionSpec {
    MotionSpec::new(CLOCK_TRANSITION_MS, Easing::EaseInOut.curve(theme))
}

fn spring_spec() -> MotionSpec {
    // `bounce = 1 - damping_ratio`, so this is the same 420ms / 0.68 policy
    // used by the reference segmented indicator.
    MotionSpec::sprung(Spring::perceptual(Duration::from_millis(420), 0.32))
}

fn presence_spec(theme: &Theme) -> MotionSpec {
    MotionSpec::new(360, Easing::EaseInOut.curve(theme))
}

fn activity_keyframes(theme: &Theme) -> Keyframes<f32> {
    let linear = MotionSpec::new(1_200, CubicBezier::new(0.0, 0.0, 1.0, 1.0));
    Keyframes::new(
        theme,
        linear,
        [
            Keyframe::new(0.0, 0.0),
            Keyframe::new(0.35, 1.0),
            // Local keyframes attach the curve to the stop being reached. Put
            // EaseOut on the 0.7 stop so the outgoing 0.35..0.7 segment has
            // the same curve as the reference implementation.
            Keyframe::new(0.7, 0.0).eased(Easing::EaseOut),
            Keyframe::new(1.0, 0.0),
        ],
    )
    .expect("the activity path has four stops")
}

fn demo_panel(theme: &Theme, caption_text: &'static str) -> gpui::Div {
    div()
        .min_h(px(260.0))
        .w_full()
        .column()
        .items_center()
        .justify_center()
        .gap(px(theme.space(Space::Lg)))
        .p(px(theme.space(Space::Xl)))
        .radius(theme, Radius::Card)
        .card_surface(theme, CardVariant::Elevated)
        .child(caption(theme, caption_text))
}

fn rolling_digit(theme: &Theme, value: f32) -> gpui::Div {
    let whole = value.floor();
    let fraction = value - whole;
    let current = (whole as i32).rem_euclid(10);
    let next = (current + 1).rem_euclid(10);
    let glyph = |digit: i32, top: f32| {
        div()
            .absolute()
            .left_0()
            .right_0()
            .top(px(top))
            .h(px(52.0))
            .flex()
            .items_center()
            .justify_center()
            .child(digit.to_string())
    };

    div()
        .relative()
        .w(px(38.0))
        .h(px(52.0))
        .overflow_hidden()
        .radius(theme, Radius::Control)
        .well(theme)
        .font_family(theme.typography.mono.clone())
        .text_size(px(40.0))
        .child(glyph(current, -52.0 * fraction))
        .child(glyph(next, 52.0 * (1.0 - fraction)))
}

fn clock_digits(minutes: u16) -> [u8; 4] {
    let hour = minutes / 60;
    let minute = minutes % 60;
    [
        (hour / 10) as u8,
        (hour % 10) as u8,
        (minute / 10) as u8,
        (minute % 10) as u8,
    ]
}

fn format_time(minutes: u16) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// The next scalar congruent to `digit`, always travelling forward around the
/// decimal wheel. Thus 8 → 0 becomes 8 → 10 rather than rolling backwards.
fn advance_digit(current: f32, digit: u8) -> f32 {
    let current_digit = (current.round() as i32).rem_euclid(10);
    let distance = (i32::from(digit) - current_digit).rem_euclid(10);
    current + distance as f32
}

pub(super) fn micro(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    // Each motion gets a cell of its own. A still frame cannot show any of
    // them moving, so what the picture has to carry instead is that there are
    // five of them, that they are the same size, and which name belongs to
    // which mark.
    let cell = |mark: MicroMark| {
        div()
            .w(px(96.0))
            .flex_none()
            .column()
            .items_center()
            .justify_center()
            .gap_token(&theme, Space::Xs)
            .py_token(&theme, Space::Md)
            .radius(&theme, Radius::Card)
            .well(&theme)
            .child(mark)
    };

    stack(&theme)
        .w(px(560.0))
        .child(caption(
            &theme,
            "named functions of time; reduced motion leaves the glyph still",
        ))
        .child(
            row(&theme)
                .gap_token(&theme, Space::Sm)
                .child(cell(MicroMark::new(
                    "scene.micro.heartbeat",
                    Micro::Heartbeat,
                    "Hb",
                )))
                .child(cell(MicroMark::new(
                    "scene.micro.bounce",
                    Micro::Bounce,
                    "Bn",
                )))
                .child(cell(MicroMark::new(
                    "scene.micro.wobble",
                    Micro::Wobble,
                    "Wb",
                )))
                .child(cell(MicroMark::new("scene.micro.pop", Micro::Pop, "Pp")))
                .child(cell(MicroMark::new(
                    "scene.micro.sparkle",
                    Micro::Sparkle,
                    "Sk",
                ))),
        )
        .child(caption(
            &theme,
            "the same functions on things that are not glyphs",
        ))
        .child(
            row(&theme)
                .gap_token(&theme, Space::Sm)
                .child(div().child(Badge::new("12 unread").neutral()).micro(
                    "scene.micro.badge",
                    Micro::Pop,
                    cx,
                ))
                .child(div().child(Tag::new("scene.micro.tag", "fixture")).micro(
                    "scene.micro.tag.mark",
                    Micro::Wobble,
                    cx,
                ))
                .child(div().child(StatusDot::new(Tone::Success)).micro(
                    "scene.micro.dot",
                    Micro::Heartbeat,
                    cx,
                )),
        )
        .child(caption(
            &theme,
            "a described motion, sampled along its own run rather than played",
        ))
        .child(described(&theme))
        .into_any_element()
}

/// A motion written with `motion!`, laid out as the frames it passes through.
///
/// This is the one thing a still frame can say about motion: a described
/// motion can be read at a point in its run without running it, so the frames
/// are drawn side by side and the picture is the same every time. Playing it
/// is [`Animator`]'s job and needs a window; the gallery is where that is
/// looked at.
fn described(theme: &Theme) -> gpui::Div {
    let arrive = crate::motion! {
        duration: 420;
        ease: overshoot;
        opacity: 0.0 => 1.0;
        y: 12.0 => 0.0;
    };

    let frame = |head: f32, label: &'static str| {
        let sample = arrive.sample(theme, head);
        div()
            .w(px(96.0))
            .flex_none()
            .column()
            .items_center()
            .gap_token(theme, Space::Xs)
            .py_token(theme, Space::Md)
            .radius(theme, Radius::Card)
            .well(theme)
            // The slot stays drawn at every head. At the start of the run the
            // mark has not arrived yet, and a cell with nothing in it reads
            // as a frame that failed to render rather than as the frame where
            // the motion has not begun.
            .child(
                div()
                    .w(px(56.0))
                    .h(px(24.0))
                    .radius(theme, Radius::Control)
                    .bg(theme.colors.track)
                    .child(
                        sample.apply(
                            div()
                                .size_full()
                                .radius(theme, Radius::Control)
                                .bg(theme.colors.accent),
                        ),
                    ),
            )
            .child(caption(theme, label))
    };

    row(theme)
        .gap_token(theme, Space::Sm)
        .child(frame(0.0, "start"))
        .child(frame(0.25, "quarter"))
        .child(frame(0.5, "half"))
        .child(frame(0.75, "three quarters"))
        .child(frame(1.0, "settled"))
}
