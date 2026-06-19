import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { PickerDialog } from "#ui/components/PickerDialog.tsx";
import { type BranchOperand } from "#ui/operands.ts";
import { Segment, Stack } from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { type FC } from "react";

type BranchPickerOption = {
	id: string;
	label: string;
	branch: BranchOperand;
};

type Props = {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	onSelectBranch: (branch: BranchOperand) => void;
};

const segmentToBranchPickerOption = ({
	segment,
	stackId,
}: {
	segment: Segment;
	stackId: string;
}): BranchPickerOption | null => {
	const refName = segment.refName;
	if (!refName) return null;

	return {
		id: JSON.stringify([stackId, refName.fullNameBytes]),
		label: refName.displayName,
		branch: { stackId, branchRef: refName.fullNameBytes },
	};
};

const stackToBranchPickerOptions = (stack: Stack): Array<BranchPickerOption> => {
	// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
	const stackId = stack.id!;
	return stack.segments.flatMap((segment): Array<BranchPickerOption> => {
		const option = segmentToBranchPickerOption({ segment, stackId });
		return option ? [option] : [];
	});
};

export const BranchPicker: FC<Props> = ({ open, onOpenChange, onSelectBranch }) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
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
