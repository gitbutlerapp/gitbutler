import useLocalStorageState from "use-local-storage-state";

export const useFullscreenPreview = (projectId: string) =>
	useLocalStorageState(`project:${projectId}:showFullscreenPreview`, {
		defaultValue: false,
	});
