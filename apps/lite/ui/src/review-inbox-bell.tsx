/**
 * @file The bell: the inbox's face in the window corner.
 *
 * A red dot says entries wait unseen; the popover lists them newest first,
 * each row answering what happened, to what, and by whom. Opening the panel
 * does not mark anything seen — clicking an entry does, the same way seeing
 * works everywhere else in this feature.
 */

import { forgeInfoOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { Icon } from "#ui/components/Icon.tsx";
import type { IconName } from "#ui/components/iconNames.ts";
import { RelativeTime } from "#ui/components/RelativeTime.tsx";
import { classes } from "#ui/components/classes.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { appliedRefsByName, openInboxEntry, type AppliedRefs } from "#ui/review-notifications.ts";
import {
	entryHeadline,
	inboxKindAttention,
	markInboxSeen,
	useInboxEntries,
	useInboxUnseenCount,
	type InboxEntry,
	type InboxKind,
} from "#ui/review-inbox.ts";
import { usePrNotificationsLevel } from "#ui/review-seen.ts";
import { Dropdown } from "#ui/components/Popup.tsx";
import { useQuery } from "@tanstack/react-query";
import { useState, type FC } from "react";
import styles from "./review-inbox-bell.module.css";

const kindIcon: Record<InboxKind, IconName> = {
	comment: "text-block",
	mention: "text-block",
	approved: "tick-circle",
	changesRequested: "cross-circle",
	reviewRequested: "user",
	committed: "commit",
	merged: "branch-merge",
	closed: "pr-close",
};

/** Semantic tints for the kinds whose meaning has a color; the rest stay gray. */
const kindTint: Partial<Record<InboxKind, string>> = {
	mention: styles.entryIconPop,
	approved: styles.entryIconSafe,
	changesRequested: styles.entryIconWarn,
};

const Entry: FC<{
	projectId: string;
	entry: InboxEntry;
	/** Shared by the bell: one head-info subscription serves every row. */
	appliedRefs: AppliedRefs | undefined;
	/** The panel closes itself once a click has somewhere to go. */
	onNavigate: () => void;
}> = ({ projectId, entry, appliedRefs, onNavigate }) => {
	const open = () => {
		// Still loading is not "not in the workspace": acting now could open
		// the forge for a local branch, and eat the unread mark doing it.
		if (appliedRefs === undefined) return;
		onNavigate();
		openInboxEntry(projectId, entry, appliedRefs);
	};

	return (
		<button className={styles.entry} onClick={open} type="button" title={entry.reviewTitle}>
			<Icon
				name={kindIcon[entry.kind]}
				className={classes(styles.entryIcon, kindTint[entry.kind])}
			/>
			<span className={styles.entryBody}>
				<span
					className={classes(
						"text-12",
						styles.entryHeadline,
						inboxKindAttention[entry.kind] === "quiet" && styles.entryHeadlineQuiet,
					)}
				>
					{entryHeadline(entry)}
				</span>
				<span className={classes("text-11", styles.entryTarget)}>
					<span className={styles.entryBranch}>{entry.sourceBranch}</span> {entry.unitSymbol}
					{entry.review}
				</span>
				{entry.snippet !== null && (
					<span className={classes("text-11", styles.entrySnippet)}>{entry.snippet}</span>
				)}
			</span>
			<span className={styles.entryEnd}>
				{!entry.seen && <span aria-hidden className={styles.entryUnseen} />}
				<RelativeTime
					timestamp={Date.parse(entry.at)}
					compact
					className={classes("text-11", styles.entryTime)}
				/>
			</span>
		</button>
	);
};

/**
 * The bell in the sidebar header. It owns its visibility: nothing renders
 * without forge review support, or below the loud dial.
 */
export const NotificationBell: FC<{ projectId: string }> = ({ projectId }) => {
	const [open, setOpen] = useState(false);
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	// Unconditional: behind `&&` the hook count would change mid-mount.
	const level = usePrNotificationsLevel();
	const shown = level === "loud" && !!forgeInfo?.capabilities.prService;
	const entries = useInboxEntries(projectId, shown);
	const unseen = useInboxUnseenCount(projectId, shown);
	const { data: appliedRefs } = useQuery({
		...headInfoQueryOptions(projectId),
		select: appliedRefsByName,
		enabled: shown,
	});

	if (!shown) return null;

	return (
		<Dropdown
			open={open}
			onOpenChange={setOpen}
			sideOffset={6}
			className={styles.panel}
			trigger={
				<button
					type="button"
					aria-label={unseen > 0 ? `Notifications, ${unseen} unread` : "Notifications"}
					className={classes(getButtonClassName({ iconOnly: true, variant: "ghost" }), styles.bell)}
				>
					<Icon name="bell" />
					{unseen > 0 && <span aria-hidden className={styles.bellDot} />}
				</button>
			}
		>
			<div className={styles.panelHeader}>
				<span className={classes("text-12", "text-semibold")}>Notifications</span>
				{unseen > 0 && (
					<button
						className={classes("text-12", styles.markAll)}
						onClick={() => markInboxSeen(projectId)}
						type="button"
					>
						Mark all read
					</button>
				)}
			</div>
			{entries.length === 0 ? (
				<div className={classes("text-12", styles.empty)}>Nothing yet</div>
			) : (
				<div className={styles.list}>
					{entries.map((entry) => (
						<Entry
							key={entry.id}
							projectId={projectId}
							entry={entry}
							appliedRefs={appliedRefs}
							onNavigate={() => setOpen(false)}
						/>
					))}
				</div>
			)}
		</Dropdown>
	);
};
