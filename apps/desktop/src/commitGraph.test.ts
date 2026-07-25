import { describe, expect, it } from "vitest";
import { layoutCommitGraph } from "./commitGraph";

const commit = (oid: string, ...parents: string[]) => ({ oid, parents });

describe("layoutCommitGraph", () => {
  it("connects a linear history without drawing above a new tip or below a root", () => {
    const layout = layoutCommitGraph([commit("b", "a"), commit("a")]);

    expect(layout.laneCount).toBe(1);
    expect(layout.rows[0].segments).toEqual([
      {
        fromLane: 0,
        toLane: 0,
        from: "node",
        to: "bottom",
        color: 0,
      },
    ]);
    expect(layout.rows[1].segments).toEqual([
      {
        fromLane: 0,
        toLane: 0,
        from: "top",
        to: "node",
        color: 0,
      },
    ]);
  });

  it("draws a merge parent back into an existing lane", () => {
    const layout = layoutCommitGraph([
      commit("merge", "main", "topic"),
      commit("main", "root"),
      commit("topic", "root"),
      commit("root"),
    ]);

    expect(layout.laneCount).toBe(2);
    expect(layout.rows[0].segments).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ from: "node", fromLane: 0, toLane: 0 }),
        expect.objectContaining({ from: "node", fromLane: 0, toLane: 1 }),
      ]),
    );
    expect(layout.rows[2].nodeLane).toBe(1);
    expect(layout.rows[2].segments).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          from: "node",
          fromLane: 1,
          toLane: 0,
        }),
      ]),
    );
    expect(layout.rows[3].nodeLane).toBe(0);
  });

  it("places a later independent branch tip to the right without shifting main", () => {
    const layout = layoutCommitGraph([
      commit("head", "main"),
      commit("main", "base"),
      commit("dev", "base"),
      commit("base"),
    ]);

    expect(layout.rows.map((row) => row.nodeLane)).toEqual([0, 0, 1, 0]);
    expect(layout.rows[2].segments).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          from: "top",
          to: "bottom",
          fromLane: 0,
          toLane: 0,
        }),
        expect.objectContaining({
          from: "node",
          to: "bottom",
          fromLane: 1,
          toLane: 0,
        }),
      ]),
    );
  });

  it("keeps existing rows stable when an older page is appended", () => {
    const commits = [
      commit("merge", "main", "topic"),
      commit("main", "root"),
      commit("topic", "root"),
      commit("root"),
    ];

    const firstPage = layoutCommitGraph(commits.slice(0, 2));
    const combinedPages = layoutCommitGraph(commits);

    expect(combinedPages.rows.slice(0, 2)).toEqual(firstPage.rows);
  });
});
