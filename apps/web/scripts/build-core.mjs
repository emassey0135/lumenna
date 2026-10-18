// Builds the core for the browser and generates its bindings into src/core.
//
// BoltFFI builds lumenna-web for wasm32-unknown-unknown with the size-tuned `web` profile, then
// packs it — the module, its loader, the wasm-bindgen half its dependencies need, and the
// TypeScript types of the core's records, typed as Android's are. The output is build output,
// not committed, as the Apple apps' Generated/ is not. `--dev` builds the debug profile, faster
// to build and larger.
//
// Needs the wasm32-unknown-unknown target (`rustup target add wasm32-unknown-unknown`),
// BoltFFI's CLI at the crates' version (`cargo install boltffi_cli --version 0.31.1 --locked`),
// and wasm-bindgen-cli at the same version as the wasm-bindgen crate in Cargo.lock, which BoltFFI
// builds and caches itself when none is on PATH.

import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const web = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const root = path.resolve(web, "..", "..");
const dev = process.argv.includes("--dev");
const profile = dev ? "dev" : "web";
const folder = dev ? "debug" : "web";
const bolt = path.join(root, "target", "boltffi-web");

mkdirSync(bolt, { recursive: true });
const overlay = path.join(bolt, "overlay.toml");
writeFileSync(
  overlay,
  `[targets.wasm]
# BoltFFI builds the debug profile unless told --release; the profile here is the web's own.
profile = "debug"
cargo_args = ["--profile", ${JSON.stringify(profile)}]
artifact_path = ${JSON.stringify(path.join(root, "target", "wasm32-unknown-unknown", folder, "lumenna_web.wasm"))}
output = ${JSON.stringify(bolt)}

[targets.wasm.npm]
output = ${JSON.stringify(path.join(web, "src", "core"))}
`,
);
// BoltFFI checks the TypeScript it writes with tsc, which is this package's own.
const binaries = path.join(web, "node_modules", ".bin");
execFileSync("boltffi", ["-v", "pack", "wasm", "--overlay", overlay], {
  cwd: path.join(web, "core"),
  stdio: "inherit",
  env: { ...process.env, PATH: `${binaries}${path.delimiter}${process.env.PATH}` },
});
