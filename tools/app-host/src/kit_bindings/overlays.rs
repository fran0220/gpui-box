//! Retained native surfaces. The slot store holds host factories, never
//! consumed elements; each body render clones its factory before calling it.
use super::*;
use gpui::div;

fn body(store: &Rc<RefCell<KitSlots>>, window: &mut Window, cx: &mut App) -> AnyElement {
    let factory = store.borrow().get("content").cloned();
    factory.map_or_else(|| div().into_any_element(), |factory| factory(window, cx))
}

pub(super) fn render(
    state: &KitState,
    node: &Node,
    slots: KitSlots,
    window: &mut Window,
    cx: &mut App,
    emit: Emit,
) -> AnyElement {
    let entry = state
        .retained
        .borrow_mut()
        .entry((node.instance, node.id.clone()))
        .or_insert_with(|| {
            let route = Rc::new(RefCell::new(Route {
                events: BTreeMap::new(),
                emit: emit.clone(),
                disabled: false,
            }));
            let store = Rc::new(RefCell::new(KitSlots::new()));
            let content = store.clone();
            let callback = route.clone();
            let (control, subscription) = if node.component.as_deref() == Some("Popover") {
                let entity = cx.new(|cx| {
                    Popover::new(node.id.clone(), window, cx)
                        .content(move |window, cx| body(&content, window, cx))
                });
                let subscription = cx.subscribe(&entity, move |_, event: &PopoverEvent, _| {
                    callback.borrow().send(
                        match event {
                            PopoverEvent::Opened => "open",
                            PopoverEvent::Closed => "close",
                            PopoverEvent::Dismissed => "dismiss",
                        },
                        Value::Null,
                    );
                });
                (Control::Popover(entity, store), subscription)
            } else {
                let entity = cx.new(|cx| {
                    Dialog::new(node.id.clone(), window, cx)
                        .content(move |window, cx| body(&content, window, cx))
                });
                let subscription = cx.subscribe(&entity, move |_, event: &DialogEvent, _| {
                    callback.borrow().send(
                        match event {
                            DialogEvent::Opened => "open",
                            DialogEvent::Closed => "close",
                            DialogEvent::Dismissed => "dismiss",
                            DialogEvent::Confirmed => "confirm",
                            DialogEvent::Cancelled => "cancel",
                        },
                        Value::Null,
                    );
                });
                (Control::Dialog(entity, store), subscription)
            };
            Rc::new(Retained {
                control,
                route,
                props: Default::default(),
                slot_data: Default::default(),
                _subscriptions: vec![subscription],
            })
        })
        .clone();
    // Never hold the map or slot store borrow while a nested factory runs.
    let changed = *entry.props.borrow() != node.props;
    let slots_changed = *entry.slot_data.borrow() != node.slots;
    *entry.route.borrow_mut() = Route {
        events: node.events.clone(),
        emit,
        disabled: false,
    };
    let optional = |key: &str| {
        node.props
            .get(key)
            .and_then(Value::as_str)
            .map(|text| SharedString::from(text.to_owned()))
    };
    let dismissable = node
        .props
        .get("dismissable")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let result = match &entry.control {
        Control::Popover(entity, store) => {
            *store.borrow_mut() = slots;
            entity.update(cx, |popover, cx| {
                if changed {
                    popover.set_trigger(text(node, "trigger"), cx);
                    popover.set_dismissable(dismissable, cx);
                    popover.set_placement(
                        match text(node, "placement").as_str() {
                            "above" => Placement::Above,
                            _ => Placement::Below,
                        },
                        cx,
                    );
                    popover.set_hang(
                        match text(node, "hang").as_str() {
                            "end" => Hang::End,
                            _ => Hang::Start,
                        },
                        cx,
                    );
                }
                if slots_changed {
                    cx.notify();
                }
            });
            entity.clone().into_any_element()
        }
        Control::Dialog(entity, store) => {
            *store.borrow_mut() = slots;
            entity.update(cx, |dialog, cx| {
                if changed {
                    dialog.set_title(text(node, "title"), cx);
                    dialog.set_description(optional("description"), cx);
                    dialog.set_confirm_label(optional("confirmLabel"), cx);
                    dialog.set_cancel_label(optional("cancelLabel"), cx);
                    dialog.set_dismissable(dismissable, cx);
                    dialog.set_destructive(flag(node, "destructive"), cx);
                }
                if slots_changed {
                    cx.notify();
                }
            });
            entity.clone().into_any_element()
        }
        _ => unreachable!("reconcile replaces changed component identity"),
    };
    *entry.props.borrow_mut() = node.props.clone();
    *entry.slot_data.borrow_mut() = node.slots.clone();
    result
}
