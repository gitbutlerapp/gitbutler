import path from 'node:path';

const ROOT = path.resolve(import.meta.dirname, '../../..');

export const BUT_SERVER_PORT = process.env.BUTLER_PORT || '6978';
export const DESKTOP_PORT = process.env.DESKTOP_PORT || '3000';
export const BUT_TESTING =
	process.env.BUT_TESTING || path.join(ROOT, 'target', 'debug', 'but-testing');
