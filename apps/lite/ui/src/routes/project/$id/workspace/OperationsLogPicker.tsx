import { useRestoreSnapshot } from "#ui/api/mutations.ts";
import { operationsLogQueryOptions } from "#ui/api/queries.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { PickerDialog, type PickerDialogGroup } from "#ui/components/PickerDialog.tsx";
import { presentableOperation } from "#ui/snapshot.ts";
import { formatRelativeTime } from "#ui/time.ts";
import type { Snapshot } from "@gitbutler/but-sdk";
import { useInfiniteQuery } from "@tanstack/react-query";
import { type FC, useState } from "react";

type Props = {
	projectId: string;
	open: boolean;
	onOpenChange: (open: boolean) => void;
};

export const OperationsLogPicker: FC<Props> = ({ open, onOpenChange, projectId }) => {
	const {
		data: groups,
		fetchNextPage,
		hasNextPage,
		isError,
		isFetchingNextPage,
		isPending,
	} = useInfiniteQuery({
		...operationsLogQueryOptions(projectId),
		select: (data): Array<PickerDialogGroup<Snapshot>> => [
			{
				value: "Operations log",
				items: data.pages.flat(),
			},
		],
	});
	const { mutate: restore } = useRestoreSnapshot({ projectId });
	const [now] = useState(() => Date.now());

	const selectSnapshot = (snapshot: Snapshot) => {
		onOpenChange(false);

		restore({ _tag: "restore", snapshot });
	};

	return (
		<PickerDialog
			ariaLabel="Operations log"
			closeLabel="Close operations log"
			emptyLabel="No operations found."
			footerAction={
				hasNextPage ? (
					<button
						type="button"
						className={getButtonClassName({ size: "small" })}
						disabled={isFetchingNextPage}
						onClick={() => void fetchNextPage()}
					>
						{isFetchingNextPage ? "Loading…" : "Load more"}
					</button>
				) : undefined
			}
			getItemKey={(snapshot) => snapshot.commitId}
			getItemLabel={(snapshot) => presentableOperation(snapshot.details).text}
			getItemType={(snapshot) => formatRelativeTime(snapshot.createdAt, now)}
			items={groups ?? []}
			open={open}
			onOpenChange={onOpenChange}
			onSelectItem={selectSnapshot}
			placeholder="Search operations…"
			selectLabel="Restore"
			statusLabel={
				isPending
					? "Loading operations log…"
					: isError
						? "Unable to load operations log."
						: undefined
			}
		/>
	);
};
