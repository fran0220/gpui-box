use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(target_os = "macos")]
use std::thread;
#[cfg(target_os = "macos")]
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

mod api;
mod dependencies;
mod site;
mod strings;
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
        (Some("api"), Some("check")) => api::check(&root()),
        (Some("api"), Some("generate")) => api::generate(&root()),
        (Some("dependencies"), Some("check")) => dependencies::check(&root(), &rest),
        (Some("site"), Some("generate")) => {
            site::generate(&root(), rest.first().map(String::as_str)).map(|_| ())
        }
        (Some("site"), Some("check")) => site::check(&root()),
        (Some("accessibility"), Some("check")) => accessibility_check(),
        (Some("scenes"), Some("list")) => scenes_list(),
        (Some("scenes"), Some("capture")) => scenes_capture(&rest),
        (Some("scenes"), Some("check")) => scenes_check(&rest),
        (Some("headless"), Some("capture")) => headless("capture", &rest),
        (Some("headless"), Some("check")) => headless("check", &rest),
        (Some("web"), Some("check")) => web_check(),
        (Some("web"), Some("build")) => web_build(),
        (Some("web"), Some("smoke")) => web_smoke(),
        (Some("web"), Some("visual")) => web_visual(&rest),
        (Some("gate"), None) => gate(false),
        (Some("gate"), Some("full")) => gate(true),
        _ => bail!(
            "usage: cargo xtask <dependencies check|accessibility check|tokens generate|tokens check|strings check|\
             strings generate|scenes list|scenes capture [name...]|\
             scenes check [name...]|headless capture [name...]|\
             headless check [name...]|web check|web build|web smoke|\
             web visual <capture|check> [name...]|gate [full]>"
        ),
    }
}

