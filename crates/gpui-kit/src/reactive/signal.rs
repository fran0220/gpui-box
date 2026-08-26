//! A value the caller owns, and a two-way pipe into a control.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{App, AppContext, Context, Entity, Subscription};

/// One caller-owned value that anything watching it is told about.
///
/// A signal is an [`Entity`] with a narrower contract: it holds a value, it
/// notifies when the value changes, and it does not render. Components never
/// create one — the caller does, keeps it, and passes a [`Binding`] or the
/// signal itself to whichever control should read and write it. That is what
/// keeps a bound control a reader of caller data rather than the owner of it.
pub struct Signal<T: 'static> {
    entity: Entity<T>,
}

impl<T: 'static> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self {
            entity: self.entity.clone(),
        }
    }
}

impl<T: 'static> PartialEq for Signal<T> {
    fn eq(&self, other: &Self) -> bool {
        self.entity.entity_id() == other.entity.entity_id()
    }
}

impl<T: 'static> Eq for Signal<T> {}

impl<T: 'static> std::fmt::Debug for Signal<T> {
    /// A signal commonly holds what somebody typed, and a credential is one
    /// of the things somebody types, so the identity is printed and the
    /// value never is.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Signal")
            .field("entity", &self.entity.entity_id())
            .finish()
    }
}

impl<T: 'static> Signal<T> {
    pub fn new(cx: &mut App, value: T) -> Self {
        Self {
            entity: cx.new(|_| value),
        }
    }

    /// Adopts an entity the caller already has.
    pub fn from_entity(entity: Entity<T>) -> Self {
        Self { entity }
    }

    /// The entity underneath, for a caller that needs GPUI's own machinery.
    pub fn entity(&self) -> &Entity<T> {
        &self.entity
    }

    pub fn read<'a>(&'a self, cx: &'a App) -> &'a T {
        self.entity.read(cx)
    }

    pub fn get(&self, cx: &App) -> T
    where
        T: Clone,
    {
        self.entity.read(cx).clone()
    }

    /// Changes the value and tells every watcher, once.
    pub fn update(&self, cx: &mut App, change: impl FnOnce(&mut T)) {
        self.entity.update(cx, |value, cx| {
            change(value);
            cx.notify();
        });
    }

    /// Replaces the value and tells every watcher, whether or not it moved.
    pub fn replace(&self, cx: &mut App, value: T) {
        self.update(cx, |slot| *slot = value);
    }

    /// Replaces the value, and says nothing when it did not move.
    ///
    /// This is the whole echo guard: a control that reports a change writes
    /// the value it already has back into the signal, and a signal that
    /// notified for it would push that same value back into the control.
    pub fn set(&self, cx: &mut App, value: T)
    where
        T: PartialEq,
    {
        if self.entity.read(cx) == &value {
            return;
        }
        self.replace(cx, value);
    }

    /// Re-renders the watching view whenever this value changes.
    ///
    /// The returned subscription is the watch: dropping it stops it, so a
    /// view keeps it for as long as it draws the value.
    pub fn watch<V: 'static>(&self, cx: &mut Context<V>) -> Subscription {
        cx.observe(&self.entity, |_, _, cx| cx.notify())
    }
}

impl<T: Clone + PartialEq + 'static> Signal<T> {
    /// The whole value, as something a control can read and write.
    pub fn binding(&self) -> Binding<T> {
        let read = self.clone();
        let write = self.clone();
        Binding::new(
            move |cx: &App| read.get(cx),
            move |value, cx: &mut App| write.set(cx, value),
        )
    }

    /// One field of the value, as something a control can read and write.
    ///
    /// The write path is a read-modify-write of the whole value, so it moves
    /// the projected field and leaves every other field exactly as it was.
    pub fn lens<F>(
        &self,
        get: impl Fn(&T) -> &F + 'static,
        set: impl Fn(&mut T, F) + 'static,
    ) -> Binding<F>
    where
        F: Clone + PartialEq + 'static,
    {
        let get = Rc::new(get);
        let read = self.clone();
        let read_get = get.clone();
        let write = self.clone();
        Binding::new(
            move |cx: &App| read_get(read.read(cx)).clone(),
            move |value: F, cx: &mut App| {
                if get(write.read(cx)) == &value {
                    return;
                }
                write.update(cx, |whole| set(whole, value));
            },
        )
    }
}

type ReadValue<T> = Rc<dyn Fn(&App) -> T>;
type WriteValue<T> = Rc<dyn Fn(T, &mut App)>;

/// A read and a write of one value, without saying where the value lives.
///
/// A binding is a pipe, not storage. It is what a control is handed so it can
/// draw the caller's current value and report a new one back to the same
/// place, and it is deliberately the only thing a control is given: a control
/// that held the [`Signal`] could keep the value, and then the value would be
/// the control's.
pub struct Binding<T: 'static> {
    get: ReadValue<T>,
    set: WriteValue<T>,
    /// Set while this binding's own write is running. A control that reports
    /// the value it was just given would otherwise write it back through the
    /// same pipe it arrived on.
    writing: Rc<Cell<bool>>,
}

impl<T: 'static> Clone for Binding<T> {
    fn clone(&self) -> Self {
        Self {
            get: self.get.clone(),
            set: self.set.clone(),
            writing: self.writing.clone(),
        }
    }
}

impl<T: 'static> std::fmt::Debug for Binding<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Binding")
    }
}

