use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::thread;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

mod api;
mod dependencies;
mod package;
mod site;
mod strings;
mod token_lint;
mod typography;
use gpui_kit_tokens::{
    BorderWeight, ControlSize, Density, Elevation, Layer, MotionEasing, OpacityRole, Radius, Space,
    SpringPreset, TokenDocument, bundled, contrast,
};

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let command = (args.next(), args.next());
    let rest = args.collect::<Vec<_>>();
    match (command.0.as_deref(), command.1.as_deref()) {
        (Some("tokens"), Some("generate")) => tokens(false),
        (Some("tokens"), Some("check")) => tokens(true),
        (Some("strings"), Some("check")) => strings::check(&root()),
        (Some("strings"), Some("generate")) => strings::generate(&root()),
        (Some("typography"), Some("check")) => typography::check(&root()),
        (Some("api"), Some("check")) => api::check(&root()),
        (Some("api"), Some("generate")) => api::generate(&root()),
        (Some("dependencies"), Some("check")) => dependencies::check(&root(), &rest),
        (Some("package"), Some("plan")) => package::plan(&root()),
        (Some("package"), Some("check")) => package::check(&root()),
        (Some("package"), Some("publish")) => {
            let release_root = env::var_os("GPUI_BOX_RELEASE_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(root);
            package::publish(&release_root, &rest)
        }
        (Some("site"), Some("generate")) => {
            web_build()?;
            site::generate(
                &root(),
                rest.first().map(String::as_str),
                &root().join("target/browser-gallery"),
            )
            .map(|_| ())
        }
        (Some("site"), Some("check")) => {
            web_build()?;
            site::check_with_browser(&root(), &root().join("target/browser-gallery"))
        }
        (Some("accessibility"), Some("check")) => accessibility_check(),
        (Some("performance"), Some("check")) => performance_check(),
        (Some("scenes"), Some("list")) => scenes_list(),
        (Some("scenes"), Some("render")) => scenes_render(&rest),
        (Some("headless"), Some("capture")) => headless("capture", &rest),
        (Some("headless"), Some("check")) => headless("check", &rest),
        (Some("web"), Some("check")) => web_check(),
        (Some("web"), Some("build")) => web_build(),
        (Some("web"), Some("smoke")) => web_smoke(),
        (Some("web"), Some("visual")) => web_visual(&rest),
        (Some("web"), Some("gate")) => web_gate(&rest),
        (Some("gate"), None) => gate(false),
        (Some("gate"), Some("full")) => gate(true),
        (Some("gate"), Some("only")) => gate_only(&rest),
        _ => bail!(
            "usage: cargo xtask <dependencies check|package plan|package check|package publish --execute|site generate [output]|site check|accessibility check|performance check|tokens generate|tokens check|strings check|\
             strings generate|typography check|scenes list|scenes render [name...]|\
             headless capture [name...]|\
             headless check [name...]|web check|web build|web smoke|\
             web visual <capture|check> [name...]|web gate [scene...]|\
             gate [full]|gate only <scene>...>"
        ),
    }
}

#[cfg(target_os = "macos")]
fn accessibility_check() -> Result<()> {
    step("cargo", &["build", "-p", "gpui-box-gallery"], None)?;
    let executable = root().join("target/debug/gpui-box-gallery");
    let mut gallery = Command::new(&executable)
        .args(["--scene", "button", "--theme", "studio-light"])
        .current_dir(root())
        .spawn()
        .context("launch the gallery for the macOS accessibility smoke check")?;

    let result = (|| {
        let enabled = osascript("tell application \"System Events\" to get UI elements enabled")?;
        if enabled.trim() != "true" {
            bail!(
                "macOS Accessibility permission is unavailable; enable it for the invoking terminal before rerunning"
            );
        }

        let output = swift_ax_check(gallery.id(), "button")?;

        for expected in [
            "Primary|AXButton|true||true",
            "Unavailable|AXButton|false||false",
            "Saving|AXButton|false||false",
            "Selected|AXCheckBox|true|true|false",
        ] {
            if !output.lines().any(|line| line.trim() == expected) {
                bail!("macOS AX tree is missing `{expected}`; received:\n{output}");
            }
        }
        println!("macOS AX roles, names, disabled state, checked state, and requested focus match");
        Ok(())
    })();

    let cleanup = (|| {
        if gallery.try_wait()?.is_none() {
            gallery
                .kill()
                .context("stop the accessibility smoke gallery")?;
        }
        gallery
            .wait()
            .context("reap the accessibility smoke gallery")?;
        Ok(())
    })();
    result.and(cleanup)?;
    thread::sleep(Duration::from_secs(1));
    editable_accessibility_check()?;
    thread::sleep(Duration::from_secs(1));
    form_accessibility_check()?;
    thread::sleep(Duration::from_secs(1));
    dialog_accessibility_check()?;
    thread::sleep(Duration::from_secs(1));
    menu_accessibility_check()?;
    thread::sleep(Duration::from_secs(1));
    tooltip_accessibility_check()?;
    thread::sleep(Duration::from_secs(1));
    toast_accessibility_check()
}

#[cfg(target_os = "macos")]
fn editable_accessibility_check() -> Result<()> {
    let executable = root().join("target/debug/gpui-box-gallery");
    let mut gallery = Command::new(&executable)
        .args(["--scene", "input", "--theme", "studio-light"])
        .current_dir(root())
        .spawn()
        .context("launch the editable macOS accessibility smoke gallery")?;

    let result = (|| {
        let output = swift_ax_check(gallery.id(), "editable")?;
        for expected in [
            "API token|AXTextField|true||false",
            "Disabled|AXTextField|false|read only|false",
            "Email|AXTextField|true|edited@example.com|true",
            "Email geometry|true",
        ] {
            if !output.lines().any(|line| line.trim() == expected) {
                bail!("macOS editable AX tree is missing `{expected}`; received:\n{output}");
            }
        }
        println!(
            "macOS AX editable names, values, enabled state, requested focus, editing, and character/caret bounds match"
        );
        Ok(())
    })();

    let cleanup = (|| {
        if gallery.try_wait()?.is_none() {
            gallery
                .kill()
                .context("stop the editable accessibility smoke gallery")?;
        }
        gallery
            .wait()
            .context("reap the editable accessibility smoke gallery")?;
        Ok(())
    })();
    result.and(cleanup)
}

#[cfg(target_os = "macos")]
fn form_accessibility_check() -> Result<()> {
    let executable = root().join("target/debug/gpui-box-gallery");
    let mut gallery = Command::new(&executable)
        .args(["--scene", "form", "--theme", "studio-light"])
        .current_dir(root())
        .spawn()
        .context("launch the form macOS accessibility smoke gallery")?;

    let result = (|| {
        let output = swift_ax_check(gallery.id(), "form")?;
        for expected in [
            "Workspace name|Shown wherever this workspace appears. A workspace with this name already exists.",
            "Retention|How long a finished run is kept. This workspace allows at most 60 days.",
        ] {
            if !output.lines().any(|line| line.trim() == expected) {
                bail!("macOS form AX tree is missing `{expected}`; received:\n{output}");
            }
        }
        println!("macOS AX field labels and complete help/error descriptions match");
        Ok(())
    })();

    let cleanup = cleanup_gallery(&mut gallery, "form accessibility smoke gallery");
    result.and(cleanup)
}

#[cfg(target_os = "macos")]
fn dialog_accessibility_check() -> Result<()> {
    let executable = root().join("target/debug/gpui-box-gallery");
    let mut gallery = Command::new(&executable)
        .args(["--scene", "dialog", "--theme", "studio-light"])
        .current_dir(root())
        .spawn()
        .context("launch the dialog macOS accessibility smoke gallery")?;

    let result = (|| {
        let output = swift_ax_check(gallery.id(), "dialog")?;
        let expected = "Replace the existing theme?|AXWindow|AXDialog|Replace|closed";
        if !output.lines().any(|line| line.trim() == expected) {
            bail!("macOS dialog AX tree is missing `{expected}`; received:\n{output}");
        }
        println!(
            "macOS AX dialog name/subrole, initial focused action, and dismissal lifetime match"
        );
        Ok(())
    })();

    let cleanup = (|| {
        if gallery.try_wait()?.is_none() {
            gallery
                .kill()
                .context("stop the dialog accessibility smoke gallery")?;
        }
        gallery
            .wait()
            .context("reap the dialog accessibility smoke gallery")?;
        Ok(())
    })();
    result.and(cleanup)
}

#[cfg(target_os = "macos")]
fn menu_accessibility_check() -> Result<()> {
    let executable = root().join("target/debug/gpui-box-gallery");
    let mut gallery = Command::new(&executable)
        .args(["--scene", "menu", "--theme", "studio-light"])
        .current_dir(root())
        .spawn()
        .context("launch the menu macOS accessibility smoke gallery")?;

    let result = (|| {
        let output = swift_ax_check(gallery.id(), "menu")?;
        let expected = "Run actions|AXMenu|Copy run id|Copy link|Export as file|closed";
        if output.trim() != expected {
            bail!("macOS menu AX check expected `{expected}`; received:\n{output}");
        }
        println!("macOS AX menu/name/action, active-item movement, and dismissal lifetime match");
        Ok(())
    })();
    let cleanup = cleanup_gallery(&mut gallery, "menu accessibility smoke gallery");
    result.and(cleanup)
}

#[cfg(target_os = "macos")]
fn tooltip_accessibility_check() -> Result<()> {
    let executable = root().join("target/debug/gpui-box-gallery");
    let mut gallery = Command::new(&executable)
        .args(["--scene", "tooltip", "--theme", "studio-light"])
        .current_dir(root())
        .spawn()
        .context("launch the tooltip macOS accessibility smoke gallery")?;

    let result = (|| {
        let output = swift_ax_check(gallery.id(), "tooltip")?;
        let expected = "Export theme|AXHelp|AXUserInterfaceTooltip|shown|hidden";
        if output.trim() != expected {
            bail!("macOS tooltip AX check expected `{expected}`; received:\n{output}");
        }
        println!("macOS AX tooltip subrole/lifetime and trigger literal AXHelp match");
        Ok(())
    })();
    let cleanup = cleanup_gallery(&mut gallery, "tooltip accessibility smoke gallery");
    result.and(cleanup)
}

