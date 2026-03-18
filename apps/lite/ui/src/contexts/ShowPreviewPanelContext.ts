import { createContext } from "react";
import type { LocalStorageState } from "use-local-storage-state";

export const ShowPreviewPanelContext = createContext<LocalStorageState<boolean> | null>(null);
