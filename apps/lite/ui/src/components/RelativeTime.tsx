import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { formatAbsoluteTime, formatRelativeTime } from "#ui/time.ts";
import { Tooltip } from "@base-ui/react";
import type { FC } from "react";

/** A relative timestamp whose hover tooltip carries the absolute time. */
export const RelativeTime: FC<{
	timestamp: number;
	/** Pin "now" for stable output across re-renders, as the row lists do. */
	now?: number;
	className?: string;
}> = ({ timestamp, now, className }) => (
	<Tooltip.Root>
		<Tooltip.Trigger render={<span className={className} />}>
			{formatRelativeTime(timestamp, now)}
		</Tooltip.Trigger>
		<Tooltip.Portal>
			<Tooltip.Positioner sideOffset={4}>
				<Tooltip.Popup render={<TooltipPopup />}>{formatAbsoluteTime(timestamp)}</Tooltip.Popup>
			</Tooltip.Positioner>
		</Tooltip.Portal>
	</Tooltip.Root>
);
