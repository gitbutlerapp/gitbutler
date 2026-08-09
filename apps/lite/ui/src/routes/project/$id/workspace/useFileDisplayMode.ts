import { guiSettingsQueryOptions } from "#ui/api/queries.ts";
import { defaultSettings } from "#ui/settings.ts";
import { useQuery } from "@tanstack/react-query";
import type { FileDisplayMode } from "./file-tree.ts";

/**
 * Whether the file lists show a flat list or a folder tree. One stored setting
 * for every list, so a mode switched in one place is the mode everywhere.
 */
export const useFileDisplayMode = (): FileDisplayMode => {
	const { data } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.fileDisplayMode ?? defaultSettings.fileDisplayMode,
	});

	return data ?? defaultSettings.fileDisplayMode;
};
