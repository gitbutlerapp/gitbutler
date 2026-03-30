import useLocalStorageState from "use-local-storage-state";

export const usePreviewPanel = () =>
	useLocalStorageState("previewPanel", {
		defaultValue: true,
	});
