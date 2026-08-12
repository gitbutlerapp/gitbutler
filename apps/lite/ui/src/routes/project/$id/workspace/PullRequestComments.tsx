import {
	useAddCommentReaction,
	useCreateReviewComment,
	useDeleteReviewComment,
	useRemoveCommentReaction,
	useUpdateReviewComment,
} from "#ui/api/mutations.ts";
import {
	currentForgeLoginQueryOptions,
	listCommentReactionsQueryOptions,
	listReviewCommentsQueryOptions,
	listReviewSubmissionsQueryOptions,
	listReviewTimelineEventsQueryOptions,
} from "#ui/api/queries.ts";
import { nativeMenuItem, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Clamped } from "#ui/components/Clamped.tsx";
import { classes } from "#ui/components/classes.ts";
import { FieldTextareaStyles } from "#ui/components/Field.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { Markdown } from "#ui/components/Markdown.tsx";
import { RelativeTime } from "#ui/components/RelativeTime.tsx";
import { ReviewUser } from "#ui/routes/project/$id/workspace/PullRequestPanel.tsx";
import {
	groupReactors,
	Reactions,
} from "#ui/routes/project/$id/workspace/PullRequestReactions.tsx";
import type {
	ForgeReview,
	ForgeReviewComment,
	ForgeReviewSubmission,
	ForgeReviewTimelineEvent,
} from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";
import { type FC, useRef, useState } from "react";
import styles from "./PullRequestComments.module.css";

const Comment: FC<{
	projectId: string;
	reviewId: number;
	comment: ForgeReviewComment;
	/** The signed-in forge login, for ownership checks and reaction toggling. */
	currentLogin: string | null | undefined;
	/** Quote this comment into the composer. */
	onReply: (comment: ForgeReviewComment) => void;
}> = ({ projectId, reviewId, comment, currentLogin, onReply }) => {
	const createdAtMs = comment.createdAt === null ? null : Date.parse(comment.createdAt);
	const isOwn = currentLogin != null && comment.author?.login === currentLogin;
	// An optimistic comment awaiting its forge id; nothing can act on it yet.
	const isSending = comment.id < 0;

	const [editing, setEditing] = useState(false);
	const [editBody, setEditBody] = useState("");
	const { isPending: isSaving, mutate: updateReviewComment } = useUpdateReviewComment();
	const { isPending: isDeleting, mutate: deleteReviewComment } = useDeleteReviewComment();

	// Who reacted, for the chip tooltips; only comments that show chips
	// spend a request on it.
	const hasReactions = comment.reactions.length > 0;
	const { data: reactors } = useQuery({
		...listCommentReactionsQueryOptions({ projectId, commentId: comment.id }),
		enabled: hasReactions,
		select: groupReactors,
	});

	const { mutate: addCommentReaction } = useAddCommentReaction({ reviewId });
	const { mutate: removeCommentReaction } = useRemoveCommentReaction({ reviewId });
	const toggleReaction = (kind: string, myReactionId: number | null) => {
		if (myReactionId === null) {
			addCommentReaction({ projectId, commentId: comment.id, kind });
		} else {
			removeCommentReaction({
				projectId,
				commentId: comment.id,
				reactionId: myReactionId,
			});
		}
	};

	const handleSave = () => {
		const body = editBody.trim();
		if (body === "" || isSaving) return;
		updateReviewComment(
			{ projectId, commentId: comment.id, body },
			{ onSuccess: () => setEditing(false) },
		);
	};

	return (
		<div className={classes(styles.comment, isSending && styles.commentSending)}>
			<div className={styles.commentMeta}>
				{comment.author !== null && <ReviewUser user={comment.author} />}
				{isSending ? (
					<span className={classes("text-12", styles.commentTime)}>Sending…</span>
				) : (
					createdAtMs !== null && (
						<RelativeTime
							timestamp={createdAtMs}
							className={classes("text-12", styles.commentTime)}
						/>
					)
				)}
				{isOwn && !isSending && !editing && (
					<button
						aria-label="Comment actions"
						className={styles.commentActions}
						disabled={isDeleting}
						onClick={(evt) =>
							void showNativeMenuFromTrigger(evt.currentTarget, [
								nativeMenuItem({
									label: "Edit comment",
									onSelect: () => {
										setEditBody(comment.body);
										setEditing(true);
									},
								}),
								nativeMenuItem({
									label: "Delete comment",
									onSelect: () => {
										// Forge deletion is permanent; double-check.
										if (window.confirm("Delete this comment? This cannot be undone."))
											deleteReviewComment({ projectId, commentId: comment.id });
									},
								}),
							])
						}
						type="button"
					>
						{isDeleting ? <Icon name="spinner" /> : <Icon name="kebab-vertical" />}
					</button>
				)}
			</div>

			{editing ? (
				<div className={styles.composer}>
					<FieldTextareaStyles
						value={editBody}
						onChange={(evt) => setEditBody(evt.currentTarget.value)}
						disabled={isSaving}
					/>
					<div className={styles.composerActions}>
						<button
							className={getButtonClassName({})}
							disabled={isSaving}
							onClick={() => setEditing(false)}
							type="button"
						>
							Cancel
						</button>
						<button
							className={getButtonClassName({ variant: "pop" })}
							disabled={isSaving || editBody.trim() === ""}
							onClick={handleSave}
							type="button"
						>
							{isSaving && <Icon name="spinner" />}
							Save
						</button>
					</div>
				</div>
			) : (
				<>
					<Clamped maxHeight="240px">
						<Markdown>{comment.body}</Markdown>
					</Clamped>
					{!isSending && (
						<div className={styles.commentFooter}>
							<Reactions
								reactions={comment.reactions}
								reactors={reactors}
								myLogin={currentLogin}
								// Until the reactor list arrives we can't tell which
								// reactions are the caller's own, and a toggle could
								// double-add; display-only for that moment.
								onToggle={hasReactions && reactors === undefined ? undefined : toggleReaction}
							/>
							<button
								className={classes("text-12", styles.replyButton)}
								onClick={() => onReply(comment)}
								type="button"
							>
								Reply
							</button>
						</div>
					)}
				</>
			)}
		</div>
	);
};

