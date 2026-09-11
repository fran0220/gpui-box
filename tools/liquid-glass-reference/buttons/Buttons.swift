import AppKit
import SwiftUI
import ScreenCaptureKit

@MainActor final class Model: ObservableObject {
    @Published var visible = false
    var layout = [String: [Double]]()
    var actions = [[String: Any]]()
    func action(_ id: String) {
        actions.append(["id": id, "uptime": ProcessInfo.processInfo.systemUptime])
    }
}

@MainActor struct Fixture: View {
    @ObservedObject var model: Model
    func rgb(_ r: Double, _ g: Double, _ b: Double) -> Color {
        Color(.sRGB, red: r/255, green: g/255, blue: b/255, opacity: 1)
    }
    var label: some View {
        Text("Actions").font(.system(size: 15, weight: .semibold)).frame(width: 144, height: 48)
    }
    func measure<V: View>(_ id: String, _ view: V) -> some View {
        view.onGeometryChange(for: CGRect.self) { $0.frame(in: .named("fixture")) } action: { rect in
            model.layout[id] = [rect.minX, rect.minY, rect.width, rect.height]
        }
    }
    var body: some View {
        ZStack(alignment: .topLeading) {
            Canvas { context, _ in
                for y in 0..<17 { for x in 0..<25 {
                    let r = CGRect(x: x*32, y: y*32, width: 32, height: 32)
                    context.fill(Path(r), with: .color((x+y)%2 == 0 ? rgb(217,230,242) : rgb(242,204,153)))
                    context.stroke(Path(r), with: .color(rgb(128,144,160)), lineWidth: 1)
                }}
                for (r, color) in [(CGRect(x:8,y:8,width:24,height:24), rgb(255,0,0)), (CGRect(x:768,y:8,width:24,height:24), rgb(0,255,0)), (CGRect(x:8,y:488,width:24,height:24), rgb(0,0,255))] {
                    context.fill(Path(r), with: .color(color))
                }
            }
            if model.visible {
                measure("glass", Button(action: { model.action("glass") }) { label }.buttonStyle(.glass))
                    .position(x: 400, y: 112)
                measure("glass-prominent", Button(action: { model.action("glass-prominent") }) { label }.buttonStyle(.glassProminent))
                    .position(x: 400, y: 256)
                measure("custom", ZStack {
                    Button("Actions") { model.action("custom") }.font(.system(size: 15, weight: .semibold)).buttonStyle(.plain)
                }.frame(width: 144, height: 48).clipShape(.rect(cornerRadius: 24))
                    .glassEffect(.regular.interactive(), in: .rect(cornerRadius: 24)))
                    .position(x: 400, y: 400)
            }
        }.frame(width: 800, height: 520).coordinateSpace(name: "fixture")
            .controlSize(.regular).clipped()
    }
}

final class FixtureWindow: NSWindow {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }
    var receipts = [[String: Any]]()
    override func sendEvent(_ event: NSEvent) {
        if event.type == .leftMouseDown || event.type == .leftMouseUp {
            receipts.append(["type": event.type == .leftMouseDown ? "down" : "up", "number": event.eventNumber,
                             "event_uptime": event.timestamp, "received_uptime": ProcessInfo.processInfo.systemUptime,
                             "point_bottom_left": [event.locationInWindow.x, event.locationInWindow.y]])
        }
        super.sendEvent(event)
    }
}

