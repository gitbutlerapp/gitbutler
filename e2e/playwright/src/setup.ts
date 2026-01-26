import { setConfig } from './config.ts';
import { BUT_SERVER_PORT, BUT_TESTING, DESKTOP_PORT, GIT_CONFIG_GLOBAL } from './env.ts';
import { type BrowserContext } from '@playwright/test';
import { ChildProcess, spawn } from 'node:child_process';
import { existsSync, mkdirSync } from 'node:fs';
import { Socket } from 'node:net';
import path from 'node:path';

export function getBaseURL() {
	const port = parseInt(DESKTOP_PORT, 10);

	return `http://localhost:${port}`;
}

export function getButlerPort(): string {
	// Zero based parallel counter
	const id = parseInt(process.env.TEST_PARALLEL_INDEX ?? '0', 10);
	// Start from default + 1 to avoid interfering with dev server
	return `${parseInt(BUT_SERVER_PORT, 10) + id + 1}`;
}

export interface GitButler {
	pathInWorkdir: (...filePathSegments: string[]) => string;
	runScript(scriptName: string, args?: string[], env?: Record<string, string>): Promise<void>;
	destroy(): Promise<void>;
}

class GitButlerManager implements GitButler {
	private workdir: string;
	private configDir: string;
	private rootDir: string;
	private scriptsDir: string;
	private butServerProcess: ChildProcess;
	private env: Record<string, string> | undefined;

	constructor(
		workdir: string,
		configDir: string,
		env?: Record<string, string>,
		config?: Record<string, unknown>
	) {
		this.workdir = workdir;
		this.configDir = configDir;
		this.rootDir = path.resolve(import.meta.dirname, '../../..');
		this.scriptsDir = path.resolve(this.rootDir, 'e2e/playwright/scripts');
		this.env = env;

		if (!existsSync(this.workdir)) {
			mkdirSync(this.workdir, { recursive: true });
		}

		if (!existsSync(this.configDir)) {
			mkdirSync(this.configDir, { recursive: true });
			if (config) {
				setConfig(config, this.configDir);
			}
		}

		const serverEnv = {
			E2E_TEST_APP_DATA_DIR: this.configDir,
			BUTLER_PORT: getButlerPort(),
			GIT_CONFIG_GLOBAL,
			...this.env
		};

		this.butServerProcess = createButServerProcess(this.rootDir, serverEnv);

		this.butServerProcess.on('message', (message) => {
			log(`but-server message: ${message}`, colors.blue);
		});

		this.butServerProcess.on('close', (code) => {
			if (code !== 0 && code !== null) {
				console.error(`but-server failed with exit code ${code}`);
			}
		});

		this.butServerProcess.on('error', (error) => {
			console.error(`Error running but-server: ${error.message}`);
		});
	}

	async init() {
		const port = getButlerPort();
		const butReady = await waitForServer(port, 'localhost');
		if (!butReady) {
			throw new Error(`Butler server failed to start on localhost:${port}`);
		}
	}

	async destroy() {
		log('Stopping GitButler...');
		this.butServerProcess.kill('SIGTERM');
	}

	pathInWorkdir(...filePathSegments: string[]): string {
		return path.join(this.workdir, ...filePathSegments);
	}

	async runScript(
		scriptName: string,
		args?: string[],
		env?: Record<string, string>
	): Promise<void> {
		const scriptPath = path.resolve(this.scriptsDir, scriptName);
		if (!existsSync(scriptPath)) log(`Script not found: ${scriptPath}`, colors.red);
		const scriptArgs = args ?? [];

		const envVars = {
			GITBUTLER_CLI_DATA_DIR: getButlerDataDir(this.configDir),
			E2E_TEST_APP_DATA_DIR: this.configDir,
			GIT_CONFIG_GLOBAL,
			...this.env,
			...env
		};

		await runCommand('bash', [scriptPath, ...scriptArgs], this.workdir, envVars);
	}
}

function createButServerProcess(rootDir: string, serverEnv: Record<string, string>): ChildProcess {
	return spawnProcess('cargo', ['run', '-p', 'but-server'], rootDir, serverEnv);
}

