import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { useNow } from "#ui/components/useNow.ts";
import { formatAbsoluteTime, formatCompactRelativeTime, formatRelativeTime } from "#ui/time.ts";
import { Tooltip } from "@base-ui/react";
import type { FC } from "react";

/**
 * How often an unpinned timestamp re-reads the clock. The wording is exact to
 * the second below a minute and coarse above it, so this is the point where
 * waiting longer would start to show.
 */
const TICK_MS = 30_000;

/** A relative timestamp whose hover tooltip carries the absolute time. */
export const RelativeTime: FC<{
	timestamp: number;
	/** Pin "now" for stable output across re-renders, as the row lists do. */
	now?: number;
	/** "26m" instead of "26 minutes ago", for dense rows. */
	compact?: boolean;
	className?: string;
}> = ({ timestamp, now, compact = false, className }) => {
	// Left unpinned, the text ages in place: nothing re-renders a settled
	// conversation, so "2 minutes ago" would still say that an hour later.
	const ticking = useNow(now === undefined ? TICK_MS : null);

	return (
		<Tooltip.Root>
			<Tooltip.Trigger render={<span className={className} />}>
				{compact
					? formatCompactRelativeTime(timestamp, now ?? ticking)
					: formatRelativeTime(timestamp, now ?? ticking)}
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup />}>{formatAbsoluteTime(timestamp)}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
