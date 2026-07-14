import { createContext } from "react";

type FilesVisibleContext = {
	filesVisible: boolean;
	toggleFiles: () => void;
};

export const FilesVisibleContext = createContext({} as FilesVisibleContext);
FilesVisibleContext.displayName = "FilesVisibleContext";