#[cfg(target_os = "macos")]
fn accessibility_check() -> Result<()> {
    step("cargo", &["build", "-p", "gpui-kit-gallery"], None)?;
    let executable = root().join("target/debug/gpui-kit-gallery");
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
    let executable = root().join("target/debug/gpui-kit-gallery");
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
        ] {
            if !output.lines().any(|line| line.trim() == expected) {
                bail!("macOS editable AX tree is missing `{expected}`; received:\n{output}");
            }
        }
        println!(
            "macOS AX editable names, values, enabled state, requested focus, and editing match"
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
fn dialog_accessibility_check() -> Result<()> {
    let executable = root().join("target/debug/gpui-kit-gallery");
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
    let executable = root().join("target/debug/gpui-kit-gallery");
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
    let executable = root().join("target/debug/gpui-kit-gallery");
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
    let executable = root().join("target/debug/gpui-kit-gallery");
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

#[cfg(target_os = "macos")]
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

#[cfg(not(target_os = "macos"))]
fn accessibility_check() -> Result<()> {
    bail!("the native accessibility smoke check currently requires macOS")
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

/// Renders scenes in every bundled theme to reviewable images.
///
/// Naming scenes captures only those, which is what a change to one component
/// needs. Naming none captures the catalog.
fn scenes_capture(only: &[String]) -> Result<()> {
    let directory = snapshots();
    let count = capture_into(&directory, only)?;
    println!("captured {count} images into {}", directory.display());
    Ok(())
}

/// Captures into a scratch directory and reports every image that differs from
/// the committed one.
///
/// This is the visual regression gate. It only means anything because captures
/// are deterministic: the gallery reads frames straight back from the GPU and
/// renders with reduced motion, so neither compositing nor an animation phase
/// leaks into a file. On one machine two runs agree to the byte.
///
/// Images are still compared as pixels rather than as bytes, because another
/// machine's GPU or OS may land an antialiased edge one channel step away,
/// and a gate that cried about a difference nobody can see would be a gate
/// nobody reads.
fn scenes_check(only: &[String]) -> Result<()> {
    let committed = snapshots();
    let scratch = root().join("target").join("scene-check");
    if scratch.exists() {
        fs::remove_dir_all(&scratch).with_context(|| format!("clear {}", scratch.display()))?;
    }
    let count = capture_into(&scratch, only)?;

    let mut differing = Vec::new();
    let mut missing = Vec::new();
    for entry in fs::read_dir(&scratch)? {
        let entry = entry?;
        let name = entry.file_name();
        let old = committed.join(&name);
        if !old.exists() {
            missing.push(name.to_string_lossy().into_owned());
            continue;
        }
        let apart = distance(&old, &entry.path())
            .with_context(|| format!("compare {}", name.to_string_lossy()))?;
        if apart > NOISE {
            differing.push((name.to_string_lossy().into_owned(), apart));
        }
    }
    differing.sort();
    missing.sort();

    if differing.is_empty() && missing.is_empty() {
        println!("{count} images match {}", committed.display());
        return Ok(());
    }
    for name in &missing {
        println!("new     {name}");
    }
    for (name, apart) in &differing {
        println!("changed {name} (by {apart})");
    }
    bail!(
        "{} changed and {} new image(s) under {}; review them, then run \
         `cargo run -p xtask -- scenes capture` to accept",
        differing.len(),
        missing.len(),
        scratch.display()
    );
}

/// Runs the checks a change has to pass.
///
/// The short form is what a work-in-progress change wants: it answers in about
/// a minute. The full form adds the two slow proofs, rendered documentation
/// and the visual regression, and is what a commit wants.
fn gate(full: bool) -> Result<()> {
    step("cargo", &["fmt", "--all", "--", "--check"], None)?;
    dependencies::check(&root(), &[])?;
    step("cargo", &["check", "--workspace", "--all-targets"], None)?;
    step("cargo", &["test", "--workspace"], None)?;
    step(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
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
    if full {
        step(
            "cargo",
            &["doc", "--no-deps", "--workspace"],
            Some(("RUSTDOCFLAGS", "-D warnings")),
        )?;
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        headless("check", &[])?;
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        scenes_check(&[])?;
    }
    println!("gate passed");
    Ok(())
}

/// Runs the Linux and Windows visual gate, which lives in its own workspace
/// with renderer-specific dependencies and an independent lockfile.
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
const WASM_BINDGEN_VERSION: &str = "0.2.126";

fn web_check() -> Result<()> {
    step(
        env!("CARGO"),
        &[
            "check",
            "-p",
            "gpui-kit",
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
    step(
        env!("CARGO"),
        &[
            "build",
            "-p",
            "gpui-kit-browser-gallery",
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
             wasm-bindgen-cli --version 0.2.126 --locked`",
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
    fs::copy(
        root().join("examples/browser-gallery/web/index.html"),
        output.join("index.html"),
    )
    .context("copy the browser gallery host page")?;
    println!("browser gallery built in {}", output.display());
    Ok(())
}

fn web_smoke() -> Result<()> {
    web_build()?;
    let package = root().join("examples/browser-gallery");
    let package = package.to_string_lossy();
    step("npm", &["--prefix", package.as_ref(), "ci"], None)?;
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
    )?;
    let config = root().join("examples/browser-gallery/playwright.config.mjs");
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
}

fn web_visual(args: &[String]) -> Result<()> {
    let Some(command @ ("capture" | "check")) = args.first().map(String::as_str) else {
        bail!("usage: cargo xtask web visual <capture|check> [scene...]");
    };
    web_build()?;
    let package = root().join("examples/browser-gallery");
    let package = package.to_string_lossy();
    step("npm", &["--prefix", package.as_ref(), "ci"], None)?;
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
    )?;

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
    if args.len() > 1 {
        playwright.env("GPUI_KIT_WEB_SCENES", args[1..].join(","));
    }
    let status = playwright.status().context("run browser visual gate")?;
    if !status.success() {
        bail!("browser visual {command} failed");
    }
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

fn snapshots() -> PathBuf {
    root().join("snapshots").join(platform()).join("scenes")
}

/// How far one channel may land from the committed image and still count as
/// the same picture.
///
/// One step is the smallest difference the format can hold, and the renderer
/// does land there occasionally on an antialiased edge. Anything a component
/// actually changed moves further than this, so the tolerance costs no
/// coverage while keeping the gate worth reading.
const NOISE: u8 = 1;

/// The largest per-channel difference between two images.
///
/// Images of different sizes are as far apart as it is possible to be, since
/// there is no pixel to compare a missing pixel against.
fn distance(left: &Path, right: &Path) -> Result<u8> {
    let left = decode(left)?;
    let right = decode(right)?;
    if left.0 != right.0 {
        return Ok(u8::MAX);
    }
    Ok(left
        .1
        .iter()
        .zip(right.1.iter())
        .map(|(left, right)| left.abs_diff(*right))
        .max()
        .unwrap_or(0))
}

/// Decodes a PNG into its dimensions and its pixels.
fn decode(path: &Path) -> Result<((u32, u32), Vec<u8>)> {
    let file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder
        .read_info()
        .with_context(|| format!("read {}", path.display()))?;
    let size = reader
        .output_buffer_size()
        .with_context(|| format!("{} is larger than this machine can hold", path.display()))?;
    let mut pixels = vec![0; size];
    let info = reader
        .next_frame(&mut pixels)
        .with_context(|| format!("decode {}", path.display()))?;
    pixels.truncate(info.buffer_size());
    Ok(((info.width, info.height), pixels))
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
        .args(["run", "--quiet", "-p", "gpui-kit-gallery", "--"])
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

fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

fn tokens(check: bool) -> Result<()> {
    contrast_gate()?;

    let mut output = String::from(
        "<!-- @generated by `cargo xtask tokens generate`; do not edit. -->\n\
         # Token reference\n\n\
         The JSON documents under `crates/gpui-kit-tokens/tokens/` are the authority. These tables are\n\
         a review aid, and every theme below is validated on each run.\n",
    );
    for document in bundled() {
        theme_section(&mut output, document)?;
    }

    let path = root().join("docs/token-reference.md");
    if check {
        let current = fs::read_to_string(&path)
            .with_context(|| format!("read generated {}", path.display()))?;
        if current != output {
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

/// Fails the task rather than the document, so a contrast regression is caught
/// before it can be committed as a generated table.
fn contrast_gate() -> Result<()> {
    let mut failed = false;
    for document in bundled() {
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
        ("color.surface.canvas".into(), color.surface.canvas.as_str()),
        ("color.surface.panel".into(), color.surface.panel.as_str()),
        ("color.surface.raised".into(), color.surface.raised.as_str()),
        (
            "color.surface.overlay".into(),
            color.surface.overlay.as_str(),
        ),
        ("color.text.primary".into(), color.text.primary.as_str()),
        ("color.text.muted".into(), color.text.muted.as_str()),
        ("color.text.faint".into(), color.text.faint.as_str()),
        ("color.text.onAccent".into(), color.text.on_accent.as_str()),
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
            "color.interactive.focus".into(),
            color.interactive.focus.as_str(),
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
            .loader
            .gradient
            .iter()
            .enumerate()
            .map(|(index, value)| (format!("color.loader.gradient.{index}"), value.as_str())),
    )
    .collect();

    for (path, source) in sources {
        writeln!(
            output,
            "| `{path}` | `{source}` | `{}` |",
            hex(tokens, source)
        )?;
    }

    output.push_str("\n### Spacing\n\n| Step | Pixels |\n|---|---:|\n");
    for (name, step) in [
        ("xs", Space::Xs),
        ("sm", Space::Sm),
        ("md", Space::Md),
        ("lg", Space::Lg),
        ("xl", Space::Xl),
        ("xxl", Space::Xxl),
    ] {
        writeln!(output, "| `{name}` | {} |", tokens.spacing(step))?;
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

    output.push_str(
        "\n### Elevation\n\n| Step | Y | Blur | Spread | Color |\n|---|---:|---:|---:|---|\n",
    );
    for (name, level) in [
        ("flat", Elevation::Flat),
        ("raised", Elevation::Raised),
        ("overlay", Elevation::Overlay),
        ("modal", Elevation::Modal),
    ] {
        let step = tokens.elevation(level);
        writeln!(
            output,
            "| `{name}` | {} | {} | {} | `{}` |",
            step.y,
            step.blur,
            step.spread,
            format_color(step.color)
        )?;
    }

    output.push_str("\n### Layers\n\n| Layer | Z index |\n|---|---:|\n");
    for layer in Layer::ALL {
        writeln!(output, "| `{layer:?}` | {} |", tokens.z_index(layer))?;
    }

    output.push_str("\n### Motion\n\n| Easing | Curve |\n|---|---|\n");
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
        ("effect.focusRingWidth", tokens.effect.focus_ring_width),
        ("effect.focusRingAlpha", tokens.effect.focus_ring_alpha),
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
