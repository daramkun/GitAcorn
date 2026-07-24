import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { getAppInfo } from "./app-info";

vi.mock("./app-info", () => ({
  getAppInfo: vi.fn(),
}));

const mockedGetAppInfo = vi.mocked(getAppInfo);

describe("App", () => {
  beforeEach(() => {
    mockedGetAppInfo.mockResolvedValue({
      schemaVersion: 1,
      name: "GitAcorn",
      version: "0.1.0",
      runtime: "Tauri 2",
    });
  });

  it("renders the typed app info returned by the Rust core", async () => {
    render(<App />);

    expect(screen.getByText("Connecting to core…")).toBeInTheDocument();
    expect(await screen.findByText("Tauri 2 · v0.1.0")).toBeInTheDocument();
  });

  it("switches between Changes and History", async () => {
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: /^History/ }));

    expect(screen.getByRole("heading", { name: "History will appear here." })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^History/ })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("shows a recoverable error state when the core cannot be reached", async () => {
    mockedGetAppInfo.mockRejectedValue(new Error("IPC unavailable"));
    render(<App />);

    expect(await screen.findByRole("alert")).toHaveTextContent("IPC unavailable");
    expect(screen.getByText("Core unavailable")).toBeInTheDocument();
  });
});
