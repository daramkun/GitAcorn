import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ConflictEditor } from "./conflict-editor";
import type { ConflictFileDto } from "./repository";

const file: ConflictFileDto = {
  schemaVersion: 1,
  base: "before\n",
  ours: "current\n",
  theirs: "incoming\n",
  worktreeOid: "worktree-oid",
  editable: true,
  segments: [
    { kind: "common", content: "header\n" },
    {
      kind: "conflict",
      index: 0,
      ours: "current\n",
      base: "before\n",
      theirs: "incoming\n",
    },
    { kind: "common", content: "footer\n" },
  ],
};

describe("ConflictEditor", () => {
  it("requires every hunk and applies the assembled result", async () => {
    const onApply = vi.fn().mockResolvedValue(true);
    render(<ConflictEditor file={file} disabled={false} onApply={onApply} />);

    const apply = screen.getByRole("button", { name: "Apply resolved file" });
    expect(apply).toBeDisabled();

    fireEvent.click(screen.getByRole("button", { name: "Use incoming" }));
    expect(screen.getByText("1 of 1 hunks resolved")).toBeInTheDocument();
    expect(apply).toBeEnabled();

    fireEvent.click(apply);
    await waitFor(() =>
      expect(onApply).toHaveBeenCalledWith("header\nincoming\nfooter\n"),
    );
  });

  it("allows a manually edited hunk result", async () => {
    const onApply = vi.fn().mockResolvedValue(true);
    render(<ConflictEditor file={file} disabled={false} onApply={onApply} />);

    fireEvent.change(
      screen.getByRole("textbox", { name: "Resolved result for hunk 1" }),
      { target: { value: "combined manually\n" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Apply resolved file" }));

    await waitFor(() =>
      expect(onApply).toHaveBeenCalledWith("header\ncombined manually\nfooter\n"),
    );
  });
});