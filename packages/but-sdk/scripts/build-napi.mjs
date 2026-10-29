import { chmod, rename } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { NapiCli, createBuildCommand } from "@napi-rs/cli";

const command = createBuildCommand(process.argv.slice(2));
const options = { ...command.getOptions(), cargoOptions: command.cargoOptions };
const cli = new NapiCli();
const outputs = await (await cli.build(options)).task;
const addon = outputs.find((output) => output.kind === "node");
if (addon) {
	const platform = basename(addon.path).split(".").at(-2);
	if (!platform.startsWith("win32-")) {
		const helperOutputs = await (
			await cli.build({
				...options,
				manifestPath: "../../crates/but-utils/Cargo.toml",
				package: "but-utils",
				bin: "but-credential-cache",
				features: undefined,
				allFeatures: false,
				noDefaultFeatures: false,
				noJsBinding: true,
				jsBinding: undefined,
				dts: undefined,
			})
		).task;
		const helper = helperOutputs.find((output) => output.kind === "exe");
		if (!helper) throw new Error("The credential-cache executable was not built");
		const destination = join(dirname(addon.path), `but-credential-cache.${platform}`);
		await rename(helper.path, destination);
		await chmod(destination, 0o755);
	}
}
