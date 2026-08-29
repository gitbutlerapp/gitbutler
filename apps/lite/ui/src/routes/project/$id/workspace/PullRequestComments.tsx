import {
	useAddCommentReaction,
	useCreateReviewComment,
	useDeleteReviewComment,
	useRemoveCommentReaction,
	useOpenInProgram,
	useUpdateReviewComment,
} from "#ui/api/mutations.ts";
import {
	currentForgeLoginQueryOptions,
	forgeInfoOptions,
	listCommentReactionsQueryOptions,
	listReviewCommentsQueryOptions,
	listReviewSubmissionsQueryOptions,
	listReviewsQueryOptions,
	listReviewThreadsQueryOptions,
	listReviewTimelineEventsQueryOptions,
	guiSettingsQueryOptions,
	headInfoQueryOptions,
	listEditorsQueryOptions,
	workspaceFileQueryOptions,
	reviewerCandidatesQueryOptions,
	userProfileQueryOptions,
} from "#ui/api/queries.ts";
import {
	nativeMenuItem,
	nativeMenuItemsFromGroups,
	type NativeMenuItem,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
} from "#ui/native-menu.ts";
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
	ForgeReviewThread,
	ForgeReviewThreadComment,
	ForgeReviewTimelineEvent,
	ForgeReviewUser,
} from "@gitbutler/but-sdk";
import { ReviewThreadReply } from "#ui/routes/project/$id/workspace/ReviewThreadReply.tsx";
import { encodeBytes } from "#ui/api/bytes.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { forgeHunkPatch, threadStillAnchoredInFile } from "#ui/review-threads.ts";
import { defaultSettings } from "#ui/settings.ts";
import { pullRequestHotkeys } from "#ui/hotkeys.ts";
import { FreshBadge, RegisterFreshItems } from "#ui/review-arrival.tsx";
import { useHotkey } from "@tanstack/react-hotkeys";
import { PatchDiff } from "@pierre/diffs/react";
import { useQuery } from "@tanstack/react-query";
import { clearReviewFocus, useRequestedComment } from "#ui/review-focus.ts";
import {
	type FC,
	type MouseEvent as ReactMouseEvent,
	type ReactNode,
	type RefObject,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import styles from "./PullRequestComments.module.css";

/** Where a notification scrolls to when its toast named this comment. */
const commentAnchorId = (commentId: number): string => `review-comment-${commentId}`;

/**
 * Whether the author is an agent of any kind — Copilot, CI, a review bot.
 * The forge's own flag when it survives the trip, else the `[bot]` login
 * suffix every GitHub App carries.
 */
const isAgent = (user: ForgeReviewUser): boolean => user.isBot || user.login.endsWith("[bot]");

/**
 * The card header's identity: round avatar plus the login, as designed. An
 * agent author carries a chip so automated feedback reads apart from human
 * conversation.
 */
const Author: FC<{ user: ForgeReviewUser }> = ({ user }) => (
	<>
		<Avatar src={user.avatarUrl} />
		<span className={classes("text-13", "text-semibold", styles.authorLogin)}>{user.login}</span>
		{isAgent(user) && <Badge variant="purple">Agent</Badge>}
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
	/** The card's stable item key, so its "New" marker can be seen away. */
	freshKey?: string;
	edited?: boolean;
	actions?: ReactNode;
	footer?: ReactNode;
	className?: string;
	/** Anchors the card so a notification can scroll to it. */
	id?: string;
	children?: ReactNode;
}> = ({
	author,
	badge,
	timestamp,
	pendingLabel,
	freshKey,
	edited = false,
	actions,
	footer,
	className,
	id,
	children,
}) => (
	<div
		id={id}
		className={classes(
			styles.card,
			author !== null && isAgent(author) && styles.cardAgent,
			className,
		)}
	>
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
					<FreshBadge timestamp={timestamp} author={author} itemKey={freshKey} />
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
			id={comment.id > 0 ? commentAnchorId(comment.id) : undefined}
			className={isSending ? styles.cardSending : undefined}
			timestamp={createdAtMs}
			freshKey={comment.id > 0 ? `c:${comment.id}` : undefined}
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

/**
 * The lines a thread hangs on, as the forge last knew them. An outdated
 * thread has no current line, so the line it was left on stands in.
 */
const threadLines = (thread: ForgeReviewThread): string => {
	const line = thread.line ?? thread.originalLine;
	if (line === null) return "";
	const start = thread.startLine;
	return start !== null && start !== line ? `:${start}-${line}` : `:${line}`;
};

/** The path a thread hangs on, its directory dimmed as file rows spell it. */
const ThreadAnchor: FC<{ thread: ForgeReviewThread }> = ({ thread }) => {
	const lastSepIdx = thread.path.lastIndexOf("/");
	const directoryPath = lastSepIdx !== -1 ? thread.path.slice(0, lastSepIdx) : null;
	const fileName = thread.path.slice(lastSepIdx + 1);

	return (
		<span className={styles.threadPath}>
			{directoryPath !== null && <span className={styles.threadDir}>{directoryPath}/</span>}
			<span className={styles.threadFile}>
				{fileName}
				{threadLines(thread)}
			</span>
		</span>
	);
};

export const ThreadComment: FC<{ comment: ForgeReviewThreadComment }> = ({ comment }) => {
	const createdAtMs = comment.createdAt === null ? null : Date.parse(comment.createdAt);

	return (
		<div
			className={styles.threadComment}
			id={comment.id > 0 ? commentAnchorId(comment.id) : undefined}
		>
			<div className={styles.cardIdentity}>
				{comment.author !== null && <Author user={comment.author} />}
				{createdAtMs !== null && (
					<RelativeTime timestamp={createdAtMs} className={classes("text-12", styles.cardTime)} />
				)}
				<FreshBadge
					timestamp={createdAtMs}
					author={comment.author}
					itemKey={comment.id > 0 ? `tc:${comment.id}` : undefined}
				/>
			</div>
			<Clamped maxHeight="200px">
				<Markdown>{comment.body}</Markdown>
			</Clamped>
		</div>
	);
};

/**
 * The code a thread hangs off, rendered by the same engine as the diff view
 * so it carries real line numbers and the app's diff settings. Pierre parses
 * a whole patch, and the forge sends only the `@@` hunk, so the file headers
 * are put back on — the same shape `synthesizeFilePatch` builds.
 */
const ThreadHunk: FC<{
	projectId: string;
	path: string;
	/** The line the thread hangs on, which an editor should open at. */
	lineNr: number | null;
	diffHunk: string;
}> = ({ projectId, path, lineNr, diffHunk }) => {
	const { data: editors } = useQuery(listEditorsQueryOptions);
	const { data: preferredEditor } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => editors?.find((editor) => editor.id === cfg.editorId),
	});
	const { isPending: isOpenInProgramPending, mutate: openInProgram } = useOpenInProgram();
	const { data: settings } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => ({
			theme: cfg.theme,
			diffBackground: cfg.diffBackground,
			diffOverflow: cfg.diffOverflow,
			lineDiffType: cfg.lineDiffType,
			diffLigatures: cfg.diffLigatures,
		}),
	});

	const openAt = (programId: string) => openInProgram({ projectId, programId, path, lineNr });

	/* The app's own menu replaces the window's, so it carries the copy the
	   window would have offered — quoted code is here to be taken away. */
	const onContextMenu = (event: ReactMouseEvent<HTMLElement>) => {
		// Pierre renders into a shadow root, whose selection the document does
		// not see; Chromium exposes it on the root itself (non-standard, hence
		// the cast).
		const root =
			event.target instanceof Node
				? (event.target.getRootNode() as { getSelection?: () => Selection | null })
				: document;
		const selection = (root.getSelection?.() ?? window.getSelection())?.toString() ?? "";

		void showNativeContextMenu(
			event,
			nativeMenuItemsFromGroups([
				[
					nativeMenuItem({
						label: "Copy",
						enabled: selection !== "",
						onSelect: () => void navigator.clipboard.writeText(selection),
					}),
				],
				[
					preferredEditor
						? nativeMenuItem({
								label: `Open in ${preferredEditor.name}`,
								enabled: !isOpenInProgramPending,
								onSelect: () => openAt(preferredEditor.id),
							})
						: nativeMenuItem({
								label: "Open In Editor",
								submenu: (editors ?? []).map((editor) =>
									nativeMenuItem({
										label: editor.name,
										enabled: !isOpenInProgramPending,
										onSelect: () => openAt(editor.id),
									}),
								),
							}),
				],
			]),
		);
	};

	const patch = useMemo(() => forgeHunkPatch(path, diffHunk), [path, diffHunk]);

	// Nothing to draw from a hunk no parser would take.
	if (patch === null) return null;

	return (
		<div className={styles.hunk} onContextMenu={onContextMenu}>
			<PatchDiff
				patch={patch}
				options={{
					// Unified whatever the diff view is set to: a comment card is too
					// narrow for two columns, and a thread hangs on one line anyway.
					diffStyle: "unified",
					themeType: settings?.theme ?? defaultSettings.theme,
					overflow: settings?.diffOverflow ?? defaultSettings.diffOverflow,
					disableBackground: !(settings?.diffBackground ?? defaultSettings.diffBackground),
					lineDiffType: settings?.lineDiffType ?? defaultSettings.lineDiffType,
					unsafeCSS: `
						:host {
							background-color: transparent;
							font-variant-ligatures: ${
								(settings?.diffLigatures ?? defaultSettings.diffLigatures) ? "normal" : "none"
							};
						}
					`,
				}}
				// FileDiff derives its header mode from this, so a null header is
				// enough — the anchor row above already names the file and its lines.
				renderCustomHeader={() => null}
			/>
		</div>
	);
};

