import { Match } from "effect";
import { operandEquals, type Operand, uncommittedChangesOperand } from "#ui/operands.ts";
import type { Placement, TransferKind } from "#ui/operations/operation.ts";
import type { WorkspaceCursorSnapshot } from "#ui/cursors.ts";
import type { AbsorptionTarget } from "@gitbutler/but-sdk";

/**
 * Presents a computed absorption plan for preview and confirmation. The plan cannot be edited in
 * this mode.
 */
export type AbsorbMode = {
	sources: Array<Operand>;
	/** The same sources in a richer representation needed by the SDK to compute the plan. */
	sourceTarget: AbsorptionTarget;
	restoreSelection: WorkspaceCursorSnapshot;
};

/**
 * Derives the operation target from the current workspace selection.
 */
export type KeyboardTransferMode = {
	sources: Array<Operand>;
	kind: TransferKind;
	placement: Placement;
	restoreSelection: WorkspaceCursorSnapshot;
};

/**
 * Stores the target and placement selected through drag-and-drop.
 */
type PointerTransferMode = {
	sources: Array<Operand>;
	target: Operand | null;
	placement: Placement | null;
};

/**
 * Interactive source transfer. Its sources, target, and placement determine the concrete operation,
 * such as move, squash, amend, or uncommit.
 */
export type TransferMode =
	| ({ _tag: "Keyboard" } & KeyboardTransferMode)
	| ({ _tag: "Pointer" } & PointerTransferMode);

export const keyboardTransferMode = ({
	sources,
	kind,
	placement,
	restoreSelection,
}: KeyboardTransferMode): TransferMode => ({
	_tag: "Keyboard",
	sources,
	kind,
	placement,
	restoreSelection,
});

export const pointerTransferMode = ({
	sources,
	target,
	placement,
}: PointerTransferMode): TransferMode => ({
	_tag: "Pointer",
	sources,
	target,
	placement,
});

export const getTransferKind = (mode: TransferMode): TransferKind =>
	mode._tag === "Keyboard" ? mode.kind : "move";

export const getTransferTarget = (
	mode: TransferMode,
	outlineSelection: Operand | null,
	workspaceList: "stacks" | "uncommitted",
): Operand | null =>
	Match.value(mode).pipe(
		Match.tagsExhaustive({
			Pointer: (mode) => mode.target,
			Keyboard: () =>
				Match.value(workspaceList).pipe(
					Match.when("uncommitted", () => uncommittedChangesOperand),
					Match.when("stacks", () => outlineSelection),
					Match.exhaustive,
				),
		}),
	);

export const absorbOutlineMode = ({
	sources,
	restoreSelection,
	sourceTarget,
}: AbsorbMode): OutlineMode => ({
	_tag: "Absorb",
	sources,
	restoreSelection,
	sourceTarget,
});

export const transferOutlineMode = (mode: TransferMode): OutlineMode => ({
	_tag: "Transfer",
	value: mode,
});

export type InlineEditOperand = Extract<Operand, { _tag: "Branch" | "Commit" }>;

export type InlineEditMode = { operand: InlineEditOperand };

export type OutlineMode =
	| { _tag: "Default" }
	| ({ _tag: "InlineEdit" } & InlineEditMode)
	| ({ _tag: "Absorb" } & AbsorbMode)
	| { _tag: "Transfer"; value: TransferMode };

export const defaultOutlineMode: OutlineMode = {
	_tag: "Default",
};

export const inlineEditOutlineMode = ({ operand }: InlineEditMode): OutlineMode => ({
	_tag: "InlineEdit",
	operand,
});

export const isValidOutlineModeForSelection = ({
	mode,
	selection,
}: {
	mode: OutlineMode;
	selection: Operand;
}): boolean =>
	Match.value(mode).pipe(
		Match.tagsExhaustive({
			Default: () => true,
			Absorb: () => true,
			Transfer: () => true,
			InlineEdit: (mode) => operandEquals(selection, mode.operand),
		}),
	);

export const getOperationSources = (mode: OutlineMode): Array<Operand> | null =>
	Match.value(mode).pipe(
		Match.tagsExhaustive({
			Default: () => null,
			Absorb: (x) => x.sources,
			Transfer: (x) => x.value.sources,
			InlineEdit: () => null,
		}),
	);
