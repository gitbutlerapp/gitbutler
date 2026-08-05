import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { Popover, Tooltip } from "@base-ui/react";
import type { ForgeReviewReaction, ForgeReviewReactionCount } from "@gitbutler/but-sdk";
import { type FC, useState } from "react";
import styles from "./PullRequestReactions.module.css";

/**
 * The kinds every forge supports (GitHub's set, in its render order), with
 * display glyphs. Kinds are an open set — forges like GitLab allow
 * arbitrary award emoji — so this is the picker's offering and the glyph
 * lookup, not a schema: unknown kinds still render, as `:shortcode:`.
 */
const reactionGlyphs: Array<[string, string]> = [
	["+1", "👍"],
	["-1", "👎"],
	["laugh", "😄"],
	["hooray", "🎉"],
	["confused", "😕"],
	["heart", "❤️"],
	["rocket", "🚀"],
	["eyes", "👀"],
];

const glyphByKind = new Map(reactionGlyphs);

const reactionNames: Record<string, string> = {
	"+1": "thumbs up",
	"-1": "thumbs down",
	laugh: "laugh",
	hooray: "hooray",
	confused: "confused",
	heart: "heart",
	rocket: "rocket",
	eyes: "eyes",
};

const reactionName = (kind: string): string => reactionNames[kind] ?? kind;

/** One listed reaction: who left it, and the id that addresses removal. */
export type ReactionEntry = { id: number; login: string };

/** Reactions per kind, for the who-reacted tooltips and own-reaction toggling. */
export type ReactorsByKind = Partial<Record<string, Array<ReactionEntry>>>;

// oxlint-disable-next-line react/only-export-components -- Chip data and its component belong together.
export const groupReactors = (reactions: Array<ForgeReviewReaction>): ReactorsByKind => {
	const grouped: ReactorsByKind = {};
	for (const reaction of reactions) {
		if (reaction.user === null) continue;
		(grouped[reaction.kind] ??= []).push({ id: reaction.id, login: reaction.user.login });
	}
	return grouped;
};

export const Reactions: FC<{
	/** Nonzero tallies; known kinds order first, the rest keep listing order. */
	reactions: Array<ForgeReviewReactionCount>;
	reactors?: ReactorsByKind;
	/** The signed-in login; highlights own reactions and enables toggling. */
	myLogin?: string | null;
	/**
	 * Toggle the caller's reaction: `myReactionId` is the reaction to remove,
	 * or null to add one of `kind`. Chips are display-only without this.
	 */
	onToggle?: (kind: string, myReactionId: number | null) => void;
}> = ({ reactions, reactors, myLogin, onToggle }) => {
	const [pickerOpen, setPickerOpen] = useState(false);

	const mineFor = (kind: string) =>
		myLogin == null ? undefined : reactors?.[kind]?.find((entry) => entry.login === myLogin);

	const known = [...glyphByKind.keys()];
	const chips = [...reactions]
		.sort((a, b) => {
			const ai = known.indexOf(a.kind);
			const bi = known.indexOf(b.kind);
			return (ai === -1 ? known.length : ai) - (bi === -1 ? known.length : bi);
		})
		.map(({ kind, count }) => ({ kind, count, who: reactors?.[kind], mine: mineFor(kind) }));
	if (chips.length === 0 && onToggle === undefined) return null;

	// An optimistically added reaction has no forge id to remove by until the
	// settle refetch lands; toggling it off waits that round trip out.
	const toggle = (kind: string, mine: ReactionEntry | undefined) => {
		if (mine !== undefined && mine.id < 0) return;
		onToggle?.(kind, mine?.id ?? null);
	};

	return (
		<div className={styles.reactions}>
			{chips.map((chip) => {
				const glyph = glyphByKind.get(chip.kind) ?? `:${chip.kind}:`;
				const chipNode =
					onToggle === undefined ? (
						<span
							key={chip.kind}
							className={classes(
								"text-12",
								styles.reactionChip,
								chip.mine !== undefined && styles.reactionChipMine,
							)}
						>
							{glyph} {chip.count}
						</span>
					) : (
						<button
							key={chip.kind}
							type="button"
							aria-label={`Toggle ${reactionName(chip.kind)} reaction`}
							className={classes(
								"text-12",
								styles.reactionChip,
								styles.reactionChipButton,
								chip.mine !== undefined && styles.reactionChipMine,
							)}
							onClick={() => toggle(chip.kind, chip.mine)}
						>
							{glyph} {chip.count}
						</button>
					);
				if (chip.who === undefined || chip.who.length === 0) return chipNode;

				return (
					<Tooltip.Root key={chip.kind}>
						<Tooltip.Trigger render={chipNode} />
						<Tooltip.Portal>
							<Tooltip.Positioner sideOffset={4}>
								<Tooltip.Popup render={<TooltipPopup />}>
									{chip.who.map((entry) => entry.login).join(", ")}
								</Tooltip.Popup>
							</Tooltip.Positioner>
						</Tooltip.Portal>
					</Tooltip.Root>
				);
			})}

			{onToggle !== undefined && (
				<Popover.Root open={pickerOpen} onOpenChange={setPickerOpen}>
					<Popover.Trigger
						render={
							<button aria-label="Add reaction" className={styles.addReaction} type="button" />
						}
					>
						<Icon name="smiley" />
					</Popover.Trigger>
					<Popover.Portal>
						<Popover.Positioner align="start" sideOffset={4}>
							<Popover.Popup className={styles.reactionPicker}>
								{reactionGlyphs.map(([kind, glyph]) => {
									const mine = mineFor(kind);
									return (
										<button
											key={kind}
											aria-label={`React with ${reactionName(kind)}`}
											aria-pressed={mine !== undefined}
											className={classes(
												styles.pickerItem,
												mine !== undefined && styles.pickerItemMine,
											)}
											onClick={() => {
												toggle(kind, mine);
												setPickerOpen(false);
											}}
											type="button"
										>
											{glyph}
										</button>
									);
								})}
							</Popover.Popup>
						</Popover.Positioner>
					</Popover.Portal>
				</Popover.Root>
			)}
		</div>
	);
};
