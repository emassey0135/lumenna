// Builds the core for the browser and generates its bindings into src/core (§16.12).
//
// Cargo builds lumenna-web for wasm32-unknown-unknown with the size-tuned `web` profile;
// wasm-bindgen then writes the module, its loader and the TypeScript types derived from the
// surface's records. The output is build output, not committed, as the Apple apps'
// Generated/ is not. `--dev` builds the debug profile, faster to build and larger.
//
// Needs the wasm32-unknown-unknown target (`rustup target add wasm32-unknown-unknown`) and
// wasm-bindgen-cli at the same version as the wasm-bindgen crate in Cargo.lock.

import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const web = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const root = path.resolve(web, "..", "..");
const dev = process.argv.includes("--dev");
const profile = dev ? "dev" : "web";
const folder = dev ? "debug" : "web";

const run = (command, args) => execFileSync(command, args, { cwd: root, stdio: "inherit" });

run("cargo", ["build", "-p", "lumenna-web", "--target", "wasm32-unknown-unknown", "--profile", profile]);
run("wasm-bindgen", [
  "--target", "web",
  "--out-dir", path.join(web, "src", "core"),
  path.join(root, "target", "wasm32-unknown-unknown", folder, "lumenna_web.wasm"),
]);
