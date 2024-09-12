import { readBinaryFile } from '@tauri-apps/api/fs';

export async function getBinFileSize(path: string): Promise<string> {
	try {
		const contents = await readBinaryFile(path);
		const sizeInBytes = contents.length;
		return formatFileSize(sizeInBytes);
	} catch (error) {
		console.error('Failed to get file size:', error);
		return 'Unknown size';
	}
}

const KB = 1024;
const MB = 1024 * 1024;
const GB = 1024 * 1024 * 1024;

function formatFileSize(bytes: number): string {
	if (bytes < KB) return bytes + ' B';
	else if (bytes < MB) return (bytes / KB).toFixed(1) + ' KB';
	else if (bytes < GB) return (bytes / MB).toFixed(1) + ' MB';
	else return (bytes / GB).toFixed(1) + ' GB';
}
