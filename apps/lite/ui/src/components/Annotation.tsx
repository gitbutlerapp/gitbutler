import { classes } from "#ui/components/classes.ts";
import { RelativeTime } from "#ui/components/RelativeTime.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import type { CommentAcknowledgement, CommentAuthorKind } from "@gitbutler/but-sdk";
import { Tooltip } from "@base-ui/react";
import type { FC, FormEventHandler, KeyboardEventHandler, ReactNode, Ref } from "react";
import styles from "./Annotation.module.css";
import { FieldTextareaStyles } from "./Field.tsx";

type Props = {
	author: string;
	defaultBody: string;
	name: string;
	textareaRef?: Ref<HTMLTextAreaElement>;
	onKeyDown?: KeyboardEventHandler<HTMLTextAreaElement>;
	onInput?: FormEventHandler<HTMLTextAreaElement>;
	mentionPicker?: ReactNode;
	updatedAt?: number;
	actions?: ReactNode;
};

type ReplyProps = {
	author: string;
	authorTitle: string | null;
	authorKind: CommentAuthorKind;
	body: string;
	createdAt: number;
	acknowledgements: Array<CommentAcknowledgement>;
	expectedAcknowledgementCount: number;
	actions?: ReactNode;
};

type AgentAvatarProps = {
	author: string;
	title: string | null;
	active?: boolean;
	selected?: boolean;
	small?: boolean;
};

const agentHue = (title: string | null, author: string): number => {
	let hash = 0;
	for (const character of title ?? author) hash = (hash * 31 + (character.codePointAt(0) ?? 0)) | 0;
	return Math.abs(hash) % 360;
};

/** Deterministic visual identity for one titled agent workstream. */
export const AgentAvatar: FC<AgentAvatarProps> = ({
	author,
	title,
	active = true,
	selected,
	small,
}) => (
	<span
		aria-hidden
		className={classes(
			styles.agentAvatar,
			!active && styles.agentAvatarInactive,
			selected && styles.agentAvatarSelected,
			small && styles.agentAvatarSmall,
		)}
		style={{
			backgroundColor: active ? `hsl(${agentHue(title, author)} 58% 48%)` : "var(--text-2)",
		}}
	>
		{author.slice(0, 1).toLocaleUpperCase()}
	</span>
);

export const Annotation: FC<Props> = ({ textareaRef, ...p }) => (
	<div className={styles.wrapper}>
		<header className={styles.header}>
			<h5 className={classes(styles.author, "text-13")}>{p.author}</h5>
			{p.updatedAt !== undefined && (
				<time
					dateTime={new Date(p.updatedAt).toISOString()}
					className={classes(styles.date, "text-12")}
				>
					<RelativeTime timestamp={p.updatedAt} />
				</time>
			)}
		</header>

		<FieldTextareaStyles
			className={classes(styles.body, "text-13")}
			defaultValue={p.defaultBody}
			name={p.name}
			onKeyDown={p.onKeyDown}
			onInput={p.onInput}
			ref={textareaRef}
			required
			rows={3}
		/>
		{p.mentionPicker}

		<div className={styles.actions}>{p.actions}</div>
	</div>
);

/** A read-only reply displayed as part of an annotation thread. */
export const AnnotationReply: FC<ReplyProps> = (p) => (
	<div
		className={classes(
			styles.wrapper,
			p.authorKind === "human" ? styles.humanReply : styles.agentReply,
		)}
	>
		<header className={styles.header}>
			{p.authorKind === "agent" && <AgentAvatar author={p.author} title={p.authorTitle} />}
			<div className={styles.authorIdentity}>
				<h5 className={classes(styles.author, "text-13")}>{p.author}</h5>
				{p.authorTitle !== null && p.authorTitle !== "" && (
					<span className={classes(styles.authorTitle, "text-12")}>{p.authorTitle}</span>
				)}
			</div>
			<div className={styles.messageMeta}>
				{p.authorKind === "human" && p.acknowledgements.length > 0 && (
					<Tooltip.Root>
						<Tooltip.Trigger
							className={classes(styles.readReceipt, "text-12")}
							aria-label={`${p.acknowledgements.length} agents acknowledged this message`}
						>
							{p.acknowledgements.length >= p.expectedAcknowledgementCount ? "✓✓" : "✓"}
						</Tooltip.Trigger>
						<Tooltip.Portal>
							<Tooltip.Positioner side="top" sideOffset={4}>
								<Tooltip.Popup render={<TooltipPopup />}>
									<span className={styles.readTooltip}>
										{p.acknowledgements.map((acknowledgement) => (
											<span key={acknowledgement.clientId} className={styles.readTooltipRow}>
												<AgentAvatar
													author={acknowledgement.author}
													title={acknowledgement.title}
													small
												/>
												<span>
													<strong>{acknowledgement.author}</strong>
													{acknowledgement.title !== null &&
														acknowledgement.title !== "" &&
														` — ${acknowledgement.title}`}
												</span>
											</span>
										))}
									</span>
								</Tooltip.Popup>
							</Tooltip.Positioner>
						</Tooltip.Portal>
					</Tooltip.Root>
				)}
				<time
					dateTime={new Date(p.createdAt).toISOString()}
					className={classes(styles.date, "text-12")}
				>
					<RelativeTime timestamp={p.createdAt} />
				</time>
			</div>
		</header>

		<p className={classes(styles.replyBody, "text-13")}>{p.body}</p>
		{p.actions !== undefined && <div className={styles.actions}>{p.actions}</div>}
	</div>
);
