use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
};

use anyhow::{Context as _, Result};
use gpui::{
    A11yCallbacks,
    accesskit::{
        Action, ActionData, ActionRequest, Invalid, Live, Node, NodeId, Orientation, Role,
        TextSelection, Toggled, TreeId, TreeUpdate,
    },
};
use wasm_bindgen::{JsCast as _, JsValue};

use crate::events::EventListenerHandle;

struct MirrorNode {
    element: web_sys::HtmlElement,
    _listeners: Vec<EventListenerHandle>,
    tag: &'static str,
    actions: Rc<Cell<[bool; 5]>>,
}

pub(crate) struct AccessibilityMirror {
    browser_window: web_sys::Window,
    canvas: web_sys::HtmlCanvasElement,
    container: web_sys::HtmlElement,
    action: Rc<Box<dyn Fn(ActionRequest) + Send>>,
    deactivation: Box<dyn Fn() + Send>,
    nodes: HashMap<NodeId, MirrorNode>,
    models: HashMap<NodeId, Node>,
    root: Option<NodeId>,
    tree_id: TreeId,
    syncing_focus: Rc<Cell<bool>>,
    selections: Rc<RefCell<HashMap<NodeId, TextSelection>>>,
}

impl AccessibilityMirror {
    pub(crate) fn new(
        browser_window: web_sys::Window,
        canvas: web_sys::HtmlCanvasElement,
        callbacks: A11yCallbacks,
    ) -> Result<Self> {
        let document = browser_window
            .document()
            .context("window has no document")?;
        let container: web_sys::HtmlElement = document
            .create_element("div")
            .map_err(dom_error("create accessibility container"))?
            .dyn_into()
            .map_err(|error| anyhow::anyhow!("accessibility container is not HTML: {error:?}"))?;
        container
            .set_attribute("data-gpui-accessibility", "")
            .map_err(dom_error("mark accessibility container"))?;
        set_styles(
            &container,
            &[
                ("position", "fixed"),
                ("overflow", "hidden"),
                ("pointer-events", "none"),
                ("opacity", "0.001"),
                ("z-index", "2147483647"),
            ],
        )?;
        document
            .body()
            .context("document has no body")?
            .append_child(&container)
            .map_err(dom_error("append accessibility container"))?;

        let A11yCallbacks {
            activation,
            action,
            deactivation,
        } = callbacks;
        let mut mirror = Self {
            browser_window,
            canvas,
            container,
            action: Rc::new(action),
            deactivation,
            nodes: HashMap::new(),
            models: HashMap::new(),
            root: None,
            tree_id: TreeId::ROOT,
            syncing_focus: Rc::new(Cell::new(false)),
            selections: Rc::new(RefCell::new(HashMap::new())),
        };
        mirror.align_to_canvas(None)?;
        if let Some(update) = activation() {
            mirror.update(update, None)?;
        }
        Ok(mirror)
    }

    pub(crate) fn align_to_canvas(&self, backing_size: Option<(u32, u32)>) -> Result<()> {
        let rect = self.canvas.get_bounding_client_rect();
        let scale = canvas_scale(
            &rect,
            backing_size,
            self.browser_window.device_pixel_ratio(),
        );
        set_styles(
            &self.container,
            &[
                ("left", &format!("{}px", rect.left())),
                ("top", &format!("{}px", rect.top())),
                ("width", &format!("{}px", rect.width())),
                ("height", &format!("{}px", rect.height())),
            ],
        )?;
        for (id, model) in &self.models {
            if let Some(node) = self.nodes.get(id) {
                update_bounds(&node.element, model, scale, &rect)?;
            }
        }
        Ok(())
    }

