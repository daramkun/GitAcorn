import { describe, expect, it } from "vitest";
import { coAuthorsFromCommitBody, gravatarUrl } from "./gravatar";

describe("gravatarUrl", () => {
  it("normalizes the email and builds a SHA-256 Gravatar URL", async () => {
    await expect(gravatarUrl(" ABC ")).resolves.toBe(
      "https://www.gravatar.com/avatar/ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad?s=40&d=identicon",
    );
  });
});

describe("coAuthorsFromCommitBody", () => {
  it("reads multiple co-author trailers and removes duplicate emails", () => {
    expect(
      coAuthorsFromCommitBody(
        [
          "Commit details.",
          "",
          "Co-authored-by: Grace Hopper <grace@example.com>",
          "Reviewed-by: Lin <lin@example.com>",
          "co-authored-by: Ada Lovelace <ADA@example.com>",
          "Co-Authored-By: Duplicate Ada <ada@example.com>",
        ].join("\n"),
      ),
    ).toEqual([
      { name: "Grace Hopper", email: "grace@example.com" },
      { name: "Ada Lovelace", email: "ADA@example.com" },
    ]);
  });
});
