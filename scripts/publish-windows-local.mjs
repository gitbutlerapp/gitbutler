import { readFileSync, readdirSync, rmSync, unlinkSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { spawn, spawnSync } from "node:child_process";

import { mergeDesktopEnv } from "./desktop-env.mjs";

const args = process.argv.slice(2);
let channel = "nightly";
let repo;
let bundleTarget = "msi";

for (let index = 0; index < args.length; index += 1) {
	const arg = args[index];

	if (arg === "--help") {
		printHelp();
		process.exit(0);
	}

	if (arg === "--channel") {
		channel = args[index + 1] ?? "";
		index += 1;
		continue;
	}

	if (arg === "--repo") {
		repo = args[index + 1] ?? "";
		index += 1;
		continue;
	}

	if (arg === "--bundle-target") {
		bundleTarget = args[index + 1] ?? "";
		index += 1;
		continue;
	}

	die(`Unknown argument: ${arg}`);
}

if (channel !== "nightly" && channel !== "release") {
	die("--channel must be either nightly or release");
}

if (bundleTarget !== "msi" && bundleTarget !== "nsis") {
	die("--bundle-target must be either msi or nsis");
}

const repoRoot = process.cwd();
const env = mergeDesktopEnv(channel, { repoRoot, includeRoot: true });
env.JS_PACKAGE_MANAGER ??= "bun";
env.WINDOWS_BUNDLE_TARGET ??= bundleTarget;

if (!env.TAURI_SIGNING_PRIVATE_KEY) {
	die(
		[
			"TAURI_SIGNING_PRIVATE_KEY is not set.",
			"Add TAURI_PRIVATE_KEY=... or TAURI_SIGNING_PRIVATE_KEY=... to .env, .env.local,",
			"apps/desktop/.env, or apps/desktop/.env.local before running this command.",
		].join(" "),
	);
}

if (!env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
	die(
		[
			"TAURI_SIGNING_PRIVATE_KEY_PASSWORD is not set.",
			"Add TAURI_KEY_PASSWORD=... or TAURI_SIGNING_PRIVATE_KEY_PASSWORD=... to .env, .env.local,",
			"apps/desktop/.env, or apps/desktop/.env.local before running this command.",
		].join(" "),
	);
}

const sha = runCapture("git", ["rev-parse", "HEAD"], repoRoot);
const shortSha = sha.slice(0, 7);
const now = new Date();
const startOfYear = Date.UTC(now.getUTCFullYear(), 0, 0);
const currentDay = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate());
const dayOfYear = Math.floor((currentDay - startOfYear) / 86_400_000);
const hourMinute = now.getUTCHours() * 100 + now.getUTCMinutes();
const version = `0.${dayOfYear}.${hourMinute}`;
const tag = `windows-build-${sha}`;
const releaseName = `GitButler Windows build ${shortSha}`;
const repository = repo || inferRepo(repoRoot);
const gitBash = resolveGitBash();

rmSync(resolve(repoRoot, "release"), { force: true, recursive: true });

await run(
	"bun",
	["run", "build:desktop", "--", "--mode", channel === "release" ? "production" : "nightly"],
	{
		cwd: repoRoot,
		env,
	},
);

if (channel !== "release") {
	await run("bun", ["run", "build:desktop"], {
		cwd: repoRoot,
		env: {
			...env,
			SVELTEKIT_OUT_DIR: "embedded-frontend",
			VITE_BUILD_TARGET: "web",
			VITE_BUTLER_API_BASE_URL: "/api",
			VITE_EMBEDDED_BUILD: "1",
		},
	});
}

const normalizedReleaseScript = createNormalizedBashScript(repoRoot, "scripts/release.sh", env);
const bashEnv = { ...env };
delete bashEnv.TAURI_PRIVATE_KEY;
delete bashEnv.TAURI_SIGNING_PRIVATE_KEY;
delete bashEnv.TAURI_KEY_PASSWORD;
delete bashEnv.TAURI_SIGNING_PRIVATE_KEY_PASSWORD;

try {
	await run(
		gitBash,
		[
			normalizedReleaseScript.bashPath,
			"--channel",
			channel,
			"--dist",
			"./release",
			"--version",
			version,
		],
		{
			cwd: repoRoot,
			env: bashEnv,
		},
	);
} finally {
	unlinkSync(normalizedReleaseScript.absolutePath);
}

const releaseDir = resolve(repoRoot, "release", "windows", "x86_64");
const assets = readdirSync(releaseDir)
	.filter((entry) => {
		if (bundleTarget === "nsis") return entry.endsWith("-setup.exe");
		return entry.endsWith(".msi") || entry.endsWith(".msi.zip") || entry.endsWith(".msi.zip.sig");
	})
	.map((entry) => resolve(releaseDir, entry));

if (assets.length === 0) {
	die(`No Windows release artifacts were found in ${releaseDir}`);
}

