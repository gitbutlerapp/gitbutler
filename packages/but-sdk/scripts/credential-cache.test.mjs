import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import {
	copyFile,
	mkdir,
	mkdtemp,
	readFile,
	realpath,
	rm,
	stat,
	writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";
import { promisify } from "node:util";
import { NapiCli } from "@napi-rs/cli";

const run = promisify(execFile);
const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const platform = `${process.platform}-${process.arch}${process.platform === "linux" ? "-gnu" : ""}`;
const helperName = `but-credential-cache.${platform}`;

async function fixture(t) {
	const root = await realpath(await mkdtemp(join(tmpdir(), "sdk # percent% ")));
	t.after(() => rm(root, { recursive: true, force: true }));
	return root;
}

async function write(path, content) {
	await mkdir(dirname(path), { recursive: true });
	await writeFile(path, content);
}

async function runtime(root) {
	await write(join(root, "package.json"), JSON.stringify({ type: "module" }));
	for (const file of ["credentialCache.js", "linear.js", "graph.js"]) {
		await mkdir(join(root, "src"), { recursive: true });
		await copyFile(join(packageRoot, "src", file), join(root, "src", file));
	}
	for (const flavor of ["linear", "graph"]) {
		await write(
			join(root, "src", "generated", flavor, "index.js"),
			"export const registrations = []; export function credentialCacheInit(path) { registrations.push(path); }",
		);
	}
}

test("each SDK flavor registers its local executable with URL-sensitive paths intact", async (t) => {
	const root = await fixture(t);
	await runtime(root);
	for (const flavor of ["linear", "graph"]) {
		const executable = join(root, "src", "generated", flavor, helperName);
		await write(executable, "helper");
		const { registrations } = await import(pathToFileURL(join(root, "src", `${flavor}.js`)));
		assert.deepEqual(registrations, process.platform === "win32" ? [] : [executable]);
	}
});

test(
	"SDK finds the executable in the optional platform package",
	{ skip: process.platform === "win32" },
	async (t) => {
		const root = await fixture(t);
		await runtime(root);
		const executable = join(root, "node_modules", "@gitbutler", `but-sdk-${platform}`, helperName);
		await write(executable, "helper");
		const { registrations } = await import(pathToFileURL(join(root, "src", "linear.js")));
		assert.deepEqual(registrations, [executable]);
	},
);

test("Electron uses the unpacked executable", { skip: process.platform === "win32" }, async (t) => {
	const root = await fixture(t);
	const archive = join(root, "app.asar");
	await runtime(archive);
	const relative = join("src", "generated", "linear", helperName);
	await write(join(archive, relative), "archived helper");
	const executable = join(root, "app.asar.unpacked", relative);
	await write(executable, "unpacked helper");
	const { registrations } = await import(pathToFileURL(join(archive, "src", "linear.js")));
	assert.deepEqual(registrations, [executable]);
});

test(
	"a missing SDK executable fails during initialization",
	{ skip: process.platform === "win32" },
	async (t) => {
		const root = await fixture(t);
		await runtime(root);
		await assert.rejects(import(pathToFileURL(join(root, "src", "linear.js"))), {
			code: "MODULE_NOT_FOUND",
		});
	},
);

async function artifacts(root) {
	await write(
		join(root, "package.json"),
		JSON.stringify({
			name: "@gitbutler/but-sdk",
			version: "0.0.1",
			napi: {
				binaryName: "gitbutler-sdk",
				targets: ["aarch64-apple-darwin", "x86_64-pc-windows-msvc"],
			},
		}),
	);
	await new NapiCli().createNpmDirs({ cwd: root });
	await write(join(root, "artifacts", "macos", "gitbutler-sdk.darwin-arm64.node"), "mac addon");
	await write(
		join(root, "artifacts", "windows", "gitbutler-sdk.win32-x64-msvc.node"),
		"windows addon",
	);
}

test("platform packages include an executable helper, while Windows needs none", async (t) => {
	const root = await fixture(t);
	await artifacts(root);
	const filename = "but-credential-cache.darwin-arm64";
	await write(join(root, "artifacts", "macos", filename), "native helper");
	await run(process.execPath, [join(packageRoot, "scripts", "artifacts.mjs"), "--cwd", root]);
	const packageDir = join(root, "npm", "darwin-arm64");
	assert.equal(await readFile(join(packageDir, filename), "utf8"), "native helper");
	if (process.platform !== "win32")
		assert.equal((await stat(join(packageDir, filename))).mode & 0o777, 0o755);
	const manifest = JSON.parse(await readFile(join(packageDir, "package.json"), "utf8"));
	assert.ok(manifest.files.includes(filename));
	assert.ok(manifest.files.includes("gitbutler-sdk.darwin-arm64.node"));
});

test("artifact collection rejects a Unix addon without its helper", async (t) => {
	const root = await fixture(t);
	await artifacts(root);
	await assert.rejects(
		run(process.execPath, [join(packageRoot, "scripts", "artifacts.mjs"), "--cwd", root]),
		/Missing credential-cache executable for darwin-arm64/,
	);
});