function getButlerDataDir(configDir: string): string {
	return path.join(configDir, 'com.gitbutler.app');
}

async function waitForServer(port: string, host = 'localhost', maxAttempts = 500) {
	const parsed = parseInt(port, 10);

	for (let i = 0; i < maxAttempts; i++) {
		if (await checkPort(parsed, host)) {
			log(`✅ Server is ready on ${host}:${port}`, colors.green);
			return true;
		}

		if (i < maxAttempts - 1) {
			await new Promise((resolve) => setTimeout(resolve, 1000));
		}
	}

	return false;
}

async function checkPort(port: number, host = 'localhost') {
	return await new Promise((resolve) => {
		const socket = new Socket();

		socket.setTimeout(500);
		socket.on('connect', () => {
			socket.destroy();
			resolve(true);
		});

		socket.on('timeout', () => {
			socket.destroy();
			resolve(false);
		});

		socket.on('error', () => {
			resolve(false);
		});

		socket.connect(port, host);
	});
}

const VITE_HOST = 'localhost';

// Colors for console output
const colors = {
	reset: '\x1b[0m',
	bright: '\x1b[1m',
	green: '\x1b[32m',
	yellow: '\x1b[33m',
	red: '\x1b[31m',
	blue: '\x1b[34m',
	cyan: '\x1b[36m'
};

function log(message: string, color = colors.reset) {
	// eslint-disable-next-line no-console
	console.log(`${color}${message}${colors.reset}`);
}

function spawnProcess(
	command: string,
	args: string[],
	cwd = process.cwd(),
	env: Record<string, string> = {}
) {
	const child = spawn(command, args, {
		cwd,
		stdio: 'inherit',
		env: {
			...process.env,
			ELECTRON_ENV: 'development',
			VITE_BUILD_TARGET: 'web',
			BUT_TESTING: BUT_TESTING,
			VITE_HOST,
			RUST_LOG: 'error',
			...env
		}
	});

	return child;
}

export async function setCookie(
	name: string,
	value: string,
	context: BrowserContext
): Promise<void> {
	await context.addCookies([
		{
			name,
			value,
			domain: 'localhost',
			path: '/',
			httpOnly: false,
			secure: false,
			sameSite: 'Lax'
		}
	]);
}

/**
 * Set the project path cookie in the browser context.
 *
 * This is needed in order for the Frontend to be able to know the absolute paths of the
 * project files. The web file picker is not able to get absolute paths for security reasons.
 */
async function setProjectPathCookie(context: BrowserContext, workdir: string): Promise<void> {
	// Set the information about the workdir
	await setCookie('PROJECT_PATH', workdir, context);
}

/**
 * Set the butler server port cookie in the browser context.
 *
 * This is needed in order for the Frontend to know which port to use to connect to the butler server.
 */
async function setButlerServerPort(context: BrowserContext): Promise<void> {
	// Set the information about the workdir
	await setCookie('butlerPort', getButlerPort().toString(), context);
}

async function runCommand(
	command: string,
	args: string[],
	cwd = process.cwd(),
	env: Record<string, string> = {}
): Promise<void> {
	return await new Promise<void>((resolve, reject) => {
		log(`Running: ${command} ${args.join(' ')}`, colors.cyan);

		const child = spawnProcess(command, args, cwd, env);

		child.on('message', (message) => {
			log(`Child process message: ${message}`, colors.blue);
		});

		child.on('close', (code) => {
			if (code === 0) {
				resolve();
			} else {
				reject(new Error(`Command failed with exit code ${code}`));
			}
		});

		child.on('error', (error) => {
			console.error(`Error running command: ${error.message}`);
			reject(error);
		});
	});
}

export async function startGitButler(
	workdir: string,
	configDir: string,
	context: BrowserContext,
	env?: Record<string, string>,
	config?: Record<string, unknown>
): Promise<GitButler> {
	const manager = new GitButlerManager(workdir, configDir, env, config);
	await manager.init();
	await setProjectPathCookie(context, workdir);
	await setButlerServerPort(context);
	return manager;
}
