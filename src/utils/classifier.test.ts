import { describe, expect, it } from "vitest";
import { getTypeIcon, getTypeLabel } from "./classifier";

describe("getTypeLabel", () => {
  it("maps each known content type to its label", () => {
    expect(getTypeLabel("text", "en")).toBe("Text");
    expect(getTypeLabel("url", "en")).toBe("Link");
    expect(getTypeLabel("code", "en")).toBe("Code Snippet");
    expect(getTypeLabel("image", "en")).toBe("Image");
    expect(getTypeLabel("color", "en")).toBe("Color Code");
    expect(getTypeLabel("email", "en")).toBe("Email Address");
    expect(getTypeLabel("file", "en")).toBe("File");
  });

  it("falls back to Text for unknown types", () => {
    expect(getTypeLabel("richtext", "en")).toBe("Text");
    expect(getTypeLabel("", "en")).toBe("Text");
  });
});

describe("getTypeIcon", () => {
  it("returns a React element for known and unknown types", () => {
    for (const type of ["text", "url", "code", "image", "color", "email", "file", "???"]) {
      const icon = getTypeIcon(type) as { type?: unknown } | null;
      expect(icon).toBeTruthy();
      expect(icon?.type).toBeDefined();
    }
  });
});