    pub(crate) fn update(
        &mut self,
        update: TreeUpdate,
        backing_size: Option<(u32, u32)>,
    ) -> Result<()> {
        let mirror_had_focus = self
            .browser_window
            .document()
            .and_then(|document| document.active_element())
            .is_some_and(|active| {
                let active: &web_sys::Node = active.as_ref();
                let container: &web_sys::Node = self.container.as_ref();
                container.contains(Some(active))
            });
        self.align_to_canvas(backing_size)?;
        self.tree_id = update.tree_id;
        if let Some(tree) = update.tree {
            self.root = Some(tree.root);
        }
        for (id, node) in update.nodes {
            self.models.insert(id, node);
        }

        let reachable = self.reachable_nodes();
        self.models.retain(|id, _| reachable.contains(id));
        self.selections
            .borrow_mut()
            .retain(|id, _| reachable.contains(id));
        for stale in self
            .nodes
            .keys()
            .copied()
            .filter(|id| !reachable.contains(id))
            .collect::<Vec<_>>()
        {
            if let Some(node) = self.nodes.remove(&stale) {
                node.element.remove();
            }
        }

        let canvas_rect = self.canvas.get_bounding_client_rect();
        let scale = canvas_scale(
            &canvas_rect,
            backing_size,
            self.browser_window.device_pixel_ratio(),
        );
        for id in &reachable {
            let Some(model) = self.models.get(id).cloned() else {
                continue;
            };
            let tag = element_tag(model.role());
            let actions = action_signature(&model);
            let replace = self.nodes.get(id).is_some_and(|node| node.tag != tag);
            if replace {
                if let Some(node) = self.nodes.remove(id) {
                    node.element.remove();
                }
            }
            if !self.nodes.contains_key(id) {
                let node = self.create_node(*id, &model, tag)?;
                self.nodes.insert(*id, node);
            }
            let mirror_node = self
                .nodes
                .get(id)
                .context("new mirror node was not retained")?;
            mirror_node.actions.set(actions);
            update_element(&mirror_node.element, &model, scale, &canvas_rect)?;
            if let Some(selection) = model.text_selection() {
                self.selections.borrow_mut().insert(*id, *selection);
                apply_selection(&mirror_node.element, selection);
            } else {
                self.selections.borrow_mut().remove(id);
            }
        }

        self.sync_parentage()?;
        if mirror_had_focus && let Some(node) = self.nodes.get(&update.focus) {
            let document = self.browser_window.document();
            let already_focused = document
                .and_then(|document| document.active_element())
                .is_some_and(|active| {
                    active == node.element.clone().unchecked_into::<web_sys::Element>()
                });
            if !already_focused {
                self.syncing_focus.set(true);
                if let Err(error) = node.element.focus() {
                    log::warn!("Failed to focus accessibility node: {error:?}");
                }
                self.syncing_focus.set(false);
            }
        }
        Ok(())
    }

