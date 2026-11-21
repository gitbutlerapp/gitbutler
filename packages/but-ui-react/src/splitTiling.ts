import type { ReactNode } from "react";
import type { IconName } from "./iconNames.ts";

/*
 * A split as SplitPreview.tsx draws it: its tiles and how they are laid out, and the rows they
 * show, for a host to light the rows of the split whose map is pointed at.
 */

/** One tile of a split: what it shows, by the row it belongs to. */
export type SplitTile = {
	/** The row the tile shows, by the host's key for it. The map on that row inks this tile. */
	id: string;
	/** The row's name, as its own row in the sidebar says it. */
	name: string;
	/** The row's glyph: `terminal` for a terminal, `branch` for a branch, `diff` for changes. */
	icon: IconName;
	/** What the tile's program is at, as its row says it: a `MetaStatus` glyph without a label, or
	 * an agent's `MetaCount`. Left out while there is nothing to say. */
	status?: ReactNode;
	/** The row the split was made on, which the preview names it after. */
	home?: boolean;
};

/** How a row's tiles are laid out: a tile, or tiles side by side (`across`) or stacked (`down`),
 * as the GPUI client splits them. `sizes` are the parts' shares of the room, equal when left out. */
export type Tiling =
	| SplitTile
	| { split: "across" | "down"; parts: Array<Tiling>; sizes?: Array<number> };

export const isTile = (tiling: Tiling): tiling is SplitTile => !("split" in tiling);

export const tiles = (tiling: Tiling): Array<SplitTile> =>
	isTile(tiling) ? [tiling] : tiling.parts.flatMap(tiles);

/** The ids of the rows `tiling`'s tiles show: the rows to light, as `linked`, while the pointer
 * is on one of their maps. */
export const tileIds = (tiling: Tiling): Array<string> => tiles(tiling).map((tile) => tile.id);