const submissionVerdictText: Record<ForgeReviewSubmission["state"], string> = {
	approved: "approved these changes",
	changesRequested: "requested changes",
	commented: "reviewed",
	dismissed: "had their review dismissed",
};

const Submission: FC<{ submission: ForgeReviewSubmission }> = ({ submission }) => {
	const submittedAtMs = submission.submittedAt === null ? null : Date.parse(submission.submittedAt);

	return (
		<div className={styles.event}>
			<div className={styles.commentMeta}>
				{submission.author !== null && <ReviewUser user={submission.author} />}
				<span className={classes("text-12", styles.eventText)}>
					{submissionVerdictText[submission.state]}
				</span>
				{submittedAtMs !== null && (
					<RelativeTime
						timestamp={submittedAtMs}
						className={classes("text-12", styles.commentTime)}
					/>
				)}
			</div>
			{submission.body !== null && (
				<div className={styles.eventBody}>
					<Markdown>{submission.body}</Markdown>
				</div>
			)}
		</div>
	);
};

const TimelineEvent: FC<{ event: ForgeReviewTimelineEvent }> = ({ event }) => {
	const createdAtMs = event.createdAt === null ? null : Date.parse(event.createdAt);
	const time = createdAtMs !== null && (
		<RelativeTime timestamp={createdAtMs} className={classes("text-12", styles.commentTime)} />
	);

	if (event.kind === "committed") {
		return (
			<div className={styles.event}>
				<div className={styles.commentMeta}>
					<Icon name="commit" />
					<span className={classes("text-12", styles.eventText)}>
						{event.commitAuthorName !== null && (
							<span className={styles.eventActor}>{event.commitAuthorName} </span>
						)}
						committed <span className={styles.eventSha}>{event.commitSha?.slice(0, 7)}</span>{" "}
						{event.commitSummary}
					</span>
					{time}
				</div>
			</div>
		);
	}

	return (
		<div className={styles.event}>
			<div className={styles.commentMeta}>
				{event.actor !== null ? <ReviewUser user={event.actor} /> : <Icon name="user" />}
				<span className={classes("text-12", styles.eventText)}>
					requested a review
					{event.requestedReviewer !== null && (
						<>
							{" "}
							from <span className={styles.eventActor}>{event.requestedReviewer.login}</span>
						</>
					)}
				</span>
				{time}
			</div>
		</div>
	);
};

type TimelineItem =
	| { kind: "opened"; at: number; review: ForgeReview }
	| { kind: "comment"; at: number; comment: ForgeReviewComment }
	| { kind: "submission"; at: number; submission: ForgeReviewSubmission }
	| { kind: "event"; at: number; key: string; event: ForgeReviewTimelineEvent };

const parseTimestamp = (value: string | null): number => {
	if (value === null) return Number.MAX_SAFE_INTEGER;
	const ms = Date.parse(value);
	// Undated items sink to the bottom rather than jumping the timeline.
	return Number.isNaN(ms) ? Number.MAX_SAFE_INTEGER : ms;
};

