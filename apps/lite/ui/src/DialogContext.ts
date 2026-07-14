import type { Dialog } from "#ui/projects/project.ts";
import { createContext } from "react";

type DialogContext = {
	dialog: Dialog;
	openDialog: (dialog: Dialog) => void;
	closeDialog: () => void;
};

export const DialogContext = createContext({} as DialogContext);
DialogContext.displayName = "DialogContext";
