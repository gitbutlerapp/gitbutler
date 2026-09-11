import { commitTitle, shortCommitId } from "#ui/commit.ts";
import { classes } from "#ui/components/classes.ts";
import { ConflictIcon } from "#ui/components/ConflictIcon.tsx";
import { RelativeTime } from "#ui/components/RelativeTime.tsx";
import type { Commit } from "@gitbutler/but-sdk";
import { type ComponentProps, type FC, useState } from "react";
import { RowLabel, RowLabelContainer, RowLabelGroup, RowMeta, RowMetaSeparator } from "./Row.tsx";
import rowStyles from "./Row.module.css";
import styles from "./CommitRowContent.module.css";

export const CommitRowContent: FC<
	{
		commit: Pick<Commit, "id" | "message" | "author" | "authoredAt">;
		/** A landed review can supply a more useful title than the merge message. */
		title?: string;
		hasConflicts?: boolean;
		descriptionId?: string;
	} & Omit<ComponentProps<typeof RowLabelGroup>, "children" | "title">
> = ({ commit, title: reviewTitle, hasConflicts = false, descriptionId, ...props }) => {
	const [now] = useState(() => Date.now());
	const title = reviewTitle ?? commitTitle(commit.message);
	const author = commit.author.name !== "" ? commit.author.name : commit.author.email;

	return (
		<RowLabelGroup {...props}>
			<RowLabelContainer>
				{hasConflicts && (
					<ConflictIcon
						variant="conflict"
						className={styles.conflictIcon}
						aria-label="Conflicted"
					/>
				)}
				<RowLabel singleLine title={title}>
					{title === undefined ? <span className={rowStyles.fadedText}>(no message)</span> : title}
				</RowLabel>
			</RowLabelContainer>
			<RowMeta id={descriptionId} className={classes(rowStyles.fadedText, styles.metadata)}>
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
