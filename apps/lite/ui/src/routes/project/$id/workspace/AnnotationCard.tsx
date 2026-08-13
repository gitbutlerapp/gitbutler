import {
	feedbackPrompt,
	type LocalAnnotation,
	type LocalAnnotationsByPath,
	useCommentArchive,
	useCommentDraftPublish,
	useCommentReply,
} from "#ui/annotation.ts";
import { AgentAvatar, Annotation, AnnotationReply } from "#ui/components/Annotation.tsx";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import type { FileParent } from "#ui/operands.ts";
import type { CommentClient, CommentParticipant } from "@gitbutler/but-sdk";
import { Tooltip } from "@base-ui/react";
import { useHotkey } from "@tanstack/react-hotkeys";
import { useRef, useState, type FC, type KeyboardEvent, type RefObject } from "react";
import styles from "#ui/components/Annotation.module.css";

type Props = {
	projectId: string;
	annotation: LocalAnnotation;
	agents: Array<CommentClient>;
	path: string;
	fileParent: FileParent;
	/** All annotations of the current file parent, for "Copy all as prompt". */
	annotationsByPath: LocalAnnotationsByPath;
	/** Set to an annotation id to focus its textarea on mount (freshly created comments). */
	focusAnnotationIdRef: RefObject<string | null>;
	selectionScopeRef: RefObject<HTMLDivElement | null>;
};

type MentionQuery = {
	start: number;
	end: number;
	query: string;
};

const bodyFromForm = (form: HTMLFormElement | null, name: string): string => {
	if (!form) throw new Error("Missing owning form");
	const body = new FormData(form).get(name);
	if (typeof body !== "string") throw new Error("Missing or invalid body");
	return body;
};

const agentTooltip = (agent: Pick<CommentClient, "author" | "title">) => (
	<span className={styles.agentTooltip}>
		<strong>{agent.author}</strong>
		{agent.title !== null && agent.title !== "" && <span>{agent.title}</span>}
	</span>
);

