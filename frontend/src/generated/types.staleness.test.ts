// P9.1's third required test: fails when the checked-in generated types
// drift from the Rust schemas that produce them. Regenerates in-memory
// (not to disk) and diffs against the committed file.
import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { exportSchemas, generate } from "../../scripts/gen-types.mjs";

describe("generated API types", () => {
  it("match the current bam-core JSON Schema export", async () => {
    const committed = readFileSync(join(import.meta.dirname, "types.ts"), "utf8");
    const fresh = await generate(exportSchemas());
    expect(fresh).toBe(committed);
  }, 30_000);
});