    fn reachable_nodes(&self) -> HashSet<NodeId> {
        let mut reachable = HashSet::new();
        let Some(root) = self.root else {
            return reachable;
        };
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            if !reachable.insert(id) {
                continue;
            }
            if let Some(node) = self.models.get(&id) {
                pending.extend(node.children().iter().copied());
            }
        }
        reachable
    }

    fn create_node(&self, id: NodeId, model: &Node, tag: &'static str) -> Result<MirrorNode> {
        let document = self
            .browser_window
            .document()
            .context("window has no document")?;
        let dom_tag = if tag.starts_with("input-") {
            "input"
        } else if tag == "text-run" {
            "div"
        } else {
            tag
        };
        let element: web_sys::HtmlElement = document
            .create_element(dom_tag)
            .map_err(dom_error("create accessibility node"))?
            .dyn_into()
            .map_err(|error| anyhow::anyhow!("accessibility node is not HTML: {error:?}"))?;
        element
            .set_attribute("data-accesskit-node", &u64::from(id).to_string())
            .map_err(dom_error("set accessibility node id"))?;
        if let Some(input_type) = tag.strip_prefix("input-") {
            element
                .set_attribute("type", input_type)
                .map_err(dom_error("set accessibility input type"))?;
        }
        if tag == "button" {
            element
                .set_attribute("type", "button")
                .map_err(dom_error("set accessibility button type"))?;
        }
        set_styles(
            &element,
            &[
                ("position", "fixed"),
                ("pointer-events", "none"),
                ("box-sizing", "border-box"),
                ("border", "0"),
                ("margin", "0"),
                ("min-width", "0"),
                ("min-height", "0"),
                ("padding", "0"),
            ],
        )?;
        let action = self.action.clone();
        let tree_id = self.tree_id;
        let syncing_focus = self.syncing_focus.clone();
        let selections = self.selections.clone();
        let actions = Rc::new(Cell::new(action_signature(model)));
        let mut listeners = Vec::new();
        if tag == "form" {
            listeners.push(EventListenerHandle::add(
                element.as_ref(),
                "submit",
                move |event| {
                    if let Some(event) = event.dyn_ref::<web_sys::Event>() {
                        event.prevent_default();
                    }
                },
            ));
        }
        listeners.push(action_listener(
            &element,
            "click",
            id,
            tree_id,
            action.clone(),
            actions.clone(),
            0,
            Action::Click,
        ));
        {
            let action = action.clone();
            let syncing_focus = syncing_focus.clone();
            let actions = actions.clone();
            listeners.push(EventListenerHandle::add(
                element.as_ref(),
                "focus",
                move |_| {
                    if actions.get()[1] && !syncing_focus.get() {
                        send_action(action.clone(), tree_id, id, Action::Focus, None);
                    }
                },
            ));
        }
        {
            let action = action.clone();
            let syncing_focus = syncing_focus.clone();
            let actions = actions.clone();
            listeners.push(EventListenerHandle::add(
                element.as_ref(),
                "blur",
                move |_| {
                    if actions.get()[2] && !syncing_focus.get() {
                        send_action(action.clone(), tree_id, id, Action::Blur, None);
                    }
                },
            ));
        }
        {
            let action = action.clone();
            let actions = actions.clone();
            listeners.push(EventListenerHandle::add(
                element.as_ref(),
                "input",
                move |event| {
                    if !actions.get()[3] {
                        return;
                    }
                    let value = event_value(&event);
                    let data = if matches!(model_role_for_value(tag), ValueKind::Numeric) {
                        value.parse().ok().map(ActionData::NumericValue)
                    } else {
                        Some(ActionData::Value(value.into_boxed_str()))
                    };
                    if let Some(data) = data {
                        send_action(action.clone(), tree_id, id, Action::SetValue, Some(data));
                    }
                },
            ));
        }
        {
            let action = action.clone();
            let actions = actions.clone();
            listeners.push(EventListenerHandle::add(
                element.as_ref(),
                "select",
                move |event| {
                    if !actions.get()[4] {
                        return;
                    }
                    let Some(mut selection) = selections.borrow().get(&id).copied() else {
                        return;
                    };
                    if let Some((start, end)) = event_selection(&event) {
                        if selection.anchor.character_index == start as usize
                            && selection.focus.character_index == end as usize
                        {
                            return;
                        }
                        selection.anchor.character_index = start as usize;
                        selection.focus.character_index = end as usize;
                        selections.borrow_mut().insert(id, selection);
                        send_action(
                            action.clone(),
                            tree_id,
                            id,
                            Action::SetTextSelection,
                            Some(ActionData::SetTextSelection(selection)),
                        );
                    }
                },
            ));
        }
        Ok(MirrorNode {
            element,
            _listeners: listeners,
            tag,
            actions,
        })
    }

    fn sync_parentage(&self) -> Result<()> {
        let Some(root) = self.root else { return Ok(()) };
        if let Some(root_node) = self.nodes.get(&root) {
            if !root_node.element.has_attribute("data-gpui-a11y-excluded") {
                let root_dom_node: &web_sys::Node = root_node.element.as_ref();
                let container_dom_node: &web_sys::Node = self.container.as_ref();
                if !root_dom_node
                    .parent_node()
                    .is_some_and(|parent| parent.is_same_node(Some(container_dom_node)))
                {
                    self.container
                        .append_child(&root_node.element)
                        .map_err(dom_error("append accessibility root"))?;
                }
            } else {
                root_node.element.remove();
            }
        }
        for (parent_id, model) in &self.models {
            let Some(parent) = self.nodes.get(parent_id) else {
                continue;
            };
            let parent_dom_node: &web_sys::Node = parent.element.as_ref();
            let mut previous: Option<web_sys::Node> = None;
            for child_id in model.children() {
                if let Some(child) = self.nodes.get(child_id) {
                    if child.element.has_attribute("data-gpui-a11y-excluded") {
                        child.element.remove();
                    } else {
                        let child_dom_node: &web_sys::Node = child.element.as_ref();
                        let expected = if let Some(previous) = &previous {
                            previous.next_sibling()
                        } else {
                            parent_dom_node.first_child()
                        };
                        if !expected
                            .as_ref()
                            .is_some_and(|node| node.is_same_node(Some(child_dom_node)))
                        {
                            parent_dom_node
                                .insert_before(child_dom_node, expected.as_ref())
                                .map_err(dom_error("order accessibility child"))?;
                        }
                        previous = Some(child_dom_node.clone());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Drop for AccessibilityMirror {
    fn drop(&mut self) {
        (self.deactivation)();
        self.container.remove();
    }
}

fn action_listener(
    element: &web_sys::HtmlElement,
    event_name: &'static str,
    id: NodeId,
    tree_id: TreeId,
    action_callback: Rc<Box<dyn Fn(ActionRequest) + Send>>,
    actions: Rc<Cell<[bool; 5]>>,
    action_index: usize,
    action: Action,
) -> EventListenerHandle {
    EventListenerHandle::add(element.as_ref(), event_name, move |event| {
        if actions.get()[action_index] {
            if let Some(event) = event.dyn_ref::<web_sys::Event>() {
                event.stop_propagation();
            }
            send_action(action_callback.clone(), tree_id, id, action, None);
        }
    })
}

fn send_action(
    callback: Rc<Box<dyn Fn(ActionRequest) + Send>>,
    tree_id: TreeId,
    id: NodeId,
    action: Action,
    data: Option<ActionData>,
) {
    wasm_bindgen_futures::spawn_local(async move {
        callback(ActionRequest {
            action,
            target_tree: tree_id,
            target_node: id,
            data,
        });
    });
}

fn action_signature(node: &Node) -> [bool; 5] {
    [
        node.supports_action(Action::Click),
        node.supports_action(Action::Focus),
        node.supports_action(Action::Blur),
        node.supports_action(Action::SetValue),
        node.supports_action(Action::SetTextSelection),
    ]
}

fn update_element(
    element: &web_sys::HtmlElement,
    node: &Node,
    scale: (f64, f64),
    canvas_rect: &web_sys::DomRect,
) -> Result<()> {
    let role = aria_role(node.role());
    set_optional_attribute(element, "role", role)?;
    set_optional_attribute(element, "aria-label", node.label())?;
    set_optional_attribute(element, "aria-description", node.description())?;
    set_optional_attribute(element, "aria-valuetext", node.value())?;
    if node.role() == Role::TextRun {
        element.set_text_content(node.value());
    }
    set_optional_attribute(element, "placeholder", node.placeholder())?;
    set_bool_attribute(element, "aria-disabled", node.is_disabled())?;
    set_optional_attribute(element, "disabled", node.is_disabled().then_some(""))?;
    set_bool_attribute(element, "aria-required", node.is_required())?;
    set_bool_attribute(element, "aria-readonly", node.is_read_only())?;
    set_bool_attribute(element, "aria-modal", node.is_modal())?;
    set_bool_attribute(element, "aria-busy", node.is_busy())?;
    set_bool_attribute(element, "aria-atomic", node.is_live_atomic())?;
    set_optional_bool_attribute(element, "aria-selected", node.is_selected())?;
    set_optional_bool_attribute(element, "aria-expanded", node.is_expanded())?;
    set_optional_attribute(
        element,
        "aria-invalid",
        node.invalid().map(|invalid| match invalid {
            Invalid::True => "true",
            Invalid::Grammar => "grammar",
            Invalid::Spelling => "spelling",
        }),
    )?;
    set_optional_attribute(
        element,
        "aria-live",
        node.live().map(|live| match live {
            Live::Off => "off",
            Live::Polite => "polite",
            Live::Assertive => "assertive",
        }),
    )?;
    set_optional_attribute(
        element,
        "aria-orientation",
        node.orientation().map(|orientation| match orientation {
            Orientation::Horizontal => "horizontal",
            Orientation::Vertical => "vertical",
        }),
    )?;
    if let Some(toggled) = node.toggled() {
        let attribute = if matches!(node.role(), Role::Button | Role::DefaultButton) {
            "aria-pressed"
        } else {
            "aria-checked"
        };
        set_optional_attribute(element, attribute, Some(toggle_value(toggled)))?;
    } else {
        element
            .remove_attribute("aria-checked")
            .map_err(dom_error("clear aria-checked"))?;
        element
            .remove_attribute("aria-pressed")
            .map_err(dom_error("clear aria-pressed"))?;
    }
    for (attribute, value) in [
        ("aria-valuenow", node.numeric_value()),
        ("aria-valuemin", node.min_numeric_value()),
        ("aria-valuemax", node.max_numeric_value()),
        ("step", node.numeric_value_step()),
    ] {
        set_optional_attribute(
            element,
            attribute,
            value.map(|value| value.to_string()).as_deref(),
        )?;
    }
    for (attribute, value) in [
        ("aria-level", node.level()),
        ("aria-posinset", node.position_in_set()),
        ("aria-setsize", node.size_of_set()),
        ("aria-rowindex", node.row_index().map(|index| index + 1)),
        ("aria-colindex", node.column_index().map(|index| index + 1)),
        ("aria-rowspan", node.row_span()),
        ("aria-colspan", node.column_span()),
    ] {
        set_optional_attribute(
            element,
            attribute,
            value.map(|value| value.to_string()).as_deref(),
        )?;
    }
    if is_value_control(node.role()) {
        let value = if matches!(
            model_role_for_value(element_tag(node.role())),
            ValueKind::Numeric
        ) {
            node.numeric_value()
                .map(|value| value.to_string())
                .unwrap_or_default()
        } else {
            node.value().unwrap_or_default().to_owned()
        };
        if let Err(error) = js_sys::Reflect::set(
            element,
            &JsValue::from_str("value"),
            &JsValue::from_str(&value),
        ) {
            log::warn!("Failed to set accessibility control value: {error:?}");
        }
    }
    element.set_tab_index(if node.supports_action(Action::Focus) {
        0
    } else {
        -1
    });

    update_bounds(element, node, scale, canvas_rect)
}

fn canvas_scale(
    canvas_rect: &web_sys::DomRect,
    backing_size: Option<(u32, u32)>,
    dpr: f64,
) -> (f64, f64) {
    let fallback = if dpr.is_finite() && dpr > 0.0 {
        1.0 / dpr
    } else {
        1.0
    };
    let (width, height) = backing_size.unwrap_or_default();
    (
        if width > 0 {
            canvas_rect.width() / f64::from(width)
        } else {
            fallback
        },
        if height > 0 {
            canvas_rect.height() / f64::from(height)
        } else {
            fallback
        },
    )
}

fn update_bounds(
    element: &web_sys::HtmlElement,
    node: &Node,
    scale: (f64, f64),
    canvas_rect: &web_sys::DomRect,
) -> Result<()> {
    let Some(bounds) = node.bounds() else {
        let is_root = matches!(node.role(), Role::Window | Role::RootWebArea);
        set_optional_attribute(
            element,
            "data-gpui-a11y-excluded",
            node.is_hidden().then_some("true"),
        )?;
        set_optional_attribute(element, "data-gpui-a11y-offscreen", None)?;
        return set_styles(
            element,
            &[
                ("left", &format!("{}px", canvas_rect.left())),
                ("top", &format!("{}px", canvas_rect.top())),
                (
                    "width",
                    &format!("{}px", if is_root { canvas_rect.width() } else { 0.0 }),
                ),
                (
                    "height",
                    &format!("{}px", if is_root { canvas_rect.height() } else { 0.0 }),
                ),
                ("overflow", "visible"),
            ],
        );
    };
    let relative_left = bounds.x0 * scale.0;
    let relative_top = bounds.y0 * scale.1;
    let left = canvas_rect.left() + relative_left;
    let top = canvas_rect.top() + relative_top;
    let node_width = (bounds.x1 - bounds.x0) * scale.0;
    let node_height = (bounds.y1 - bounds.y0) * scale.1;
    let offscreen = node_width <= 0.0
        || node_height <= 0.0
        || relative_left + node_width <= 0.0
        || relative_top + node_height <= 0.0
        || relative_left >= canvas_rect.width()
        || relative_top >= canvas_rect.height();
    set_optional_attribute(
        element,
        "data-gpui-a11y-excluded",
        node.is_hidden().then_some("true"),
    )?;
    set_optional_attribute(
        element,
        "data-gpui-a11y-offscreen",
        offscreen.then_some("true"),
    )?;
    set_styles(
        element,
        &[
            ("left", &format!("{left}px")),
            ("top", &format!("{top}px")),
            ("width", &format!("{}px", node_width.max(0.0))),
            ("height", &format!("{}px", node_height.max(0.0))),
            ("overflow", "visible"),
        ],
    )
}

fn set_styles(element: &web_sys::HtmlElement, properties: &[(&str, &str)]) -> Result<()> {
    for (property, value) in properties {
        element
            .style()
            .set_property(property, value)
            .map_err(dom_error("set accessibility style"))?;
    }
    Ok(())
}

fn set_bool_attribute(element: &web_sys::HtmlElement, name: &str, value: bool) -> Result<()> {
    set_optional_attribute(element, name, value.then_some("true"))
}

fn set_optional_bool_attribute(
    element: &web_sys::HtmlElement,
    name: &str,
    value: Option<bool>,
) -> Result<()> {
    set_optional_attribute(
        element,
        name,
        value.map(|value| if value { "true" } else { "false" }),
    )
}

fn set_optional_attribute(
    element: &web_sys::HtmlElement,
    name: &str,
    value: Option<&str>,
) -> Result<()> {
    if let Some(value) = value {
        element
            .set_attribute(name, value)
            .map_err(dom_error("set accessibility attribute"))?;
    } else {
        element
            .remove_attribute(name)
            .map_err(dom_error("clear accessibility attribute"))?;
    }
    Ok(())
}

fn dom_error(operation: &'static str) -> impl FnOnce(JsValue) -> anyhow::Error {
    move |error| anyhow::anyhow!("Failed to {operation}: {error:?}")
}

fn toggle_value(toggled: Toggled) -> &'static str {
    match toggled {
        Toggled::False => "false",
        Toggled::True => "true",
        Toggled::Mixed => "mixed",
    }
}

#[derive(Copy, Clone)]
enum ValueKind {
    Text,
    Numeric,
}

fn model_role_for_value(tag: &str) -> ValueKind {
    if tag == "input-range" || tag == "input-number" {
        ValueKind::Numeric
    } else {
        ValueKind::Text
    }
}

fn is_value_control(role: Role) -> bool {
    matches!(
        role,
        Role::TextInput
            | Role::MultilineTextInput
            | Role::SearchInput
            | Role::DateInput
            | Role::DateTimeInput
            | Role::WeekInput
            | Role::MonthInput
            | Role::TimeInput
            | Role::EmailInput
            | Role::NumberInput
            | Role::PasswordInput
            | Role::PhoneNumberInput
            | Role::UrlInput
            | Role::EditableComboBox
            | Role::Slider
            | Role::SpinButton
    )
}

fn element_tag(role: Role) -> &'static str {
    match role {
        Role::TextRun => "text-run",
        Role::Button | Role::DefaultButton => "button",
        Role::MultilineTextInput => "textarea",
        Role::ComboBox | Role::ListBox => "select",
        Role::Slider => "input-range",
        Role::SpinButton | Role::NumberInput => "input-number",
        Role::TextInput
        | Role::SearchInput
        | Role::DateInput
        | Role::DateTimeInput
        | Role::WeekInput
        | Role::MonthInput
        | Role::TimeInput
        | Role::EmailInput
        | Role::PasswordInput
        | Role::PhoneNumberInput
        | Role::UrlInput
        | Role::EditableComboBox => "input",
        Role::Link => "a",
        Role::Image => "img",
        Role::Heading => "h2",
        Role::List => "ul",
        Role::ListItem => "li",
        Role::Table | Role::Grid => "table",
        Role::Row => "tr",
        Role::Cell | Role::GridCell => "td",
        Role::RowHeader | Role::ColumnHeader => "th",
        Role::Form => "form",
        Role::Main => "main",
        Role::Navigation => "nav",
        _ => "div",
    }
}

fn aria_role(role: Role) -> Option<&'static str> {
    Some(match role {
        Role::Window | Role::RootWebArea | Role::Application => "application",
        Role::Button | Role::DefaultButton => "button",
        Role::CheckBox => "checkbox",
        Role::RadioButton => "radio",
        Role::Switch => "switch",
        Role::TextInput
        | Role::MultilineTextInput
        | Role::EmailInput
        | Role::PasswordInput
        | Role::PhoneNumberInput
        | Role::UrlInput
        | Role::DateInput
        | Role::DateTimeInput
        | Role::WeekInput
        | Role::MonthInput
        | Role::TimeInput
        | Role::NumberInput => "textbox",
        Role::SearchInput => "searchbox",
        Role::Dialog => "dialog",
        Role::AlertDialog => "alertdialog",
        Role::Menu => "menu",
        Role::MenuBar => "menubar",
        Role::MenuItem => "menuitem",
        Role::MenuItemCheckBox => "menuitemcheckbox",
        Role::MenuItemRadio => "menuitemradio",
        Role::List => "list",
        Role::ListItem => "listitem",
        Role::ListBox => "listbox",
        Role::ListBoxOption => "option",
        Role::ComboBox | Role::EditableComboBox => "combobox",
        Role::Slider => "slider",
        Role::SpinButton => "spinbutton",
        Role::ProgressIndicator => "progressbar",
        Role::Meter => "meter",
        Role::Link => "link",
        Role::Image => "img",
        Role::Heading => "heading",
        Role::Table => "table",
        Role::Grid => "grid",
        Role::Row => "row",
        Role::Cell => "cell",
        Role::GridCell => "gridcell",
        Role::RowHeader => "rowheader",
        Role::ColumnHeader => "columnheader",
        Role::Tab => "tab",
        Role::TabList => "tablist",
        Role::TabPanel => "tabpanel",
        Role::Tree => "tree",
        Role::TreeItem => "treeitem",
        Role::TreeGrid => "treegrid",
        Role::Toolbar => "toolbar",
        Role::Status => "status",
        Role::Alert => "alert",
        Role::Navigation => "navigation",
        Role::Main => "main",
        Role::Form => "form",
        Role::Region => "region",
        Role::Group => "group",
        Role::GenericContainer | Role::TextRun | Role::Label | Role::Paragraph | Role::Unknown => {
            return None;
        }
        _ => "group",
    })
}

fn event_value(event: &JsValue) -> String {
    event
        .dyn_ref::<web_sys::Event>()
        .and_then(|event| event.target())
        .and_then(|target| js_sys::Reflect::get(&target, &JsValue::from_str("value")).ok())
        .and_then(|value| value.as_string())
        .unwrap_or_default()
}

fn event_selection(event: &JsValue) -> Option<(u32, u32)> {
    let target = event.dyn_ref::<web_sys::Event>()?.target()?;
    element_selection(&target)
}

fn element_selection(element: &JsValue) -> Option<(u32, u32)> {
    let start = js_sys::Reflect::get(element, &JsValue::from_str("selectionStart"))
        .ok()?
        .as_f64()? as u32;
    let end = js_sys::Reflect::get(element, &JsValue::from_str("selectionEnd"))
        .ok()?
        .as_f64()? as u32;
    Some((start, end))
}

fn apply_selection(element: &web_sys::HtmlElement, selection: &TextSelection) {
    let desired = (
        selection.anchor.character_index as u32,
        selection.focus.character_index as u32,
    );
    if element_selection(element.as_ref()) == Some(desired) {
        return;
    }
    let start = JsValue::from_f64(selection.anchor.character_index as f64);
    let end = JsValue::from_f64(selection.focus.character_index as f64);
    if let Ok(function) = js_sys::Reflect::get(element, &JsValue::from_str("setSelectionRange"))
        .and_then(|value| value.dyn_into::<js_sys::Function>())
    {
        if let Err(error) = function.call2(element, &start, &end) {
            log::warn!("Failed to synchronize accessibility text selection: {error:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_roles_have_explicit_aria_mappings() {
        assert_eq!(aria_role(Role::Button), Some("button"));
        assert_eq!(aria_role(Role::MenuItemCheckBox), Some("menuitemcheckbox"));
        assert_eq!(aria_role(Role::TreeGrid), Some("treegrid"));
        assert_eq!(aria_role(Role::TextRun), None);
        assert_ne!(element_tag(Role::TextRun), element_tag(Role::Paragraph));
    }
}
