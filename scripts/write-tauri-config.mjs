import { readFileSync, writeFileSync } from "node:fs";

const [inputPath, outputPath, version, externalBinJson, bundleTargetsJson] = process.argv.slice(2);

if (!inputPath || !outputPath || !version || !externalBinJson) {
	console.error(
		"Usage: node scripts/write-tauri-config.mjs <input> <output> <version> <externalBinJson> [bundleTargetsJson]",
	);
	process.exit(1);
}

const config = JSON.parse(readFileSync(inputPath, "utf8"));
config.version = version;
config.bundle.externalBin = JSON.parse(externalBinJson);

if (bundleTargetsJson) {
	config.bundle.targets = JSON.parse(bundleTargetsJson);
}

writeFileSync(outputPath, `${JSON.stringify(config, null, "\t")}\n`, "utf8");
