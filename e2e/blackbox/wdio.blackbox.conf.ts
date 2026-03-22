import { TestRecorder } from "./record.js";
import { spawn, type ChildProcess } from "node:child_process";
import os from "node:os";
import path from "node:path";
import type { Frameworks } from "@wdio/types";

const videoRecorder = new TestRecorder();
let tauriDriver: ChildProcess;

export const config = {
	hostname: "127.0.0.1",
	runner: "local",
	port: 4444,
	specs: ["./tests/**/*.spec.ts"],
	maxInstances: 1,
	capabilities: [
		{
			"tauri:options": {
				application: "../target/debug/gitbutler-tauri",
			},
		},
	],
	reporters: ["spec"],
	framework: "mocha",
	mochaOpts: {
		ui: "bdd",
		timeout: 60000,
	},
	autoCompileOpts: {
		autoCompile: true,
		tsNodeOpts: {
			project: "./tsconfig.json",
			transpileOnly: true,
		},
	},

	waitforTimeout: 10000,
	connectionRetryTimeout: 120000,
	connectionRetryCount: 0,

	beforeTest: async function (test: Frameworks.Test) {
		const videoPath = path.join(import.meta.dirname, "videos");
		videoRecorder.start(test, videoPath);
	},

	afterTest: async function (_test: Frameworks.Test, result: Frameworks.TestResult) {
		await sleep(2000); // Let browser settle before stopping.
		videoRecorder.stop();

		if (result.error) {
			// Dump browser console logs on failure.
			try {
				const logs = await browser.getLogs("browser");
				if (logs.length > 0) {
					const logDir = path.join(import.meta.dirname, "logs");
					mkdirSync(logDir, { recursive: true });
					const logPath = path.join(logDir, `${Date.now()}-console.json`);
					writeFileSync(logPath, JSON.stringify(logs, null, 2));
					console.error("[E2E] Browser console logs:");
					for (const entry of logs) {
						console.error(`  [${entry.level}] ${entry.message}`);
					}
				}
			} catch (e) {
				// getLogs may not be supported by all WebDriver implementations.
				console.error("[E2E] Could not retrieve browser console logs:", e);
			}
		}
	},

	// ensure we are running `tauri-driver` before the session starts so that we can proxy the webdriver requests
	beforeSession: () =>
		(tauriDriver = spawn(path.resolve(os.homedir(), ".cargo", "bin", "tauri-driver"), [], {
			stdio: [null, process.stdout, process.stderr],
		})),

	afterSession: () => {
		tauriDriver.kill();
	},
};

async function sleep(ms: number) {
	return await new Promise((resolve) => setTimeout(resolve, ms));
}
