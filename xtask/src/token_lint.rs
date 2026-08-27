use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{
    Attribute, Expr, ExprCall, ExprMethodCall, ForeignItem, ImplItem, Item, ItemFn, Lit, Meta,
    Token, TraitItem, UnOp,
};

const SPACING: &[&str] = &[
    "gap", "gap_x", "gap_y", "p", "px", "py", "pt", "pr", "pb", "pl", "m", "mx", "my", "mt", "mr",
    "mb", "ml",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Diagnostic {
    pub path: PathBuf,
    pub rule: &'static str,
}

pub(crate) fn check(root: &Path) -> Result<()> {
    let mut diagnostics = Vec::new();
    for source_root in [
        root.join("crates/gpui-kit/src"),
        root.join("crates/gpui-kit-theme/src"),
    ] {
        lint_tree(&source_root, &mut diagnostics)?;
    }
    if diagnostics.is_empty() {
        println!("production theme and component token consumption is semantic");
        return Ok(());
    }
    for diagnostic in &diagnostics {
        eprintln!("{}: {}", diagnostic.path.display(), diagnostic.rule);
    }
    bail!(
        "production theme/component token lint found {} violation(s)",
        diagnostics.len()
    )
}

fn lint_tree(path: &Path, diagnostics: &mut Vec<Diagnostic>) -> Result<()> {
    for entry in fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "scenes") {
                continue;
            }
            lint_tree(&path, diagnostics)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let source = fs::read_to_string(&path)
                .with_context(|| format!("read production Rust source {}", path.display()))?;
            diagnostics.extend(lint_source(&path, &source)?);
        }
    }
    Ok(())
}

pub(crate) fn lint_source(path: &Path, source: &str) -> Result<Vec<Diagnostic>> {
    if path.components().any(|part| part.as_os_str() == "scenes") {
        return Ok(Vec::new());
    }
    let file = syn::parse_file(source).with_context(|| format!("parse {}", path.display()))?;
    let mut visitor = TokenVisitor {
        path,
        diagnostics: Vec::new(),
        theme_adapter: path
            .components()
            .any(|part| part.as_os_str() == "gpui-kit-theme"),
    };
    visitor.visit_file(&file);
    Ok(visitor.diagnostics)
}

struct TokenVisitor<'a> {
    path: &'a Path,
    diagnostics: Vec<Diagnostic>,
    theme_adapter: bool,
}

impl TokenVisitor<'_> {
    fn report(&mut self, rule: &'static str) {
        self.diagnostics.push(Diagnostic {
            path: self.path.to_owned(),
            rule,
        });
    }
}

impl<'ast> Visit<'ast> for TokenVisitor<'_> {
    fn visit_item(&mut self, item: &'ast Item) {
        if item_attrs(item).is_some_and(has_cfg_test) {
            return;
        }
        visit::visit_item(self, item);
    }

    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        if has_cfg_test(&function.attrs)
            || function
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("test"))
        {
            return;
        }
        visit::visit_item_fn(self, function);
    }

    fn visit_impl_item(&mut self, item: &'ast ImplItem) {
        if impl_item_attrs(item).is_some_and(has_cfg_test) {
            return;
        }
        visit::visit_impl_item(self, item);
    }

    fn visit_trait_item(&mut self, item: &'ast TraitItem) {
        if trait_item_attrs(item).is_some_and(has_cfg_test) {
            return;
        }
        visit::visit_trait_item(self, item);
    }

    fn visit_foreign_item(&mut self, item: &'ast ForeignItem) {
        if foreign_item_attrs(item).is_some_and(has_cfg_test) {
            return;
        }
        visit::visit_foreign_item(self, item);
    }

    fn visit_expr_method_call(&mut self, call: &'ast ExprMethodCall) {
        let method = call.method.to_string();
        if let Some(argument) = call.args.first() {
            if SPACING.contains(&method.as_str())
                && (px_numeric(argument).is_some_and(|zero| !zero)
                    || px_has_additive_literal(argument))
            {
                self.report("spacing: use a shared spacing token instead of a non-zero px literal");
            } else if (method == "rounded" || method.starts_with("rounded_"))
                && px_numeric(argument).is_some()
            {
                self.report("radius: use a shared radius token instead of a px literal");
            } else if matches!(method.as_str(), "text_size" | "line_height")
                && (px_numeric(argument).is_some() || px_has_scaled_literal(argument))
            {
                self.report("typography: use a shared typography token instead of a px literal");
            } else if method == "font_weight" && font_weight_numeric(argument) {
                self.report(
                    "typography: use a shared typography weight instead of FontWeight(number)",
                );
            }
        }
        if method == "opacity"
            && let Some(value) = call.args.first().and_then(numeric)
        {
            if self.theme_adapter {
                self.report(
                    "theme opacity: source recipe alpha from effect or opacity tokens instead of a literal",
                );
            } else if is_theme_color(&call.receiver) {
                self.report("color opacity: use a shared semantic color recipe instead of theme.colors.<role>.opacity(number)");
            } else if is_style_binding(&call.receiver) {
                self.report(
                    "element opacity: use an opacity or effect token instead of style.opacity(number)",
                );
            } else if value != 0.0 && value != 1.0 {
                self.report(
                    "color opacity: use a token or a named local data-paint constant instead of opacity(number)",
                );
            }
        }
        visit::visit_expr_method_call(self, call);
    }
}

