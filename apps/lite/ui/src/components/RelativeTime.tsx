import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { formatAbsoluteTime, formatCompactRelativeTime, formatRelativeTime } from "#ui/time.ts";
import { Tooltip } from "@base-ui/react";
import type { FC } from "react";

/** A relative timestamp whose hover tooltip carries the absolute time. */
export const RelativeTime: FC<{
	timestamp: number;
	/** Pin "now" for stable output across re-renders, as the row lists do. */
	now?: number;
	/** "26m" instead of "26 minutes ago", for dense rows. */
	compact?: boolean;
	className?: string;
}> = ({ timestamp, now, compact = false, className }) => (
	<Tooltip.Root>
		<Tooltip.Trigger render={<span className={className} />}>
			{compact ? formatCompactRelativeTime(timestamp, now) : formatRelativeTime(timestamp, now)}
		</Tooltip.Trigger>
		<Tooltip.Portal>
			<Tooltip.Positioner sideOffset={4}>
				<Tooltip.Popup render={<TooltipPopup />}>{formatAbsoluteTime(timestamp)}</Tooltip.Popup>
			</Tooltip.Positioner>
		</Tooltip.Portal>
	</Tooltip.Root>
);
