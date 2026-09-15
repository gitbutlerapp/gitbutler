import { app } from "electron";
import { installCliV2 } from "@gitbutler/but-sdk";
import path from "node:path";

export const installCli = async (): Promise<boolean> => {
	if (process.platform !== "darwin" || !app.isPackaged)
		throw new Error("CLI installation requires the packaged macOS app");
	if (!app.isInApplicationsFolder())
		throw new Error("Move GitButler to Applications before installing the CLI");

	return installCliV2(path.join(process.resourcesPath, "bin", "but"), "Refuse");
};
