import { createContext } from "react";
import { UseState } from "#ui/hooks/useLocalStorageState.ts";

export const ShowPreviewPanelContext = createContext<UseState<boolean> | null>(null);