@main struct Main {
    @MainActor static func main() {
        let app = NSApplication.shared
        app.setActivationPolicy(.regular)
        Task { @MainActor in
            do { try await run(); app.terminate(nil) }
            catch { fputs("Capture refused: \(error)\n", stderr); exit(1) }
        }
        app.run()
    }
    @MainActor static func run() async throws {
        let out = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
        func require(_ value: Bool, _ reason: String) throws {
            if !value { throw NSError(domain: reason, code: 1) }
        }
        try require(CGPreflightScreenCaptureAccess(), "Screen capture permission required; no permission requested")
        let ws = NSWorkspace.shared
        let settings = ["reduce_motion": ws.accessibilityDisplayShouldReduceMotion,
                        "reduce_transparency": ws.accessibilityDisplayShouldReduceTransparency,
                        "increase_contrast": ws.accessibilityDisplayShouldIncreaseContrast,
                        "invert_colors": ws.accessibilityDisplayShouldInvertColors,
                        "differentiate_without_color": ws.accessibilityDisplayShouldDifferentiateWithoutColor]
        try require(!settings.values.contains(true), "Unsupported accessibility settings")
        let model = Model()
        let window = FixtureWindow(contentRect: NSRect(x:0,y:0,width:800,height:520), styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.title = "Native glass button style experiment"
        window.contentView = NSHostingView(rootView: Fixture(model: model))
        window.center(); window.makeKeyAndOrderFront(nil); NSApplication.shared.activate(ignoringOtherApps: true)
        try await Task.sleep(for: .seconds(1))
        let available = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
        guard let target = available.windows.first(where: { $0.windowID == CGWindowID(window.windowNumber) }) else {
            try require(false, "Window unavailable"); return
        }
        let filter = SCContentFilter(desktopIndependentWindow: target)
        try require(filter.contentRect.size == CGSize(width:800,height:520), "Wrong window bounds")
        let scale = Double(filter.pointPixelScale)
        let config = SCStreamConfiguration()
        config.width = Int(800*scale); config.height = Int(520*scale)
        config.showsCursor = false; config.ignoreShadowsSingleWindow = true
        let origin = ProcessInfo.processInfo.systemUptime
        var frames = [[String: Any]]()
        var runs = [[String: Any]]()
        func save() throws {
            let data: [String: Any] = ["schema": "native-button-styles-evidence-1", "backend": "ScreenCaptureKit.SCScreenshotManager.desktopIndependentWindow",
                "logical_size": [800,520], "scale": scale, "clock_origin_uptime": origin, "settings": settings,
                "input": "synthetic NSEvent via NSApplication.postEvent; no HID or global cursor movement",
                "layout_bounds": model.layout, "frames": frames, "runs": runs]
            try JSONSerialization.data(withJSONObject: data, options: [.prettyPrinted,.sortedKeys]).write(to: out.appendingPathComponent("native.json"))
        }
        func capture(_ appearance: String, _ id: String, _ phase: String, _ index: Int) async throws {
            try require(window.isKeyWindow, "Window lost key status")
            try require(window.effectiveAppearance.bestMatch(from: [.aqua,.darkAqua]) == (appearance == "light" ? .aqua : .darkAqua), "Appearance changed")
            let start = ProcessInfo.processInfo.systemUptime-origin
            let image = try await SCScreenshotManager.captureImage(contentFilter: filter, configuration: config)
            let end = ProcessInfo.processInfo.systemUptime-origin
            let name = "\(appearance)-\(id)-\(phase)-\(index)"
            guard let png = NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:]) else {
                try require(false, "PNG encoding failed"); return
            }
            try png.write(to: out.appendingPathComponent(name+".png"))
            var rgba = [UInt8](repeating: 0, count: image.width*image.height*4)
            let ok = rgba.withUnsafeMutableBytes { bytes -> Bool in
                guard let ctx = CGContext(data: bytes.baseAddress, width: image.width, height: image.height, bitsPerComponent:8, bytesPerRow:image.width*4, space:CGColorSpace(name:CGColorSpace.sRGB)!, bitmapInfo:CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
                ctx.draw(image, in: CGRect(x:0,y:0,width:image.width,height:image.height)); return true
            }
            try require(ok, "RGB conversion failed")
            var rgb = [UInt8](repeating:0, count:image.width*image.height*3)
            for i in 0..<(image.width*image.height) { for c in 0..<3 { rgb[i*3+c] = rgba[i*4+c] } }
            var ppm = Data("P6\n\(image.width) \(image.height)\n255\n".utf8); ppm.append(contentsOf: rgb)
            try ppm.write(to: out.appendingPathComponent(name+".ppm"))
            frames.append(["appearance":appearance,"case":id,"phase":phase,"index":index,"png":name+".png","ppm":name+".ppm",
                           "pixel_size":[image.width,image.height],"capture_start":start,"capture_end":end,"wall_end":ISO8601DateFormatter().string(from:Date())])
            try save()
        }
        for appearance in ["light","dark"] {
            window.appearance = NSAppearance(named: appearance == "light" ? .aqua : .darkAqua)
            model.visible = false; try await Task.sleep(for:.seconds(0.6))
            try await capture(appearance,"all","background",0)
            model.visible = true; try await Task.sleep(for:.seconds(1))
            for (id, y) in [("glass",112.0),("glass-prominent",256.0),("custom",400.0)] {
                model.actions.removeAll(); window.receipts.removeAll()
                try await Task.sleep(for:.seconds(0.5))
                try await capture(appearance,id,"before",0)
                var dispatches = [[String:Any]]()
                func dispatch(_ type: NSEvent.EventType, _ number: Int) throws {
                    let time = ProcessInfo.processInfo.systemUptime
                    guard let e = NSEvent.mouseEvent(with:type,location:NSPoint(x:400,y:520-y),modifierFlags:[],timestamp:time,windowNumber:window.windowNumber,context:nil,eventNumber:number,clickCount:1,pressure:type == .leftMouseDown ? 1 : 0) else {
                        try require(false,"Event unavailable"); return
                    }
                    dispatches.append(["type":type == .leftMouseDown ? "down":"up","number":number,"event_uptime":time,"dispatch_uptime":ProcessInfo.processInfo.systemUptime])
                    NSApplication.shared.postEvent(e,atStart:false)
                }
                try dispatch(.leftMouseDown,101)
                for i in 0..<4 {
                    try await Task.sleep(for:.seconds(i == 0 ? 0.02 : 0.06)); try await capture(appearance,id,"held",i)
                }
                try require(model.actions.isEmpty,"Action fired before release")
                try dispatch(.leftMouseUp,102)
                for i in 0..<9 {
                    try await Task.sleep(for:.seconds(i == 8 ? 1 : i == 0 ? 0.02 : 0.06)); try await capture(appearance,id,"release",i)
                }
                runs.append(["appearance":appearance,"case":id,"dispatches":dispatches,"receipts":window.receipts,"actions":model.actions,"layout_bounds":model.layout])
                try save()
                try require(window.receipts.map { $0["number"] as? Int } == [101,102],"Wrong event receipts")
                try require(model.actions.count == 1 && model.actions[0]["id"] as? String == id,"Wrong action acknowledgement")
                try require((model.actions[0]["uptime"] as! Double) >= (window.receipts[1]["received_uptime"] as! Double),"Action preceded release receipt")
            }
        }
        window.close()
    }
}