/**
 * One diff-anchored conversation. Resolved threads open collapsed: they are
 * settled, and a review bot alone can leave a dozen of them.
 */
const Thread: FC<{
	projectId: string;
	reviewId: number;
	thread: ForgeReviewThread;
	/** Whether the review's branch is applied — only then does the working
	 * file speak for it; otherwise it holds other branches' content. */
	branchApplied: boolean;
}> = ({ projectId, reviewId, thread, branchApplied }) => {
	const [expanded, setExpanded] = useState(!thread.isResolved);
	// The thread hangs where its first comment was left; later replies carry
	// the same hunk.
	const firstComment = thread.comments[0];

	// A notification naming a comment in here needs its anchor mounted — a
	// collapsed thread would leave the scroll with nothing to land on.
	// Adjusted during render (the documented prop-driven-state shape), and
	// only once per request so the reader can still fold it back up.
	const requested = useRequestedComment(reviewId);
	const [answeredRequest, setAnsweredRequest] = useState<number | null>(null);
	if (
		requested !== null &&
		requested !== answeredRequest &&
		thread.comments.some((comment) => comment.id === requested)
	) {
		setAnsweredRequest(requested);
		setExpanded(true);
	}

	// The forge's own flag only knows the head it was told about, so a branch
	// amended since keeps its threads looking current. Reading the file settles
	// it; `staleTime` because this read carries no cache tag of its own, and a
	// held copy would answer for a file that has since been edited. Old-side
	// threads are left to the forge's flag — their line numbers the pre-image,
	// which no working file holds.
	const { data: file } = useQuery({
		...workspaceFileQueryOptions({ projectId, relativePath: thread.path, version: 0 }),
		staleTime: 0,
		enabled: branchApplied && !thread.isResolved && thread.line !== null && thread.side !== "old",
	});
	const outdated =
		thread.isOutdated ||
		(file?.content != null &&
			file.mimeType === null &&
			!threadStillAnchoredInFile(thread, file.content));

	return (
		<div className={styles.thread}>
			<button
				className={classes("text-12", styles.threadAnchor)}
				onClick={() => setExpanded((current) => !current)}
				type="button"
			>
				<Icon name={expanded ? "chevron-down" : "chevron-right"} size={12} />
				<ThreadAnchor thread={thread} />
				{/* Both states keep a thread off the diff, so each says so rather
				    than leaving a reader hunting for it there. */}
				{thread.isResolved ? (
					<Badge variant="lightGray" title="Settled, so it is not shown on the diff.">
						Resolved
					</Badge>
				) : (
					outdated && (
						<Badge
							variant="lightGray"
							title="These lines have changed since the review saw them, so this is not shown on the diff."
						>
							Outdated
						</Badge>
					)
				)}
				{!expanded && (
					<span className={styles.threadCount}>
						{thread.comments.length === 1 ? "1 comment" : `${thread.comments.length} comments`}
					</span>
				)}
			</button>
			{expanded && (
				<div className={styles.threadComments}>
					{firstComment?.diffHunk != null && (
						<ThreadHunk
							projectId={projectId}
							path={thread.path}
							lineNr={thread.line ?? thread.originalLine}
							diffHunk={firstComment.diffHunk}
						/>
					)}
					{thread.comments.map((comment) => (
						<ThreadComment
							comment={comment}
							key={comment.id !== 0 ? comment.id : comment.htmlUrl}
						/>
					))}
					<ReviewThreadReply projectId={projectId} reviewId={reviewId} threadId={thread.id} />
				</div>
			)}
		</div>
	);
};

