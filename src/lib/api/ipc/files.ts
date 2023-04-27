import { invoke } from '$lib/ipc';
import { clone } from '$lib/utils';

const cache: Record<string, Record<string, Promise<Record<string, string>>>> = {};

export const list = async (params: { projectId: string; sessionId: string; paths?: string[] }) => {
	const sessionFilesCache = cache[params.projectId] || {};
	if (params.sessionId in sessionFilesCache) {
		return sessionFilesCache[params.sessionId].then((files) =>
			Object.fromEntries(
				Object.entries(clone(files)).filter(([path]) =>
					params.paths ? params.paths.includes(path) : true
				)
			)
		);
	}

	const promise = invoke<Record<string, string>>('list_session_files', {
		sessionId: params.sessionId,
		projectId: params.projectId
	});
	sessionFilesCache[params.sessionId] = promise;
	cache[params.projectId] = sessionFilesCache;
	return promise.then((files) =>
		Object.fromEntries(
			Object.entries(clone(files)).filter(([path]) =>
				params.paths ? params.paths.includes(path) : true
			)
		)
	);
};
