import { chmod, copyFile, readdir, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { NapiCli, createArtifactsCommand, readNapiConfig } from "@napi-rs/cli";

const options = createArtifactsCommand(process.argv.slice(2)).getOptions();
const { targets, binaryName } = await readNapiConfig(
	resolve(options.cwd, options.packageJsonPath),
	options.configPath && resolve(options.cwd, options.configPath),
);
const helpers = new Map();
const addons = new Set();

async function collect(directory) {
	for (const entry of await readdir(directory, { withFileTypes: true })) {
		const source = join(directory, entry.name);
		if (entry.isDirectory()) {
			if (entry.name !== "node_modules") await collect(source);
			continue;
		}
		for (const target of targets) {
			if (entry.name === `but-credential-cache.${target.platformArchABI}`) {
				helpers.set(target.platformArchABI, source);
			}
			if (entry.name === `${binaryName}.${target.platformArchABI}.node`) addons.add(target);
		}
	}
}

await collect(resolve(options.cwd, options.outputDir));
for (const target of addons) {
	if (target.platform !== "win32" && !helpers.has(target.platformArchABI)) {
		throw new Error(`Missing credential-cache executable for ${target.platformArchABI}`);
	}
}
await new NapiCli().artifacts(options);
for (const target of addons) {
	if (target.platform === "win32") continue;
	const filename = `but-credential-cache.${target.platformArchABI}`;
	const packageDir = resolve(options.cwd, options.npmDir, target.platformArchABI);
	const destination = join(packageDir, filename);
	await copyFile(helpers.get(target.platformArchABI), destination);
	await chmod(destination, 0o755);
	const manifestPath = join(packageDir, "package.json");
	const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
	manifest.files = [...new Set([...(manifest.files ?? []), filename])];
	await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
}
