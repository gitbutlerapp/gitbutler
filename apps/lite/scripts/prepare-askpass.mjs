import { copyFileSync, chmodSync, mkdirSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const liteRoot = path.resolve(path.dirname(scriptPath), "..");
const repoRoot = path.resolve(liteRoot, "../..");
const release =
	process.argv.includes("--release") ||
	process.env.CI === "true" ||
	process.env.CHANNEL === "nightly" ||
	process.env.CHANNEL === "release";
const profile = release ? "release" : "debug";
const exeExtension = process.platform === "win32" ? ".exe" : "";
const binName = `gitbutler-git-askpass${exeExtension}`;

const cargoArgs = ["build", "-p", "gitbutler-git", "--bin", "gitbutler-git-askpass"];
if (release) cargoArgs.splice(1, 0, "--release");

const cargo = spawnSync("cargo", cargoArgs, {
	cwd: repoRoot,
	stdio: "inherit",
});

if (cargo.status !== 0) process.exit(cargo.status ?? 1);

const targetDir =
	process.env.CARGO_TARGET_DIR !== undefined
		? path.resolve(repoRoot, process.env.CARGO_TARGET_DIR)
		: path.join(repoRoot, "target");
const targetRoot =
	process.env.CARGO_BUILD_TARGET !== undefined
		? path.join(targetDir, process.env.CARGO_BUILD_TARGET, profile)
		: path.join(targetDir, profile);
const source = path.join(targetRoot, binName);
const destinationDir = path.join(liteRoot, "resources/bin");
const destination = path.join(destinationDir, binName);

mkdirSync(destinationDir, { recursive: true });
copyFileSync(source, destination);
if (process.platform !== "win32") chmodSync(destination, 0o755);

process.stdout.write(
	`Copied ${path.relative(repoRoot, source)} to ${path.relative(repoRoot, destination)}\n`,
);
