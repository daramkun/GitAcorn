import type { CommitDto } from "./repository";

export type GraphSegment = {
  fromLane: number;
  toLane: number;
  from: "top" | "node";
  to: "node" | "bottom";
  color: number;
};

export type CommitGraphRow = {
  nodeLane: number;
  nodeColor: number;
  laneCount: number;
  segments: GraphSegment[];
};

export type CommitGraphLayout = {
  laneCount: number;
  rows: CommitGraphRow[];
};

type Lane = {
  key: number;
  oid: string;
  color: number;
};

export function layoutCommitGraph(
  commits: ReadonlyArray<Pick<CommitDto, "oid" | "parents">>,
): CommitGraphLayout {
  let nextLaneKey = 0;
  let lanes: Lane[] = [];
  let maximumLaneCount = 1;
  const rows: CommitGraphRow[] = [];

  for (const commit of commits) {
    let nodeLane = lanes.findIndex((lane) => lane.oid === commit.oid);
    const startsHere = nodeLane === -1;
    if (startsHere) {
      lanes.push({
        key: nextLaneKey,
        oid: commit.oid,
        color: nextLaneKey,
      });
      nextLaneKey += 1;
      nodeLane = lanes.length - 1;
    }

    const before = lanes;
    const currentLane = before[nodeLane];
    const after = before.slice();
    const parentLanes: Lane[] = [];
    const seenParents = new Set<string>();

    const firstParent = commit.parents[0];
    if (firstParent) {
      seenParents.add(firstParent);
      const existingParent = after.find(
        (lane) => lane.key !== currentLane.key && lane.oid === firstParent,
      );
      if (existingParent) {
        after.splice(nodeLane, 1);
        parentLanes.push(existingParent);
      } else {
        const continuedLane = { ...currentLane, oid: firstParent };
        after[nodeLane] = continuedLane;
        parentLanes.push(continuedLane);
      }
    } else {
      after.splice(nodeLane, 1);
    }

    let insertedParents = 0;
    for (const parent of commit.parents.slice(1)) {
      if (seenParents.has(parent)) continue;
      seenParents.add(parent);

      let parentLane = after.find((lane) => lane.oid === parent);
      if (!parentLane) {
        parentLane = {
          key: nextLaneKey,
          oid: parent,
          color: nextLaneKey,
        };
        nextLaneKey += 1;
        const insertionIndex = Math.min(nodeLane + 1 + insertedParents, after.length);
        after.splice(insertionIndex, 0, parentLane);
        insertedParents += 1;
      }
      parentLanes.push(parentLane);
    }

    const segments: GraphSegment[] = [];
    for (const lane of before) {
      if (lane.key === currentLane.key) continue;
      const nextIndex = after.findIndex((candidate) => candidate.key === lane.key);
      if (nextIndex !== -1) {
        segments.push({
          fromLane: before.indexOf(lane),
          toLane: nextIndex,
          from: "top",
          to: "bottom",
          color: lane.color,
        });
      }
    }

    if (!startsHere) {
      segments.push({
        fromLane: nodeLane,
        toLane: nodeLane,
        from: "top",
        to: "node",
        color: currentLane.color,
      });
    }

    for (const parentLane of parentLanes) {
      const parentIndex = after.findIndex((lane) => lane.key === parentLane.key);
      segments.push({
        fromLane: nodeLane,
        toLane: parentIndex,
        from: "node",
        to: "bottom",
        color: parentLane.color,
      });
    }

    const laneCount = Math.max(before.length, after.length, nodeLane + 1, 1);
    maximumLaneCount = Math.max(maximumLaneCount, laneCount);
    rows.push({
      nodeLane,
      nodeColor: currentLane.color,
      laneCount,
      segments,
    });
    lanes = after;
  }

  return { laneCount: maximumLaneCount, rows };
}