fn item_attrs(item: &Item) -> Option<&[Attribute]> {
    Some(match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        _ => return None,
    })
}

fn impl_item_attrs(item: &ImplItem) -> Option<&[Attribute]> {
    Some(match item {
        ImplItem::Const(item) => &item.attrs,
        ImplItem::Fn(item) => &item.attrs,
        ImplItem::Type(item) => &item.attrs,
        ImplItem::Macro(item) => &item.attrs,
        _ => return None,
    })
}

fn trait_item_attrs(item: &TraitItem) -> Option<&[Attribute]> {
    Some(match item {
        TraitItem::Const(item) => &item.attrs,
        TraitItem::Fn(item) => &item.attrs,
        TraitItem::Type(item) => &item.attrs,
        TraitItem::Macro(item) => &item.attrs,
        _ => return None,
    })
}

fn foreign_item_attrs(item: &ForeignItem) -> Option<&[Attribute]> {
    Some(match item {
        ForeignItem::Fn(item) => &item.attrs,
        ForeignItem::Static(item) => &item.attrs,
        ForeignItem::Type(item) => &item.attrs,
        ForeignItem::Macro(item) => &item.attrs,
        _ => return None,
    })
}

fn has_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("cfg")
            && attr
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .is_ok_and(|metas| metas.iter().any(meta_mentions_test))
    })
}

fn meta_mentions_test(meta: &Meta) -> bool {
    if meta.path().is_ident("test") {
        return true;
    }
    match meta {
        Meta::List(list) => list
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .is_ok_and(|metas| metas.iter().any(meta_mentions_test)),
        _ => false,
    }
}

fn px_numeric(expr: &Expr) -> Option<bool> {
    numeric(px_argument(expr)?).map(|value| value == 0.0)
}

fn px_has_additive_literal(expr: &Expr) -> bool {
    fn contains(expr: &Expr) -> bool {
        let Expr::Binary(binary) = transparent(expr) else {
            return false;
        };
        match binary.op {
            syn::BinOp::Add(_) | syn::BinOp::Sub(_) => {
                numeric(&binary.left).is_some_and(|value| value != 0.0)
                    || numeric(&binary.right).is_some_and(|value| value != 0.0)
                    || contains(&binary.left)
                    || contains(&binary.right)
            }
            // Multiplication and division describe runtime transforms as
            // often as style. Do not descend into them: `-8.0 * lift` and
            // `(scale - 1.0) / overshoot` are animation keyframe geometry,
            // which the token ownership contract deliberately leaves local.
            _ => false,
        }
    }

    px_argument(expr).is_some_and(contains)
}

fn px_has_scaled_literal(expr: &Expr) -> bool {
    fn contains(expr: &Expr) -> bool {
        let Expr::Binary(binary) = transparent(expr) else {
            return false;
        };
        if matches!(
            binary.op,
            syn::BinOp::Add(_) | syn::BinOp::Sub(_) | syn::BinOp::Mul(_) | syn::BinOp::Div(_)
        ) && (numeric(&binary.left).is_some_and(|value| value != 0.0)
            || numeric(&binary.right).is_some_and(|value| value != 0.0))
        {
            return true;
        }
        contains(&binary.left) || contains(&binary.right)
    }

    px_argument(expr).is_some_and(contains)
}

fn px_argument(expr: &Expr) -> Option<&Expr> {
    let Expr::Call(ExprCall { func, args, .. }) = transparent(expr) else {
        return None;
    };
    let Expr::Path(path) = transparent(func) else {
        return None;
    };
    (path.path.segments.last()?.ident == "px" && args.len() == 1).then(|| args.first())?
}

fn font_weight_numeric(expr: &Expr) -> bool {
    let Expr::Call(call) = transparent(expr) else {
        return false;
    };
    let Expr::Path(path) = transparent(&call.func) else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "FontWeight")
        && call.args.len() == 1
        && call
            .args
            .first()
            .is_some_and(|argument| numeric(argument).is_some())
}

fn numeric(expr: &Expr) -> Option<f64> {
    match transparent(expr) {
        Expr::Lit(literal) => match &literal.lit {
            Lit::Int(value) => value.base10_parse().ok(),
            Lit::Float(value) => value.base10_parse().ok(),
            _ => None,
        },
        Expr::Unary(unary) if matches!(unary.op, UnOp::Neg(_)) => numeric(&unary.expr).map(|n| -n),
        _ => None,
    }
}