const timelineItems = (
	review: ForgeReview,
	comments: Array<ForgeReviewComment> | undefined,
	submissions: Array<ForgeReviewSubmission> | undefined,
	events: Array<ForgeReviewTimelineEvent> | undefined,
): Array<TimelineItem> => {
	const items: Array<TimelineItem> = [];
	if (review.createdAt !== null)
		items.push({ kind: "opened", at: parseTimestamp(review.createdAt), review });
	for (const comment of comments ?? [])
		items.push({ kind: "comment", at: parseTimestamp(comment.createdAt), comment });
	for (const submission of submissions ?? [])
		items.push({ kind: "submission", at: parseTimestamp(submission.submittedAt), submission });
	// Timeline events have no forge id; the list position is stable enough
	// for a render key since the forge returns them in a fixed order.
	(events ?? []).forEach((event, index) =>
		items.push({
			kind: "event",
			at: parseTimestamp(event.createdAt),
			key: `event-${index}`,
			event,
		}),
	);
	return items.sort((a, b) => a.at - b.at);
};

export const PullRequestComments: FC<{ projectId: string; review: ForgeReview }> = ({
	projectId,
	review,
}) => {
	const reviewId = review.number;
	const { data: comments, isPending } = useQuery(
		listReviewCommentsQueryOptions({ projectId, reviewId }),
	);
	const { data: submissions, isPending: submissionsPending } = useQuery(
		listReviewSubmissionsQueryOptions({ projectId, reviewId }),
	);
	const { data: events } = useQuery(listReviewTimelineEventsQueryOptions({ projectId, reviewId }));
	const { data: currentLogin } = useQuery(currentForgeLoginQueryOptions(projectId));
	const { mutate: createReviewComment } = useCreateReviewComment();
	const [draft, setDraft] = useState("");
	const composerRef = useRef<HTMLTextAreaElement | null>(null);

	const handleSubmit = () => {
		const body = draft.trim();
		if (body === "") return;
		// Optimistic: the comment appears in the timeline immediately, so the
		// composer clears right away — and comes back on failure, unless a new
		// draft is already underway.
		setDraft("");
		createReviewComment(
			{ projectId, reviewId, body },
			{ onError: () => setDraft((current) => (current === "" ? body : current)) },
		);
	};

	const handleReply = (comment: ForgeReviewComment) => {
		const quote = comment.body
			.split("\n")
			.map((line) => `> ${line}`)
			.join("\n");
		setDraft((current) =>
			current.trim() === "" ? `${quote}\n\n` : `${current.trimEnd()}\n\n${quote}\n\n`,
		);
		composerRef.current?.focus();
	};

	const items = timelineItems(review, comments, submissions, events);

	return (
		<div className={styles.comments}>
			<h4 className={classes("text-14", styles.commentsHeading)}>Activity</h4>

			{isPending || submissionsPending ? (
				<div className={classes("text-13", styles.commentsEmpty)}>Loading…</div>
			) : (
				<div className={styles.commentList}>
					{items.map((item) =>
						item.kind === "opened" ? (
							<div key="opened" className={styles.event}>
								<div className={styles.commentMeta}>
									{item.review.author !== null && <ReviewUser user={item.review.author} />}
									<span className={classes("text-12", styles.eventText)}>
										opened this pull request
									</span>
									<RelativeTime
										timestamp={item.at}
										className={classes("text-12", styles.commentTime)}
									/>
								</div>
							</div>
						) : item.kind === "comment" ? (
							<Comment
								key={`comment-${item.comment.id}`}
								projectId={projectId}
								reviewId={reviewId}
								comment={item.comment}
								currentLogin={currentLogin}
								onReply={handleReply}
							/>
						) : item.kind === "submission" ? (
							<Submission key={`submission-${item.submission.id}`} submission={item.submission} />
						) : (
							<TimelineEvent key={item.key} event={item.event} />
						),
					)}
				</div>
			)}

			<div className={styles.composer}>
				<FieldTextareaStyles
					ref={composerRef}
					placeholder="Write a comment…"
					value={draft}
					onChange={(evt) => setDraft(evt.currentTarget.value)}
				/>
				<div className={styles.composerActions}>
					<button
						className={getButtonClassName({ variant: "pop" })}
						disabled={draft.trim() === ""}
						onClick={handleSubmit}
						type="button"
					>
						Comment
					</button>
				</div>
			</div>
		</div>
	);
};
