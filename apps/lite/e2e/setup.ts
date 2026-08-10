import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";

const repoRoot = path.resolve(import.meta.dirname, "../../..");
const fixtureScriptsDir = path.join(repoRoot, "e2e/playwright/scripts");
const fixtureGitConfig = path.join(repoRoot, "e2e/playwright/fixtures/.gitconfig");

export type LiteTestEnvironment = {
	appDataDir: string;
	electronUserDataDir: string;
	gitConfig: string;
	rootDir: string;
	workdir: string;
};

export const processEnvironment = (overrides: Record<string, string>): Record<string, string> =>
	Object.fromEntries(
		Object.entries({ ...process.env, ...overrides }).filter(
			(entry): entry is [string, string] => entry[1] !== undefined,
		),
	);

export const createLiteTestEnvironment = (): LiteTestEnvironment => {
	const rootDir = mkdtempSync(path.join(os.tmpdir(), "gitbutler-lite-e2e-"));
	const appDataDir = path.join(rootDir, "app-data");
	const electronUserDataDir = path.join(rootDir, "electron-user-data");
	const gitConfig = path.join(rootDir, "gitconfig");
	const workdir = path.join(rootDir, "workdir");

	for (const directory of [appDataDir, electronUserDataDir, workdir])
		mkdirSync(directory, { recursive: true });

	const credentialStore = path.join(rootDir, "git-credentials");
	const baseGitConfig = readFileSync(fixtureGitConfig, "utf8").trimEnd();
	writeFileSync(
		gitConfig,
		`${baseGitConfig}\n[credential]\n\thelper = store --file ${credentialStore}\n`,
	);
	writeFileSync(
		path.join(electronUserDataDir, "settings.json"),
		JSON.stringify({ version: 1, autoUpdate: false, theme: "light" }, null, "\t"),
	);

	return { appDataDir, electronUserDataDir, gitConfig, rootDir, workdir };
};

export const removeLiteTestEnvironment = (environment: LiteTestEnvironment): void => {
	rmSync(environment.rootDir, { force: true, maxRetries: 3, recursive: true, retryDelay: 100 });
};

export const seedScenario = async (
	scenario: string,
	environment: LiteTestEnvironment,
): Promise<void> => {
	const scriptPath = path.join(fixtureScriptsDir, scenario);
	if (!existsSync(scriptPath)) throw new Error(`Fixture script does not exist: ${scriptPath}`);

	const but = process.env.BUT ?? path.join(repoRoot, "target/debug/but");
	if (!existsSync(but)) {
		throw new Error(
			`GitButler CLI does not exist at ${but}; build it with \`cargo build -p but\`.`,
		);
	}

	const { promise, resolve, reject } = Promise.withResolvers<void>();
	const child = spawn("bash", [scriptPath], {
		cwd: environment.workdir,
		stdio: "inherit",
		env: processEnvironment({
			BUT: but,
			E2E_TEST_APP_DATA_DIR: environment.appDataDir,
			GIT_CONFIG_GLOBAL: environment.gitConfig,
		}),
	});

	child.on("error", reject);
	child.on("close", (code) => {
		if (code === 0) resolve();
		else reject(new Error(`Fixture script ${scenario} failed with exit code ${code ?? "unknown"}`));
	});

	await promise;
};

export const paths = {
	electronMain: path.join(repoRoot, "apps/lite/dist/electron/main.js"),
	repoRoot,
};
