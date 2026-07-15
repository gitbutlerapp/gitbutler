import { encodeBytes } from "#ui/api/bytes.ts";
import { useApply } from "#ui/api/mutations.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { useHeadInfoQuery, useListBranchesQuery } from "#ui/api/queries.ts";
import { PickerDialog, type PickerDialogGroup } from "#ui/components/PickerDialog.tsx";
import { BranchListing } from "@gitbutler/but-sdk";
import { type FC, useState } from "react";

type ApplyBranchPickerOption = {
	branchRef: string;
	label: string;
	type: string;
	updatedAt: number;
};

type Props = {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	projectId: string;
};

const relativeTimeFormatter = new Intl.RelativeTimeFormat(undefined, {
	numeric: "auto",
	style: "short",
});

const formatRelativeTime = (timestamp: number, now = Date.now()) => {
	const seconds = Math.round((timestamp - now) / 1000);
	const absSeconds = Math.abs(seconds);

	if (absSeconds < 60) return relativeTimeFormatter.format(seconds, "seconds");
	if (absSeconds < 60 * 60)
		return relativeTimeFormatter.format(Math.round(seconds / 60), "minutes");
	if (absSeconds < 60 * 60 * 24)
		return relativeTimeFormatter.format(Math.round(seconds / 60 / 60), "hours");
	if (absSeconds < 60 * 60 * 24 * 30)
		return relativeTimeFormatter.format(Math.round(seconds / 60 / 60 / 24), "days");
	if (absSeconds < 60 * 60 * 24 * 365)
		return relativeTimeFormatter.format(Math.round(seconds / 60 / 60 / 24 / 30), "months");
	return relativeTimeFormatter.format(Math.round(seconds / 60 / 60 / 24 / 365), "years");
};

const branchListingToApplyBranchPickerOptions = (
	branch: BranchListing,
): Array<ApplyBranchPickerOption> => {
	if (branch.hasLocal) {
		return [
			{
				branchRef: `refs/heads/${branch.name}`,
				label: branch.name,
				type: "Local",
				updatedAt: branch.updatedAt,
			},
		];
	}

	return branch.remotes.map((remote) => ({
		branchRef: `refs/remotes/${remote}/${branch.name}`,
		label: branch.name,
		type: remote,
		updatedAt: branch.updatedAt,
	}));
};

const groupApplyBranchPickerOptions = (
	items: Array<ApplyBranchPickerOption>,
): Array<PickerDialogGroup<ApplyBranchPickerOption>> =>
	Array.from(
		Map.groupBy(items, (item) => item.type),
		([value, items]): PickerDialogGroup<ApplyBranchPickerOption> => ({
			value,
			items: items.toSorted((a, b) => {
				if (value === "Local" && a.updatedAt !== b.updatedAt) return b.updatedAt - a.updatedAt;
				return a.label.localeCompare(b.label);
			}),
		}),
	).toSorted((a, b) => {
		if (a.value === "Local" && b.value !== "Local") return -1;
		if (a.value !== "Local" && b.value === "Local") return 1;
		return a.value.localeCompare(b.value);
	});

export const ApplyBranchPicker: FC<Props> = ({ open, onOpenChange, projectId }) => {
	const { data: headInfo } = useHeadInfoQuery(projectId);
	const headInfoIndex = headInfo ? getHeadInfoIndex(headInfo) : undefined;
	const branchesQuery = useListBranchesQuery({ projectId, filter: null });
	const [now] = useState(() => Date.now());
	const items =
		branchesQuery.data
			?.flatMap(branchListingToApplyBranchPickerOptions)
			// Filter out branches that are applied in the *current* workspace.
			.filter((option) => !headInfoIndex?.branchContextByRefBytes(encodeBytes(option.branchRef))) ??
		[];
	const apply = useApply();
	const statusLabel =
		items.length === 0
			? branchesQuery.isLoading
				? "Loading branches…"
				: branchesQuery.isError
					? "Unable to load branches."
					: undefined
			: undefined;

	const selectBranch = (option: ApplyBranchPickerOption) => {
		onOpenChange(false);
		apply.mutate({ projectId, existingBranch: option.branchRef });
	};

	return (
		<PickerDialog
			ariaLabel="Apply branch"
			closeLabel="Close apply branch picker"
			emptyLabel="No available branches found."
			getItemKey={(x) => x.branchRef}
			getItemLabel={(x) => x.label}
			getItemType={(x) => formatRelativeTime(x.updatedAt, now)}
			itemToStringValue={(x) => x.label}
			items={groupApplyBranchPickerOptions(items)}
			open={open}
			onOpenChange={onOpenChange}
			onSelectItem={selectBranch}
			placeholder="Search for branches to apply…"
			statusLabel={statusLabel}
		/>
	);
};
