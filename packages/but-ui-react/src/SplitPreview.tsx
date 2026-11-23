import { mergeProps, PreviewCard } from "@base-ui/react";
import { useRef, type ComponentProps, type FC, type ReactNode } from "react";
import { classes } from "./classes.ts";
import { Popup, PopupItem, PopupSection } from "./Popup.tsx";
import styles from "./SplitPreview.module.css";
import { isTile, tiles, type SplitTile, type Tiling } from "./splitTiling.ts";

/** Lays out `tiling`, drawing each tile with `tile`. */
const Layout: FC<{ tiling: Tiling; tile: (tile: SplitTile) => ReactNode; className?: string }> = ({
	tiling,
	tile,
	className,
}) =>
	isTile(tiling) ? (
		tile(tiling)
	) : (
		<div className={classes(className, styles[tiling.split])}>
			{tiling.parts.map((part, at) => (
				// oxlint-disable-next-line react/no-array-index-key -- A part is its place in the split; a split of tiles has no id of its own.
				<div key={at} className={styles.part} style={{ flexGrow: tiling.sizes?.[at] ?? 1 }}>
					<Layout tiling={part} tile={tile} className={className} />
				</div>
			))}
		</div>
	);

/** The split's tiles in miniature, the tile of the row `here` inked. */
const Miniature: FC<{ tiling: Tiling; here: string | undefined }> = ({ tiling, here }) => (
	<span className={styles.map}>
		<Layout
			tiling={tiling}
			className={styles.cells}
			tile={(tile) => <span className={classes(styles.cell, tile.id === here && styles.here)} />}
		/>
	</span>
);

/**
 * What a row's split holds: named after the row it was made on, the rows its tiles show, each as
 * its row in the sidebar says it, with its state. The row it was opened from (`here`) is marked.
 * It answers "what did I put there?" while another row is open, so it names the tiles and says
 * nothing of their content.
 *
 * A {@link SplitPreviewMap} opens it under the pointer. On its own it is the card, for a place
 * that shows it open. It reads, never acts, as a tooltip does: the pointer passes through it, so it
 * closes once the pointer leaves the map, and its items take no hover or press.
 *
 * @figma The examples are the layouts drawn open; the map's hover opens them in the app.
 * @import import { SplitPreview } from "@gitbutler/but-ui-react/SplitPreview.tsx";
 */
export const SplitPreview: FC<
	{
		tiling: Tiling;
		/** The `id` of the tile of the row it was opened from. */
		here?: string;
		/** Grows out of the corner it opens from, as the map's preview does. */
		anchored?: boolean;
	} & Omit<ComponentProps<"div">, "children">
> = ({ tiling, here, anchored = false, ...props }) => {
	const all = tiles(tiling);
	const home = all.find((tile) => tile.home === true) ?? all[0];
	return (
		<Popup {...props} anchored={anchored} className={classes(props.className, styles.preview)}>
			<PopupSection label={home !== undefined && `Split on ${home.name}`}>
				{all.map((tile) => (
					<PopupItem
						key={tile.id}
						render={<div />}
						icon={tile.icon}
						end={tile.status}
						data-highlighted={tile.id === here ? "" : undefined}
					>
						{tile.name}
					</PopupItem>
				))}
			</PopupSection>
		</Popup>
	);
};

/**
 * The mark on every row a split shows: a small map of its tiles, this row's (`here`) inked. The row
 * the split was made on carries it, and so does each row whose tile is in it, as a terminal beside a
 * branch's page. It goes in the row's `split` cell and opens the {@link SplitPreview} while the
 * pointer is over it. It is a mark, not a button: the row itself opens the split again.
 *
 * While its preview is open, the host lights the split's other rows: `linked` on the rows
 * `tileIds` (splitTiling.ts) names, following `onOpenChange`. The rows that go together then read
 * as one.
 *
 * The preview opens beside the row, just past the nearest ancestor marked
 * `data-split-preview-edge` (the sidebar), so it covers none of the rows it lights; without one, it
 * opens beside the map.
 *
 * A screen reader hears the tiles' names, as the preview only shows under the pointer.
 *
 * @see packages/but-ui/design/components/sidebar-trees.md
 * @import import { SplitPreviewMap } from "@gitbutler/but-ui-react/SplitPreview.tsx";
 */
export const SplitPreviewMap: FC<
	{
		tiling: Tiling;
		/** The `id` of the tile this row shows: the one inked. */
		here: string;
	} & Pick<PreviewCard.Root.Props, "open" | "defaultOpen" | "onOpenChange"> &
		Omit<ComponentProps<"span">, "children">
> = ({ tiling, here, open, defaultOpen, onOpenChange, ...props }) => {
	const trigger = useRef<HTMLSpanElement>(null);
	// The row's height at the edge of what it sits in, read as the preview opens.
	const besideEdge = () => {
		const map = trigger.current;
		if (map === null) return null;
		const edge = map.closest("[data-split-preview-edge]");
		if (edge === null) return map;
		const row = map.getBoundingClientRect();
		const right = edge.getBoundingClientRect().right;
		return { getBoundingClientRect: () => new DOMRect(right, row.top, 0, row.height) };
	};
	return (
		<PreviewCard.Root open={open} defaultOpen={defaultOpen} onOpenChange={onOpenChange}>
			<PreviewCard.Trigger
				render={
					<span
						ref={trigger}
						{...mergeProps<"span">(props, {
							role: "img",
							"aria-label": `Split: ${tiles(tiling)
								.map((tile) => tile.name)
								.join(", ")}`,
							className: styles.trigger,
						})}
					/>
				}
			>
				<Miniature tiling={tiling} here={here} />
			</PreviewCard.Trigger>
			<PreviewCard.Portal>
				<PreviewCard.Positioner
					anchor={besideEdge}
					side="right"
					sideOffset={8}
					className={styles.positioner}
				>
					<PreviewCard.Popup render={<SplitPreview tiling={tiling} here={here} anchored />} />
				</PreviewCard.Positioner>
			</PreviewCard.Portal>
		</PreviewCard.Root>
	);
};
