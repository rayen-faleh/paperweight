// Build a .mcpb bundle for the current platform from a release build of
// paperweight-mcp, then validate and pack it with the official mcpb CLI.
//
//   cargo build --release -p paperweight-mcp
//   cd packaging/mcpb && npm ci && npm run build
//
// Output: dist-mcpb/paperweight-<version>-<platform>-<arch>.mcpb
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync, chmodSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../..");
const platform = process.platform; // darwin | win32 | linux
const exe = platform === "win32" ? ".exe" : "";
const binary = join(root, "target", process.env.CARGO_BUILD_TARGET ?? "", "release", `paperweight-mcp${exe}`);
if (!existsSync(binary)) {
  console.error(`Missing ${binary}. Run: cargo build --release -p paperweight-mcp`);
  process.exit(1);
}

const manifest = JSON.parse(readFileSync(join(here, "manifest.template.json"), "utf8"));
const cargo = readFileSync(join(root, "Cargo.toml"), "utf8");
manifest.version = /\[workspace\.package\][^[]*?version\s*=\s*"([^"]+)"/s.exec(cargo)[1];
manifest.compatibility.platforms = [platform];

const stage = join(root, "dist-mcpb", "stage");
rmSync(stage, { recursive: true, force: true });
mkdirSync(join(stage, "server"), { recursive: true });
copyFileSync(binary, join(stage, "server", `paperweight-mcp${exe}`));
if (!exe) chmodSync(join(stage, "server", "paperweight-mcp"), 0o755);
copyFileSync(join(root, "crates/app/icons/icon.png"), join(stage, "icon.png"));
copyFileSync(join(root, "LICENSE"), join(stage, "LICENSE"));
writeFileSync(join(stage, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");

const mcpb = join(here, "node_modules", ".bin", platform === "win32" ? "mcpb.cmd" : "mcpb");
const run = (...args) => execFileSync(mcpb, args, { stdio: "inherit", shell: platform === "win32" });
run("validate", join(stage, "manifest.json"));
const out = join(root, "dist-mcpb", `paperweight-${manifest.version}-${platform}-${process.arch}.mcpb`);
run("pack", stage, out);
run("info", out);
console.log(`\nBuilt ${out}`);
