import { commitTitle, shortCommitId } from "#ui/commit.ts";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { ConflictIcon } from "@gitbutler/ui-react/ConflictIcon.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { RelativeTime } from "@gitbutler/ui-react/RelativeTime.tsx";
import type { Commit, TargetCommitReview } from "@gitbutler/but-sdk";
import { type ComponentProps, type FC, useState } from "react";
import { BranchRowHeadline } from "./BranchRowHeadline.tsx";
import { RowLabel, RowLabelContainer, RowLabelGroup, RowMeta, RowMetaSeparator } from "./Row.tsx";
import rowStyles from "./Row.module.css";
import styles from "./CommitRowContent.module.css";

export const CommitRowContent: FC<
	{
		commit: Pick<Commit, "id" | "message" | "author" | "authoredAt">;
		review?: Pick<TargetCommitReview, "title" | "sourceBranch" | "labels"> | null;
		hasConflicts?: boolean;
		descriptionId?: string;
	} & Omit<ComponentProps<typeof RowLabelGroup>, "children" | "title">
> = ({ commit, review, hasConflicts = false, descriptionId, ...props }) => {
	const [now] = useState(() => Date.now());
	const title = commitTitle(commit.message);
	const author = commit.author.name !== "" ? commit.author.name : commit.author.email;

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
			<RowMeta className={classes(rowStyles.fadedText, styles.metadata)}>
				{review && review.sourceBranch !== "" && (
					<>
						<span className={classes(rowStyles.metaItem, rowStyles.metaItemShrinkable)}>
							<Icon name="branch" size={12} />
							<span className={rowStyles.metaItemText} title={review.sourceBranch}>
								{review.sourceBranch}
							</span>
						</span>
						<RowMetaSeparator />
					</>
				)}
				{author !== "" && (
					<>
						<span className={styles.author} title={commit.author.email}>
							{author}
						</span>
						<RowMetaSeparator />
					</>
				)}
				<RelativeTime timestamp={commit.authoredAt} now={now} />
				<RowMetaSeparator />
				<span className={styles.commitId} title={commit.id}>
					{shortCommitId(commit.id)}
				</span>
			</RowMeta>
		</RowLabelGroup>
	);
};
