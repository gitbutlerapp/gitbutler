import { createContext } from "react";

type FilesVisibleContext = {
	filesVisible: boolean;
	toggleFiles: () => void;
};

export const FilesVisibleContext = createContext({} as FilesVisibleContext);
FilesVisibleContext.displayName = "FilesVisibleContext";

type FilesVisibleRegistryContext = (projectId: string) => FilesVisibleContext;

export const FilesVisibleRegistryContext = createContext({} as FilesVisibleRegistryContext);
FilesVisibleRegistryContext.displayName = "FilesVisibleRegistryContext";
