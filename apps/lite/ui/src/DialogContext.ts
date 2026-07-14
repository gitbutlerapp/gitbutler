import { createContext } from "react";

export type Dialog =
	| { _tag: "None" }
	| { _tag: "ApplyBranchPicker" }
	| { _tag: "BranchPicker" }
	| { _tag: "CommandPalette" }
	| { _tag: "ProjectPicker" }
	| { _tag: "Settings" };

type DialogContext = {
	dialog: Dialog;
	openDialog: (dialog: Dialog) => void;
	closeDialog: () => void;
};

export const DialogContext = createContext({} as DialogContext);
DialogContext.displayName = "DialogContext";