/** One backend-persisted comment thread rendered inline in the diff. */
export const AnnotationCard: FC<Props> = (p) => {
	const { annotation, focusAnnotationIdRef } = p;
	const { mutate: publishDraft } = useCommentDraftPublish();
	const { mutate: reply } = useCommentReply();
	const { mutate: archiveComment } = useCommentArchive();
	const [isReplying, setIsReplying] = useState(false);
	const [mentionQuery, setMentionQuery] = useState<MentionQuery | null>(null);
	const [mentionedClientIds, setMentionedClientIds] = useState<Array<string>>([]);
	const draft =
		annotation.messages.length === 1 &&
		annotation.messages[0]?.author === "You" &&
		annotation.messages[0].authorKind === "human" &&
		annotation.messages[0].payload.trim() === ""
			? annotation.messages[0]
			: undefined;
	const draftTextareaRef = useRef<HTMLTextAreaElement | null>(null);
	const replyTextareaRef = useRef<HTMLTextAreaElement | null>(null);
	const persistedDraftRef = useRef(draft?.payload ?? "");
	const replyName = `${annotation.id}-reply`;
	const persistDraft = (body: string) => {
		if (!draft || body.trim() === "" || body === persistedDraftRef.current) return;
		persistedDraftRef.current = body;
		publishDraft({
			projectId: p.projectId,
			commentId: annotation.id,
			messageId: draft.id,
			payload: body,
			mentionedClientIds,
		});
	};

	const updateMentionQuery = (textarea: HTMLTextAreaElement) => {
		const cursor = textarea.selectionStart;
		const match = textarea.value.slice(0, cursor).match(/(?:^|\s)@([^\s@]*)$/u);
		if (!match) return setMentionQuery(null);
		const query = match[1] ?? "";
		setMentionQuery({ start: cursor - query.length - 1, end: cursor, query });
	};

	const mentionCandidates = mentionQuery
		? p.agents.filter((agent) => {
				if (
					annotation.agentParticipantIds.includes(agent.id) ||
					mentionedClientIds.includes(agent.id)
				)
					return false;
				const query = mentionQuery.query.toLocaleLowerCase();
				return (
					agent.author.toLocaleLowerCase().includes(query) ||
					agent.title?.toLocaleLowerCase().includes(query)
				);
			})
		: [];
	const invitedActive = annotation.agentParticipants.filter((agent) => agent.active);
	const invitedInactive = annotation.agentParticipants.filter((agent) => !agent.active);
	const availableAgents = p.agents.filter(
		(agent) => !annotation.agentParticipantIds.includes(agent.id),
	);

	const togglePendingInvitation = (agent: CommentClient) => {
		setMentionedClientIds((ids) =>
			ids.includes(agent.id) ? ids.filter((id) => id !== agent.id) : ids.concat(agent.id),
		);
		if (!draft) setIsReplying(true);
	};

	const selectMention = (agent: CommentClient) => {
		const textarea = draft ? draftTextareaRef.current : replyTextareaRef.current;
		if (!textarea || !mentionQuery) return;
		textarea.setRangeText("", mentionQuery.start, mentionQuery.end, "end");
		setMentionedClientIds((ids) => (ids.includes(agent.id) ? ids : ids.concat(agent.id)));
		setMentionQuery(null);
		textarea.focus();
	};

	const invitedAvatar = (agent: CommentParticipant) => (
		<Tooltip.Root key={agent.id}>
			<Tooltip.Trigger render={<span className={styles.agentParticipant} />}>
				<AgentAvatar author={agent.author} title={agent.title} active={agent.active} />
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner side="top" sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup />}>{agentTooltip(agent)}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);

	const agentBar =
		invitedActive.length + invitedInactive.length + availableAgents.length > 0 ? (
			<div className={styles.agentParticipants} aria-label="Thread agents">
				{invitedActive.map(invitedAvatar)}
				{invitedInactive.map(invitedAvatar)}
				{annotation.agentParticipants.length > 0 && availableAgents.length > 0 && (
					<span className={styles.agentParticipantSeparator} aria-hidden>
						·
					</span>
				)}
				{availableAgents.map((agent) => {
					const selected = mentionedClientIds.includes(agent.id);
					return (
						<Tooltip.Root key={agent.id}>
							<Tooltip.Trigger
								render={
									<button
										type="button"
										className={styles.agentParticipantButton}
										aria-label={`${selected ? "Cancel invitation for" : "Invite"} ${agent.author}`}
										onClick={() => togglePendingInvitation(agent)}
									/>
								}
							>
								<AgentAvatar
									author={agent.author}
									title={agent.title}
									active={false}
									selected={selected}
								/>
							</Tooltip.Trigger>
							<Tooltip.Portal>
								<Tooltip.Positioner side="top" sideOffset={4}>
									<Tooltip.Popup render={<TooltipPopup />}>{agentTooltip(agent)}</Tooltip.Popup>
								</Tooltip.Positioner>
							</Tooltip.Portal>
						</Tooltip.Root>
					);
				})}
			</div>
		) : null;

	const selectFirstMentionWithKeyboard = (evt: KeyboardEvent<HTMLTextAreaElement>): boolean => {
		const first = mentionCandidates[0];
		if (!first || (evt.key !== "Enter" && evt.key !== "Tab") || evt.metaKey || evt.ctrlKey)
			return false;
		evt.preventDefault();
		evt.stopPropagation();
		selectMention(first);
		return true;
	};

	const mentionPicker = mentionQuery && (
		<div className={styles.mentionPicker} aria-label="Invite an agent">
			{mentionCandidates.length > 0 ? (
				mentionCandidates.map((agent) => (
					<button
						key={agent.id}
						type="button"
						className={styles.mentionOption}
						onMouseDown={(evt) => evt.preventDefault()}
						onClick={() => selectMention(agent)}
					>
						<span>@{agent.author}</span>
						{agent.title !== null && agent.title !== "" && <small>{agent.title}</small>}
					</button>
				))
			) : (
				<div className={styles.mentionEmpty}>
					{p.agents.length === 0 ? "No agents are listening" : "No matching agents"}
				</div>
			)}
		</div>
	);

	const archiveAndRefocus = () => {
		draftTextareaRef.current?.form?.reset();
		archiveComment({ projectId: p.projectId, id: annotation.id });
		p.selectionScopeRef.current?.focus({ focusVisible: false });
	};

	const cancelReplyAndRefocus = () => {
		replyTextareaRef.current?.form?.reset();
		setIsReplying(false);
		setMentionQuery(null);
		setMentionedClientIds([]);
		p.selectionScopeRef.current?.focus({ focusVisible: false });
	};

	const copyAll = () => {
		const feedback = p.annotationsByPath
			.entries()
			.flatMap(([path, annotations]) =>
				annotations.map((annotation) => ({ annotation, fileParent: p.fileParent, path })),
			);
		void window.lite.clipboardWriteText(feedbackPrompt(feedback.toArray()));
	};

	useHotkey("Mod+Enter", () => draftTextareaRef.current?.form?.requestSubmit(), {
		conflictBehavior: "allow",
		enabled: draft !== undefined,
		ignoreInputs: false,
		target: draftTextareaRef,
	});

	useHotkey("Escape", archiveAndRefocus, {
		conflictBehavior: "allow",
		enabled: draft !== undefined,
		ignoreInputs: false,
		target: draftTextareaRef,
	});

	if (draft) {
		return (
			// oxlint-disable-next-line jsx-a11y/no-noninteractive-element-interactions -- Equivalent to a blur event on each input.
			<form
				data-local-annotation
				onBlur={(evt) => {
					const next = evt.relatedTarget;
					if (next instanceof Node && evt.currentTarget.contains(next)) return;
					persistDraft(bodyFromForm(evt.currentTarget, draft.id));
				}}
				onSubmit={(evt) => {
					evt.preventDefault();
					persistDraft(bodyFromForm(evt.currentTarget, draft.id));
					p.selectionScopeRef.current?.focus({ focusVisible: false });
				}}
			>
				<Annotation
					author={draft.author}
					defaultBody={draft.payload}
					updatedAt={draft.updatedAtMs}
					name={draft.id}
					mentionPicker={mentionPicker}
					onInput={(evt) => updateMentionQuery(evt.currentTarget)}
					onKeyDown={selectFirstMentionWithKeyboard}
					textareaRef={(textarea) => {
						draftTextareaRef.current = textarea;
						if (textarea && focusAnnotationIdRef.current === annotation.id) {
							focusAnnotationIdRef.current = null;
							textarea.focus();
						}
					}}
					actions={
						<>
							<button type="submit" className={getButtonClassName({ variant: "pop" })}>
								Save
							</button>
							<button
								type="button"
								className={getButtonClassName({ variant: "ghost" })}
								onClick={archiveAndRefocus}
							>
								Cancel
							</button>
						</>
					}
				/>
				{agentBar}
			</form>
		);
	}

	return (
		<div>
			{annotation.messages.map((message, index) => (
				<AnnotationReply
					key={message.id}
					author={message.author}
					authorTitle={message.authorTitle}
					authorKind={message.authorKind}
					body={message.payload}
					createdAt={message.updatedAtMs}
					acknowledgements={message.acknowledgements}
					expectedAcknowledgementCount={message.expectedAcknowledgementCount}
					actions={
						index === annotation.messages.length - 1 ? (
							<>
								<button
									type="button"
									className={getButtonClassName({ variant: "ghost" })}
									onClick={archiveAndRefocus}
								>
									Resolve
								</button>
								{!isReplying && (
									<button
										type="button"
										className={getButtonClassName({ variant: "ghost" })}
										onClick={() => setIsReplying(true)}
									>
										Reply
									</button>
								)}
								<button
									type="button"
									aria-label="Copy as prompt"
									title="Copy as prompt"
									style={{ marginLeft: "auto" }}
									className={getButtonClassName({ variant: "ghost", iconOnly: true })}
									onClick={() =>
										void window.lite.clipboardWriteText(
											feedbackPrompt([
												{
													annotation,
													fileParent: p.fileParent,
													path: p.path,
												},
											]),
										)
									}
								>
									<Icon name="copy" />
								</button>
								<button
									type="button"
									aria-label="Copy all as prompt"
									title="Copy all as prompt"
									className={getButtonClassName({ variant: "ghost", iconOnly: true })}
									onClick={copyAll}
								>
									<Icon name="checklist" />
								</button>
							</>
						) : undefined
					}
				/>
			))}

			{isReplying && (
				<form
					onSubmit={(evt) => {
						evt.preventDefault();
						const payload = bodyFromForm(evt.currentTarget, replyName);
						if (payload.trim() === "") return;
						reply({
							projectId: p.projectId,
							id: annotation.id,
							message: {
								id: crypto.randomUUID(),
								author: "You",
								authorKind: "human",
								authorClientId: null,
								mentionedClientIds,
								payload,
							},
							acknowledgeThrough: null,
						});
						evt.currentTarget.reset();
						setIsReplying(false);
						setMentionedClientIds([]);
						setMentionQuery(null);
					}}
				>
					<Annotation
						author="You"
						defaultBody=""
						name={replyName}
						mentionPicker={mentionPicker}
						onKeyDown={(evt) => {
							if (evt.key === "Escape" && evt.currentTarget.value.trim() === "") {
								evt.preventDefault();
								evt.stopPropagation();
								cancelReplyAndRefocus();
								return;
							}
							if (selectFirstMentionWithKeyboard(evt)) return;
							if (evt.key !== "Enter" || !(evt.metaKey || evt.ctrlKey)) return;
							evt.preventDefault();
							evt.stopPropagation();
							evt.currentTarget.form?.requestSubmit();
						}}
						onInput={(evt) => updateMentionQuery(evt.currentTarget)}
						textareaRef={(textarea) => {
							replyTextareaRef.current = textarea;
							textarea?.focus();
						}}
						actions={
							<>
								<button type="submit" className={getButtonClassName({ variant: "pop" })}>
									Reply
								</button>
								<button
									type="button"
									className={getButtonClassName({ variant: "ghost" })}
									onClick={cancelReplyAndRefocus}
								>
									Cancel
								</button>
							</>
						}
					/>
				</form>
			)}
			{agentBar}
		</div>
	);
};
