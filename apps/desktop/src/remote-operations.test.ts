import { describe, expect, it } from "vitest";
import type { OperationEventDto } from "./repository";
import { updateRepositoryOperation } from "./remote-operations";

function event(repoId: string, operationId: string): OperationEventDto {
  return {
    schemaVersion: 1,
    operationId,
    repoId,
    kind: "fetch",
    state: "running",
  };
}

describe("repository remote operation state", () => {
  it("keeps progress isolated by repository ID", () => {
    const atlas = event("atlas", "operation-atlas");
    const acorn = event("acorn", "operation-acorn");
    const state = updateRepositoryOperation(
      updateRepositoryOperation({}, "atlas", atlas),
      "acorn",
      acorn,
    );

    expect(state.atlas.operationId).toBe("operation-atlas");
    expect(state.acorn.operationId).toBe("operation-acorn");
  });

  it("ignores a channel event scoped to another repository", () => {
    const current = { atlas: event("atlas", "operation-atlas") };
    expect(updateRepositoryOperation(current, "atlas", event("acorn", "foreign"))).toBe(
      current,
    );
  });
});
