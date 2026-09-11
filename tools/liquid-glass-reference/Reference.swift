import AppKit
import SwiftUI
import ScreenCaptureKit

// Independently authored public-API fixture; no private framework or bitmap renderer.
@MainActor final class State: ObservableObject {
    @Published var visible = false
    @Published var expanded = false
    var actionAcknowledgements = [Double]()
}

struct Pill: Identifiable {
    let id: String
    let rect: [Double]
    let material: String
    let label: String
    init(_ data: [String: Any]) {
        id = data["id"] as! String
        rect = data["rect"] as! [Double]
        material = data["material"] as! String
        label = data["label"] as! String
    }
}

@MainActor struct Reference: View {
    @ObservedObject var state: State
    let fixture: [String: Any]
    @Namespace private var namespace
    func color(_ rgb: [Int]) -> Color {
        Color(.sRGB, red: Double(rgb[0])/255, green: Double(rgb[1])/255,
              blue: Double(rgb[2])/255, opacity: 1)
    }
    var body: some View {
        ZStack(alignment: .topLeading) {
            Canvas { context, _ in
                for y in 0..<20 { for x in 0..<30 {
                    let rect = CGRect(x: CGFloat(x*32), y: CGFloat(y*32), width: 32, height: 32)
                    context.fill(Path(rect), with: .color(color((x+y)%2 == 0 ? [217,230,242] : [242,204,153])))
                    context.stroke(Path(rect), with: .color(color([128,144,160])), lineWidth: 1)
                }}
                context.draw(Text("Glass reference 012345").font(.system(size: 12, design: .monospaced)).foregroundColor(.black), at: CGPoint(x: 480, y: 608))
                for marker in fixture["fiducials"] as! [[String: Any]] {
                    let r = marker["rect"] as! [Int]
                    context.fill(Path(CGRect(x: CGFloat(r[0]), y: CGFloat(r[1]), width: CGFloat(r[2]), height: CGFloat(r[3]))), with: .color(color(marker["rgb"] as! [Int])))
                }
            }
            if state.visible {
                ForEach((fixture["pills"] as! [[String: Any]]).map(Pill.init)) { pill in
                    let r = pill.rect
                    let kind = pill.material
                    Text(pill.label)
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(kind == "clear" ? Color.white : Color.primary)
                        .frame(width: r[2], height: r[3])
                        .glassEffect(kind == "clear" ? .clear : kind == "tinted" ? .regular.tint(.orange).interactive() : .regular, in: Capsule())
                        .background { if kind == "clear" { Capsule().fill(.black.opacity(0.3)) } }
                        .offset(x: r[0], y: r[1])
                        .accessibilityIdentifier(pill.id)
                }
                fusion(gap: 12).offset(x: 64, y: 304)
                fusion(gap: 64).offset(x: 448, y: 304)
                GlassEffectContainer(spacing: 32) {
                    ZStack {
                        if state.expanded {
                            VStack(alignment: .leading, spacing: 12) {
                                Text("Copy"); Text("Share"); Text("Delete")
                            }
                            .accessibilityIdentifier("expanded-menu")
                        } else {
                            Button("Actions") {
                                state.actionAcknowledgements.append(ProcessInfo.processInfo.systemUptime)
                                withAnimation(.linear(duration: 0.8)) { state.expanded = true }
                            }
                                .buttonStyle(.plain)
                                .accessibilityIdentifier("actions-button")
                        }
                    }
                    .frame(width: state.expanded ? 272 : 144, height: state.expanded ? 128 : 48)
                    .clipShape(.rect(cornerRadius: 24))
                    .glassEffect(.regular.interactive(), in: .rect(cornerRadius: 24))
                    .glassEffectID("menu", in: namespace)
                }.frame(width: 272, height: 128, alignment: .topLeading).offset(x: 64, y: 448)
            }
        }.frame(width: 960, height: 640).clipped()
    }
    func fusion(gap: CGFloat) -> some View {
        GlassEffectContainer(spacing: 32) {
            HStack(spacing: gap) {
                Text("Pane A").frame(width: 128, height: 72).glassEffect(.regular, in: .rect(cornerRadius: 20))
                Text("Pane B").frame(width: 128, height: 72).glassEffect(.regular, in: .rect(cornerRadius: 20))
            }
        }
    }
}

final class ReferenceWindow: NSWindow {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }
    var pointerReceipts = [[String: Any]]()
    override func sendEvent(_ event: NSEvent) {
        if event.type == .leftMouseDown || event.type == .leftMouseUp {
            pointerReceipts.append(["type": event.type == .leftMouseDown ? "down" : "up",
                                    "event_number": event.eventNumber, "event_uptime": event.timestamp,
                                    "received_uptime": ProcessInfo.processInfo.systemUptime])
        }
        super.sendEvent(event)
    }
}