/**
 * The threads a review left, with the settled ones folded away. A bot review
 * can resolve a dozen conversations, and each still costs a row once it is
 * done being read.
 */
const ThreadList: FC<{
	projectId: string;
	reviewId: number;
	threads: Array<ForgeReviewThread>;
	branchApplied: boolean;
}> = ({ projectId, reviewId, threads, branchApplied }) => {
	const [showResolved, setShowResolved] = useState(false);
	const open = threads.filter((thread) => !thread.isResolved);
	const resolved = threads.filter((thread) => thread.isResolved);

	// Same courtesy as the threads themselves: a comment the reader was sent
	// to must not stay hidden behind the fold. Adjusted during render, once
	// per request, so the fold still closes at the reader's word.
	const requested = useRequestedComment(reviewId);
	const [answeredRequest, setAnsweredRequest] = useState<number | null>(null);
	if (
		requested !== null &&
		requested !== answeredRequest &&
		resolved.some((thread) => thread.comments.some((comment) => comment.id === requested))
	) {
		setAnsweredRequest(requested);
		setShowResolved(true);
	}

	const render = (thread: ForgeReviewThread) => (
		<Thread
			key={thread.id}
			projectId={projectId}
			reviewId={reviewId}
			thread={thread}
			branchApplied={branchApplied}
		/>
	);

	return (
		<div className={styles.threads}>
			{open.map(render)}

			{resolved.length > 0 && (
				<>
					<button
						className={classes("text-12", styles.resolvedFold)}
						onClick={() => setShowResolved((current) => !current)}
						type="button"
					>
						<Icon name={showResolved ? "chevron-down" : "chevron-right"} size={12} />
						{resolved.length === 1
							? "1 resolved conversation"
							: `${resolved.length} resolved conversations`}
					</button>
					{/* Kept mounted rather than dropped, so a thread opened in here is
					    still open if the fold is closed and reopened. */}
					<div className={styles.threads} hidden={!showResolved}>
						{resolved.map(render)}
					</div>
				</>
			)}
		</div>
	);
};

