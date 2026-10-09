// 终端前端胶水：xterm.js 与宿主（WebView2）之间的桥。
//
// 消息协议（JSON）：
//   JS  -> C#：{ type: "ready" }                 xterm 就绪，宿主可以开始推输出
//              { type: "input", data: base64 }   键盘 / 粘贴
//              { type: "resize", cols, rows }     终端尺寸变化
//   C#  -> JS：{ type: "output", data: base64 }  终端输出
//              { type: "exit", code }             子进程退出
//
// 二进制一律 base64：PTY 输出是裸字节流（可能含被截断的多字节 UTF-8），
// 直接当字符串传会损坏。

(() => {
    "use strict";

    const bridge = window.chrome && window.chrome.webview ? window.chrome.webview : null;

    function post(message) {
        if (bridge) {
            bridge.postMessage(message);
        }
    }

    function bytesToBase64(bytes) {
        let binary = "";
        const chunk = 0x8000;
        for (let i = 0; i < bytes.length; i += chunk) {
            binary += String.fromCharCode.apply(null, bytes.subarray(i, i + chunk));
        }
        return btoa(binary);
    }

    function base64ToBytes(text) {
        const binary = atob(text);
        const bytes = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i++) {
            bytes[i] = binary.charCodeAt(i);
        }
        return bytes;
    }

    const term = new Terminal({
        cursorBlink: true,
        fontFamily: "Cascadia Mono, Consolas, monospace",
        fontSize: 14,
        scrollback: 5000,
    });
    const fitAddon = new FitAddon.FitAddon();
    term.loadAddon(fitAddon);
    term.open(document.getElementById("terminal"));

    const encoder = new TextEncoder();

    // 键盘 / 粘贴 -> 宿主。onData 给的是字符串，按 UTF-8 编码成字节。
    term.onData((data) => {
        post({ type: "input", data: bytesToBase64(encoder.encode(data)) });
    });

    // fit 导致的尺寸变化 -> 宿主。resize 在 rAF 之后才由 fit 触发，避免
    // fit 尚未完成就把旧尺寸发回去，造成 PTY 与 xterm 错位。
    term.onResize(({ cols, rows }) => {
        post({ type: "resize", cols, rows });
    });

    let fitScheduled = false;
    function scheduleFit() {
        if (fitScheduled) {
            return;
        }
        fitScheduled = true;
        requestAnimationFrame(() => {
            fitScheduled = false;
            try {
                fitAddon.fit();
            } catch (err) {
                // 尺寸为 0（例如窗口尚未布局）时忽略。
            }
        });
    }

    document.addEventListener("keydown", (event) => {
        // 平台快捷键由宿主 / WebView2 处理；这里只把焦点保持在终端。
        if (event.key === "Escape" && document.activeElement !== term.textarea) {
            term.focus();
        }
    });

    window.addEventListener("resize", scheduleFit);
    if (window.ResizeObserver) {
        new ResizeObserver(scheduleFit).observe(document.getElementById("terminal"));
    }

    if (bridge) {
        bridge.addEventListener("message", (event) => {
            const message = event.data;
            if (!message || typeof message !== "object") {
                return;
            }
            if (message.type === "output" && typeof message.data === "string") {
                term.write(base64ToBytes(message.data));
            } else if (message.type === "exit") {
                term.write("\r\n\x1b[90m[进程已退出]\x1b[0m\r\n");
            }
        });
    }

    // 首次布局完成后再 fit，然后通知宿主可以推输出。
    requestAnimationFrame(() => {
        scheduleFit();
        requestAnimationFrame(() => {
            term.focus();
            post({ type: "ready" });
        });
    });
})();