impl<T: 'static> Binding<T> {
    pub fn new(get: impl Fn(&App) -> T + 'static, set: impl Fn(T, &mut App) + 'static) -> Self {
        Self {
            get: Rc::new(get),
            set: Rc::new(set),
            writing: Rc::new(Cell::new(false)),
        }
    }

    pub fn get(&self, cx: &App) -> T {
        (self.get)(cx)
    }

    pub fn set(&self, cx: &mut App, value: T) {
        if self.writing.get() {
            return;
        }
        self.writing.set(true);
        (self.set)(value, cx);
        self.writing.set(false);
    }

    /// The same value in another type, converted in both directions.
    ///
    /// `from` is what the control should see; `into` writes what the control
    /// reported back onto the value the caller owns.
    pub fn map<U: 'static>(
        &self,
        from: impl Fn(&T) -> U + 'static,
        into: impl Fn(&mut T, U) + 'static,
    ) -> Binding<U> {
        let read = self.clone();
        let write = self.clone();
        Binding::new(
            move |cx: &App| from(&read.get(cx)),
            move |value: U, cx: &mut App| {
                let mut whole = write.get(cx);
                into(&mut whole, value);
                write.set(cx, whole);
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::TestAppContext;

    use super::*;

    #[derive(Clone, PartialEq, Debug)]
    struct Profile {
        name: String,
        age: usize,
    }

    #[gpui::test]
    fn an_update_notifies_a_watcher_exactly_once(cx: &mut TestAppContext) {
        let signal = cx.update(|cx| Signal::new(cx, 1usize));
        let seen = Rc::new(Cell::new(0usize));
        let subscription = cx.update(|cx| {
            let seen = seen.clone();
            cx.observe(signal.entity(), move |_, _| seen.set(seen.get() + 1))
        });

        cx.update(|cx| signal.update(cx, |value| *value += 1));
        cx.run_until_parked();
        assert_eq!(seen.get(), 1);
        cx.update(|cx| assert_eq!(signal.get(cx), 2));

        cx.update(|cx| signal.update(cx, |value| *value += 1));
        cx.run_until_parked();
        assert_eq!(seen.get(), 2);
        drop(subscription);
    }

    #[gpui::test]
    fn setting_the_value_it_already_has_notifies_nobody(cx: &mut TestAppContext) {
        let signal = cx.update(|cx| Signal::new(cx, String::from("ada")));
        let seen = Rc::new(Cell::new(0usize));
        let subscription = cx.update(|cx| {
            let seen = seen.clone();
            cx.observe(signal.entity(), move |_, _| seen.set(seen.get() + 1))
        });

        cx.update(|cx| signal.set(cx, String::from("ada")));
        cx.run_until_parked();
        assert_eq!(seen.get(), 0, "an unchanged value is not a change");

        cx.update(|cx| signal.set(cx, String::from("grace")));
        cx.run_until_parked();
        assert_eq!(seen.get(), 1);
        drop(subscription);
    }

    #[gpui::test]
    fn a_lens_writes_only_its_own_field(cx: &mut TestAppContext) {
        let signal = cx.update(|cx| {
            Signal::new(
                cx,
                Profile {
                    name: String::from("ada"),
                    age: 36,
                },
            )
        });
        let name = signal.lens(
            |profile| &profile.name,
            |profile, value| profile.name = value,
        );

        cx.update(|cx| assert_eq!(name.get(cx), "ada"));
        cx.update(|cx| name.set(cx, String::from("grace")));
        cx.update(|cx| {
            assert_eq!(
                signal.get(cx),
                Profile {
                    name: String::from("grace"),
                    age: 36,
                }
            )
        });
    }

    #[gpui::test]
    fn a_map_converts_in_both_directions(cx: &mut TestAppContext) {
        let signal = cx.update(|cx| Signal::new(cx, 3usize));
        let text = signal.binding().map(
            |count| count.to_string(),
            |count, text: String| *count = text.parse().unwrap_or(*count),
        );

        cx.update(|cx| assert_eq!(text.get(cx), "3"));
        cx.update(|cx| text.set(cx, String::from("11")));
        cx.update(|cx| assert_eq!(signal.get(cx), 11));
        cx.update(|cx| assert_eq!(text.get(cx), "11"));

        // A value the conversion cannot read keeps the last one that held.
        cx.update(|cx| text.set(cx, String::from("eleven")));
        cx.update(|cx| assert_eq!(signal.get(cx), 11));
    }

    #[gpui::test]
    fn a_write_does_not_re_enter_the_binding_it_arrived_on(cx: &mut TestAppContext) {
        let signal = cx.update(|cx| Signal::new(cx, 0usize));
        let writes = Rc::new(Cell::new(0usize));
        // A control that reports the value it was just handed, back through
        // the pipe the value arrived on. Without the guard this is unbounded
        // recursion; with it, the second write is not a change anybody made.
        let echo: Rc<std::cell::RefCell<Option<Binding<usize>>>> = Rc::default();
        let binding = Binding::new(
            {
                let read = signal.clone();
                move |cx: &App| read.get(cx)
            },
            {
                let write = signal.clone();
                let writes = writes.clone();
                let echo = echo.clone();
                move |value: usize, cx: &mut App| {
                    writes.set(writes.get() + 1);
                    write.replace(cx, value);
                    let same = echo.borrow().clone();
                    if let Some(same) = same {
                        same.set(cx, value);
                    }
                }
            },
        );
        *echo.borrow_mut() = Some(binding.clone());

        cx.update(|cx| binding.set(cx, 5));
        cx.update(|cx| assert_eq!(signal.get(cx), 5));
        assert_eq!(writes.get(), 1);
        *echo.borrow_mut() = None;
    }
}
