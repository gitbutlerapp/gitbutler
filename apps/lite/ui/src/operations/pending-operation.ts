import { Match } from "effect";
import { addressEquals, type Address, uncommittedChangesAddress } from "#ui/addresses.ts";
import type { Placement, TransferKind } from "#ui/operations/operation.ts";
import type { WorkspaceCursorSnapshot } from "#ui/cursors.ts";
import type { FocusScope } from "#ui/focus-scopes.ts";
import type { AbsorptionTarget } from "@gitbutler/but-sdk";

/**
 * A computed absorption plan held for preview and confirmation. The plan cannot be edited while
 * pending.
 */
export type PendingAbsorb = {
	sources: Array<Address>;
	/** The same sources in a richer representation needed by the SDK to compute the plan. */
	sourceTarget: AbsorptionTarget;
	restoreSelection: WorkspaceCursorSnapshot;
};

/**
 * Derives the operation target from the current workspace selection.
 */
export type KeyboardTransfer = {
	sources: Array<Address>;
	kind: TransferKind;
	placement: Placement;
	restoreSelection: WorkspaceCursorSnapshot;
	restoreFocus: FocusScope | null;
};

/**
 * Stores the target and placement selected through drag-and-drop.
 */
type PointerTransfer = {
	sources: Array<Address>;
	target: Address | null;
	placement: Placement | null;
};

/**
 * Interactive source transfer. Its sources, target, and placement determine the concrete operation,
 * such as move, squash, amend, or uncommit.
 */
export type PendingTransfer =
	| ({ _tag: "Keyboard" } & KeyboardTransfer)
	| ({ _tag: "Pointer" } & PointerTransfer);

export const keyboardTransfer = ({
	sources,
	kind,
	placement,
	restoreSelection,
	restoreFocus,
}: KeyboardTransfer): PendingTransfer => ({
	_tag: "Keyboard",
	sources,
	kind,
	placement,
	restoreSelection,
	restoreFocus,
});

export const pointerTransfer = ({
	sources,
	target,
	placement,
}: PointerTransfer): PendingTransfer => ({
	_tag: "Pointer",
	sources,
	target,
	placement,
});

export const getTransferKind = (transfer: PendingTransfer): TransferKind =>
	transfer._tag === "Keyboard" ? transfer.kind : "move";

export const getTransferTarget = (
	transfer: PendingTransfer,
	appliedSelection: Address | null,
	activeList: "applied" | "uncommitted",
): Address | null =>
	Match.value(transfer).pipe(
		Match.tagsExhaustive({
			Pointer: (transfer) => transfer.target,
			Keyboard: () =>
				Match.value(activeList).pipe(
					Match.when("uncommitted", () => uncommittedChangesAddress),
					Match.when("applied", () => appliedSelection),
					Match.exhaustive,
				),
		}),
	);

export const pendingAbsorb = ({
	sources,
	restoreSelection,
	sourceTarget,
}: PendingAbsorb): PendingOperation => ({
	_tag: "Absorb",
	sources,
	restoreSelection,
	sourceTarget,
});

export const pendingTransfer = (transfer: PendingTransfer): PendingOperation => ({
	_tag: "Transfer",
	value: transfer,
});

export type InlineEditAddress = Extract<Address, { _tag: "Branch" | "Commit" }>;

export type PendingInlineEdit = { address: InlineEditAddress };

export type PendingOperation =
	| { _tag: "None" }
	| ({ _tag: "InlineEdit" } & PendingInlineEdit)
	| ({ _tag: "Absorb" } & PendingAbsorb)
	| { _tag: "Transfer"; value: PendingTransfer };

export const noPendingOperation: PendingOperation = {
	_tag: "None",
};

export const pendingInlineEdit = ({ address }: PendingInlineEdit): PendingOperation => ({
	_tag: "InlineEdit",
	address,
});

export const isValidPendingOperationForSelection = ({
	pendingOperation,
	selection,
}: {
	pendingOperation: PendingOperation;
	selection: Address;
}): boolean =>
	Match.value(pendingOperation).pipe(
		Match.tagsExhaustive({
			None: () => true,
			Absorb: () => true,
			Transfer: () => true,
			InlineEdit: (pending) => addressEquals(selection, pending.address),
		}),
	);

export const getOperationSources = (pendingOperation: PendingOperation): Array<Address> | null =>
	Match.value(pendingOperation).pipe(
		Match.tagsExhaustive({
			None: () => null,
			Absorb: (x) => x.sources,
			Transfer: (x) => x.value.sources,
			InlineEdit: () => null,
		}),
	);
