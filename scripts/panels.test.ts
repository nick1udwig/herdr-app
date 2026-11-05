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

test("Changes refreshes literal changed paths and preserves unrelated entries", async () => {
  const source = await readFile(new URL("../defaults/mods/changes/main.js", import.meta.url), "utf8");
  const refresh = source.slice(source.indexOf("  refresh(cx, paths) {"), source.indexOf("  async load("));
  const calls: string[][] = [];
  const exec = async (_: string, args: string[]) => {
    calls.push(args);
    if (args.includes("--show-toplevel")) return { code: 0, stdout: "/repo" };
    if (args.includes("status")) return { code: 0, stdout: " M a[1]\0" };
    if (args.includes("diff")) return { code: 0, stdout: "2\t1\ta[1]\0" };
    return { code: 0, stdout: "head" };
  };
  const Panel = new Function("exec", "location", "key", "READ", `return class { ${refresh} }`)(exec, () => ({ cwd: "/repo" }), () => "same", []);
  const panel = new Panel();
  panel.context = "same"; panel.files = [{ path: "unrelated", add: 4 }, { path: "gone", add: 1 }]; panel.closed = true;
  panel.watch = () => {}; panel.setLines = () => {};
  let task: Promise<void>;
  const cx = { notify() {}, spawn(fn: Function) { task = fn(cx); } };
  panel.refresh(cx, ["/repo/a[1]", "/repo/gone"]);
  await task!;
  expect(panel.error).toBeNull();
  expect(panel.files.map((f: any) => f.path)).toEqual(["unrelated", "a[1]"]);
  for (const args of calls.filter(a => a.includes("status") || a.includes("diff"))) {
    expect(args).toContain("--literal-pathspecs");
    expect(args.slice(args.indexOf("--") + 1)).toEqual(["a[1]", "gone"]);
  }
});
