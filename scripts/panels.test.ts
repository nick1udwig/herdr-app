import { test, expect } from "bun:test";
import { readFile } from "node:fs/promises";

for (const name of ["files", "changes"]) {
  const source = await readFile(new URL(`../defaults/mods/${name}/main.js`, import.meta.url), "utf8");
  const invalidate = source.slice(source.indexOf("  invalidate(cx, paths) {"), source.indexOf("  refresh(cx, paths) {"));
  const render = source.slice(source.indexOf("  render(cx) {"), source.indexOf("    const t = theme()", source.indexOf("  render(cx) {")));
  const Panel = new Function("key", `return class { ${invalidate} ${render} } }`)(() => "same");
  test(`${name} coalesces hidden refreshes and refreshes when shown`, () => {
    const panel = new Panel();
    panel.context = name === "files" ? { key: "same" } : "same";
    let scans = 0, notices = 0;
    panel.refresh = () => scans++;
    const cx = { notify: () => notices++ };
    panel.invalidate(cx, ["/repo/a"]);
    panel.invalidate(cx, ["/repo/b", "/repo/a"]);
    expect(panel.refreshPaths).toEqual(["/repo/a", "/repo/b"]);
    expect(scans).toBe(0);
    expect(notices).toBe(2);
    panel.loading = true;
    panel.render(cx);
    expect(scans).toBe(0);
    panel.loading = false;
    panel.render(cx);
    expect(scans).toBe(1);
    panel.render(cx);
    expect(scans).toBe(1);
  });
}
