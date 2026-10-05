import { describe, expect, it, vi } from "vitest";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ConsoleView } from "../components/ConsoleView";
import type { ConsoleLine } from "../lib/api";
import { appendConsole, resetConsoles, setConsole, useConsole } from "../state/console";

const line = (seq: number, text: string, history = false): ConsoleLine => ({ seq, text, history });

const sample = [
  line(1, "---- Glasscraft session 2026-10-05 ----", true),
  line(2, "[10:00:00] [Server thread/INFO]: Starting minecraft server version 26.3", true),
  line(3, "[10:00:02] [Server thread/WARN]: Can't keep up!"),
  line(4, '[10:00:03] [Server thread/INFO]: Done (1.23s)! For help, type "help"'),
  line(5, "[10:00:09] [Server thread/INFO]: Steve joined the game"),
];

describe("ConsoleView", () => {
  it("shows lines, marks history read-only and colours warnings", () => {
    render(<ConsoleView lines={sample} />);
    const log = screen.getByRole("log", { name: /server console/i });
    expect(within(log).getByText(/Steve joined the game/)).toBeInTheDocument();
    expect(within(log).getByText(/Starting minecraft server/)).toHaveClass("history");
    expect(within(log).getByText(/Can't keep up/)).toHaveClass("warn");
    // Read-only view: no command input.
    expect(screen.queryByRole("textbox", { name: /console command/i })).not.toBeInTheDocument();
  });

  it("filters by search with a match count and highlights matches", async () => {
    const user = userEvent.setup();
    render(<ConsoleView lines={sample} />);
    await user.type(screen.getByRole("searchbox", { name: /search console/i }), "steve");
    const log = screen.getByRole("log");
    expect(within(log).queryByText(/Done/)).not.toBeInTheDocument();
    expect(screen.getByText("1 match")).toBeInTheDocument();
    expect(log.querySelector("mark")?.textContent).toBe("Steve");
  });

  it("sends commands, strips a leading slash, clears the input and recalls history", async () => {
    const user = userEvent.setup();
    const onSend = vi.fn();
    render(<ConsoleView lines={sample} onSend={onSend} />);
    const input = screen.getByRole("textbox", { name: /console command/i });

    await user.type(input, "/op Steve{Enter}");
    expect(onSend).toHaveBeenCalledWith("op Steve");
    expect(input).toHaveValue("");

    await user.type(input, "say hi{Enter}");
    await user.keyboard("{ArrowUp}");
    expect(input).toHaveValue("say hi");
    await user.keyboard("{ArrowUp}");
    expect(input).toHaveValue("/op Steve");
    await user.keyboard("{ArrowDown}{ArrowDown}");
    expect(input).toHaveValue("");
  });

  it("disables the command input while the server is not running", () => {
    render(<ConsoleView lines={[]} onSend={() => {}} inputDisabled />);
    expect(screen.getByRole("textbox", { name: /console command/i })).toBeDisabled();
    expect(screen.getByText(/console is empty/i)).toBeInTheDocument();
  });

  it("only renders a window of a long console", () => {
    const many = Array.from({ length: 5000 }, (_, i) => line(i + 1, `spam line ${i + 1}`));
    render(<ConsoleView lines={many} />);
    const rendered = screen.getByRole("log").querySelectorAll(".console-line");
    expect(rendered.length).toBeLessThan(200);
    expect(rendered[rendered.length - 1].textContent).toBe("spam line 5000");
  });
});

describe("console store", () => {
  function Probe({ id }: { id: string }) {
    const lines = useConsole(id);
    return <div data-testid="count">{lines.map((l) => l.seq).join(",")}</div>;
  }

  it("merges a snapshot with live batches without duplicates and caps at 5000", () => {
    resetConsoles();
    render(<Probe id="a" />);
    act(() => {
      setConsole("a", [line(1, "a"), line(2, "b")]);
      appendConsole("a", [line(2, "b"), line(3, "c")]);
    });
    expect(screen.getByTestId("count").textContent).toBe("1,2,3");

    act(() => appendConsole("a", Array.from({ length: 6000 }, (_, i) => line(i + 4, "x"))));
    const seqs = screen.getByTestId("count").textContent!.split(",");
    expect(seqs).toHaveLength(5000);
    expect(seqs.at(-1)).toBe("6003");
  });
});
