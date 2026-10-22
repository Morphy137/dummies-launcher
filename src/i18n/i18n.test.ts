import { describe, expect, it } from "vitest";
import en from "./en";
import es from "./es";

function collectKeys(value: unknown, prefix = ""): string[] {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return [prefix];
  }

  return Object.entries(value as Record<string, unknown>)
    .flatMap(([key, child]) => collectKeys(child, prefix ? `${prefix}.${key}` : key))
    .sort();
}

describe("locale key parity", () => {
  it("keeps English and Spanish keys aligned", () => {
    expect(collectKeys(es)).toEqual(collectKeys(en));
  });

  it("does not use the Discord name in the app title", () => {
    expect(en.app.title.toLowerCase()).not.toContain("discord");
    expect(es.app.title.toLowerCase()).not.toContain("discord");
  });
});
