import { addressEquals, addressIdentityKey, type Address } from "#ui/addresses.ts";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { getOperation, type Placement } from "#ui/operations/operation.ts";
import { getTransferKind, getTransferTarget } from "#ui/operations/pending-operation.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import {
	OperationTarget as OperationTarget_,
	type OperationTargetOutline,
} from "#ui/routes/project/$id/workspace/OperationTarget.tsx";
import { useOperationDropTarget } from "#ui/routes/project/$id/workspace/useOperationDropTarget.ts";
import { useAppSelector } from "#ui/store.ts";
import { useActiveList, useIsCursorAt, useSelection } from "#ui/use-cursor.ts";
import { addressSpaceIncludes } from "#ui/workspace/address-space.ts";
import { mergeProps, Tooltip, useRender } from "@base-ui/react";
import { Match } from "effect";
import type { FC } from "react";
import { treeItemId } from "../Row-utils.ts";
import { useAbsorptionTargetCommitIds, useAddressSpace } from "./context.tsx";

/*
 * The wrappers every row of the applied tree renders into: the ARIA tree item,
 * and the operation source and target around it. Shared by the stack cards and
 * the worktree lanes.
 */

export const TreeItem: FC<
	{
		address: Address;
	} & useRender.ComponentProps<"div">
> = ({ address, render, ...props }) => {
	const addressSpace = useAddressSpace();
	const isSelected = useIsCursorAt("applied", addressSpace, address);

	return useRender({
		render,
		defaultTagName: "div",
		props: mergeProps<"div">(props, {
			id: treeItemId(address),
			role: "treeitem",
			"aria-selected": isSelected,
		}),
	});
};

export const OperationTarget: FC<
	{
		enabled: boolean;
		address: Address;
		projectId: string;
		outline: OperationTargetOutline;
	} & useRender.ComponentProps<"button">
> = ({ enabled, address, projectId, outline, render, ...props }) => {
	const dropRef = useOperationDropTarget({ enabled, target: address, projectId });

	const absorptionTargetCommitIds = useAbsorptionTargetCommitIds();
	const addressSpace = useAddressSpace();

	type ActiveOperation = { placement: Placement; tooltip?: string | undefined };
	// The cursor only picks the target of a keyboard transfer, so follow it only
	// then: a target that tracks the cursor at all times re-renders, along with
	// the whole row it wraps, on every cursor move.
	const keyboardTransferPending = useAppSelector((state) => {
		const pendingOperation = projectSlice.selectors.selectPendingOperation(state, projectId);
		return pendingOperation._tag === "Transfer" && pendingOperation.value._tag === "Keyboard";
	});
	const selection = useSelection("applied", keyboardTransferPending ? addressSpace : null);
	const activeList = useActiveList();
	const activeOperation = useAppSelector((state) => {
		const pendingOperation = projectSlice.selectors.selectPendingOperation(state, projectId);

		return Match.value(pendingOperation).pipe(
			Match.tags({
				Absorb: (): ActiveOperation | null => {
					const isActive =
						address._tag === "Commit" && absorptionTargetCommitIds.has(address.commitId);
					if (!isActive) return null;

					return { placement: "into", tooltip: "Absorb target" };
				},
				Transfer: ({ value: mode }): ActiveOperation | null => {
					if (mode.placement === null) return null;

					const target = getTransferTarget(mode, selection, activeList);
					const isActive = target !== null && addressEquals(target, address);
					if (!isActive) return null;

					return {
						placement: mode.placement,
						tooltip: getOperation({
							sources: mode.sources,
							target: address,
							placement: mode.placement,
							kind: getTransferKind(mode),
						})?.label,
					};
				},
			}),
			Match.orElse(() => null),
		);
	});

	return (
		<Tooltip.Root
			open={activeOperation?.tooltip !== undefined}
			disableHoverablePopup
			onOpenChange={(_, eventDetails) => {
				// Allow escape to bubble up from tree so it triggers the cancel
				// operation shortcut.
				if (eventDetails.reason === "escape-key") eventDetails.allowPropagation();
			}}
		>
			<Tooltip.Trigger
				{...props}
				render={
					<OperationTarget_
						ref={(el) => {
							dropRef.current = el;
						}}
						placement={activeOperation?.placement}
						outline={outline}
						render={render}
					/>
				}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8} side="right">
					<Tooltip.Popup render={<TooltipPopup />}>{activeOperation?.tooltip}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

export const AddressC: FC<
	{
		projectId: string;
		address: Address;
		outline: OperationTargetOutline;
	} & useRender.ComponentProps<"div">
> = ({ projectId, address, outline, render, ...props }) => {
	const addressSpace = useAddressSpace();

	return useRender({
		render: (
			<OperationSourceC
				projectId={projectId}
				sources={[address]}
				respectChecked={address._tag === "Commit" || address._tag === "File"}
				outline={outline}
				render={
					<OperationTarget
						enabled={addressSpaceIncludes(addressSpace, address, addressIdentityKey)}
						projectId={projectId}
						address={address}
						outline={outline}
						render={render}
					/>
				}
			/>
		),
		defaultTagName: "div",
		props,
	});
};
