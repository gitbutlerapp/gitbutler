import fs from 'node:fs';
import path from 'node:path';

/**
 * Write text to a file.
 *
 * The file and directory will be created if they do not exist.
 */
export function writeToFile(filePath: string, content: string): void {
	ensureDirectoryExists(filePath);
	fs.writeFileSync(filePath, content, { flag: 'w+', encoding: 'utf-8' });
}

function ensureDirectoryExists(filePath: string): void {
	const dir = path.dirname(filePath);
	if (!fs.existsSync(dir)) {
		fs.mkdirSync(dir, { recursive: true });
	}
}

/**
 * Write multiple files.
 */
export function writeFiles(files: Record<string, string>): void {
	for (const [filePath, content] of Object.entries(files)) {
		writeToFile(filePath, content);
	}
}
