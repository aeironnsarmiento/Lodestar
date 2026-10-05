import { useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import type { ConsoleLine } from "../lib/api";
import { GlassButton } from "./glass/GlassButton";
import { GlassInput } from "./glass/GlassInput";
import { Icon } from "./Icon";

const LINE_HEIGHT = 19;
const OVERSCAN = 30;
/** Rendered when the viewport size is unknown (first paint, tests). */
const FALLBACK_ROWS = 80;

interface ConsoleViewProps {
  lines: ConsoleLine[];
  /** Sends a console command; omitted for read-only views. */
  onSend?: (command: string) => Promise<void> | void;
  /** The server is not running, so commands cannot be sent. */
  inputDisabled?: boolean;
}

function lineClass(line: ConsoleLine): string {
  const t = line.text;
  const classes = ["console-line"];
  if (line.history) classes.push("history");
  if (t.startsWith("[Glasscraft]") || t.startsWith("---- Glasscraft")) classes.push("note");
  else if (t.startsWith("> ")) classes.push("command");
  else if (/\bERROR\b|Exception|\/FATAL\]/.test(t)) classes.push("error");
  else if (/\/WARN\]/.test(t)) classes.push("warn");
  return classes.join(" ");
}

function highlight(text: string, query: string): ReactNode {
  if (!query) return text;
  const lower = text.toLowerCase();
  const q = query.toLowerCase();
  const out: ReactNode[] = [];
  let i = 0;
  let k = 0;
  for (let at = lower.indexOf(q); at !== -1; at = lower.indexOf(q, i)) {
    out.push(text.slice(i, at), <mark key={k++}>{text.slice(at, at + q.length)}</mark>);
    i = at + q.length;
  }
  out.push(text.slice(i));
  return out;
}

export function ConsoleView({ lines, onSend, inputDisabled }: ConsoleViewProps) {
  const [query, setQuery] = useState("");
  const [command, setCommand] = useState("");
  const [history, setHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState<number | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [height, setHeight] = useState(0);
  const scroller = useRef<HTMLDivElement>(null);
  const follow = useRef(true);

  const visible = useMemo(() => {
    if (!query) return lines;
    const q = query.toLowerCase();
    return lines.filter((l) => l.text.toLowerCase().includes(q));
  }, [lines, query]);

  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    setHeight(el.clientHeight);
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => setHeight(el.clientHeight));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Stay pinned to the newest line unless the user scrolled up.
  useLayoutEffect(() => {
    const el = scroller.current;
    if (el && follow.current) {
      el.scrollTop = el.scrollHeight;
      setScrollTop(el.scrollTop);
    }
  }, [visible.length, height]);

  const total = visible.length * LINE_HEIGHT;
  let start: number;
  let end: number;
  if (height > 0) {
    start = Math.max(0, Math.floor(scrollTop / LINE_HEIGHT) - OVERSCAN);
    end = Math.min(visible.length, Math.ceil((scrollTop + height) / LINE_HEIGHT) + OVERSCAN);
  } else {
    end = visible.length;
    start = Math.max(0, end - FALLBACK_ROWS);
  }
  const slice = visible.slice(start, end);

  const onScroll = () => {
    const el = scroller.current;
    if (!el) return;
    setScrollTop(el.scrollTop);
    follow.current = el.scrollTop + el.clientHeight >= el.scrollHeight - LINE_HEIGHT * 2;
  };

  const submit = async () => {
    const cmd = command.trim();
    if (!cmd || !onSend) return;
    setHistory((h) => (h[h.length - 1] === cmd ? h : [...h, cmd].slice(-100)));
    setHistoryIndex(null);
    setCommand("");
    follow.current = true;
    await onSend(cmd.replace(/^\//, ""));
  };

  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      submit();
    } else if (e.key === "ArrowUp" && history.length) {
      e.preventDefault();
      const i = historyIndex === null ? history.length - 1 : Math.max(0, historyIndex - 1);
      setHistoryIndex(i);
      setCommand(history[i]);
    } else if (e.key === "ArrowDown" && historyIndex !== null) {
      e.preventDefault();
      const i = historyIndex + 1;
      if (i >= history.length) {
        setHistoryIndex(null);
        setCommand("");
      } else {
        setHistoryIndex(i);
        setCommand(history[i]);
      }
    }
  };

  return (
    <div className="console">
      <div className="console-toolbar">
        <div className="console-search">
          <Icon name="search" size={15} />
          <input
            type="search"
            aria-label="Search console"
            placeholder="Search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        {query && (
          <span className="faint" aria-live="polite">
            {visible.length} {visible.length === 1 ? "match" : "matches"}
          </span>
        )}
      </div>
      <div className="console-scroll" ref={scroller} onScroll={onScroll} role="log" aria-label="Server console">
        <div style={{ height: total, position: "relative" }}>
          <div style={{ position: "absolute", top: start * LINE_HEIGHT, left: 0, right: 0 }}>
            {slice.map((l) => (
              <div key={l.seq} className={lineClass(l)} style={{ height: LINE_HEIGHT }}>
                {highlight(l.text, query)}
              </div>
            ))}
          </div>
        </div>
        {lines.length === 0 && <div className="console-empty">The console is empty. Start the server to see its log.</div>}
      </div>
      {onSend && (
        <form
          className="console-input"
          onSubmit={(e) => {
            e.preventDefault();
            submit();
          }}
        >
          <span className="prompt mono">&gt;</span>
          <GlassInput
            aria-label="Console command"
            placeholder={inputDisabled ? "Start the server to send commands" : "Type a command, e.g. say hello"}
            value={command}
            disabled={inputDisabled}
            onChange={(e) => {
              setCommand(e.target.value);
              setHistoryIndex(null);
            }}
            onKeyDown={onKey}
            className="mono"
          />
          <GlassButton type="submit" iconOnly aria-label="Send command" icon={<Icon name="send" size={16} />} disabled={inputDisabled || !command.trim()} />
        </form>
      )}
    </div>
  );
}
