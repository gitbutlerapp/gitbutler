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
	listReviewsQueryOptions,
	listReviewTimelineEventsQueryOptions,
	reviewerCandidatesQueryOptions,
	userProfileQueryOptions,
} from "#ui/api/queries.ts";
import { nativeMenuItem, type NativeMenuItem, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import * as md from "#ui/markdown-editing.ts";
import { applyToTextarea } from "#ui/markdown-textarea.ts";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { Tooltip } from "@base-ui/react";
import { Badge, type BadgeVariant } from "#ui/components/Badge.tsx";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Clamped } from "#ui/components/Clamped.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { Kbd } from "#ui/components/Kbd.tsx";
import type { IconName } from "#ui/components/iconNames.ts";
import { Markdown } from "#ui/components/Markdown.tsx";
import { MarkdownAttachments } from "#ui/components/MarkdownAttachments.tsx";
import { MarkdownToolbar } from "#ui/components/MarkdownToolbar.tsx";
import { RelativeTime } from "#ui/components/RelativeTime.tsx";
import {
	groupReactors,
	Reactions,
} from "#ui/routes/project/$id/workspace/PullRequestReactions.tsx";
import type {
	ForgeReview,
	ForgeReviewComment,
	ForgeReviewSubmission,
	ForgeReviewTimelineEvent,
	ForgeReviewUser,
} from "@gitbutler/but-sdk";
import { pullRequestHotkeys } from "#ui/hotkeys.ts";
import { useHotkey } from "@tanstack/react-hotkeys";
import { useQuery } from "@tanstack/react-query";
import { type FC, type ReactNode, type RefObject, useRef, useState } from "react";
import styles from "./PullRequestComments.module.css";

/** The card header's identity: round avatar plus the login, as designed. */
const Author: FC<{ user: ForgeReviewUser }> = ({ user }) => (
	<>
		<Avatar src={user.avatarUrl} />
		<span className={classes("text-13", "text-semibold", styles.authorLogin)}>{user.login}</span>
	</>
);

const Avatar: FC<{ src: string | null | undefined }> = ({ src }) =>
	src != null ? (
		<img src={src} className={styles.avatar} alt="" />
	) : (
		<span className={styles.avatar} />
	);

/**
 * The card shell shared by comments and review submissions: an identity row
 * carrying time and an optional verdict badge, a body, and an optional
 * footer separated by an inset rule.
 */
const Card: FC<{
	author: ForgeReviewUser | null;
	badge?: { variant: BadgeVariant; label: string };
	timestamp: number | null;
	/** Shown in place of the time while an optimistic write is in flight. */
	pendingLabel?: string;
	edited?: boolean;
	actions?: ReactNode;
	footer?: ReactNode;
	className?: string;
	children?: ReactNode;
}> = ({
	author,
	badge,
	timestamp,
	pendingLabel,
	edited = false,
	actions,
	footer,
	className,
	children,
}) => (
	<div className={classes(styles.card, className)}>
		<div className={styles.cardBody}>
			<div className={styles.cardHeader}>
				<div className={styles.cardIdentity}>
					{author !== null && <Author user={author} />}
					{badge !== undefined && <Badge variant={badge.variant}>{badge.label}</Badge>}
					{pendingLabel !== undefined ? (
						<span className={classes("text-12", styles.cardTime)}>{pendingLabel}</span>
					) : (
						timestamp !== null && (
							<RelativeTime timestamp={timestamp} className={classes("text-12", styles.cardTime)} />
						)
					)}
					{edited && <span className={classes("text-12", styles.cardEdited)}>edited</span>}
				</div>
				{actions}
			</div>
			{children !== undefined && <div className={styles.cardContent}>{children}</div>}
		</div>
		{footer !== undefined && <div className={styles.cardFooter}>{footer}</div>}
	</div>
);

/**
 * The in-card editor: a bordered box holding the source and its own action
 * row, so the surrounding card chrome stays put while editing.
 */
