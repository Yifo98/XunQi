import AppKit
import Foundation
import WebKit

final class PdfNavigationDelegate: NSObject, WKNavigationDelegate {
    private let webView: WKWebView
    private let destination: URL
    private var finished = false

    init(webView: WKWebView, destination: URL) {
        self.webView = webView
        self.destination = destination
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        guard !finished else { return }
        finished = true
        let script = "Math.max(document.body.scrollHeight, document.documentElement.scrollHeight, 1)"
        webView.evaluateJavaScript(script) { [weak self] result, error in
            guard let self else { return }
            if let error {
                fail("无法测量文章页面：\(error.localizedDescription)")
            }
            let height = ceil((result as? NSNumber)?.doubleValue ?? 0)
            guard height > 0, height <= 100_000 else {
                fail("文章页面高度异常，无法安全生成 PDF")
            }
            let configuration = WKPDFConfiguration()
            configuration.rect = CGRect(x: 0, y: 0, width: 794, height: height)
            webView.createPDF(configuration: configuration) { result in
                switch result {
                case .success(let data):
                    do {
                        try data.write(to: self.destination, options: .atomic)
                        exit(EXIT_SUCCESS)
                    } catch {
                        fail("无法写入 PDF：\(error.localizedDescription)")
                    }
                case .failure(let error):
                    fail("WebKit 生成 PDF 失败：\(error.localizedDescription)")
                }
            }
        }
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        fail("文章页面加载失败：\(error.localizedDescription)")
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        fail("文章页面无法打开：\(error.localizedDescription)")
    }
}

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(EXIT_FAILURE)
}

guard CommandLine.arguments.count == 3 else {
    fail("用法：xunqi-pdf-renderer <离线HTML> <输出PDF>")
}

let source = URL(fileURLWithPath: CommandLine.arguments[1]).standardizedFileURL
let destination = URL(fileURLWithPath: CommandLine.arguments[2]).standardizedFileURL
guard FileManager.default.fileExists(atPath: source.path) else {
    fail("没有找到离线文章页面")
}

let application = NSApplication.shared
application.setActivationPolicy(.prohibited)
let configuration = WKWebViewConfiguration()
configuration.websiteDataStore = .nonPersistent()
let webView = WKWebView(frame: CGRect(x: 0, y: 0, width: 794, height: 1200), configuration: configuration)
let delegate = PdfNavigationDelegate(webView: webView, destination: destination)
webView.navigationDelegate = delegate

Timer.scheduledTimer(withTimeInterval: 45, repeats: false) { _ in
    fail("离线文章页面加载超过 45 秒")
}
webView.loadFileURL(source, allowingReadAccessTo: source.deletingLastPathComponent())
application.run()
