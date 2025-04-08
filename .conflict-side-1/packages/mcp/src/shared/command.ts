import { z } from 'zod';
import { spawnSync } from 'child_process';

export function getGitButlerExecutable(): string | undefined {
	return process.env.GITBUTLER_EXECUTABLE_PATH;
}

export function hasGitButlerExecutable(): boolean {
	return !!getGitButlerExecutable();
}

/**
 * Executes a GitButler command with the given arguments and schema.
 *
 * The command is executed synchronously, and the output is parsed using the provided schema.
 */
export function executeGitButlerCommand<T>(
	projectDirectory: string,
	args: string[],
	schema: z.Schema<T>
): T {
	const executable = getGitButlerExecutable();

	if (!executable) throw new Error('Command error: No executable configured');

	const result = spawnSync(executable, ['--json', '-C', projectDirectory, ...args], {
		encoding: 'utf-8'
	});
	if (result.error) {
		throw new Error(`Command error: ${result.error.message}`);
	}

	if (result.status !== 0) {
		throw new Error(`Command error: ${result.stderr}`);
	}

	const parsed = JSON.parse(result.stdout);

	return schema.parse(parsed);
}
