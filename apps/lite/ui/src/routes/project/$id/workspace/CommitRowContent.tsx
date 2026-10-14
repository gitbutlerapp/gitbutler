import { commitTitle } from "#ui/commit.ts";
import { ConflictIcon } from "@gitbutler/ui-react/ConflictIcon.tsx";
import type { Commit, TargetCommitReview } from "@gitbutler/but-sdk";
import type { ComponentProps, FC } from "react";
import { BranchRowHeadline } from "./BranchRowHeadline.tsx";
import { RowLabel, RowLabelContainer, RowLabelGroup } from "./Row.tsx";
import rowStyles from "./Row.module.css";
import styles from "./CommitRowContent.module.css";

export const CommitRowContent: FC<
	{
		commit: Pick<Commit, "message">;
		review?: Pick<TargetCommitReview, "title" | "labels"> | null;
		hasConflicts?: boolean;
		descriptionId?: string;
	} & Omit<ComponentProps<typeof RowLabelGroup>, "children" | "title">
> = ({ commit, review, hasConflicts = false, descriptionId, ...props }) => {
	const title = commitTitle(commit.message);

	return (
		<RowLabelGroup {...props} id={descriptionId}>
			{review ? (
				<BranchRowHeadline title={review.title} labels={review.labels} />
			) : (
				<RowLabelContainer>
					{hasConflicts && (
						<ConflictIcon
							variant="conflict"
							className={styles.conflictIcon}
							aria-label="Conflicted"
						/>
					)}
					<RowLabel singleLine title={title}>
						{title === undefined ? (
							<span className={rowStyles.fadedText}>(no message)</span>
						) : (
							title
						)}
					</RowLabel>
				</RowLabelContainer>
			)}
		</RowLabelGroup>
	);
};