@main struct Main {
    @MainActor static func main() {
        let app = NSApplication.shared
        app.setActivationPolicy(.regular)
        Task { @MainActor in
            do { try await run(); app.terminate(nil) } catch {
                fputs("Native reference refused: \(error)\n", stderr)
                exit(1)
            }
        }
        app.run()
    }
    @MainActor static func run() async throws {
        let args = CommandLine.arguments
        let out = URL(fileURLWithPath: args[1], isDirectory: true)
        let fixture = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: args[2]))) as! [String: Any]
        func refuse(_ message: String) throws { throw NSError(domain: message, code: 1) }
        guard ProcessInfo.processInfo.operatingSystemVersion.majorVersion >= 26 else { try refuse("macOS 26 required"); return }
        guard CGPreflightScreenCaptureAccess() else { try refuse("Screen Recording permission required; grant to app in System Settings then rerun (no prompt in CI)"); return }
        let workspace = NSWorkspace.shared
        let accessibility: [String: Bool] = [
            "reduce_transparency": workspace.accessibilityDisplayShouldReduceTransparency,
            "reduce_motion": workspace.accessibilityDisplayShouldReduceMotion,
            "increase_contrast": workspace.accessibilityDisplayShouldIncreaseContrast,
            "differentiate_without_color": workspace.accessibilityDisplayShouldDifferentiateWithoutColor,
            "invert_colors": workspace.accessibilityDisplayShouldInvertColors]
        guard !accessibility.values.contains(true) else { try refuse("Nonstandard accessibility display settings: \(accessibility)"); return }
        let app = NSApplication.shared
        let state = State()
        let window = ReferenceWindow(contentRect: NSRect(x: 0, y: 0, width: 960, height: 640), styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.title = "GPUI Box native Liquid Glass reference"
        window.contentView = NSHostingView(rootView: Reference(state: state, fixture: fixture))
        window.center()
        window.makeKeyAndOrderFront(nil)
        app.activate(ignoringOtherApps: true)
        try await Task.sleep(for: .seconds(1))
        let content = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
        guard let target = content.windows.first(where: { $0.windowID == CGWindowID(window.windowNumber) }) else { try refuse("Composited window unavailable"); return }
        let filter = SCContentFilter(desktopIndependentWindow: target)
        guard filter.contentRect.size == CGSize(width: 960, height: 640), window.isKeyWindow else { try refuse("Window geometry or active appearance unavailable"); return }
        let scale = Double(filter.pointPixelScale)
        let configuration = SCStreamConfiguration()
        configuration.width = Int(960 * scale)
        configuration.height = Int(640 * scale)
        configuration.showsCursor = false
        configuration.ignoreShadowsSingleWindow = true
        var frames = [[String: Any]]()
        let clockOrigin = ProcessInfo.processInfo.systemUptime
        func capture(_ appearance: String, _ phase: String, _ index: Int, _ trigger: Double?) async throws {
            let expected: NSAppearance.Name = appearance == "light" ? .aqua : .darkAqua
            guard window.isKeyWindow, window.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == expected else { try refuse("Active window appearance changed during capture"); return }
            let start = ProcessInfo.processInfo.systemUptime - clockOrigin
            let image = try await SCScreenshotManager.captureImage(contentFilter: filter, configuration: configuration)
            let end = ProcessInfo.processInfo.systemUptime - clockOrigin
            let wallEnd = ISO8601DateFormatter().string(from: Date())
            let name = "\(appearance)-\(phase)-\(index)"
            let bitmap = NSBitmapImageRep(cgImage: image)
            guard let png = bitmap.representation(using: .png, properties: [:]) else { try refuse("PNG conversion failed"); return }
            try png.write(to: out.appendingPathComponent(name + ".png"))
            // A lossless sRGB RGB sidecar of the actual compositor capture for stdlib validation.
            var rgba = [UInt8](repeating: 0, count: image.width * image.height * 4)
            guard let space = CGColorSpace(name: CGColorSpace.sRGB) else { try refuse("sRGB unavailable"); return }
            let ok = rgba.withUnsafeMutableBytes { bytes -> Bool in
                guard let ctx = CGContext(data: bytes.baseAddress, width: image.width, height: image.height, bitsPerComponent: 8, bytesPerRow: image.width*4, space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
                ctx.draw(image, in: CGRect(x: 0, y: 0, width: CGFloat(image.width), height: CGFloat(image.height)))
                return true
            }
            guard ok else { try refuse("Capture conversion unavailable"); return }
            var ppm = Data("P6\n\(image.width) \(image.height)\n255\n".utf8)
            var rgb = [UInt8](repeating: 0, count: image.width * image.height * 3)
            for pixel in 0..<(image.width * image.height) {
                rgb[pixel*3] = rgba[pixel*4]
                rgb[pixel*3+1] = rgba[pixel*4+1]
                rgb[pixel*3+2] = rgba[pixel*4+2]
            }
            ppm.append(contentsOf: rgb)
            try ppm.write(to: out.appendingPathComponent(name + ".ppm"))
            var row: [String: Any] = ["file": name + ".png", "rgb_file": name + ".ppm", "appearance": appearance, "phase": phase, "index": index, "pixel_size": [image.width, image.height], "capture_start": start, "capture_end": end, "wall_end": wallEnd, "rgb_color_space": "sRGB", "capture_color_space": image.colorSpace?.name as String? ?? "unnamed"]
            if let trigger { row["trigger_time"] = trigger }
            frames.append(row)
        }
        for appearance in ["light", "dark"] {
            window.appearance = NSAppearance(named: appearance == "light" ? .aqua : .darkAqua)
            state.visible = false; state.expanded = false
            try await Task.sleep(for: .seconds(0.6))
            try await capture(appearance, "background", 0, nil)
            state.visible = true
            try await Task.sleep(for: .seconds(0.8))
            try await capture(appearance, "static", 0, nil)
            let trigger = ProcessInfo.processInfo.systemUptime - clockOrigin
            withAnimation(.linear(duration: 0.8)) { state.expanded = true }
            for index in 0..<16 {
                try await capture(appearance, "transition", index, trigger)
                try await Task.sleep(for: .seconds(0.08))
            }
        }
        let manifest: [String: Any] = ["schema": 1, "capture_backend": "ScreenCaptureKit.SCScreenshotManager.desktopIndependentWindow", "clock": "systemUptime capture request/completion brackets; not presentation timestamps", "logical_size": [960,640], "scale": scale, "accessibility": accessibility, "interaction": "programmatic-state-change; no pointer dispatched", "frames": frames]
        try JSONSerialization.data(withJSONObject: manifest, options: [.prettyPrinted, .sortedKeys]).write(to: out.appendingPathComponent("native.json"))
        // Separate experiment: native event queue input, never direct action invocation.
        frames.removeAll()
        var pointerRuns = [[String: Any]]()
        for appearance in ["light", "dark"] {
            window.appearance = NSAppearance(named: appearance == "light" ? .aqua : .darkAqua)
            state.expanded = false
            state.actionAcknowledgements.removeAll()
            window.pointerReceipts.removeAll()
            try await Task.sleep(for: .seconds(1))
            try await capture(appearance, "pointer-before", 0, nil)
            var dispatches = [[String: Any]]()
            func dispatch(_ type: NSEvent.EventType, _ number: Int) throws {
                let time = ProcessInfo.processInfo.systemUptime
                guard let event = NSEvent.mouseEvent(with: type, location: NSPoint(x: 136, y: 168), modifierFlags: [], timestamp: time, windowNumber: window.windowNumber, context: nil, eventNumber: number, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0) else { try refuse("Mouse event construction failed"); return }
                dispatches.append(["type": type == .leftMouseDown ? "down" : "up", "event_number": number, "event_uptime": time, "dispatch_uptime": ProcessInfo.processInfo.systemUptime])
                app.postEvent(event, atStart: false)
            }
            try dispatch(.leftMouseDown, 101)
            for index in 0..<3 {
                try await Task.sleep(for: .seconds(0.08))
                try await capture(appearance, "pointer-held", index, nil)
            }
            guard state.actionAcknowledgements.isEmpty else { try refuse("Action fired before release"); return }
            try dispatch(.leftMouseUp, 102)
            for index in 0..<16 {
                try await Task.sleep(for: .seconds(0.04))
                try await capture(appearance, "pointer-release", index, nil)
            }
            let run: [String: Any] = ["appearance": appearance, "dispatches": dispatches, "window_receipts": window.pointerReceipts, "action_acknowledgements_uptime": state.actionAcknowledgements, "expanded": state.expanded]
            pointerRuns.append(run)
            guard window.pointerReceipts.map({ $0["event_number"] as? Int }) == [101, 102],
                  window.pointerReceipts.map({ $0["type"] as? String }) == ["down", "up"],
                  state.actionAcknowledgements.count == 1, state.expanded,
                  state.actionAcknowledgements[0] >= (window.pointerReceipts[1]["received_uptime"] as! Double) else {
                try JSONSerialization.data(withJSONObject: pointerRuns, options: [.prettyPrinted, .sortedKeys]).write(to: out.appendingPathComponent("pointer-failure.json"))
                try refuse("Native pointer dispatch not acknowledged exactly once"); return
            }
        }
        let pointer: [String: Any] = ["schema": 1, "interaction": "synthetic NSEvent via NSApplication.postEvent; native hit testing and SwiftUI Button action; not hardware input", "clock_origin_uptime": clockOrigin, "point_top_left": [136,472], "point_window_bottom_left": [136,168], "runs": pointerRuns, "frames": frames]
        try JSONSerialization.data(withJSONObject: pointer, options: [.prettyPrinted, .sortedKeys]).write(to: out.appendingPathComponent("pointer-native.json"))
        window.close()
    }
}