/**
 * Split threads into those posted as part of a review submission, which
 * belong under its card, and those with none, which stand on their own row.
 */
const fileThreadsUnderSubmissions = (
	submissions: Array<ForgeReviewSubmission> | undefined,
	threads: Array<ForgeReviewThread> | undefined,
): { filed: Map<number, Array<ForgeReviewThread>>; loose: Array<ForgeReviewThread> } => {
	const known = new Set((submissions ?? []).map((submission) => submission.id));
	const filed = new Map<number, Array<ForgeReviewThread>>();
	const loose: Array<ForgeReviewThread> = [];
	for (const thread of threads ?? []) {
		const reviewId = thread.comments[0]?.reviewId ?? null;
		if (reviewId === null || !known.has(reviewId)) {
			loose.push(thread);
			continue;
		}
		const under = filed.get(reviewId) ?? [];
		under.push(thread);
		filed.set(reviewId, under);
	}
	return { filed, loose };
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

const Submission: FC<{
	projectId: string;
	reviewId: number;
	submission: ForgeReviewSubmission;
	threads: Array<ForgeReviewThread>;
	branchApplied: boolean;
}> = ({ projectId, reviewId, submission, threads, branchApplied }) => {
	const submittedAtMs = submission.submittedAt === null ? null : Date.parse(submission.submittedAt);
	const body = submission.body?.trim() === "" ? null : submission.body;

	return (
		<Card
			author={submission.author}
			badge={submissionBadge[submission.state]}
			timestamp={submittedAtMs}
			freshKey={`s:${submission.id}`}
			id={submission.id > 0 ? commentAnchorId(submission.id) : undefined}
		>
			{/* A review that only left diff comments has no body of its own;
			    without its threads the card would say nothing at all. */}
			{body === null && threads.length === 0 ? undefined : (
				<>
					{body !== null && (
						<Clamped maxHeight="240px">
							<Markdown>{body}</Markdown>
						</Clamped>
					)}
					{threads.length > 0 && (
						<ThreadList
							projectId={projectId}
							reviewId={reviewId}
							threads={threads}
							branchApplied={branchApplied}
						/>
					)}
				</>
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
const FeedEvent: FC<{
	icon: IconName;
	timestamp: number | null;
	freshKey?: string;
	author?: ForgeReviewUser | null;
	children: ReactNode;
}> = ({ icon, timestamp, freshKey, author, children }) => (
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
			<FreshBadge timestamp={timestamp} author={author} itemKey={freshKey} />
		</div>
	</div>
);

/** A referenced object inside an event line — an actor, a branch, a sha. */
const Ref: FC<{ children: ReactNode; mono?: boolean }> = ({ children, mono = false }) => (
	<span className={classes(styles.eventRef, mono && styles.eventMono)}>{children}</span>
);

const TimelineEvent: FC<{ event: ForgeReviewTimelineEvent }> = ({ event }) => {
	const createdAtMs = event.createdAt === null ? null : Date.parse(event.createdAt);

	const freshKey = event.createdAt === null ? undefined : `e:${event.kind}:${event.createdAt}`;

	if (event.kind === "committed") {
		return (
			<FeedEvent icon="commit" timestamp={createdAtMs} freshKey={freshKey} author={event.actor}>
				{event.commitAuthorName !== null && <Ref>{event.commitAuthorName}</Ref>} committed{" "}
				{event.commitSha !== null && <Ref mono>{event.commitSha.slice(0, 7)}</Ref>}{" "}
				{event.commitSummary}
			</FeedEvent>
		);
	}

	return (
		<FeedEvent icon="user" timestamp={createdAtMs} freshKey={freshKey} author={event.actor}>
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
	| { kind: "thread"; at: number; thread: ForgeReviewThread }
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
	looseThreads: Array<ForgeReviewThread>,
	events: Array<ForgeReviewTimelineEvent> | undefined,
): Array<TimelineItem> => {
	const items: Array<TimelineItem> = [];
	if (review.createdAt !== null)
		items.push({ kind: "opened", at: parseTimestamp(review.createdAt), review });
	for (const comment of comments ?? [])
		items.push({ kind: "comment", at: parseTimestamp(comment.createdAt), comment });
	for (const submission of submissions ?? [])
		items.push({ kind: "submission", at: parseTimestamp(submission.submittedAt), submission });
	for (const thread of looseThreads) {
		items.push({
			kind: "thread",
			at: parseTimestamp(thread.comments[0]?.createdAt ?? null),
			thread,
		});
	}
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

/**
 * How many happenings the Activity section shows before asking. Recent
 * pushes are the point of the section, so the rest stays folded away.
 */
const collapsedTimelineCount = 5;

/**
 * The compact happenings — opened, commits, review requests — for the side
 * panel's Activity section, newest first. The conversation itself stays in
 * the main column.
 */
export const ReviewTimeline: FC<{ projectId: string; review: ForgeReview }> = ({
	projectId,
	review,
}) => {
	const reviewId = review.number;
	const { data: events, isPending } = useQuery(
		listReviewTimelineEventsQueryOptions({ projectId, reviewId }),
	);
	const { data: currentLogin } = useQuery(currentForgeLoginQueryOptions(projectId));
	const [expanded, setExpanded] = useState(false);

	// Events render here, so this surface owns their skip registration —
	// keyed the way `TimelineEvent` keys its markers. Memoized: the list's
	// identity feeds `RegisterFreshItems`'s effect, so a fresh copy per
	// render would re-register on every poll.
	const freshEvents = useMemo(
		() =>
			(events ?? [])
				.filter(
					(event) =>
						event.createdAt !== null &&
						!(currentLogin != null && event.actor?.login === currentLogin),
				)
				.map((event) => ({
					key: `e:${event.kind}:${event.createdAt ?? ""}`,
					atMs: Date.parse(event.createdAt ?? ""),
				})),
		[events, currentLogin],
	);

	// `timelineItems` sorts oldest first, which the conversation wants and this
	// does not: here the latest push is the point.
	const items = useMemo(
		() =>
			timelineItems(review, undefined, undefined, [], events)
				.filter((item) => item.kind === "opened" || item.kind === "event")
				.reverse(),
		[review, events],
	);

	if (isPending) return <div className={classes("text-13", styles.commentsEmpty)}>Loading…</div>;
	const shown = expanded ? items : items.slice(0, collapsedTimelineCount);
	const hidden = items.length - shown.length;

	return (
		<div className={styles.commentList}>
			<RegisterFreshItems source="timeline" items={freshEvents} />
			{shown.map((item) =>
				item.kind === "opened" ? (
					<FeedEvent key="opened" icon="pr" timestamp={item.at}>
						{item.review.author !== null && <Ref>{item.review.author.login}</Ref>} opened this pull
						request
					</FeedEvent>
				) : (
					<TimelineEvent key={item.key} event={item.event} />
				),
			)}
			{hidden > 0 && (
				<button
					className={classes("text-12", styles.timelineMore)}
					onClick={() => setExpanded(true)}
					type="button"
				>
					Show {hidden} more
				</button>
			)}
		</div>
	);
};

/** The conversation, oldest first: comment, review and thread cards, then the composer. */
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
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const { data: threads } = useQuery({
		...listReviewThreadsQueryOptions({ projectId, reviewId }),
		// The conversation mounts optimistically while `forgeInfo` loads;
		// this endpoint's capability gate has to hold here instead.
		enabled: forgeInfo?.capabilities.reviewComments === true,
	});
	const { data: currentLogin } = useQuery(currentForgeLoginQueryOptions(projectId));
	// Whether the review's branch is in the workspace; its threads only check
	// themselves against the working file when it is.
	const { data: sourceBranchApplied } = useQuery({
		...headInfoQueryOptions(projectId),
		select: (headInfo) =>
			getHeadInfoIndex(headInfo).isApplied(encodeBytes(`refs/heads/${review.sourceBranch}`)),
	});
	// The forge has no "authenticated user" endpoint, so the GitButler account
	// picture stands in until the caller has actually posted here.
	const { data: profile } = useQuery(userProfileQueryOptions);
	const { mutate: createReviewComment } = useCreateReviewComment(projectId);
	const [draft, setDraft] = useState("");
	const composerRef = useRef<HTMLTextAreaElement | null>(null);

	// What the dwell may record as skipped: the conversation's own unread-
	// eligible items. Memoized: this component re-renders per draft
	// keystroke, and the derivation walks every listing.
	const freshItems = useMemo(() => {
		const own = (login: string | null | undefined) =>
			currentLogin != null && login != null && login.toLowerCase() === currentLogin.toLowerCase();
		return [
			...(comments ?? [])
				.filter(
					(comment) => comment.id > 0 && comment.createdAt !== null && !own(comment.author?.login),
				)
				.map((comment) => ({ key: `c:${comment.id}`, atMs: Date.parse(comment.createdAt ?? "") })),
			...(submissions ?? [])
				.filter((submission) => submission.submittedAt !== null && !own(submission.author?.login))
				.map((submission) => ({
					key: `s:${submission.id}`,
					atMs: Date.parse(submission.submittedAt ?? ""),
				})),
			...(threads ?? [])
				.flatMap((thread) => thread.comments)
				.filter(
					(comment) => comment.id > 0 && comment.createdAt !== null && !own(comment.author?.login),
				)
				.map((comment) => ({ key: `tc:${comment.id}`, atMs: Date.parse(comment.createdAt ?? "") })),
		];
	}, [comments, submissions, threads, currentLogin]);

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

	// Memoized like `freshItems`: this component re-renders per composer
	// keystroke, and a fresh grouping would re-render every card below.
	const { filed, loose } = useMemo(
		() => fileThreadsUnderSubmissions(submissions, threads),
		[submissions, threads],
	);
	// Events render in the side panel's Activity section instead.
	const items = useMemo(
		() => timelineItems(review, comments, submissions, loose, []),
		[review, comments, submissions, loose],
	);

	// A notification named a comment: scroll to it once it is on the page.
	// While the listing still loads the request is left standing for a later
	// run — consuming it early is a no-op scroll. Once loaded, a comment
	// still absent (inside a collapsed thread, or deleted) is cleared anyway,
	// so a stale request cannot fire at the next review.
	const requestedComment = useRequestedComment(review.number);
	const loading = isPending || submissionsPending;
	useEffect(() => {
		if (requestedComment === null || loading) return;
		// The anchor may not be in the DOM yet — a collapsed thread or the
		// resolved fold is still opening for it — so a miss waits rather than
		// giving up; the window below bounds the waiting. The cards around it
		// also keep laying out after it appears (diff hunks and images grow
		// and push it away from wherever one scroll put it), so it is kept
		// centred until its position holds still. The reader scrolling is a
		// better aim than this one: any scroll input stops it.
		let target: HTMLElement | null = null;
		let lastTop = Number.NaN;
		const aim = () => {
			target ??= document.getElementById(commentAnchorId(requestedComment));
			if (target === null) return;
			const top = target.getBoundingClientRect().top;
			if (top !== lastTop) target.scrollIntoView({ block: "center" });
			lastTop = top;
		};
		aim();
		const interval = window.setInterval(aim, 200);
		const done = () => {
			window.clearInterval(interval);
			window.clearTimeout(letGo);
			for (const kind of ["wheel", "touchmove", "keydown"] as const)
				window.removeEventListener(kind, done);
			clearReviewFocus();
		};
		const letGo = window.setTimeout(done, 1600);
		for (const kind of ["wheel", "touchmove", "keydown"] as const)
			window.addEventListener(kind, done, { passive: true });
		return () => {
			window.clearInterval(interval);
			window.clearTimeout(letGo);
			for (const kind of ["wheel", "touchmove", "keydown"] as const)
				window.removeEventListener(kind, done);
		};
	}, [requestedComment, loading, items.length]);

	return (
		<div className={styles.comments}>
			<RegisterFreshItems source="conversation" items={freshItems} />
			{loading ? (
				<div className={classes("text-13", styles.commentsEmpty)}>Loading…</div>
			) : (
				<div className={styles.commentList}>
					{items.map((item) =>
						item.kind === "comment" ? (
							<Comment
								key={`comment-${item.comment.id}`}
								projectId={projectId}
								reviewId={reviewId}
								comment={item.comment}
								currentLogin={currentLogin}
								onReply={handleReply}
							/>
						) : item.kind === "submission" ? (
							<Submission
								key={`submission-${item.submission.id}`}
								projectId={projectId}
								reviewId={reviewId}
								submission={item.submission}
								threads={filed.get(item.submission.id) ?? []}
								branchApplied={sourceBranchApplied === true}
							/>
						) : item.kind === "thread" ? (
							<div className={styles.card} key={`thread-${item.thread.id}`}>
								<div className={styles.cardBody}>
									<Thread
										projectId={projectId}
										reviewId={reviewId}
										thread={item.thread}
										branchApplied={sourceBranchApplied === true}
									/>
								</div>
							</div>
						) : null,
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
