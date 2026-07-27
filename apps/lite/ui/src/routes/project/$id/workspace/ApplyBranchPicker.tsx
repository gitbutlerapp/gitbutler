import { useApply } from "#ui/api/mutations.ts";
import { branchListQueryOptions } from "#ui/api/queries.ts";
import { branchDetailsParams } from "#ui/branch.ts";
import { PickerDialog, type PickerDialogGroup } from "#ui/components/PickerDialog.tsx";
import { formatRelativeTime } from "#ui/time.ts";
import type { ListedBranch, ListedStack } from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";
import { type FC, useState } from "react";

type ApplyBranchPickerOption = {
	branchRef: string;
	label: string;
	type: string;
	updatedAt: number | null;
};

type Props = {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	projectId: string;
};

const listedBranchToApplyBranchPickerOptions = (
	branch: ListedBranch,
): Array<ApplyBranchPickerOption> => {
	if (branch.hasLocal) {
		return [
			{
				branchRef: branch.refName.full,
				label: branch.displayName,
				type: "Local",
				updatedAt: branch.updatedAtMs,
			},
		];
	}

	return branch.remoteRefs.flatMap(({ full }) => {
		const { remote } = branchDetailsParams(full);

		return remote !== null
			? {
					branchRef: full,
					label: branch.displayName,
					type: remote,
					updatedAt: branch.updatedAtMs,
				}
			: [];
	});
};

const groupApplyBranchPickerOptions = (
	items: Array<ApplyBranchPickerOption>,
): Array<PickerDialogGroup<ApplyBranchPickerOption>> =>
	Array.from(
		Map.groupBy(items, (item) => item.type),
		([value, items]): PickerDialogGroup<ApplyBranchPickerOption> => ({
			value,
			items: items.toSorted((a, b) => {
				if (value === "Local" && a.updatedAt !== b.updatedAt)
					return (b.updatedAt ?? 0) - (a.updatedAt ?? 0);
				return a.label.localeCompare(b.label);
			}),
		}),
	).toSorted((a, b) => {
		if (a.value === "Local" && b.value !== "Local") return -1;
		if (a.value !== "Local" && b.value === "Local") return 1;
		return a.value.localeCompare(b.value);
	});

const listedStacksToApplyBranchPickerGroups = (
	stacks: Array<ListedStack>,
): Array<PickerDialogGroup<ApplyBranchPickerOption>> =>
	groupApplyBranchPickerOptions(
		stacks.flatMap(({ status, branches }) =>
			status === "unapplied" || status === "standalone"
				? branches.flatMap(listedBranchToApplyBranchPickerOptions)
				: [],
		),
	);

export const ApplyBranchPicker: FC<Props> = ({ open, onOpenChange, projectId }) => {
	const {
		data: groups,
		isError,
		isPending,
	} = useQuery({
		...branchListQueryOptions(projectId),
		select: listedStacksToApplyBranchPickerGroups,
	});
	const [now] = useState(() => Date.now());
	const { mutate: apply } = useApply();
	const statusLabel = isPending
		? "Loading branches…"
		: isError
			? "Unable to load branches."
			: undefined;

	const selectBranch = (option: ApplyBranchPickerOption) => {
		onOpenChange(false);
		apply({ projectId, existingBranch: option.branchRef });
	};

	return (
		<PickerDialog
			ariaLabel="Apply branch"
			closeLabel="Close apply branch picker"
			emptyLabel="No available branches found."
			getItemKey={(x) => x.branchRef}
			getItemLabel={(x) => x.label}
			getItemType={(x) => (x.updatedAt === null ? undefined : formatRelativeTime(x.updatedAt, now))}
			itemToStringValue={(x) => x.label}
			items={groups ?? []}
			open={open}
			onOpenChange={onOpenChange}
			onSelectItem={selectBranch}
			placeholder="Search for branches to apply…"
			statusLabel={statusLabel}
		/>
	);
};
