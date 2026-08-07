import { describe, expect, it } from "vitest";
import { toPlainText } from "./text-utils";

describe("toPlainText", () => {
  it("removes markup while keeping readable paragraphs and entities", () => {
    expect(
      toPlainText(
        "<h1>Digital Deluxe Edition</h1><p>Includes &amp; extras<br>for you.</p>",
      ),
    ).toBe("Digital Deluxe Edition\nIncludes & extras\nfor you.");
  });
});
