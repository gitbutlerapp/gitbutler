import { spawn } from 'child_process';
import net from 'net';
import path from 'path';

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

const VITE_PORT = 1421;
const VITE_HOST = 'localhost';
const BUTLER_PORT = 6978;
const BUTLER_HOST = 'localhost';

let viteProcess = null;
let electronProcess = null;
let butProcess = null;

function log(message, color = colors.reset) {
	// eslint-disable-next-line no-console
	console.log(`${color}${message}${colors.reset}`);
}

function spawnProcess(command, args, cwd = process.cwd(), options = {}) {
	return spawn(command, args, {
		cwd,
		stdio: 'inherit',
		// shell: true,
		...options,
		env: {
			...process.env,
			ELECTRON_ENV: 'development',
			VITE_BUILD_TARGET: 'web',
			VITE_PORT,
			VITE_HOST,
			BUTLER_PORT,
			BUTLER_HOST
		}
	});
}

async function runCommand(command, args, cwd = process.cwd()) {
	return await new Promise((resolve, reject) => {
		log(`Running: ${command} ${args.join(' ')}`, colors.cyan);

		const child = spawnProcess(command, args, cwd);

		child.on('close', (code) => {
			if (code === 0) {
				resolve();
			} else {
				reject(new Error(`Command failed with exit code ${code}`));
			}
		});

		child.on('error', (error) => {
			reject(error);
		});
	});
}

async function checkPort(port, host = 'localhost') {
	return await new Promise((resolve) => {
		const socket = new net.Socket();

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

async function waitForServer(port, host = 'localhost', maxAttempts = 30) {
	log(`Waiting for server on ${host}:${port}...`, colors.yellow);

	for (let i = 0; i < maxAttempts; i++) {
		if (await checkPort(port, host)) {
			log(`✅ Server is ready on ${host}:${port}`, colors.green);
			return true;
		}

		if (i < maxAttempts - 1) {
			await new Promise((resolve) => setTimeout(resolve, 1000));
		}
	}

	return false;
}

function cleanup() {
	log('\n👋 Shutting down...', colors.yellow);

	if (electronProcess) {
		log('Stopping Electron...', colors.yellow);
		electronProcess.kill();
		electronProcess = null;
	}

	if (viteProcess) {
		log('Stopping Vite dev server...', colors.yellow);
		viteProcess.kill();
		viteProcess = null;
	}

	if (butProcess) {
		log('Stopping butler server...', colors.yellow);
		butProcess.kill();
		butProcess = null;
	}
}

async function main() {
	try {
		log('🚀 Starting GitButler Buzz Development Server', colors.bright + colors.green);

		// Get paths
		const rootDir = path.resolve(import.meta.dirname, '../..');
		const desktopDir = path.resolve(rootDir, 'apps/desktop');
		const buzzDir = import.meta.dirname;

		log('\n🔧 Building TypeScript for Buzz...', colors.yellow);

		// Build the buzz TypeScript
		await runCommand('pnpm', ['build-ts'], buzzDir);

		log('\n📦 Starting Vite dev server...', colors.yellow);

		// Start the Vite dev server
		viteProcess = spawnProcess('pnpm', ['dev', '--port', VITE_PORT], desktopDir);

		viteProcess.on('close', (code) => {
			if (code !== 0 && code !== null) {
				log(`Vite dev server exited with code ${code}`, colors.red);
			}
		});

		viteProcess.on('error', (error) => {
			log(`Vite dev server error: ${error.message}`, colors.red);
		});

		// Wait for Vite to be ready
		const serverReady = await waitForServer(VITE_PORT, VITE_HOST);

		if (!serverReady) {
			throw new Error(`Vite dev server failed to start on ${VITE_HOST}:${VITE_PORT}`);
		}

		log('\n📦 Starting but-server server...', colors.yellow);

		// Start the but-server server
		butProcess = spawnProcess('cargo', ['run', '-p', 'but', '--', 'serve'], rootDir);

		butProcess.on('close', (code) => {
			if (code !== 0 && code !== null) {
				log(`Butler server exited with code ${code}`, colors.red);
			}
		});

		butProcess.on('error', (error) => {
			log(`Butler server error: ${error.message}`, colors.red);
		});

		// Wait for Vite to be ready
		const butReady = await waitForServer(BUTLER_PORT, BUTLER_HOST);

		if (!butReady) {
			throw new Error(`Butler server failed to start on ${BUTLER_HOST}:${BUTLER_PORT}`);
		}

		log('\n⚡ Starting Electron app...', colors.green);

		// Start the electron app
		electronProcess = spawnProcess('electron', ['.'], buzzDir);

		electronProcess.on('close', (code) => {
			log(`Electron app exited with code ${code}`, colors.yellow);
			cleanup();
			process.exit(code || 0);
		});

		electronProcess.on('error', (error) => {
			log(`Electron app error: ${error.message}`, colors.red);
			cleanup();
			process.exit(1);
		});
	} catch (error) {
		log(`\n❌ Error: ${error.message}`, colors.red);
		cleanup();
		process.exit(1);
	}
}

// Handle process termination
process.on('SIGINT', () => {
	cleanup();
	process.exit(0);
});

process.on('SIGTERM', () => {
	cleanup();
	process.exit(0);
});

process.on('exit', () => {
	cleanup();
});

main();
