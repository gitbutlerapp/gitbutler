import { invoke, listen } from '$lib/ipc';

export type Contents = { type: 'utf8'; value: string } | { type: 'binary' } | { type: 'large' };

export namespace Contents {
	export function isLarge(contents: Contents): contents is { type: 'large' } {
		return contents.type === 'large';
	}

	export function isBinary(contents: Contents): contents is { type: 'binary' } {
		return contents.type === 'binary';
	}

	export function isUtf8(contents: Contents): contents is { type: 'utf8'; value: string } {
		return contents.type === 'utf8';
	}

	export function value(contents: Contents): string | undefined {
		if (isUtf8(contents)) {
			return contents.value;
		}
		return undefined;
	}
}

export async function list(params: { projectId: string; sessionId: string; paths?: string[] }) {
	return invoke<Partial<Record<string, Contents>>>('list_session_files', params);
}

export function subscribe(
	params: { projectId: string; sessionId: string },
	callback: (params: { filePath: string; contents: Contents | null }) => Promise<void> | void
) {
	return listen<{ contents: Contents | null; filePath: string }>(
		`project://${params.projectId}/sessions/${params.sessionId}/files`,
		(event) => callback({ ...params, ...event.payload })
	);
}