const BodyEditor: FC<{
	value: string;
	onChange: (value: string) => void;
	onCancel: () => void;
	onSave: () => void;
	saving: boolean;
	label: string;
	saveLabel: string;
}> = ({ value, onChange, onCancel, onSave, saving, label, saveLabel }) => (
	<div className={styles.editor}>
		<textarea
			aria-label={label}
			className={classes("text-13", "text-body", styles.editorInput)}
			disabled={saving}
			onChange={(evt) => onChange(evt.currentTarget.value)}
			value={value}
		/>
		<div className={styles.editorActions}>
			<button className={getButtonClassName({})} disabled={saving} onClick={onCancel} type="button">
				Cancel
			</button>
			<button
				className={getButtonClassName({ variant: "gray" })}
				disabled={saving || value.trim() === ""}
				onClick={onSave}
				type="button"
			>
				{saveLabel}
				<Icon name={saving ? "spinner" : "tick"} />
			</button>
		</div>
	</div>
);

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
	const { isPending: isSaving, mutate: updateReviewComment } = useUpdateReviewComment(projectId);
	const { isPending: isDeleting, mutate: deleteReviewComment } = useDeleteReviewComment(projectId);

	// Who reacted, for the chip tooltips; only comments that show chips
	// spend a request on it.
	const hasReactions = comment.reactions.length > 0;
	const { data: reactors } = useQuery({
		...listCommentReactionsQueryOptions({ projectId, commentId: comment.id }),
		enabled: hasReactions,
		select: groupReactors,
	});

	const { mutate: addCommentReaction } = useAddCommentReaction({ projectId, reviewId });
	const { mutate: removeCommentReaction } = useRemoveCommentReaction({ projectId, reviewId });
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

	const actions = isOwn && !isSending && !editing && (
		<button
			aria-label="Comment actions"
			className={classes(getButtonClassName({ variant: "ghost", iconOnly: true }), styles.kebab)}
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
			<Icon name={isDeleting ? "spinner" : "kebab"} />
		</button>
	);

	return (
		<Card
			author={comment.author}
			className={isSending ? styles.cardSending : undefined}
			timestamp={createdAtMs}
			pendingLabel={isSending ? "Sending…" : undefined}
			edited={comment.modifiedAt !== null && comment.modifiedAt !== comment.createdAt}
			actions={actions}
			footer={
				editing || isSending ? undefined : (
					<>
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
							className={getButtonClassName({ variant: "ghost" })}
							onClick={() => onReply(comment)}
							type="button"
						>
							Reply
						</button>
					</>
				)
			}
		>
			{editing ? (
				<BodyEditor
					label="Edit comment"
					onCancel={() => setEditing(false)}
					onChange={setEditBody}
					onSave={handleSave}
					saveLabel="Save changes"
					saving={isSaving}
					value={editBody}
				/>
			) : (
				<Clamped maxHeight="240px">
					<Markdown>{comment.body}</Markdown>
				</Clamped>
			)}
		</Card>
	);
};

/** The verdict a submission carries into its card header. */
const submissionBadge: Record<
	ForgeReviewSubmission["state"],
	{ variant: BadgeVariant; label: string }
> = {
	approved: { variant: "safe", label: "Approved changes" },
	changesRequested: { variant: "danger", label: "Requested changes" },
	commented: { variant: "lightGray", label: "Reviewed" },
	dismissed: { variant: "lightGray", label: "Review dismissed" },
};

const Submission: FC<{ submission: ForgeReviewSubmission }> = ({ submission }) => {
	const submittedAtMs = submission.submittedAt === null ? null : Date.parse(submission.submittedAt);

	return (
		<Card
			author={submission.author}
			badge={submissionBadge[submission.state]}
			timestamp={submittedAtMs}
		>
			{submission.body === null || submission.body.trim() === "" ? undefined : (
				<Clamped maxHeight="240px">
					<Markdown>{submission.body}</Markdown>
				</Clamped>
			)}
		</Card>
	);
};

/**
 * A non-card timeline row: a 12px glyph in its own column, then one line of
 * muted meta text ending in the relative time. Actors and shas within the
 * text carry a dotted underline, which is what distinguishes the referenced
 * objects from the connective prose.
 */
