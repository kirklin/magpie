import { describe, expect, it } from "vitest";
import { getTypeIcon, getTypeLabel } from "./classifier";

describe("getTypeLabel", () => {
  it("maps each known content type to its label", () => {
    expect(getTypeLabel("text")).toBe("Text");
    expect(getTypeLabel("url")).toBe("Link");
    expect(getTypeLabel("code")).toBe("Code Snippet");
    expect(getTypeLabel("image")).toBe("Image");
    expect(getTypeLabel("color")).toBe("Color Code");
    expect(getTypeLabel("email")).toBe("Email Address");
    expect(getTypeLabel("file")).toBe("File");
  });

  it("falls back to Text for unknown types", () => {
    expect(getTypeLabel("richtext")).toBe("Text");
    expect(getTypeLabel("")).toBe("Text");
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
