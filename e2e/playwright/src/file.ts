import fs from 'node:fs';
import path from 'node:path';

/**
 * Write text to a file.
 *
 * The file and directory will be created if they do not exist.
 */
export function writeToFile(filePath: string, content: string): void {
	ensuereDirectoryExists(filePath);
	fs.writeFileSync(filePath, content, { flag: 'w+', encoding: 'utf-8' });
}

function ensuereDirectoryExists(filePath: string): void {
	const dir = path.dirname(filePath);
	if (!fs.existsSync(dir)) {
		fs.mkdirSync(dir, { recursive: true });
	}
}
