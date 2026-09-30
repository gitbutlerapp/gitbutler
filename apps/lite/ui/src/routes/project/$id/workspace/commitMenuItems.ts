import { commitBody, commitTitle } from "#ui/commit.ts";
import { nativeMenuItem, type NativeMenuItem } from "#ui/native-menu.ts";
import type { Commit } from "@gitbutler/but-sdk";

/** The Copy submenu of a commit, shared by the workspace and branches lists. */
export const commitCopyMenuItem = (
	commit: Commit,
	message: string = commit.message,
): NativeMenuItem => {
	const title = commitTitle(message);
	const body = commitBody(message);

	return nativeMenuItem({
		label: "Copy",
		submenu: [
			nativeMenuItem({
				label: "Change ID",
				onSelect: () => window.lite.clipboardWriteText(commit.changeId),
			}),
			nativeMenuItem({
				label: "Commit ID",
				onSelect: () => window.lite.clipboardWriteText(commit.id),
			}),
			nativeMenuItem({
				label: "Commit Title",
				enabled: title !== undefined,
				onSelect: () => window.lite.clipboardWriteText(title ?? ""),
			}),
			nativeMenuItem({
				label: "Commit Body",
				enabled: body !== undefined,
				onSelect: () => window.lite.clipboardWriteText(body ?? ""),
			}),
		],
	});
};
