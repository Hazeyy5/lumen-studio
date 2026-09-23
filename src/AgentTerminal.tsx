import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { imageBrief, imageFilesFromTransfer, savePastedFiles } from "./pasteImage";
import { ptyBus } from "./ptyBus";
import "@xterm/xterm/css/xterm.css";

export function AgentTerminal({
  sessionId,
  projectPath,
  active,
}: {
  sessionId: string;
  projectPath?: string;
  active: boolean;
}) {
  const hostRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    const term = new Terminal({
      cursorBlink: true,
      cursorStyle: "bar",
      fontSize: 13,
      lineHeight: 1.25,
      fontFamily: '"Cascadia Mono", Consolas, "Courier New", monospace',
      theme: {
        background: "#1c1712",
        foreground: "#f3eee4",
        cursor: "#b85c38",
        cursorAccent: "#1c1712",
        selectionBackground: "#b85c3866",
        black: "#1c1712",
        red: "#c45c4a",
        green: "#6a9a78",
        yellow: "#d4a574",
        blue: "#6b8cae",
        magenta: "#b85c38",
        cyan: "#7a9e8e",
        white: "#f3eee4",
        brightBlack: "#6b6156",
        brightRed: "#e07a66",
        brightGreen: "#8fbf9a",
        brightYellow: "#e8c39e",
        brightBlue: "#8eadd0",
        brightMagenta: "#d47852",
        brightCyan: "#9dc4b4",
        brightWhite: "#fffaf3",
      },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(host);

    const sendResize = () => {
      if (host.clientWidth < 8 || host.clientHeight < 8) return;
      try {
        fit.fit();
      } catch {
        return;
      }
      const { rows, cols } = term;
      if (rows > 1 && cols > 1) {
        void invoke("resize_agent", { sessionId, rows, cols });
      }
    };

    const injectImages = async (files: File[]) => {
      if (!projectPath || !files.length) return;
      try {
        const saved = await savePastedFiles(projectPath, files);
        if (!saved.length) return;
        await invoke("write_agent", {
          sessionId,
          data: `${imageBrief(saved)} `,
        });
      } catch (error) {
        term.writeln(`\r\n[Lumen] collage image : ${String(error)}`);
      }
    };

    const onPaste = (event: ClipboardEvent) => {
      const files = imageFilesFromTransfer(event.clipboardData);
      if (!files.length) return;
      event.preventDefault();
      event.stopPropagation();
      void injectImages(files);
    };
    const onDragOver = (event: DragEvent) => {
      if (imageFilesFromTransfer(event.dataTransfer).length) event.preventDefault();
    };
    const onDrop = (event: DragEvent) => {
      const files = imageFilesFromTransfer(event.dataTransfer);
      if (!files.length) return;
      event.preventDefault();
      event.stopPropagation();
      void injectImages(files);
    };

    const dataSub = term.onData((data) => {
      void invoke("write_agent", { sessionId, data });
    });
    const ptySub = ptyBus.subscribe(sessionId, (chunk) => {
      term.write(chunk);
    });
    const observer = new ResizeObserver(() => sendResize());
    observer.observe(host);
    host.addEventListener("paste", onPaste, true);
    host.addEventListener("dragover", onDragOver);
    host.addEventListener("drop", onDrop);
    const frame = requestAnimationFrame(sendResize);
    const later = window.setTimeout(sendResize, 80);
    if (active) term.focus();

    return () => {
      cancelAnimationFrame(frame);
      window.clearTimeout(later);
      dataSub.dispose();
      ptySub();
      observer.disconnect();
      host.removeEventListener("paste", onPaste, true);
      host.removeEventListener("dragover", onDragOver);
      host.removeEventListener("drop", onDrop);
      term.dispose();
    };
  }, [sessionId, projectPath]);

  useEffect(() => {
    if (active) hostRef.current?.querySelector("textarea")?.focus();
  }, [active]);

  return <div className="agent-term" ref={hostRef} />;
}
