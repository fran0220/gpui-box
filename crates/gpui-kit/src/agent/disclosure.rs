/// How agent evidence is presented when a disclosure is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentDisclosurePresentation {
    /// Evidence sits on a distinct inset body surface.
    #[default]
    Inset,
    /// Evidence remains inline in the surrounding information flow.
    Flow,
}
