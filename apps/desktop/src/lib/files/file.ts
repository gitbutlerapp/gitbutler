export type FileInfo = {
	/**
	 * File contents, or `null` when the backend has none to provide (e.g. a
	 * binary file). Mirrors the backend's `Option<String>` (`gitbutler_repo::FileInfo`).
	 */
	content: string | null;
	name?: string;
	mimeType?: string;
	size?: number;
};
