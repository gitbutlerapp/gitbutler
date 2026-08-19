import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { PickerDialog } from "#ui/components/PickerDialog.tsx";
import type { BranchAddress } from "#ui/addresses.ts";
import type { Segment, Stack } from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";
import type { FC } from "react";

type BranchPickerOption = {
	id: string;
	label: string;
	branch: BranchAddress;
};

type Props = {
	projectId: string;
	open: boolean;
	onOpenChange: (open: boolean) => void;
	onSelectBranch: (branch: BranchAddress) => void;
};

const segmentToBranchPickerOption = ({
	segment,
}: {
	segment: Segment;
}): BranchPickerOption | null => {
	const refName = segment.refName;
	if (!refName) return null;

	return {
		id: refName.fullNameBytes.join(","),
		label: refName.displayName,
		branch: { branchRef: refName.fullNameBytes },
	};
};

const stackToBranchPickerOptions = (stack: Stack): Array<BranchPickerOption> =>
	stack.segments.flatMap((segment): Array<BranchPickerOption> => {
		const option = segmentToBranchPickerOption({ segment });
		return option ? [option] : [];
	});

export const BranchPicker: FC<Props> = ({ projectId, open, onOpenChange, onSelectBranch }) => {
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const selectBranch = (option: BranchPickerOption) => {
		onOpenChange(false);
		onSelectBranch(option.branch);
	};

	return (
		<PickerDialog
			ariaLabel="Select branch"
			closeLabel="Close branch picker"
			emptyLabel="No results found."
			getItemKey={(x) => x.id}
			getItemLabel={(x) => x.label}
			getItemType={() => "Branch"}
			itemToStringValue={(x) => x.label}
			items={[
				{
					value: "Branches",
					items: headInfo?.stacks.flatMap(stackToBranchPickerOptions) ?? [],
				},
			]}
			open={open}
			onOpenChange={onOpenChange}
			onSelectItem={selectBranch}
			placeholder="Search for branches…"
		/>
	);
};
