import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { openLinkExternally } from "#ui/external-link.ts";
import type { ForgeReviewThread } from "@gitbutler/but-sdk";
import { ThreadComment } from "#ui/routes/project/$id/workspace/PullRequestComments.tsx";
import { ReviewThreadReply } from "#ui/routes/project/$id/workspace/ReviewThreadReply.tsx";
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
				<ThreadComment comment={comment} key={comment.id !== 0 ? comment.id : comment.htmlUrl} />
			))}
		</div>

		<div className={styles.footer}>
			<ReviewThreadReply projectId={projectId} reviewId={reviewId} threadId={thread.id} />
			{thread.comments[0] !== undefined && (
				<a
					className={classes("text-12", styles.forgeLink)}
					href={thread.comments[0].htmlUrl}
					onClick={openLinkExternally}
				>
					Open on the forge
					<Icon name="arrow-up-right" size={12} />
				</a>
			)}
		</div>
	</div>
);
