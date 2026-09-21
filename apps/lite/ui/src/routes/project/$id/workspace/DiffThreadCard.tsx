import { classes } from "@gitbutler/ui-react/classes.ts";
import { TextLink } from "@gitbutler/ui-react/TextLink.tsx";
import type { ForgeReviewThread } from "@gitbutler/but-sdk";
import { ThreadComment } from "#ui/routes/project/$id/workspace/PullRequestComments.tsx";
import { ReviewThreadReply } from "#ui/routes/project/$id/workspace/ReviewThreadReply.tsx";
import { openLinkExternally } from "#ui/external-link.ts";
import type { FC } from "react";
import styles from "./DiffThreadCard.module.css";

type Props = {
	projectId: string;
	/** The review the thread hangs on, which is how its cache is keyed. */
	reviewId: number;
	thread: ForgeReviewThread;
};

/**
 * A review thread where it was left: the conversation alone, since the line
 * it hangs on is the one above it. Replies post from here; everything else
 * the forge offers is a link away.
 */
export const DiffThreadCard: FC<Props> = ({ projectId, reviewId, thread }) => (
	<div className={styles.card}>
		<div className={styles.comments}>
			{thread.comments.map((comment) => (
				<ThreadComment
					compact
					comment={comment}
					key={comment.id !== 0 ? comment.id : comment.htmlUrl}
				/>
			))}
		</div>

		<div className={styles.footer}>
			<ReviewThreadReply projectId={projectId} reviewId={reviewId} threadId={thread.id} />
			{thread.comments[0] !== undefined && (
				<TextLink
					className={classes("text-12", styles.forgeLink)}
					href={thread.comments[0].htmlUrl}
					onClick={openLinkExternally}
				>
					Open on the forge
				</TextLink>
			)}
		</div>
	</div>
);