#[cfg(target_os = "macos")]
fn toast_accessibility_check() -> Result<()> {
    let executable = root().join("target/debug/gpui-box-gallery");
    let mut gallery = Command::new(&executable)
        .args(["--scene", "toast", "--theme", "studio-light"])
        .current_dir(root())
        .spawn()
        .context("launch the toast macOS accessibility smoke gallery")?;

    let result = (|| {
        let output = swift_ax_check(gallery.id(), "toast")?;
        let expected = "Refreshing the model catalog failed|AXApplicationStatus|Dismiss|closed";
        if output.trim() != expected {
            bail!("macOS status AX check expected `{expected}`; received:\n{output}");
        }
        println!("macOS AX status subrole/action and dismissal lifetime match");
        Ok(())
    })();
    let cleanup = cleanup_gallery(&mut gallery, "toast accessibility smoke gallery");
    result.and(cleanup)
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn cleanup_gallery(gallery: &mut std::process::Child, description: &str) -> Result<()> {
    if gallery.try_wait()?.is_none() {
        gallery
            .kill()
            .with_context(|| format!("stop the {description}"))?;
    }
    gallery
        .wait()
        .with_context(|| format!("reap the {description}"))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn swift_ax_check(pid: u32, mode: &str) -> Result<String> {
    let mut child = Command::new("/usr/bin/swift")
        .args(["-e", AX_NATIVE_SCRIPT])
        .env("GPUI_KIT_AX_PID", pid.to_string())
        .env("GPUI_KIT_AX_MODE", mode)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("launch the bounded native macOS AX {mode} check"))?;
    let deadline = Instant::now() + Duration::from_secs(20);
    while child.try_wait()?.is_none() {
        if Instant::now() >= deadline {
            child.kill().context("stop the timed-out native AX check")?;
            let _ = child.wait();
            bail!("native macOS AX {mode} check timed out after 20 seconds");
        }
        thread::sleep(Duration::from_millis(50));
    }
    let output = child
        .wait_with_output()
        .context("reap the native AX check")?;
    if !output.status.success() {
        bail!(
            "native macOS AX {mode} check failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(target_os = "windows")]
fn accessibility_check() -> Result<()> {
    step("cargo", &["build", "-p", "gpui-box-gallery"], None)?;
    let executable = root().join("target/debug/gpui-box-gallery.exe");
    for (scene, mode) in [("input", "editable"), ("form", "form"), ("menu", "menu")] {
        let mut gallery = Command::new(&executable)
            .args(["--scene", scene, "--theme", "studio-light"])
            .current_dir(root())
            .spawn()
            .with_context(|| format!("launch the {scene} Windows UI Automation gallery"))?;
        let result = windows_uia_check(gallery.id(), mode).map(|output| {
            println!("Windows UIA {mode}: {}", output.trim());
        });
        let cleanup = cleanup_gallery(&mut gallery, "Windows UI Automation gallery");
        result.and(cleanup)?;
        thread::sleep(Duration::from_secs(1));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_uia_check(pid: u32, mode: &str) -> Result<String> {
    let script = root().join("tools/accessibility/windows-smoke.ps1");
    let mut child = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(script)
        .arg("-TargetProcessId")
        .arg(pid.to_string())
        .args(["-Mode", mode])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("launch the bounded native Windows UIA {mode} check"))?;
    let deadline = Instant::now() + Duration::from_secs(30);
    while child.try_wait()?.is_none() {
        if Instant::now() >= deadline {
            child
                .kill()
                .context("stop the timed-out native UIA check")?;
            let _ = child.wait();
            bail!("native Windows UIA {mode} check timed out after 30 seconds");
        }
        thread::sleep(Duration::from_millis(50));
    }
    let output = child
        .wait_with_output()
        .context("reap the native Windows UIA check")?;
    if !output.status.success() {
        bail!(
            "native Windows UIA {mode} check failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn accessibility_check() -> Result<()> {
    bail!("the native accessibility smoke check currently requires macOS or Windows")
}

#[cfg(target_os = "macos")]
fn osascript(script: &str) -> Result<String> {
    let mut child = Command::new("osascript")
        .args(["-e", script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("query macOS accessibility through System Events")?;
    let deadline = Instant::now() + Duration::from_secs(7);
    while child.try_wait()?.is_none() {
        if Instant::now() >= deadline {
            child
                .kill()
                .context("stop a timed-out macOS accessibility query")?;
            let _ = child.wait();
            bail!("macOS accessibility query timed out after 7 seconds");
        }
        thread::sleep(Duration::from_millis(50));
    }
    let output = child
        .wait_with_output()
        .context("collect the macOS accessibility query")?;
    if !output.status.success() {
        bail!(
            "macOS accessibility query failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(target_os = "macos")]
#[allow(dead_code)]
const AX_SMOKE_SCRIPT: &str = r#"
tell application "System Events"
  with timeout of 5 seconds
  set matches to every process whose unix id is __PID__
  if (count of matches) is 0 then return ""
  tell first item of matches
    set frontmost to true
    set axItems to entire contents of window 1
    set output to ""
    repeat with itemRef in axItems
      try
        set itemName to name of itemRef as text
        if itemName is "Primary" or itemName is "Unavailable" or itemName is "Saving" or itemName is "Selected" then
          set itemValue to ""
          if itemName is "Selected" then set itemValue to value of itemRef as text
          if itemName is "Primary" then
            set focused of itemRef to true
            delay 0.1
          end if
          set output to output & itemName & "|" & (role of itemRef as text) & "|" & (enabled of itemRef as text) & "|" & itemValue & "|" & (focused of itemRef as text) & linefeed
        end if
      end try
    end repeat
    return output
  end tell
  end timeout
end tell
"#;

#[cfg(target_os = "macos")]
#[allow(dead_code)]
const AX_EDITABLE_SCRIPT: &str = r#"
tell application "System Events"
  with timeout of 5 seconds
  set matches to every process whose unix id is __PID__
  if (count of matches) is 0 then return ""
  tell first item of matches
    set frontmost to true
    set axItems to entire contents of window 1
    set output to ""
    repeat with itemRef in axItems
      try
        set itemName to name of itemRef as text
        if itemName is "API token" or itemName is "Disabled" or itemName is "Email" then
          if itemName is "Email" then
            set focused of itemRef to true
            delay 0.1
            set value of itemRef to "edited@example.com"
            delay 0.1
          end if
          set itemValue to ""
          try
            set itemValue to value of itemRef as text
            if itemValue is "missing value" then set itemValue to ""
          end try
          set output to output & itemName & "|" & (role of itemRef as text) & "|" & (enabled of itemRef as text) & "|" & itemValue & "|" & (focused of itemRef as text) & linefeed
        end if
      end try
    end repeat
    return output
  end tell
  end timeout
end tell
"#;

#[cfg(target_os = "macos")]
const AX_NATIVE_SCRIPT: &str = r#"
import AppKit
import ApplicationServices
import Foundation

let environment = ProcessInfo.processInfo.environment
guard let pidText = environment["GPUI_KIT_AX_PID"], let pid = pid_t(pidText),
      let mode = environment["GPUI_KIT_AX_MODE"] else { exit(2) }
let application = AXUIElementCreateApplication(pid)
NSRunningApplication(processIdentifier: pid)?.activate(options: [])

func attribute(_ element: AXUIElement, _ key: CFString) -> CFTypeRef? {
    var value: CFTypeRef?
    return AXUIElementCopyAttributeValue(element, key, &value) == .success ? value : nil
}
func string(_ element: AXUIElement, _ key: CFString) -> String {
    attribute(element, key) as? String ?? ""
}
func bool(_ element: AXUIElement, _ key: CFString) -> Bool {
    attribute(element, key) as? Bool ?? false
}
func children(_ element: AXUIElement) -> [AXUIElement] {
    attribute(element, kAXChildrenAttribute as CFString) as? [AXUIElement] ?? []
}
func descendants(_ element: AXUIElement) -> [AXUIElement] {
    children(element).flatMap { [$0] + descendants($0) }
}
func title(_ element: AXUIElement) -> String {
    let title = string(element, kAXTitleAttribute as CFString)
    return title.isEmpty ? string(element, kAXDescriptionAttribute as CFString) : title
}
func matches(_ role: String, _ subrole: String = "", _ titleText: String = "") -> [AXUIElement] {
    descendants(application).filter {
        string($0, kAXRoleAttribute as CFString) == role &&
        (subrole.isEmpty || string($0, kAXSubroleAttribute as CFString) == subrole) &&
        (titleText.isEmpty || title($0) == titleText)
    }
}
func pollUntil(_ predicate: () -> Bool, seconds: Double = 10) -> Bool {
    let deadline = Date().addingTimeInterval(seconds)
    repeat {
        if predicate() { return true }
        Thread.sleep(forTimeInterval: 0.1)
    } while Date() < deadline
    return false
}
func press(_ element: AXUIElement) -> Bool {
    AXUIElementPerformAction(element, kAXPressAction as CFString) == .success
}
func actions(_ element: AXUIElement) -> [String] {
    var names: CFArray?
    guard AXUIElementCopyActionNames(element, &names) == .success else { return [] }
    return names as? [String] ?? []
}
func key(_ code: CGKeyCode) {
    CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: true)?.post(tap: .cghidEventTap)
    CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: false)?.post(tap: .cghidEventTap)
}
func point(_ element: AXUIElement, _ key: CFString) -> CGPoint? {
    guard let rawValue = attribute(element, key) else { return nil }
    let value = rawValue as! AXValue
    guard AXValueGetType(value) == .cgPoint else { return nil }
    var point = CGPoint.zero
    return AXValueGetValue(value, .cgPoint, &point) ? point : nil
}
func size(_ element: AXUIElement) -> CGSize? {
    guard let rawValue = attribute(element, kAXSizeAttribute as CFString) else { return nil }
    let value = rawValue as! AXValue
    guard AXValueGetType(value) == .cgSize else { return nil }
    var size = CGSize.zero
    return AXValueGetValue(value, .cgSize, &size) ? size : nil
}
func moveMouse(_ point: CGPoint) {
    CGEvent(mouseEventSource: nil, mouseType: .mouseMoved,
            mouseCursorPosition: point, mouseButton: .left)?.post(tap: .cghidEventTap)
}
func boundsForRange(_ element: AXUIElement, _ range: CFRange) -> CGRect? {
    var mutableRange = range
    guard let rangeValue = AXValueCreate(.cfRange, &mutableRange) else { return nil }
    var rawBounds: CFTypeRef?
    guard AXUIElementCopyParameterizedAttributeValue(
        element,
        kAXBoundsForRangeParameterizedAttribute as CFString,
        rangeValue,
        &rawBounds
    ) == .success, let rawBounds else { return nil }
    let boundsValue = rawBounds as! AXValue
    guard AXValueGetType(boundsValue) == .cgRect else { return nil }
    var bounds = CGRect.zero
    return AXValueGetValue(boundsValue, .cgRect, &bounds) ? bounds : nil
}
func selectedRange(_ element: AXUIElement) -> CFRange? {
    guard let rawValue = attribute(element, kAXSelectedTextRangeAttribute as CFString) else {
        return nil
    }
    let value = rawValue as! AXValue
    guard AXValueGetType(value) == .cfRange else { return nil }
    var range = CFRange()
    return AXValueGetValue(value, .cfRange, &range) ? range : nil
}
func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(1)
}

switch mode {
case "button":
    guard pollUntil({ matches("AXButton", "", "Primary").count == 1 }) else {
        fail("Primary AXButton did not appear")
    }
    guard let primary = matches("AXButton", "", "Primary").first,
          AXUIElementSetAttributeValue(primary, kAXFocusedAttribute as CFString,
                                       kCFBooleanTrue) == .success,
          pollUntil({ bool(primary, kAXFocusedAttribute as CFString) }) else {
        fail("assistive-technology focus request did not focus Primary")
    }
    func buttonLine(_ role: String, _ name: String, _ includeValue: Bool = false) -> String {
        guard let node = matches(role, "", name).first else { fail("missing \(name) \(role)") }
        let value: String
        if includeValue {
            if let number = attribute(node, kAXValueAttribute as CFString) as? NSNumber {
                value = number.boolValue ? "true" : "false"
            } else {
                value = string(node, kAXValueAttribute as CFString)
            }
        } else { value = "" }
        return "\(name)|\(role)|\(bool(node, kAXEnabledAttribute as CFString))|\(value)|\(bool(node, kAXFocusedAttribute as CFString))"
    }
    print(buttonLine("AXButton", "Primary"))
    print(buttonLine("AXButton", "Unavailable"))
    print(buttonLine("AXButton", "Saving"))
    print(buttonLine("AXCheckBox", "Selected", true))

case "editable":
    guard pollUntil({ matches("AXTextField", "", "Email").count == 1 }) else {
        fail("Email AXTextField did not appear")
    }
    guard let email = matches("AXTextField", "", "Email").first,
          AXUIElementSetAttributeValue(email, kAXFocusedAttribute as CFString,
                                       kCFBooleanTrue) == .success,
          AXUIElementSetAttributeValue(email, kAXValueAttribute as CFString,
                                       "edited@example.com" as CFString) == .success,
          pollUntil({ string(email, kAXValueAttribute as CFString) == "edited@example.com" &&
                      bool(email, kAXFocusedAttribute as CFString) }) else {
        fail("native requested focus/editing did not update Email")
    }
    func editableLine(_ name: String, redactValue: Bool = false) -> String {
        guard let node = matches("AXTextField", "", name).first else { fail("missing \(name) AXTextField") }
        let value = redactValue ? "" : string(node, kAXValueAttribute as CFString)
        return "\(name)|AXTextField|\(bool(node, kAXEnabledAttribute as CFString))|\(value)|\(bool(node, kAXFocusedAttribute as CFString))"
    }
    print(editableLine("API token", redactValue: true))
    print(editableLine("Disabled"))
    print(editableLine("Email"))
    let editedLength = 18
    guard pollUntil({
        guard let first = boundsForRange(email, CFRange(location: 0, length: 1)),
              let middle = boundsForRange(email, CFRange(location: 5, length: 1)),
              let caret = boundsForRange(email, CFRange(location: editedLength, length: 0)),
              let selection = selectedRange(email) else { return false }
        return first.width > 0 && first.height > 0 &&
               middle.width > 0 && middle.height > 0 &&
               first.origin.x != middle.origin.x &&
               caret.height > 0 && caret.origin.x > middle.origin.x &&
               selection.location == editedLength && selection.length == 0
    }) else {
        fail("Email did not expose range-dependent character bounds and its end caret")
    }
    print("Email geometry|true")

case "form":
    guard pollUntil({ matches("AXTextField", "", "Workspace name").count == 1 &&
                      matches("AXTextField", "", "Retention").count == 1 }) else {
        fail("labelled form AXTextFields did not appear")
    }
    guard let workspace = matches("AXTextField", "", "Workspace name").first,
          let retention = matches("AXTextField", "", "Retention").first else {
        fail("labelled form AXTextFields disappeared")
    }
    print("Workspace name|\(string(workspace, kAXHelpAttribute as CFString))")
    print("Retention|\(string(retention, kAXHelpAttribute as CFString))")

case "dialog":
    let dialogTitle = "Replace the existing theme?"
    guard pollUntil({ matches("AXWindow", "AXDialog", dialogTitle).count == 1 }) else {
        fail("unique named AXDialog did not appear")
    }
    guard let replace = matches("AXButton", "", "Replace").first,
          bool(replace, kAXFocusedAttribute as CFString) else {
        fail("dialog Replace action was not natively focused")
    }
    key(53)
    guard pollUntil({ matches("AXWindow", "AXDialog", dialogTitle).isEmpty }) else {
        fail("AXDialog did not disappear after Escape")
    }
    print("\(dialogTitle)|AXWindow|AXDialog|Replace|closed")

case "menu":
    guard pollUntil({ matches("AXMenu", "", "Run actions").count == 1 }) else {
        fail("unique named AXMenu did not appear")
    }
    guard let action = matches("AXMenuItem", "", "Copy run id").first,
          actions(action).contains(kAXPressAction as String) else {
        fail("Copy run id did not expose AXPress on its AXMenuItem")
    }
    guard pollUntil({ matches("AXMenuItem", "", "Copy link").contains { bool($0, kAXFocusedAttribute as CFString) } }) else {
        fail("initial active AXMenuItem was not focused")
    }
    key(125)
    guard pollUntil({ matches("AXMenuItem", "", "Export as file").contains { bool($0, kAXFocusedAttribute as CFString) } }) else {
        fail("ArrowDown did not move native focus to Export as file")
    }
    key(53)
    key(53)
    guard pollUntil({ matches("AXMenu", "", "Run actions").isEmpty }) else {
        fail("AXMenu did not disappear after Escape")
    }
    print("Run actions|AXMenu|Copy run id|Copy link|Export as file|closed")

case "tooltip":
    let help = "Writes the theme to a file on disk"
    guard pollUntil({ matches("AXButton", "", "Export theme").count == 1 }) else {
        fail("Export theme AXButton did not appear")
    }
    guard let trigger = matches("AXButton", "", "Export theme").first,
          string(trigger, kAXHelpAttribute as CFString) == help,
          let origin = point(trigger, kAXPositionAttribute as CFString),
          let dimensions = size(trigger) else {
        fail("Export theme did not expose its literal AXHelp and geometry")
    }
    let tooltips = { matches("AXGroup", "AXUserInterfaceTooltip", help) }
    guard pollUntil({ tooltips().count == 1 }) else {
        fail("the scene's named AXUserInterfaceTooltip did not appear")
    }
    moveMouse(CGPoint(x: origin.x + dimensions.width / 2, y: origin.y + dimensions.height / 2))
    guard pollUntil({ tooltips().count == 2 }) else {
        fail("hover did not show a second AXUserInterfaceTooltip")
    }
    guard let window = matches("AXWindow").first,
          let windowOrigin = point(window, kAXPositionAttribute as CFString),
          let windowSize = size(window) else { fail("gallery AXWindow had no geometry") }
    moveMouse(CGPoint(x: windowOrigin.x + windowSize.width - 40,
                      y: windowOrigin.y + windowSize.height - 40))
    guard pollUntil({ tooltips().count == 1 }) else {
        fail("hover tooltip did not disappear while the scene tooltip remained")
    }
    print("Export theme|AXHelp|AXUserInterfaceTooltip|shown|hidden")

case "toast":
    let statusTitle = "Refreshing the model catalog failed"
    let statusNodes = { matches("AXGroup", "AXApplicationStatus", statusTitle) }
    guard pollUntil({ statusNodes().count == 1 }) else {
        fail("unique named AXApplicationStatus did not appear")
    }
    guard let status = statusNodes().first,
          let dismiss = descendants(status).first(where: {
              string($0, kAXRoleAttribute as CFString) == "AXButton" && title($0) == "Dismiss"
          }), press(dismiss) else {
        fail("status Dismiss action did not expose or accept AXPress")
    }
    guard pollUntil({ statusNodes().isEmpty }) else {
        fail("dismissed AXApplicationStatus did not disappear")
    }
    guard matches("AXGroup", "AXApplicationStatus", "The host refused to publish this run").count == 1 else {
        fail("dismissing one status incorrectly removed another")
    }
    print("\(statusTitle)|AXApplicationStatus|Dismiss|closed")

default:
    fail("unsupported native AX check mode: \(mode)")
}
"#;

#[cfg(target_os = "macos")]
#[allow(dead_code)]
const AX_DIALOG_SCRIPT: &str = r#"
tell application "System Events"
  with timeout of 5 seconds
  set matches to every process whose unix id is __PID__
  if (count of matches) is 0 then return ""
  tell first item of matches
    set frontmost to true
    set dialogName to ""
    set dialogRole to ""
    set dialogSubrole to ""
    set focusedAction to ""
    set dialogCount to 0
    set focusedActionCount to 0
    repeat with itemRef in entire contents of window 1
      try
        if (subrole of itemRef as text) is "AXDialog" and (name of itemRef as text) is "Replace the existing theme?" then
          set dialogCount to dialogCount + 1
          set dialogName to name of itemRef as text
          set dialogRole to role of itemRef as text
          set dialogSubrole to subrole of itemRef as text
          set dialogItems to entire contents of itemRef
          repeat with childRef in dialogItems
            try
              if (role of childRef as text) is "AXButton" and (name of childRef as text) is "Replace" and (focused of childRef) is true then
                set focusedAction to name of childRef as text
                set focusedActionCount to focusedActionCount + 1
              end if
            end try
          end repeat
        end if
      end try
    end repeat
    if dialogCount is not 1 or focusedActionCount is not 1 then return ""
    key code 53
    set remainsOpen to true
    repeat 10 times
      set remainsOpen to false
      repeat with itemRef in entire contents of window 1
        try
          if (subrole of itemRef as text) is "AXDialog" and (name of itemRef as text) is dialogName then
            set remainsOpen to true
          end if
        end try
      end repeat
      if not remainsOpen then exit repeat
      delay 0.1
    end repeat
    if remainsOpen then return ""
    return dialogName & "|" & dialogRole & "|" & dialogSubrole & "|" & focusedAction & "|closed"
  end tell
  end timeout
end tell
"#;

#[cfg(target_os = "macos")]
#[allow(dead_code)]
const AX_MENU_SCRIPT: &str = r#"
tell application "System Events"
  with timeout of 5 seconds
  set matches to every process whose unix id is __PID__
  if (count of matches) is 0 then return ""
  tell first item of matches
    set frontmost to true
    set axItems to entire contents of window 1
    set menuCount to 0
    set actionCount to 0
    set initialFocusCount to 0
    repeat 1 times
      set menuCount to 0
      set actionCount to 0
      set initialFocusCount to 0
      set axItems to entire contents of window 1
      repeat with itemRef in axItems
        try
          if (role of itemRef as text) is "AXMenu" and (name of itemRef as text) is "Run actions" then
            set menuCount to menuCount + 1
          end if
          if (role of itemRef as text) is "AXMenuItem" and (name of itemRef as text) is "Copy run id" then
            if (name of every action of itemRef) contains "AXPress" then set actionCount to actionCount + 1
          end if
          if (role of itemRef as text) is "AXMenuItem" and (name of itemRef as text) is "Copy link" and (focused of itemRef) is true then
            set initialFocusCount to initialFocusCount + 1
          end if
        end try
      end repeat
      if menuCount is 1 and actionCount is 1 and initialFocusCount is 1 then exit repeat
      delay 0.1
    end repeat
    if menuCount is not 1 or actionCount is not 1 or initialFocusCount is not 1 then return ""
    key code 125
    delay 0.1
    set movedFocusCount to 0
    repeat 10 times
      set movedFocusCount to 0
      repeat with itemRef in entire contents of window 1
        try
          if (role of itemRef as text) is "AXMenuItem" and (name of itemRef as text) is "Export as file" and (focused of itemRef) is true then
            set movedFocusCount to movedFocusCount + 1
          end if
        end try
      end repeat
      if movedFocusCount is 1 then exit repeat
      delay 0.1
    end repeat
    if movedFocusCount is not 1 then return ""
    key code 53
    key code 53
    delay 0.2
    set remainsOpen to false
    repeat with itemRef in entire contents of window 1
      try
        if (role of itemRef as text) is "AXMenu" and (name of itemRef as text) is "Run actions" then set remainsOpen to true
      end try
    end repeat
    if remainsOpen then return ""
    return "Run actions|AXMenu|Copy run id|Copy link|Export as file|closed"
  end tell
  end timeout
end tell
"#;

#[cfg(target_os = "macos")]
#[allow(dead_code)]
const AX_TOOLTIP_TARGET_SCRIPT: &str = r#"
tell application "System Events"
  with timeout of 5 seconds
  set matches to every process whose unix id is __PID__
  if (count of matches) is 0 then return ""
  tell first item of matches
    set frontmost to true
    set triggerRef to missing value
    set staticHelpCount to 0
    repeat 1 times
      set triggerRef to missing value
      set staticHelpCount to 0
      repeat with itemRef in entire contents of window 1
        try
          if (role of itemRef as text) is "AXButton" and (name of itemRef as text) is "Export theme" then
            if (value of attribute "AXHelp" of itemRef as text) is "Writes the theme to a file on disk" then set triggerRef to itemRef
          end if
          if (role of itemRef as text) is "AXGroup" and (subrole of itemRef as text) is "AXUserInterfaceTooltip" and (name of itemRef as text) is "Writes the theme to a file on disk" then
            set staticHelpCount to staticHelpCount + 1
          end if
        end try
      end repeat
      if triggerRef is not missing value and staticHelpCount >= 1 then exit repeat
      delay 0.1
    end repeat
    if triggerRef is missing value or staticHelpCount < 1 then return ""
    set triggerPosition to position of triggerRef
    set triggerSize to size of triggerRef
    set windowPosition to position of window 1
    set windowSize to size of window 1
    set hoverX to (item 1 of triggerPosition) + ((item 1 of triggerSize) div 2)
    set hoverY to (item 2 of triggerPosition) + ((item 2 of triggerSize) div 2)
    set awayX to (item 1 of windowPosition) + (item 1 of windowSize) - 40
    set awayY to (item 2 of windowPosition) + (item 2 of windowSize) - 40
    return (hoverX as text) & "|" & (hoverY as text) & "|" & (awayX as text) & "|" & (awayY as text)
  end tell
  end timeout
end tell
"#;

#[cfg(target_os = "macos")]
#[allow(dead_code)]
const AX_TOOLTIP_COUNT_SCRIPT: &str = r#"
tell application "System Events"
  with timeout of 5 seconds
  set matches to every process whose unix id is __PID__
  if (count of matches) is 0 then return ""
  tell first item of matches
    set frontmost to true
    set helpCount to 0
    set triggerPresent to false
    repeat 1 times
      set helpCount to 0
      set triggerPresent to false
      repeat with itemRef in entire contents of window 1
        try
          if (role of itemRef as text) is "AXGroup" and (subrole of itemRef as text) is "AXUserInterfaceTooltip" and (name of itemRef as text) is "Writes the theme to a file on disk" then
            set helpCount to helpCount + 1
          end if
          if (role of itemRef as text) is "AXButton" and (name of itemRef as text) is "Export theme" then
            if (value of attribute "AXHelp" of itemRef as text) is "Writes the theme to a file on disk" then set triggerPresent to true
          end if
        end try
      end repeat
      if triggerPresent then exit repeat
      delay 0.1
    end repeat
    if triggerPresent then return (helpCount as text) & "|trigger"
    return (helpCount as text) & "|missing-trigger"
  end tell
  end timeout
end tell
"#;

#[cfg(target_os = "macos")]
#[allow(dead_code)]
const AX_TOAST_SCRIPT: &str = r#"
tell application "System Events"
  with timeout of 5 seconds
  set matches to every process whose unix id is __PID__
  if (count of matches) is 0 then return ""
  tell first item of matches
    set frontmost to true
    set statusCount to 0
    set dismissCount to 0
    set dismissRef to missing value
    repeat 1 times
      set statusCount to 0
      set dismissCount to 0
      set dismissRef to missing value
      repeat with itemRef in entire contents of window 1
        try
          if (role of itemRef as text) is "AXGroup" and (subrole of itemRef as text) is "AXApplicationStatus" and (name of itemRef as text) is "Refreshing the model catalog failed" then
            set statusCount to statusCount + 1
            repeat with childRef in entire contents of itemRef
              try
                if (role of childRef as text) is "AXButton" and (name of childRef as text) is "Dismiss" then
                  if (name of every action of childRef) contains "AXPress" then
                    set dismissRef to childRef
                    set dismissCount to dismissCount + 1
                  end if
                end if
              end try
            end repeat
          end if
        end try
      end repeat
      if statusCount is 1 and dismissCount is 1 then exit repeat
      delay 0.1
    end repeat
    if statusCount is not 1 or dismissCount is not 1 then return ""
    perform action "AXPress" of dismissRef
    delay 1
    set statusRemains to false
    set otherStatusRemains to false
    repeat with itemRef in entire contents of window 1
      try
        if (role of itemRef as text) is "AXGroup" and (subrole of itemRef as text) is "AXApplicationStatus" and (name of itemRef as text) is "Refreshing the model catalog failed" then set statusRemains to true
        if (role of itemRef as text) is "AXGroup" and (subrole of itemRef as text) is "AXApplicationStatus" and (name of itemRef as text) is "The host refused to publish this run" then set otherStatusRemains to true
      end try
    end repeat
    if statusRemains or not otherStatusRemains then return ""
    return "Refreshing the model catalog failed|AXGroup|Dismiss|closed"
  end tell
  end timeout
end tell
"#;

fn scenes_list() -> Result<()> {
    for scene in gpui_kit::scenes::catalog() {
        println!("{}", scene.name);
    }
    Ok(())
}

/// Renders scenes in every bundled theme to images a person can look at.
///
/// This opens a real window, so it is how motion and the text caret get
/// reviewed, and it is not the gate: a window negotiates its size with the
/// display it opens on, so two Macs produce two different pictures of the same
/// scene. `xtask headless check` holds the baseline instead.
///
/// Naming scenes renders only those, which is what a change to one component
/// needs. Naming none renders the catalog.
fn scenes_render(only: &[String]) -> Result<()> {
    let directory = root().join("target").join("scenes");
    if directory.exists() {
        fs::remove_dir_all(&directory).with_context(|| format!("clear {}", directory.display()))?;
    }
    let count = capture_into(&directory, only)?;
    println!("rendered {count} images into {}", directory.display());
    Ok(())
}

/// Runs the checks a change has to pass.
///
/// The short form is what a work-in-progress change wants: it answers in about
/// a minute. The full form adds the two slow proofs, rendered documentation
/// and the visual regression, and is what a commit wants.
fn gate(full: bool) -> Result<()> {
    step("cargo", &["fmt", "--all", "--", "--check"], None)?;
    dependencies::check(&root(), &[])?;
    future_compatibility_check()?;
    step("cargo", &["test", "--workspace"], None)?;
    // GPUI Box owns the complete framework and kit source. A warning anywhere
    // in the workspace is therefore a gate failure rather than upstream debt.
    step(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
        None,
    )?;
    tokens(true)?;
    strings::check(&root())?;
    api::check(&root())?;
    site::check(&root())?;
    performance_check()?;
    if full {
        step(
            "cargo",
            &["doc", "--no-deps", "--workspace"],
            Some(("RUSTDOCFLAGS", "-D warnings")),
        )?;
        headless("check", &[])?;
    }
    println!("gate passed");
    Ok(())
}

fn performance_check() -> Result<()> {
    step(
        "cargo",
        &[
            "run",
            "-p",
            "gpui-box-performance",
            "--",
            "--output",
            "target/performance/report.json",
        ],
        None,
    )
}

fn future_compatibility_check() -> Result<()> {
    let args = [
        "check",
        "--workspace",
        "--all-targets",
        "--all-features",
        "--future-incompat-report",
    ];
    println!("== cargo {}", args.join(" "));
    let output = Command::new("cargo")
        .args(args)
        .current_dir(root())
        .output()
        .context("run Cargo future-incompatibility check")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    print!("{stdout}");
    eprint!("{stderr}");
    if !output.status.success() {
        bail!("cargo {} failed", args.join(" "));
    }
    if !stderr.contains("0 dependencies had future-incompatible warnings") {
        bail!("Cargo reported future-incompatible dependencies");
    }
    Ok(())
}

/// Runs the part of the gate one component can invalidate.
///
/// The full gate compiles and tests every workspace member and renders the
/// whole catalog, which is minutes of waiting for an edit to one file. This
/// answers the same questions about the named scenes: the library still builds
/// and lints clean, the tests whose names mention them still pass, the
/// generated artifacts are still current, and those scenes still look the way
/// the baseline says.
///
/// It is a shortcut while iterating, not a substitute for `gate` before a
/// commit. It says nothing about the other members, the doctests, or a scene
/// the edit reached without anybody predicting it.
fn gate_only(scenes: &[String]) -> Result<()> {
    if scenes.is_empty() {
        bail!("usage: cargo xtask gate only <scene>...; `xtask scenes list` names them");
    }
    for scene in scenes {
        if gpui_kit::scenes::find(scene).is_none() {
            bail!("unknown scene `{scene}`; `xtask scenes list` names them");
        }
    }
    step("cargo", &["fmt", "--all", "--", "--check"], None)?;
    step(
        "cargo",
        &[
            "clippy",
            "-p",
            "gpui-box-kit",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
        None,
    )?;
    // A scene name is also how its tests are named, which is what makes one
    // argument enough to select both.
    for scene in scenes {
        step("cargo", &["test", "-p", "gpui-box-kit", scene], None)?;
    }
    tokens(true)?;
    strings::check(&root())?;
    api::check(&root())?;
    headless("check", scenes)?;
    println!("gate only {} passed", scenes.join(" "));
    Ok(())
}

/// Runs the visual gate, which lives in its own workspace with
/// renderer-specific dependencies and an independent lockfile.
///
/// It renders offscreen at a size it names, so it is the gate on every
/// platform. `scenes` still opens a real window, which is how motion and the
/// text caret get looked at, but a window negotiates its size with the display
/// it opens on and so cannot hold a baseline another machine can reproduce.
fn headless(command: &str, only: &[String]) -> Result<()> {
    let manifest = root()
        .join("tools")
        .join("headless-visual")
        .join("Cargo.toml");
    println!("== headless {command} {}", only.join(" "));
    let status = Command::new(env!("CARGO"))
        .args(["run", "--quiet", "--manifest-path"])
        .arg(&manifest)
        .arg("--")
        .arg(command)
        .args(only)
        .current_dir(root())
        .status()
        .context("run the headless visual gate")?;
    if !status.success() {
        bail!("headless {command} failed");
    }
    Ok(())
}

const WASM_TARGET: &str = "wasm32-unknown-unknown";
const WASM_BINDGEN_VERSION: &str = "0.2.127";
const WASM_DENY_WARNINGS: &str = "target.wasm32-unknown-unknown.rustflags=['-Dwarnings']";

fn web_check() -> Result<()> {
    step(
        env!("CARGO"),
        &[
            "check",
            "--config",
            WASM_DENY_WARNINGS,
            "-p",
            "gpui-box-kit",
            "--lib",
            "--features",
            "fixtures",
            "--target",
            WASM_TARGET,
        ],
        None,
    )
}

fn web_build() -> Result<()> {
    web_install_node_tools()?;
    step(
        env!("CARGO"),
        &[
            "build",
            "--config",
            WASM_DENY_WARNINGS,
            "-p",
            "gpui-box-browser-gallery",
            "--target",
            WASM_TARGET,
            "--release",
        ],
        None,
    )?;

    let version = Command::new("wasm-bindgen")
        .arg("--version")
        .current_dir(root())
        .output()
        .context(
            "wasm-bindgen-cli is required; install it with `cargo install \
             wasm-bindgen-cli --version 0.2.127 --locked`",
        )?;
    let expected = format!("wasm-bindgen {WASM_BINDGEN_VERSION}");
    if !version.status.success() || String::from_utf8_lossy(&version.stdout).trim() != expected {
        bail!(
            "web build requires `{expected}` to match Cargo.lock; install it with \
             `cargo install wasm-bindgen-cli --version {WASM_BINDGEN_VERSION} --locked`"
        );
    }

    let output = root().join("target/browser-gallery");
    fs::create_dir_all(&output).with_context(|| format!("create {}", output.display()))?;
    let wasm = root().join(format!(
        "target/{WASM_TARGET}/release/gpui_kit_browser_gallery.wasm"
    ));
    let status = Command::new("wasm-bindgen")
        .args(["--target", "web", "--no-typescript", "--out-name"])
        .arg("gpui_kit_browser_gallery")
        .arg("--out-dir")
        .arg(&output)
        .arg(&wasm)
        .current_dir(root())
        .status()
        .context("generate browser bindings")?;
    if !status.success() {
        bail!("wasm-bindgen failed");
    }
    shrink_browser_wasm(&output)?;
    fs::copy(
        root().join("examples/browser-gallery/web/index.html"),
        output.join("index.html"),
    )
    .context("copy the browser gallery host page")?;
    println!("browser gallery built in {}", output.display());
    Ok(())
}

fn web_install_node_tools() -> Result<()> {
    let package = root().join("examples/browser-gallery");
    let package = package.to_string_lossy();
    step("npm", &["--prefix", package.as_ref(), "ci"], None)
}

/// Cloudflare Workers rejects a static asset above 25 MiB. The release
/// wasm-bindgen output now sits just over that; Binaryen's `-Oz` brings it
/// back under without changing the surface the compose page loads. Use the
/// workspace-pinned Binaryen: older releases can silently export wasm-bindgen's
/// fixed function table as `__wbindgen_externrefs`, which only fails when the
/// generated JavaScript first grows that table.
fn shrink_browser_wasm(output: &Path) -> Result<()> {
    let wasm = output.join("gpui_kit_browser_gallery_bg.wasm");
    let tmp = output.join("gpui_kit_browser_gallery_bg.opt.wasm");
    let package = root().join("examples/browser-gallery");
    let package = package.to_string_lossy();
    let status = Command::new("npm")
        .args([
            "--prefix",
            package.as_ref(),
            "exec",
            "--",
            "wasm-opt",
            "-Oz",
            "--enable-bulk-memory",
            "--enable-nontrapping-float-to-int",
            "--enable-sign-ext",
            "--enable-mutable-globals",
            "--enable-reference-types",
            "-o",
        ])
        .arg(&tmp)
        .arg(&wasm)
        .current_dir(root())
        .status()
        .context(
            "run the browser workspace's pinned wasm-opt; run \
             `npm --prefix examples/browser-gallery ci` to install it",
        )?;
    if !status.success() {
        let _ = fs::remove_file(&tmp);
        bail!("wasm-opt failed");
    }
    fs::rename(&tmp, &wasm).with_context(|| format!("replace {}", wasm.display()))?;
    Ok(())
}

fn web_smoke() -> Result<()> {
    web_build()?;
    web_prepare()?;
    web_smoke_prepared()?;
    web_site_smoke_prepared()
}

fn web_prepare() -> Result<()> {
    let package = root().join("examples/browser-gallery");
    let package = package.to_string_lossy();
    step(
        "npm",
        &[
            "--prefix",
            package.as_ref(),
            "exec",
            "--",
            "playwright",
            "install",
            "chromium",
        ],
        None,
    )
}

fn web_smoke_prepared() -> Result<()> {
    let package = root().join("examples/browser-gallery");
    let package = package.to_string_lossy();
    let config = root().join("examples/browser-gallery/playwright.config.mjs");
    let config = config.to_string_lossy();
    #[cfg(target_os = "linux")]
    {
        step(
            "xvfb-run",
            &[
                "-a",
                "npm",
                "--prefix",
                package.as_ref(),
                "exec",
                "--",
                "playwright",
                "test",
                "--config",
                config.as_ref(),
            ],
            None,
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        step(
            "npm",
            &[
                "--prefix",
                package.as_ref(),
                "exec",
                "--",
                "playwright",
                "test",
                "--config",
                config.as_ref(),
            ],
            None,
        )
    }
}

fn web_site_smoke_prepared() -> Result<()> {
    let output = root().join("target/site-browser-smoke");
    let browser_gallery = root().join("target/browser-gallery");
    let result = (|| {
        site::generate(&root(), output.to_str(), &browser_gallery)?;
        let package = root().join("examples/browser-gallery");
        let package = package.to_string_lossy();
        let config = root().join("examples/browser-gallery/site.config.mjs");
        let config = config.to_string_lossy();
        step(
            "npm",
            &[
                "--prefix",
                package.as_ref(),
                "exec",
                "--",
                "playwright",
                "test",
                "--config",
                config.as_ref(),
            ],
            None,
        )
    })();
    let cleanup: Result<()> = if output.exists() {
        fs::remove_dir_all(&output)
    } else {
        Ok(())
    }
    .map_err(Into::into);
    result.and(cleanup)
}

fn web_visual(args: &[String]) -> Result<()> {
    let Some(command @ ("capture" | "check")) = args.first().map(String::as_str) else {
        bail!("usage: cargo xtask web visual <capture|check> [scene...]");
    };
    web_build()?;
    web_prepare()?;
    web_visual_prepared(command, &args[1..])
}

fn web_visual_prepared(command: &str, scenes: &[String]) -> Result<()> {
    let package = root().join("examples/browser-gallery");
    let package = package.to_string_lossy();
    let config = root().join("examples/browser-gallery/visual.config.mjs");
    let mut playwright = Command::new("npm");
    playwright
        .args([
            "--prefix",
            package.as_ref(),
            "exec",
            "--",
            "playwright",
            "test",
            "--config",
        ])
        .arg(config)
        .current_dir(root());
    if command == "capture" {
        playwright.arg("--update-snapshots");
    }
    if !scenes.is_empty() {
        playwright.env("GPUI_KIT_WEB_SCENES", scenes.join(","));
    }
    let status = playwright.status().context("run browser visual gate")?;
    if !status.success() {
        bail!("browser visual {command} failed");
    }
    Ok(())
}

/// Runs the complete browser proof after preparing its build and browser once.
fn web_gate(scenes: &[String]) -> Result<()> {
    web_check()?;
    web_build()?;
    web_prepare()?;
    web_smoke_prepared()?;
    web_site_smoke_prepared()?;
    web_visual_prepared("check", scenes)?;
    println!("web gate passed");
    Ok(())
}

fn step(program: &str, args: &[&str], env: Option<(&str, &str)>) -> Result<()> {
    println!("== {program} {}", args.join(" "));
    let mut command = Command::new(program);
    command.args(args).current_dir(root());
    if let Some((name, value)) = env {
        command.env(name, value);
    }
    let status = command
        .status()
        .with_context(|| format!("run {program} {}", args.join(" ")))?;
    if !status.success() {
        bail!("{program} {} failed", args.join(" "));
    }
    Ok(())
}

/// Drives one gallery process over the whole catalog.
///
/// A GPUI application owns the window system for its lifetime, so the gallery
/// swaps the scene on a live window rather than opening a process per image.
fn capture_into(directory: &Path, only: &[String]) -> Result<usize> {
    let _held = Capturing::claim()?;
    fs::create_dir_all(directory).with_context(|| format!("create {}", directory.display()))?;
    let mut command = Command::new(env!("CARGO"));
    command
        .args(["run", "--quiet", "-p", "gpui-box-gallery", "--"])
        .arg("--capture-all")
        .arg(directory)
        .current_dir(root());
    if !only.is_empty() {
        for name in only {
            if gpui_kit::scenes::find(name).is_none() {
                bail!("unknown scene `{name}`");
            }
        }
        command.arg("--only").arg(only.join(","));
    }
    let status = command.status().context("run the gallery")?;
    if !status.success() {
        bail!("capturing scenes failed");
    }
    // Naming every image the run owed rather than counting what is there:
    // a run that stopped early would otherwise report as a complete one, and
    // a comparison against images it never wrote would pass. Counting alone
    // cannot tell the difference when the destination already holds the rest
    // of the catalog.
    let wanted = expected_images(only);
    let absent: Vec<&String> = wanted
        .iter()
        .filter(|name| !directory.join(name).exists())
        .collect();
    if !absent.is_empty() {
        bail!(
            "the capture owed {} images and {} never arrived under {}: {}",
            wanted.len(),
            absent.len(),
            directory.display(),
            absent
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(wanted.len())
}

/// The right to be the only capture running on this machine.
///
/// Two galleries cannot capture at once. They take the foreground from each
/// other, and a window nobody is compositing hands back the frame it drew
/// last, so both runs read each other's scenes. That failure looks exactly
/// like a component having changed, which is the one thing this tool exists
/// to report truthfully.
struct Capturing(PathBuf);

impl Capturing {
    fn claim() -> Result<Self> {
        let path = root().join("target").join("capturing.lock");
        fs::create_dir_all(path.parent().expect("target has a parent"))?;
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(_) => Ok(Self(path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => bail!(
                "another capture is running. Wait for it, or delete {} if nothing is.",
                path.display()
            ),
            Err(error) => Err(error).with_context(|| format!("claim {}", path.display())),
        }
    }
}

impl Drop for Capturing {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Every file name a capture of `only` owes, or of the catalog when empty.
fn expected_images(only: &[String]) -> Vec<String> {
    let scenes: Vec<String> = if only.is_empty() {
        gpui_kit::scenes::catalog()
            .iter()
            .map(|scene| scene.name.to_owned())
            .collect()
    } else {
        only.to_vec()
    };
    let themes: Vec<String> = bundled()
        .iter()
        .map(|theme| theme.meta.id.clone())
        .collect();
    scenes
        .iter()
        .flat_map(|scene| {
            themes
                .iter()
                .map(move |theme| format!("{scene}-{theme}.png"))
        })
        .collect()
}

fn tokens(check: bool) -> Result<()> {
    contrast_gate()?;
    palette_parity_gate()?;
    if check {
        token_lint::check(&root())?;
    }

    let mut output = String::from(
        "<!-- @generated by `cargo xtask tokens generate`; do not edit. -->\n\
         # Token reference\n\n\
         The JSON documents under `crates/gpui-kit-tokens/tokens/` are the authority. These tables are\n\
         a review aid, and every theme below is validated on each run.\n",
    );
    for document in gpui_kit_tokens::all() {
        theme_section(&mut output, document)?;
    }

    let path = root().join("docs/token-reference.md");
    if check {
        let current = fs::read_to_string(&path)
            .with_context(|| format!("read generated {}", path.display()))?;
        if !same_generated_text(&current, &output) {
            bail!(
                "{} is stale; run `cargo xtask tokens generate`",
                path.display()
            );
        }
        println!("{} is current", path.display());
    } else {
        fs::write(&path, output).with_context(|| format!("write {}", path.display()))?;
        println!("generated {}", path.display());
    }
    Ok(())
}

fn same_generated_text(current: &str, expected: &str) -> bool {
    current.replace("\r\n", "\n") == expected
}

/// Every theme carries the same palette groups and steps, so a component or a
/// caller that names `{group.step}` resolves in every theme it can be shown
/// under, not only the one it was written against.
fn palette_parity_gate() -> Result<()> {
    let themes = gpui_kit_tokens::all();
    let (reference, rest) = themes.split_first().expect("at least one shipped theme");
    let shape = |document: &TokenDocument| -> Vec<String> {
        document
            .color
            .palette
            .iter()
            .flat_map(|(group, steps)| steps.keys().map(move |step| format!("{group}.{step}")))
            .collect()
    };
    let expected = shape(reference);
    let mut failed = false;
    for document in rest {
        let actual = shape(document);
        for missing in expected.iter().filter(|key| !actual.contains(key)) {
            failed = true;
            eprintln!(
                "{}: palette is missing `{missing}` carried by {}",
                document.meta.id, reference.meta.id
            );
        }
        for extra in actual.iter().filter(|key| !expected.contains(key)) {
            failed = true;
            eprintln!(
                "{}: palette carries `{extra}` absent from {}",
                document.meta.id, reference.meta.id
            );
        }
    }
    if failed {
        bail!("palette key sets differ between themes");
    }
    Ok(())
}

/// Fails the task rather than the document, so a contrast regression is caught
/// before it can be committed as a generated table.
fn contrast_gate() -> Result<()> {
    let mut failed = false;
    for document in gpui_kit_tokens::all() {
        for failure in contrast::failures(document) {
            failed = true;
            eprintln!(
                "{}: {} on {} is {:.2}:1, below the {:.1}:1 minimum",
                document.meta.id,
                failure.foreground,
                failure.background,
                failure.ratio,
                failure.minimum
            );
        }
        for failure in contrast::separation_failures(document) {
            failed = true;
            eprintln!(
                "{}: {} over {} gains {:.1} L*, below the {:.1} minimum",
                document.meta.id, failure.near, failure.behind, failure.distance, failure.minimum
            );
        }
        for failure in contrast::distinction_failures(document) {
            failed = true;
            eprintln!(
                "{}: {} stands only {:.1} L* further from the page than {}, below the {:.1} minimum",
                document.meta.id, failure.tone, failure.distance, failure.against, failure.minimum
            );
        }
        // The three checks below are enforced by `TokenDocument::validate`,
        // and were not reported here. A gate that stops at the checks it
        // happens to know about tells an author their theme is fine and then
        // fails somewhere further from the edit.
        for failure in contrast::line_failures(document) {
            failed = true;
            eprintln!(
                "{}: {} on {} gains {:.2} L*, below the {:.2} minimum",
                document.meta.id, failure.line, failure.surface, failure.distance, failure.minimum
            );
        }
        for failure in contrast::placeholder_failures(document) {
            failed = true;
            eprintln!(
                "{}: {} over {} reads {:.1} L*, outside the {:.1} to {:.1} band",
                document.meta.id,
                failure.role,
                failure.surface,
                failure.distance,
                failure.minimum,
                failure.maximum
            );
        }
        for failure in contrast::series_failures(document)
            .into_iter()
            .chain(contrast::canvas_failures(document))
        {
            failed = true;
            if failure.maximum.is_finite() {
                eprintln!(
                    "{}: {} of {} is {:.3}, above the {:.3} maximum",
                    document.meta.id,
                    failure.measure,
                    failure.subject,
                    failure.value,
                    failure.maximum
                );
            } else {
                eprintln!(
                    "{}: {} of {} is {:.3}, below the {:.3} minimum",
                    document.meta.id,
                    failure.measure,
                    failure.subject,
                    failure.value,
                    failure.minimum
                );
            }
        }
    }
    if failed {
        bail!("contrast requirements are not met");
    }
    Ok(())
}

/// Emits one theme, in a fixed order.
///
/// The table is built from typed accessors rather than by iterating the parsed
/// JSON, because whether `serde_json` preserves document order depends on
/// feature unification across the workspace, and a generated file must not
/// change with an unrelated dependency.
fn theme_section(output: &mut String, tokens: &TokenDocument) -> Result<()> {
    write!(
        output,
        "\n## {} (`{}`)\n\nAppearance: `{:?}`.\n",
        tokens.meta.name, tokens.meta.id, tokens.meta.appearance
    )?;

    output.push_str("\n### Palette\n\n| Token | Value |\n|---|---|\n");
    for (group, steps) in &tokens.color.palette {
        for (step, value) in steps {
            writeln!(output, "| `color.palette.{group}.{step}` | `{value}` |")?;
        }
    }

    output.push_str("\n### Semantic color\n\n| Token | Source | Resolved |\n|---|---|---|\n");
    let color = &tokens.color;
    let sources: Vec<(String, &str)> = vec![
        (
            "color.surface.backdrop".into(),
            color.surface.backdrop.as_str(),
        ),
        ("color.surface.canvas".into(), color.surface.canvas.as_str()),
        ("color.surface.sunken".into(), color.surface.sunken.as_str()),
        ("color.surface.panel".into(), color.surface.panel.as_str()),
        ("color.surface.raised".into(), color.surface.raised.as_str()),
        (
            "color.surface.overlay".into(),
            color.surface.overlay.as_str(),
        ),
        ("color.text.primary".into(), color.text.primary.as_str()),
        ("color.text.muted".into(), color.text.muted.as_str()),
        ("color.text.faint".into(), color.text.faint.as_str()),
        (
            "color.text.placeholder".into(),
            color.text.placeholder.as_str(),
        ),
        ("color.text.disabled".into(), color.text.disabled.as_str()),
        ("color.text.onAccent".into(), color.text.on_accent.as_str()),
        (
            "color.text.onPrimaryFill".into(),
            color.text.on_primary_fill.as_str(),
        ),
        (
            "color.interactive.hover".into(),
            color.interactive.hover.as_str(),
        ),
        (
            "color.interactive.active".into(),
            color.interactive.active.as_str(),
        ),
        (
            "color.interactive.selected".into(),
            color.interactive.selected.as_str(),
        ),
        (
            "color.interactive.hairline".into(),
            color.interactive.hairline.as_str(),
        ),
        (
            "color.interactive.hairlineStrong".into(),
            color.interactive.hairline_strong.as_str(),
        ),
        (
            "color.interactive.track".into(),
            color.interactive.track.as_str(),
        ),
        (
            "color.interactive.divider".into(),
            color.interactive.divider.as_str(),
        ),
        (
            "color.interactive.focus".into(),
            color.interactive.focus.as_str(),
        ),
        (
            "color.interactive.primaryFill".into(),
            color.interactive.primary_fill.as_str(),
        ),
        (
            "color.interactive.whiteFill".into(),
            color.interactive.white_fill.as_str(),
        ),
        (
            "color.interactive.whiteFillHover".into(),
            color.interactive.white_fill_hover.as_str(),
        ),
        (
            "color.interactive.whiteFillActive".into(),
            color.interactive.white_fill_active.as_str(),
        ),
        (
            "color.semantic.accent".into(),
            color.semantic.accent.as_str(),
        ),
        (
            "color.semantic.accentStrong".into(),
            color.semantic.accent_strong.as_str(),
        ),
        (
            "color.semantic.danger".into(),
            color.semantic.danger.as_str(),
        ),
        (
            "color.semantic.warning".into(),
            color.semantic.warning.as_str(),
        ),
        (
            "color.semantic.success".into(),
            color.semantic.success.as_str(),
        ),
        ("color.semantic.info".into(), color.semantic.info.as_str()),
    ]
    .into_iter()
    .chain(
        color
            .sequence
            .categorical
            .iter()
            .enumerate()
            .map(|(index, value)| {
                (
                    format!("color.sequence.categorical.{index}"),
                    value.as_str(),
                )
            }),
    )
    .chain([
        (
            "color.node.headerWash".into(),
            color.node.header_wash.as_str(),
        ),
        ("color.node.portIdle".into(), color.node.port_idle.as_str()),
        (
            "color.node.portHover".into(),
            color.node.port_hover.as_str(),
        ),
        (
            "color.node.portConnected".into(),
            color.node.port_connected.as_str(),
        ),
        ("color.node.edge".into(), color.node.edge.as_str()),
        (
            "color.node.edgeActive".into(),
            color.node.edge_active.as_str(),
        ),
        (
            "color.node.edgeFeedback".into(),
            color.node.edge_feedback.as_str(),
        ),
        (
            "color.node.edgeFeedbackActive".into(),
            color.node.edge_feedback_active.as_str(),
        ),
        (
            "color.node.labelWash".into(),
            color.node.label_wash.as_str(),
        ),
        ("color.node.grid".into(), color.node.grid.as_str()),
        (
            "color.node.gridStrong".into(),
            color.node.grid_strong.as_str(),
        ),
        ("color.node.gridAxis".into(), color.node.grid_axis.as_str()),
    ])
    .chain([
        ("color.loader.mark".into(), color.loader.mark.as_str()),
        ("color.loader.track".into(), color.loader.track.as_str()),
        (
            "color.loader.placeholder".into(),
            color.loader.placeholder.as_str(),
        ),
        ("color.loader.sheen".into(), color.loader.sheen.as_str()),
    ])
    .chain([
        (
            "color.terminal.background".into(),
            color.terminal.background.as_str(),
        ),
        (
            "color.terminal.selection".into(),
            color.terminal.selection.as_str(),
        ),
    ])
    .chain(
        color
            .terminal
            .ansi
            .iter()
            .enumerate()
            .map(|(index, value)| (format!("color.terminal.ansi.{index}"), value.as_str())),
    )
    .collect();

    for (path, source) in sources {
        writeln!(
            output,
            "| `{path}` | `{source}` | `{}` |",
            hex(tokens, source)
        )?;
    }

    output.push_str("\n### Palette variant steps\n\n| Recipe | Preferred steps |\n|---|---|\n");
    for (name, steps) in [
        ("filled", &tokens.color.palette_steps.filled),
        ("hover", &tokens.color.palette_steps.hover),
        ("active", &tokens.color.palette_steps.active),
        ("readableDark", &tokens.color.palette_steps.readable_dark),
        ("readableLight", &tokens.color.palette_steps.readable_light),
    ] {
        writeln!(
            output,
            "| `color.paletteSteps.{name}` | `{}` |",
            steps.join(" → ")
        )?;
    }

    output.push_str("\n### Spacing\n\n| Step | Pixels |\n|---|---:|\n");
    for (name, step) in [
        ("xxs", Space::Xxs),
        ("xs", Space::Xs),
        ("sm", Space::Sm),
        ("md", Space::Md),
        ("lg", Space::Lg),
        ("xl", Space::Xl),
        ("xxl", Space::Xxl),
    ] {
        writeln!(output, "| `{name}` | {} |", tokens.spacing(step))?;
    }

    output.push_str("\n### Measures\n\n| Token | Pixels |\n|---|---:|\n");
    for (name, value) in [
        ("measure.readableWidth", tokens.measure.readable_width),
        ("measure.dialogWidth", tokens.measure.dialog_width),
        ("measure.menuMinWidth", tokens.measure.menu_min_width),
        (
            "measure.compactMenuMinWidth",
            tokens.measure.compact_menu_min_width,
        ),
        ("measure.menuMaxHeight", tokens.measure.menu_max_height),
        (
            "measure.compactMenuMaxHeight",
            tokens.measure.compact_menu_max_height,
        ),
        ("measure.standaloneIcon", tokens.measure.standalone_icon),
        ("measure.scrollbarTrack", tokens.measure.scrollbar_track),
        ("measure.scrollbarThumb", tokens.measure.scrollbar_thumb),
        (
            "measure.scrollbarMinThumb",
            tokens.measure.scrollbar_min_thumb,
        ),
        ("measure.caretWidth", tokens.measure.caret_width),
        (
            "measure.textDecorationWidth",
            tokens.measure.text_decoration_width,
        ),
        (
            "measure.progressTrackHeight",
            tokens.measure.progress_track_height,
        ),
        (
            "measure.sliderTrackHeight",
            tokens.measure.slider_track_height,
        ),
        (
            "measure.sliderVerticalHeight",
            tokens.measure.slider_vertical_height,
        ),
        ("measure.containerSmall", tokens.measure.container_small),
        ("measure.containerMedium", tokens.measure.container_medium),
        ("measure.containerLarge", tokens.measure.container_large),
        (
            "measure.containerExtraLarge",
            tokens.measure.container_extra_large,
        ),
        (
            "measure.compactOverlayWidth",
            tokens.measure.compact_overlay_width,
        ),
        (
            "measure.mediaViewerHeight",
            tokens.measure.media_viewer_height,
        ),
        (
            "measure.timelineRailWidth",
            tokens.measure.timeline_rail_width,
        ),
        ("measure.statusMark", tokens.measure.status_mark),
    ] {
        writeln!(output, "| `{name}` | {value} |")?;
    }

    output.push_str("\n### Radius\n\n| Step | Pixels |\n|---|---:|\n");
    for (name, step) in [
        ("small", Radius::Small),
        ("control", Radius::Control),
        ("card", Radius::Card),
        ("dialog", Radius::Dialog),
        ("bubble", Radius::Bubble),
        ("pill", Radius::Pill),
    ] {
        writeln!(output, "| `{name}` | {} |", tokens.radius(step))?;
    }

    output.push_str(
        "\n### Control scale\n\n| Step | Height | Padding X | Gap | Font | Icon |\n|---|---:|---:|---:|---:|---:|\n",
    );
    for (name, size) in [
        ("xs", ControlSize::Xs),
        ("sm", ControlSize::Sm),
        ("md", ControlSize::Md),
        ("lg", ControlSize::Lg),
    ] {
        let step = tokens.control(size);
        writeln!(
            output,
            "| `{name}` | {} | {} | {} | {} | {} |",
            step.height, step.padding_x, step.gap, step.font_size, step.icon_size
        )?;
    }

    output.push_str("\n### Density\n\n| Axis | Space | Control | Font |\n|---|---:|---:|---:|\n");
    for (name, density) in [
        ("compact", Density::Compact),
        ("comfortable", Density::Comfortable),
    ] {
        let scale = tokens.density(density);
        writeln!(
            output,
            "| `{name}` | {} | {} | {} |",
            scale.space, scale.control, scale.font
        )?;
    }

    writeln!(
        output,
        "\n### Typography\n\nNumeric readout scale: {}×\n\n| Step | Size | Line height | Weight |\n|---|---:|---:|---:|---:|",
        tokens.typography.readout_scale
    )?;
    for (name, step) in [
        ("caption", &tokens.typography.scale.caption),
        ("label", &tokens.typography.scale.label),
        ("body", &tokens.typography.scale.body),
        ("strong", &tokens.typography.scale.strong),
        ("subtitle", &tokens.typography.scale.subtitle),
        ("title", &tokens.typography.scale.title),
        ("code", &tokens.typography.scale.code),
    ] {
        writeln!(
            output,
            "| `{name}` | {} | {} | {} |",
            step.size, step.line_height, step.weight
        )?;
    }

    output.push_str(
        "\n### Elevation\n\n| Step | Layer | Y | Blur | Spread | Color |\n|---|---:|---:|---:|---:|---|\n",
    );
    for (name, level) in [
        ("flat", Elevation::Flat),
        ("raised", Elevation::Raised),
        ("overlay", Elevation::Overlay),
        ("modal", Elevation::Modal),
    ] {
        let step = tokens.elevation(level);
        if step.layers.is_empty() {
            writeln!(output, "| `{name}` | | | | | |")?;
            continue;
        }
        for (index, layer) in step.layers.iter().enumerate() {
            writeln!(
                output,
                "| `{name}` | {index} | {} | {} | {} | `{}` |",
                layer.y,
                layer.blur,
                layer.spread,
                format_color(layer.color)
            )?;
        }
    }

    output.push_str("\n### Layers\n\n| Layer | Z index |\n|---|---:|\n");
    for layer in Layer::ALL {
        writeln!(output, "| `{layer:?}` | {} |", tokens.z_index(layer))?;
    }

    output.push_str("\n### Motion\n\n| Duration | Milliseconds |\n|---|---:|\n");
    for (name, value) in [
        ("instant", tokens.motion.duration_ms.instant),
        ("quick", tokens.motion.duration_ms.quick),
        ("exit", tokens.motion.duration_ms.exit),
        ("menu", tokens.motion.duration_ms.menu),
        ("dialog", tokens.motion.duration_ms.dialog),
        ("resize", tokens.motion.duration_ms.resize),
        ("entrance", tokens.motion.duration_ms.entrance),
        ("spin", tokens.motion.duration_ms.spin),
        ("slow", tokens.motion.duration_ms.slow),
        ("staggerStep", tokens.motion.duration_ms.stagger_step),
        ("microBounce", tokens.motion.duration_ms.micro_bounce),
        ("microWobble", tokens.motion.duration_ms.micro_wobble),
        ("microPop", tokens.motion.duration_ms.micro_pop),
        ("pulse", tokens.motion.duration_ms.pulse),
        ("shimmer", tokens.motion.duration_ms.shimmer),
        ("toast", tokens.motion.duration_ms.toast),
        ("hoverCardOpen", tokens.motion.duration_ms.hover_card_open),
        ("hoverCardGrace", tokens.motion.duration_ms.hover_card_grace),
        ("feedback", tokens.motion.duration_ms.feedback),
        ("celebration", tokens.motion.duration_ms.celebration),
        ("confirmation", tokens.motion.duration_ms.confirmation),
    ] {
        writeln!(output, "| `motion.durationMs.{name}` | {value} |")?;
    }
    writeln!(
        output,
        "\nRow stagger maximum items: `{}`.",
        tokens.motion.stagger_max_items
    )?;
    output.push_str("\n| Easing | Curve |\n|---|---|\n");
    for easing in MotionEasing::ALL {
        writeln!(
            output,
            "| `{}` | `{:?}` |",
            easing.name(),
            tokens.easing(easing)
        )?;
    }
    output.push_str("\n| Spring | Stiffness | Damping | Mass |\n|---|---:|---:|---:|\n");
    for preset in SpringPreset::ALL {
        let spring = tokens.spring(preset);
        writeln!(
            output,
            "| `{}` | {} | {} | {} |",
            preset.name(),
            spring.stiffness,
            spring.damping,
            spring.mass
        )?;
    }
    output.push_str("\n| Response | Pixels |\n|---|---:|\n");
    for (name, value) in [
        ("motion.pressOffsetPx", tokens.press_offset()),
        ("motion.hoverLiftPx", tokens.hover_lift()),
    ] {
        writeln!(output, "| `{name}` | {value} |")?;
    }

    output.push_str("\n| Gesture | Value |\n|---|---:|\n");
    for (name, value) in [
        ("motion.flickVelocityPxPerSec", tokens.flick_velocity()),
        ("motion.rubberBandTension", tokens.rubber_band_tension()),
    ] {
        writeln!(output, "| `{name}` | {value} |")?;
    }

    output.push_str("\n### Border and opacity\n\n| Token | Value |\n|---|---:|\n");
    for (name, value) in [
        (
            "border.hairline",
            tokens.border_width(BorderWeight::Hairline),
        ),
        ("border.thick", tokens.border_width(BorderWeight::Thick)),
        ("opacity.disabled", tokens.opacity(OpacityRole::Disabled)),
        ("opacity.muted", tokens.opacity(OpacityRole::Muted)),
        ("opacity.scrim", tokens.opacity(OpacityRole::Scrim)),
    ] {
        writeln!(output, "| `{name}` | {value} |")?;
    }

    output.push_str("\n### Effects\n\n| Token | Value |\n|---|---:|\n");
    for (name, value) in [
        ("effect.edgeFadeBand", tokens.effect.edge_fade_band),
        (
            "effect.selectedRingAlpha",
            tokens.effect.selected_ring_alpha,
        ),
        (
            "effect.selectionRailWidth",
            tokens.effect.selection_rail_width,
        ),
        ("effect.focusRingWidth", tokens.effect.focus_ring_width),
        ("effect.focusRingAlpha", tokens.effect.focus_ring_alpha),
        ("effect.glowAlpha", tokens.effect.glow_alpha),
        ("effect.glowBlur", tokens.effect.glow_blur),
        ("effect.glowSpread", tokens.effect.glow_spread),
        ("effect.glassAlpha", tokens.effect.glass_alpha),
        ("effect.glassFrostBlur", tokens.effect.glass_frost_blur),
        ("effect.glassBevelRatio", tokens.effect.glass_bevel_ratio),
        ("effect.glassBevelMin", tokens.effect.glass_bevel_min),
        ("effect.glassBevelMax", tokens.effect.glass_bevel_max),
        ("effect.glassRefraction", tokens.effect.glass_refraction),
        ("effect.glassDispersion", tokens.effect.glass_dispersion),
        ("effect.glassSpecular", tokens.effect.glass_specular),
        (
            "effect.glassTransmissionGain",
            tokens.effect.glass_transmission_gain,
        ),
        ("effect.glassOpticalLift", tokens.effect.glass_optical_lift),
        ("effect.glassHairline", tokens.effect.glass_hairline),
        (
            "effect.glassSpecularSharpness",
            tokens.effect.glass_specular_sharpness,
        ),
        ("effect.glassLightAngle", tokens.effect.glass_light_angle),
        (
            "effect.glassMergeDistance",
            tokens.effect.glass_merge_distance,
        ),
        (
            "effect.glassContrastFlipLow",
            tokens.effect.glass_contrast_flip_low,
        ),
        (
            "effect.glassContrastFlipHigh",
            tokens.effect.glass_contrast_flip_high,
        ),
        ("effect.glassPressDepth", tokens.effect.glass_press_depth),
        ("effect.sheenAlpha", tokens.effect.sheen_alpha),
        ("effect.areaWashAlpha", tokens.effect.area_wash_alpha),
        ("effect.headerTintAlpha", tokens.effect.header_tint_alpha),
        (
            "effect.nodeActiveWashAlpha",
            tokens.effect.node_active_wash_alpha,
        ),
        (
            "effect.nodeActiveStrokeAlpha",
            tokens.effect.node_active_stroke_alpha,
        ),
        ("effect.nodeTrafficAlpha", tokens.effect.node_traffic_alpha),
        ("effect.nodePreviewAlpha", tokens.effect.node_preview_alpha),
        ("effect.nodeMinimapAlpha", tokens.effect.node_minimap_alpha),
        (
            "effect.nodeOverviewVeilAlpha",
            tokens.effect.node_overview_veil_alpha,
        ),
        ("effect.railWidth", tokens.effect.rail_width),
        (
            "effect.semanticWashFaintAlpha",
            tokens.effect.semantic_wash_faint_alpha,
        ),
        (
            "effect.semanticWashAlpha",
            tokens.effect.semantic_wash_alpha,
        ),
        (
            "effect.semanticWashStrongAlpha",
            tokens.effect.semantic_wash_strong_alpha,
        ),
        (
            "effect.semanticBorderAlpha",
            tokens.effect.semantic_border_alpha,
        ),
        (
            "effect.accentBorderAlpha",
            tokens.effect.accent_border_alpha,
        ),
        (
            "effect.accentBorderStrongAlpha",
            tokens.effect.accent_border_strong_alpha,
        ),
        ("effect.subtleHoverAlpha", tokens.effect.subtle_hover_alpha),
        (
            "effect.softContrastAlpha",
            tokens.effect.soft_contrast_alpha,
        ),
        (
            "effect.contrastTintAlpha",
            tokens.effect.contrast_tint_alpha,
        ),
        (
            "effect.trackRestingAlpha",
            tokens.effect.track_resting_alpha,
        ),
        ("effect.contentVeilAlpha", tokens.effect.content_veil_alpha),
        (
            "effect.criticalFillAlpha",
            tokens.effect.critical_fill_alpha,
        ),
        (
            "effect.criticalInactiveAlpha",
            tokens.effect.critical_inactive_alpha,
        ),
        (
            "effect.variantLightAlpha",
            tokens.effect.variant_light_alpha,
        ),
        (
            "effect.variantLightHoverAlpha",
            tokens.effect.variant_light_hover_alpha,
        ),
        (
            "effect.variantLightActiveAlpha",
            tokens.effect.variant_light_active_alpha,
        ),
        (
            "effect.variantOutlineHoverAlpha",
            tokens.effect.variant_outline_hover_alpha,
        ),
        (
            "effect.variantOutlineActiveAlpha",
            tokens.effect.variant_outline_active_alpha,
        ),
        (
            "effect.variantSubtleHoverAlpha",
            tokens.effect.variant_subtle_hover_alpha,
        ),
        (
            "effect.variantSubtleActiveAlpha",
            tokens.effect.variant_subtle_active_alpha,
        ),
        (
            "effect.primaryHoverOpacity",
            tokens.effect.primary_hover_opacity,
        ),
        (
            "effect.customColorReadableDarkFloor",
            tokens.effect.custom_color_readable_dark_floor,
        ),
        (
            "effect.customColorReadableLightCeiling",
            tokens.effect.custom_color_readable_light_ceiling,
        ),
        (
            "effect.customColorHoverLightnessDelta",
            tokens.effect.custom_color_hover_lightness_delta,
        ),
        (
            "effect.customColorActiveLightnessDelta",
            tokens.effect.custom_color_active_lightness_delta,
        ),
    ] {
        writeln!(output, "| `{name}` | {value} |")?;
    }

    output.push_str(
        "\n### Contrast\n\n| Foreground | Background | Ratio | Minimum |\n|---|---|---:|---:|\n",
    );
    for check in contrast::report(tokens) {
        writeln!(
            output,
            "| `{}` | `{}` | {:.2} | {:.1} |",
            check.foreground, check.background, check.ratio, check.minimum
        )?;
    }

    output.push_str(
        "\n### Surface separation\n\n\
         How far each surface reads from the one behind it, in CIE L\\*. The WCAG ratio \
         cannot answer this: it flattens everything near black, so two surfaces nobody \
         can tell apart and two that are plainly different both report about 1.03:1.\n\n\
         | Surface | Behind | Distance | Minimum |\n|---|---|---:|---:|\n",
    );
    for check in contrast::separation_report(tokens) {
        writeln!(
            output,
            "| `{}` | `{}` | {:.1} | {:.1} |",
            check.near, check.behind, check.distance, check.minimum
        )?;
    }

    output.push_str(
        "\n### Tone distinction\n\n\
         How much further from the page each tone stands than the next one down, \
         in CIE L\\*. Contrast alone cannot answer this: three tones that are all \
         legible and all the same colour pass every ratio in the table above and \
         still leave a reader unable to tell a value that is merely secondary from \
         one that is absent from one that cannot be used.\n\n\
         | Tone | Above | Distance | Minimum |\n|---|---|---:|---:|\n",
    );
    for check in contrast::distinction_report(tokens) {
        writeln!(
            output,
            "| `{}` | `{}` | {:.1} | {:.1} |",
            check.tone, check.against, check.distance, check.minimum
        )?;
    }
    Ok(())
}

fn hex(tokens: &TokenDocument, source: &str) -> String {
    let color = gpui_kit_tokens::Color::resolve("reference", source, &tokens.color.palette)
        .expect("the document is validated before it is documented");
    format_color(color)
}

fn format_color(color: gpui_kit_tokens::Color) -> String {
    let channel = |value: f32| (value * 255.0).round() as u8;
    let rgb = format!(
        "#{:02x}{:02x}{:02x}",
        channel(color.red),
        channel(color.green),
        channel(color.blue)
    );
    if color.alpha >= 1.0 {
        rgb
    } else {
        format!("{rgb}{:02x}", channel(color.alpha))
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives under the repository root")
        .to_path_buf()
}

#[cfg(test)]
mod generated_text_tests {
    use super::same_generated_text;

    #[test]
    fn accepts_git_materialized_crlf() {
        assert!(same_generated_text("one\r\ntwo\r\n", "one\ntwo\n"));
    }

    #[test]
    fn rejects_content_changes_and_lone_carriage_returns() {
        assert!(!same_generated_text("one\rchanged\n", "one\ntwo\n"));
        assert!(!same_generated_text("one\rtwo\n", "one\ntwo\n"));
    }
}