if (hasRelease(tag, repository)) {
	await run("gh", ["release", "upload", tag, ...assets, "--clobber", "-R", repository], {
		cwd: repoRoot,
		env,
	});
} else {
	await run(
		"gh",
		[
			"release",
			"create",
			tag,
			...assets,
			"--target",
			sha,
			"--title",
			releaseName,
			"--generate-notes",
			"--prerelease",
			"-R",
			repository,
		],
		{
			cwd: repoRoot,
			env,
		},
	);
}

console.log(`Published ${assets.length} Windows artifact(s) to ${repository} release ${tag}.`);

/**
 * @param {string} command
 * @param {string[]} commandArgs
 * @param {{ cwd: string, env: NodeJS.ProcessEnv }} options
 * @returns {Promise<void>}
 */
function run(command, commandArgs, options) {
	return new Promise((resolvePromise, rejectPromise) => {
		const child = spawn(command, commandArgs, {
			cwd: options.cwd,
			env: options.env,
			stdio: "inherit",
		});

		child.on("exit", (code, signal) => {
			if (signal) {
				rejectPromise(new Error(`${command} exited from signal ${signal}`));
				return;
			}

			if (code !== 0) {
				rejectPromise(new Error(`${command} exited with code ${code ?? 1}`));
				return;
			}

			resolvePromise();
		});

		child.on("error", rejectPromise);
	});
}

/**
 * @param {string} tag
 * @param {string} repository
 * @returns {boolean}
 */
function hasRelease(tag, repository) {
	const result = spawnSync("gh", ["release", "view", tag, "-R", repository], {
		encoding: "utf8",
		stdio: "ignore",
	});
	return result.status === 0;
}

/**
 * @param {string} command
 * @param {string[]} commandArgs
 * @param {string} cwd
 * @returns {string}
 */
function runCapture(command, commandArgs, cwd) {
	const result = spawnSync(command, commandArgs, {
		cwd,
		encoding: "utf8",
		stdio: ["ignore", "pipe", "inherit"],
	});

	if (result.status !== 0) {
		die(`${command} exited with code ${result.status ?? 1}`);
	}

	return result.stdout.trim();
}

/**
 * @returns {string}
 */
function resolveGitBash() {
	if (process.platform !== "win32") return "bash";

	const gitPath = runCapture("where", ["git"], process.cwd()).split(/\r?\n/).find(Boolean);

	if (!gitPath) {
		die("Could not locate git.exe to derive Git Bash.");
	}

	const gitBashPath = resolve(gitPath, "..", "..", "bin", "bash.exe");
	return gitBashPath;
}

/**
 * @param {string} cwd
 * @returns {string}
 */
function inferRepo(cwd) {
	const remote = runCapture("git", ["config", "--get", "remote.origin.url"], cwd);
	const match =
		remote.match(/github\.com[:/](.+?)(?:\.git)?$/) ||
		remote.match(/^([A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+)$/);

	if (!match) {
		die("Could not determine the GitHub repository. Pass --repo owner/name.");
	}

	return match[1];
}

function printHelp() {
	console.log(
		"Usage: node scripts/publish-windows-local.mjs [--channel nightly|release] [--repo owner/name] [--bundle-target msi|nsis]",
	);
	console.log("");
	console.log(
		"Loads env from .env/.env.local and apps/desktop/.env files, builds locally, then publishes the Windows artifacts to a GitHub prerelease.",
	);
	console.log(
		"Use TAURI_PRIVATE_KEY and TAURI_KEY_PASSWORD in your env file; they are mapped to the Tauri signing env names automatically.",
	);
}

/**
 * @param {string} repoRoot
 * @param {string} relativePath
 * @param {NodeJS.ProcessEnv} env
 * @returns {{ absolutePath: string, bashPath: string }}
 */
function createNormalizedBashScript(repoRoot, relativePath, env) {
	const sourcePath = resolve(repoRoot, relativePath);
	const targetPath = resolve(repoRoot, "scripts", ".release.local.sh");
	const normalized = readFileSync(sourcePath, "utf8").replace(/\r\n/g, "\n");
	const exports = [
		toHereDocExport("TAURI_SIGNING_PRIVATE_KEY", env.TAURI_SIGNING_PRIVATE_KEY ?? ""),
		toHereDocExport(
			"TAURI_SIGNING_PRIVATE_KEY_PASSWORD",
			env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ?? "",
		),
	];
	writeFileSync(targetPath, `${exports.join("\n")}\n${normalized}`, "utf8");
	return {
		absolutePath: targetPath,
		bashPath: "./scripts/.release.local.sh",
	};
}

/**
 * @param {string} name
 * @param {string} value
 * @returns {string}
 */
function toHereDocExport(name, value) {
	return [`export ${name}="$(cat <<'__COPILOT_${name}__'`, value, `__COPILOT_${name}__`, ')"'].join(
		"\n",
	);
}

/**
 * @param {string} message
 * @returns {never}
 */
function die(message) {
	console.error(message);
	process.exit(1);
}
