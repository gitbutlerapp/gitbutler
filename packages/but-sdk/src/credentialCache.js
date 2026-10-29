import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);

export function initializeCredentialCache(flavor, initialize) {
	if (process.platform === "win32") return;
	const platform = `${process.platform}-${process.arch}${process.platform === "linux" ? "-gnu" : ""}`;
	const filename = `but-credential-cache.${platform}`;
	let executable = fileURLToPath(new URL(`./generated/${flavor}/${filename}`, import.meta.url));
	if (!existsSync(executable)) {
		executable = require.resolve(`@gitbutler/but-sdk-${platform}/${filename}`);
	}
	// Electron keeps executable assets outside its application archive.
	const unpacked = executable.replace(/\.asar([/\\])/, ".asar.unpacked$1");
	if (unpacked !== executable && existsSync(unpacked)) executable = unpacked;
	initialize(executable);
}
