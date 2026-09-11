import AppKit
import SwiftUI

func rgba(_ values: [Double]) -> String {
    values.map { String(format: "%.12f", $0) }.joined(separator: ", ")
}

for (name, scheme) in [("light", ColorScheme.light), ("dark", ColorScheme.dark)] {
    var environment = EnvironmentValues()
    environment.colorScheme = scheme
    for (label, color) in [("Color.orange", Color.orange),
                           ("explicit #FF9500", Color(.sRGB, red: 1, green: 149.0/255, blue: 0, opacity: 1))] {
        let r = color.resolve(in: environment)
        print("\(name) / \(label)")
        print("  linear sRGB RGBA = \(rgba([Double(r.linearRed), Double(r.linearGreen), Double(r.linearBlue), Double(r.opacity)]))")
        print("  encoded sRGB RGBA = \(rgba([Double(r.red), Double(r.green), Double(r.blue), Double(r.opacity)]))")
        print("  CGColor space = \(r.cgColor.colorSpace?.name as String? ?? "unnamed")")
        if let s = NSColor(cgColor: r.cgColor)?.usingColorSpace(.sRGB) {
            print("  NSColor converted sRGB RGBA = \(rgba([Double(s.redComponent), Double(s.greenComponent), Double(s.blueComponent), Double(s.alphaComponent)]))")
            print(String(format: "  rounded encoded RGB8 = #%02X%02X%02X", Int((s.redComponent*255).rounded()), Int((s.greenComponent*255).rounded()), Int((s.blueComponent*255).rounded())))
        } else {
            fatalError("Resolved CGColor cannot convert to NSColor sRGB")
        }
    }
}