const FeedEvent: FC<{ icon: IconName; timestamp: number | null; children: ReactNode }> = ({
	icon,
	timestamp,
	children,
}) => (
	<div className={classes("text-12", styles.event)}>
		<Icon name={icon} size={12} className={styles.eventIcon} />
		{/* One flowing paragraph, so a long summary wraps under itself and the
		    time trails the prose instead of being pinned to the far edge. */}
		<div className={styles.eventMeta}>
			{children}
			{timestamp !== null && (
				<>
					{" "}
					<span aria-hidden>·</span> <RelativeTime timestamp={timestamp} />
				</>
			)}
		</div>
	</div>
);

/** A referenced object inside an event line — an actor, a branch, a sha. */
const Ref: FC<{ children: ReactNode; mono?: boolean }> = ({ children, mono = false }) => (
	<span className={classes(styles.eventRef, mono && styles.eventMono)}>{children}</span>
);

const TimelineEvent: FC<{ event: ForgeReviewTimelineEvent }> = ({ event }) => {
	const createdAtMs = event.createdAt === null ? null : Date.parse(event.createdAt);

	if (event.kind === "committed") {
		return (
			<FeedEvent icon="commit" timestamp={createdAtMs}>
				{event.commitAuthorName !== null && <Ref>{event.commitAuthorName}</Ref>} committed{" "}
				{event.commitSha !== null && <Ref mono>{event.commitSha.slice(0, 7)}</Ref>}{" "}
				{event.commitSummary}
			</FeedEvent>
		);
	}

	return (
		<FeedEvent icon="user" timestamp={createdAtMs}>
			{event.actor !== null && <Ref>{event.actor.login}</Ref>} requested a review
			{event.requestedReviewer !== null && (
				<>
					{" "}
					from <Ref>{event.requestedReviewer.login}</Ref>
				</>
			)}
		</FeedEvent>
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

/**
 * The signed-in user's forge avatar. There is no endpoint for it, so it is
 * read off whichever timeline entry they authored — exact when it hits, but
 * blank on a review they have not posted to yet, which is the common case.
 * The caller falls back to the GitButler profile picture for that.
 */
const ownForgeAvatar = (
	items: Array<TimelineItem>,
	currentLogin: string | null | undefined,
): string | null => {
	if (currentLogin == null) return null;
	for (const item of items) {
		const author =
			item.kind === "comment"
				? item.comment.author
				: item.kind === "submission"
					? item.submission.author
					: null;
		if (author?.login === currentLogin && author.avatarUrl !== null) return author.avatarUrl;
	}
	return null;
};

/* An empty native menu opens as a bare rectangle, which reads as a broken
   button rather than an empty list — say why instead. */
const orEmptyNotice = (items: Array<NativeMenuItem>, notice: string): Array<NativeMenuItem> =>
	items.length > 0 ? items : [nativeMenuItem({ label: notice, enabled: false })];

/**
 * Insert an `@mention` or a `#reference` at the caret. Candidates come from
 * the forge — collaborators and open reviews — and are picked from a native
 * menu, the same way the panel's reviewer and label pickers work.
 */
const ForgeInserts: FC<{
	projectId: string;
	targetRef: RefObject<HTMLTextAreaElement | null>;
	onInput: (value: string) => void;
}> = ({ projectId, targetRef, onInput }) => {
	const { data: candidates } = useQuery(reviewerCandidatesQueryOptions(projectId));
	const { data: reviews } = useQuery(
		listReviewsQueryOptions({
			projectId,
			cacheConfig: { cacheWithFallback: { max_age_seconds: 300 } },
		}),
	);

	const insert = (snippet: string) => {
		const target = targetRef.current;
		if (target !== null) onInput(applyToTextarea(target, md.insert(snippet)));
	};

	const button = (
		label: string,
		icon: IconName,
		items: () => Array<NativeMenuItem>,
		notice: string,
	) => (
		<Tooltip.Root>
			<Tooltip.Trigger
				className={getButtonClassName({ variant: "ghost", iconOnly: true })}
				render={<button aria-label={label} type="button" />}
				// Keeps the caret in the textarea: a plain click would blur it
				// first, so the insert would have no position to act on.
				onMouseDown={(evt) => evt.preventDefault()}
				onClick={(evt) =>
					void showNativeMenuFromTrigger(evt.currentTarget, orEmptyNotice(items(), notice))
				}
			>
				<Icon name={icon} />
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup />}>{label}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);

	return (
		<>
			{button(
				"Mention someone",
				"user",
				() =>
					(candidates ?? []).map((candidate) =>
						nativeMenuItem({
							label: candidate.login,
							onSelect: () => insert(`@${candidate.login} `),
						}),
					),
				"No one to mention",
			)}
			{button(
				"Reference a pull request",
				"hash",
				() =>
					(reviews?.reviews ?? []).map((review) =>
						nativeMenuItem({
							label: `#${review.number} ${review.title}`,
							onSelect: () => insert(`#${review.number} `),
						}),
					),
				"No pull requests to reference",
			)}
		</>
	);
};

/** The bottom composer: toolbar, avatar + source, then the action footer. */
const Composer: FC<{
	draft: string;
	setDraft: (update: string | ((current: string) => string)) => void;
	onSubmit: () => void;
	textareaRef: RefObject<HTMLTextAreaElement | null>;
	avatarUrl: string | null | undefined;
	projectId: string;
}> = ({ draft, setDraft, onSubmit, textareaRef, avatarUrl, projectId }) => {
	const [scrolled, setScrolled] = useState(false);
	const empty = draft.trim() === "";
	const composerRef = useRef<HTMLDivElement | null>(null);

	// Scoped to the composer, so the same chord on the description form below
	// still submits that instead.
	useHotkey(pullRequestHotkeys.comment.hotkey, onSubmit, {
		conflictBehavior: "allow",
		enabled: !empty,
		target: composerRef,
	});

	return (
		<div className={styles.composer} data-body-scrolled={scrolled || undefined} ref={composerRef}>
			<MarkdownToolbar
				className={styles.composerToolbar}
				onInput={setDraft}
				targetRef={textareaRef}
			/>

			<div className={styles.composerBody}>
				<Avatar src={avatarUrl} />
				<textarea
					aria-label="Write a comment"
					className={classes("text-13", "text-body", styles.composerInput)}
					onChange={(evt) => setDraft(evt.currentTarget.value)}
					// Only the flip re-renders: React bails out of an unchanged state.
					onScroll={(evt) => setScrolled(evt.currentTarget.scrollTop > 0)}
					placeholder="Write a comment…"
					ref={textareaRef}
					value={draft}
				/>
			</div>

			<div className={styles.composerFooter}>
				<div className={styles.composerFooterStart}>
					<MarkdownAttachments onInput={setDraft} targetRef={textareaRef} />
					<ForgeInserts onInput={setDraft} projectId={projectId} targetRef={textareaRef} />
				</div>
				<button
					className={getButtonClassName({ variant: "gray" })}
					disabled={empty}
					onClick={onSubmit}
					type="button"
				>
					Comment
					<Kbd hotkey={pullRequestHotkeys.comment.hotkey} variant="button" />
				</button>
			</div>
		</div>
	);
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
	// The forge has no "authenticated user" endpoint, so the GitButler account
	// picture stands in until the caller has actually posted here.
	const { data: profile } = useQuery(userProfileQueryOptions);
	const { mutate: createReviewComment } = useCreateReviewComment(projectId);
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
			{isPending || submissionsPending ? (
				<div className={classes("text-13", styles.commentsEmpty)}>Loading…</div>
			) : (
				<div className={styles.commentList}>
					{items.map((item) =>
						item.kind === "opened" ? (
							<FeedEvent key="opened" icon="pr" timestamp={item.at}>
								{item.review.author !== null && <Ref>{item.review.author.login}</Ref>} opened this
								pull request
							</FeedEvent>
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

			<Composer
				avatarUrl={ownForgeAvatar(items, currentLogin) ?? profile?.picture}
				projectId={projectId}
				draft={draft}
				onSubmit={handleSubmit}
				setDraft={setDraft}
				textareaRef={composerRef}
			/>
		</div>
	);
};
