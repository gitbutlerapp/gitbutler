import useLocalStorageState from "use-local-storage-state";

export const usePreviewFullscreen = (projectId: string) =>
	useLocalStorageState(`project:${projectId}:showPreviewFullscreen`, {
		defaultValue: false,
	});
