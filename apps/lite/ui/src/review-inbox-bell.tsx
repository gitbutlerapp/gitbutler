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
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";
import { Toggle, ToggleGroup } from "@base-ui/react";
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
	isBotEntry,
	type InboxEntry,
	type InboxKind,
} from "#ui/review-inbox.ts";
import { usePrNotificationsLevel } from "#ui/review-seen.ts";
import { Dropdown } from "#ui/components/Popup.tsx";
import { useQuery } from "@tanstack/react-query";
import { useState, type FC } from "react";
import styles from "./review-inbox-bell.module.css";

type NotificationType = "humans" | "agents";

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
	const [tab, setTab] = useState<NotificationType>("humans");
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	// Unconditional: behind `&&` the hook count would change mid-mount.
	const level = usePrNotificationsLevel();
	const shown = level === "loud" && !!forgeInfo?.capabilities.prService;
	const allEntries = useInboxEntries(projectId, shown);
	const humanEntries = allEntries.filter((entry) => !isBotEntry(entry));
	const agentEntries = allEntries.filter(isBotEntry);
	const humanUnseen = humanEntries.filter((entry) => !entry.seen).length;
	const agentUnseen = agentEntries.filter((entry) => !entry.seen).length;
	const unseen = humanUnseen + agentUnseen;
	const entries = tab === "agents" ? agentEntries : humanEntries;
	const tabUnseen = tab === "agents" ? agentUnseen : humanUnseen;
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
			</div>
			<div className={styles.switcher}>
				<ToggleGroup
					render={<ToggleGroupStyles />}
					aria-label="Notification type"
					value={[tab]}
					onValueChange={([next]) => {
						if (next !== undefined) setTab(next);
					}}
				>
					<Toggle
						render={<ToggleStyles size="small" />}
						value={"humans" satisfies NotificationType}
					>
						Humans{humanUnseen > 0 && ` (${humanUnseen})`}
					</Toggle>
					<Toggle
						render={<ToggleStyles size="small" />}
						value={"agents" satisfies NotificationType}
					>
						Agents{agentUnseen > 0 && ` (${agentUnseen})`}
					</Toggle>
				</ToggleGroup>
				{tabUnseen > 0 && (
					<button
						className={classes("text-12", styles.markAll)}
						onClick={() =>
							markInboxSeen(
								projectId,
								entries.map((entry) => entry.id),
							)
						}
						type="button"
					>
						Mark all read
					</button>
				)}
			</div>
			<div className={styles.list}>
				{entries.length === 0 ? (
					<div className={classes("text-12", styles.empty)}>
						{tab === "agents" ? "No agent notifications yet" : "No human notifications yet"}
					</div>
				) : (
					entries.map((entry) => (
						<Entry
							key={entry.id}
							projectId={projectId}
							entry={entry}
							appliedRefs={appliedRefs}
							onNavigate={() => setOpen(false)}
						/>
					))
				)}
			</div>
		</Dropdown>
	);
};
