import type { FileParent } from "#ui/addresses.ts";
import {
	changesFileHotkeys,
	selectionOperationHotkeys,
	toElectronAccelerator,
} from "#ui/hotkeys.ts";
import { type NativeMenuItem, nativeMenuItem } from "#ui/native-menu.ts";
import type { useFileSetActions, useFileSetSubject } from "./useFileSetActions.ts";
import type { TreeChange } from "@gitbutler/but-sdk";
import { Match } from "effect";

/**
 * The acting half of a row's menu — cut, absorb, uncommit, discard — addressed to
 * whatever the row stands for. A file row and a directory row differ only in their
 * subject, so they share these items rather than each spelling them out.
 *
 * Labels count what they are about to act on. A single file keeps the wording it has
 * always had, so a menu over one file reads no differently than before.
 */
export const fileSetMenuItems = ({
	actions,
	subject,
	fileParent,
}: {
	actions: ReturnType<typeof useFileSetActions>;
	subject: ReturnType<typeof useFileSetSubject>;
	fileParent: FileParent;
}): Array<Array<NativeMenuItem>> => {
	const { count } = subject;
	const plural = (verb: string) => `${verb} ${count.toLocaleString()} Files`;

	const cut: NativeMenuItem = nativeMenuItem({
		label: count > 1 ? plural("Cut") : "Cut File",
		enabled: actions.canCut,
		accelerator: toElectronAccelerator(selectionOperationHotkeys.cut.hotkey),
		onSelect: () => actions.cut(subject.addresses()),
	});

	const discard: NativeMenuItem = nativeMenuItem({
		label: count > 1 ? `Discard Changes in ${count.toLocaleString()} Files` : "Discard Changes",
		enabled: actions.canDiscard,
		accelerator: toElectronAccelerator(changesFileHotkeys.discard.hotkey),
		onSelect: () => actions.discard(subject.addresses()),
	});

	const absorb: NativeMenuItem = nativeMenuItem({
		label: count > 1 ? plural("Absorb") : "Absorb",
		enabled: actions.canAbsorb,
		accelerator: toElectronAccelerator(changesFileHotkeys.absorb.hotkey),
		onSelect: () => actions.absorb(subject.addresses()),
	});

	const uncommit: NativeMenuItem = nativeMenuItem({
		label: count > 1 ? plural("Uncommit") : "Uncommit",
		enabled: actions.canUncommit,
		accelerator: toElectronAccelerator(changesFileHotkeys.uncommit.hotkey),
		onSelect: () => actions.uncommit(subject.addresses()),
	});

	return Match.value(fileParent).pipe(
		Match.withReturnType<Array<Array<NativeMenuItem>>>(),
		Match.tagsExhaustive({
			Commit: () => [[cut], [uncommit, discard]],
			// A linked worktree's files cut into the workspace like any other, but absorb and
			// discard act on the project's own checkout, so they are offered neither.
			UncommittedChanges: ({ worktree }) =>
				worktree === undefined ? [[cut], [absorb, discard]] : [[cut]],
			// We currently don't support any operations on branch files.
			Branch: () => [],
		}),
	);
};

/**
 * Reviewing, addressed to the row's own changes: its file, or every change below a
 * directory. Unlike the acts above it does not give way to the checked set — how far
 * a reader has got through a row is the row's own business, as marking its conflicts
 * resolved is — and it is the one item branch files get, since review state is kept
 * for them too.
 *
 * The row is already saying which way round this reads: a file row wears the tick once
 * its diff is reviewed, a directory row once every diff below it is.
 */
export const reviewedMenuItem = ({
	actions,
	changes,
	isReviewed,
}: {
	actions: ReturnType<typeof useFileSetActions>;
	changes: Array<TreeChange>;
	isReviewed: boolean;
}): NativeMenuItem => {
	const verb = isReviewed ? "Unreviewed" : "Reviewed";

	return nativeMenuItem({
		label:
			changes.length > 1
				? `Mark ${changes.length.toLocaleString()} Files as ${verb}`
				: `Mark as ${verb}`,
		onSelect: () => actions.setReviewed(changes, !isReviewed),
	});
};
