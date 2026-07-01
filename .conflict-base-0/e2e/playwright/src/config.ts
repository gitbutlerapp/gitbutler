import fs from "fs";
import path from "path";

const SETTIGNS_FILE = path.join("gitbutler", "settings.json");

export function setConfig(config: Record<string, unknown>, configDir: string): void {
	fs.mkdirSync(path.join(configDir, "gitbutler"), { recursive: true });
	const settingsPath = path.join(configDir, SETTIGNS_FILE);
	fs.writeFileSync(settingsPath, JSON.stringify(config, null, 2), "utf-8");
}