fn transparent(mut expr: &Expr) -> &Expr {
    loop {
        expr = match expr {
            Expr::Paren(inner) => &inner.expr,
            Expr::Group(inner) => &inner.expr,
            Expr::Reference(inner) => &inner.expr,
            _ => return expr,
        };
    }
}

fn is_theme_color(expr: &Expr) -> bool {
    let Expr::Field(role) = transparent(expr) else {
        return false;
    };
    let Expr::Field(colors) = transparent(&role.base) else {
        return false;
    };
    if !matches!(&colors.member, syn::Member::Named(name) if name == "colors") {
        return false;
    }
    match transparent(&colors.base) {
        Expr::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "theme"),
        Expr::MethodCall(call) => call.method == "theme" && call.args.is_empty(),
        _ => false,
    }
}

fn is_style_binding(expr: &Expr) -> bool {
    let Expr::Path(path) = transparent(expr) else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|segment| matches!(segment.ident.to_string().as_str(), "element" | "style"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(source: &str) -> Vec<&'static str> {
        lint_source(Path::new("crates/gpui-kit/src/component.rs"), source)
            .expect("test source should parse")
            .into_iter()
            .map(|diagnostic| diagnostic.rule)
            .collect()
    }

    #[test]
    fn reports_each_literal_token_rule() {
        let found = rules(
            "fn render() { div().gap(px(4)).rounded_lg(px(6)).text_size(px(13)).line_height(px(18)).font_weight(FontWeight(500)); }",
        );
        assert_eq!(found.len(), 5);
        assert!(found.iter().any(|rule| rule.starts_with("spacing:")));
        assert!(found.iter().any(|rule| rule.starts_with("radius:")));
        assert_eq!(
            found
                .iter()
                .filter(|rule| rule.starts_with("typography:"))
                .count(),
            3
        );
    }

    #[test]
    fn reports_literals_hidden_inside_spacing_and_type_arithmetic() {
        let found = rules(
            "fn render() { div().gap(px(theme.spacing.md + 2.0)).text_size(px(step.size * 0.86)); }",
        );
        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|rule| rule.starts_with("spacing:")));
        assert!(found.iter().any(|rule| rule.starts_with("typography:")));
    }

    #[test]
    fn allows_token_composition_and_runtime_type_scaling() {
        assert!(
            rules("fn render() { div().gap(px(theme.spacing.md + theme.spacing.xxs)).text_size(px(step.size * viewport.zoom)); }")
                .is_empty()
        );
    }

    #[test]
    fn allows_local_animation_keyframe_geometry() {
        assert!(
            rules("fn render() { div().mt(px(-6.0 * (scale - 1.0) / overshoot)); }").is_empty()
        );
    }

    #[test]
    fn allows_spacing_zero_and_named_values() {
        assert!(
            rules("fn render() { let inset = px(7); div().p(px(0)).gap(inset).w(px(99)); }")
                .is_empty()
        );
    }

    #[test]
    fn ignores_test_items_and_scene_paths() {
        assert!(rules("#[cfg(test)] mod tests { fn fixture() { div().p(px(8)); } } #[test] fn direct() { div().rounded(px(2)); } impl Widget { #[cfg(test)] fn fixture() { div().gap(px(3)); } }").is_empty());
        assert!(
            lint_source(
                Path::new("crates/gpui-kit/src/scenes/demo.rs"),
                "fn scene() { div().p(px(8)); }"
            )
            .expect("scene test source should parse")
            .is_empty()
        );
    }

    #[test]
    fn reports_both_theme_color_receivers_through_references() {
        let found = rules(
            "fn render() { (&theme.colors.accent).opacity(0.2); (&(cx.theme().colors.warning)).opacity(1); }",
        );
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|rule| rule.starts_with("color opacity:")));
    }

    #[test]
    fn reports_literal_element_opacity_but_allows_token_and_animated_values() {
        let found = rules(
            "fn render() { element.opacity(0.4); style.opacity(1); element.opacity(theme.opacity.disabled); element.opacity(progress); }",
        );
        assert_eq!(found.len(), 2);
        assert!(
            found
                .iter()
                .all(|rule| rule.starts_with("element opacity:"))
        );
    }

    #[test]
    fn color_alpha_requires_a_token_or_named_local_policy() {
        let found = rules(
            "fn paint() { color.opacity(0.4); color.opacity(LOCAL_ALPHA); color.opacity(0.0); color.opacity(1.0); }",
        );
        assert_eq!(found.len(), 1);
        assert!(found[0].starts_with("color opacity:"));
    }

    #[test]
    fn theme_adapter_cannot_hide_recipe_alpha_literals() {
        let found = lint_source(
            Path::new("crates/gpui-kit-theme/src/lib.rs"),
            "fn recipe(color: Hsla) { color.opacity(0.4); }",
        )
        .expect("theme test source should parse");
        assert_eq!(found.len(), 1);
        assert!(found[0].rule.starts_with("theme opacity:"));
    }
}
