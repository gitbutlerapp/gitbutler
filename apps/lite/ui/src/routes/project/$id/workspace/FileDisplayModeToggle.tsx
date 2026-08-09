import { useSaveGUISettings } from "#ui/api/mutations.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { Toolbar } from "@base-ui/react";
import type { FC } from "react";
import { getRowButtonClassName } from "./Row-utils.ts";
import { useFileDisplayMode } from "./useFileDisplayMode.ts";

/**
 * Switches every file list between flat and folder-shaped. It shows the mode it
 * would switch to rather than the one in force — the list below it is already
 * saying which that is.
 */
export const FileDisplayModeToggle: FC = () => {
	const mode = useFileDisplayMode();
	const { mutate: saveGUISettings } = useSaveGUISettings();
	const nextMode = mode === "tree" ? "list" : "tree";

	return (
		<Toolbar.Button
			aria-label={nextMode === "tree" ? "Show as tree" : "Show as list"}
			onClick={() => saveGUISettings({ fileDisplayMode: nextMode })}
			className={getRowButtonClassName({ size: "regular", iconOnly: true })}
		>
			<Icon name={nextMode === "tree" ? "folder-tree" : "list"} />
		</Toolbar.Button>
	);
};
