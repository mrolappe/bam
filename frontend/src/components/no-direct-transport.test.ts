// P9.1's fourth required test: components must go through BamClient, never
// a transport directly — otherwise a component quietly becomes Tauri-only
// or web-only, which is the exact failure mode this phase's seam prevents.
import { describe, expect, it } from "vitest";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const componentsDir = join(import.meta.dirname, ".");

function collectFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((entry) => {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) return collectFiles(path);
    return path.endsWith(".vue") || path.endsWith(".ts") ? [path] : [];
  });
}

describe("components never import a transport directly", () => {
  it.each(collectFiles(componentsDir).filter((f) => !f.endsWith(".test.ts")))("%s", (file) => {
    const src = readFileSync(file, "utf8");
    expect(src).not.toMatch(/@tauri-apps\/api/);
    expect(src).not.toMatch(/\bfetch\(/);
  });
});
