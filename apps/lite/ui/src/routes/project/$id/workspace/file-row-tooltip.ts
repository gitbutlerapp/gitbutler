import type { FocusScope } from "#ui/focus-scopes.ts";
import { Tooltip } from "@base-ui/react";
import type { HotkeySequence } from "@tanstack/react-hotkeys";

export type FileRowTooltipPayload = {
	content: string;
	kbd?: string | HotkeySequence;
	kbdScope?: FocusScope;
};

export type FileRowTooltipHandles = {
	row: Tooltip.Handle<FileRowTooltipPayload>;
	control: Tooltip.Handle<FileRowTooltipPayload>;
};

/**
 * Passed to useState as a lazy initializer so each FilesTree gets one stable, unique pair of
 * stores. Sharing stores would let same-path rows in different trees replace each other's trigger.
 */
export const createFileRowTooltipHandles = (): FileRowTooltipHandles => ({
	row: Tooltip.createHandle<FileRowTooltipPayload>(),
	// Nested triggers must use a different store: opening a control intentionally closes the row's
	// hover tooltip, but must not close the control tooltip it just opened.
	control: Tooltip.createHandle<FileRowTooltipPayload>(),
});
