import useLocalStorageState from "use-local-storage-state";

export const usePreviewVisible = () =>
	useLocalStorageState("previewVisible", {
		defaultValue: true,
	});
