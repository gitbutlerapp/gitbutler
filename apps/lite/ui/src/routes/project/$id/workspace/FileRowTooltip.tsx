import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { Tooltip } from "@base-ui/react";
import type { FC } from "react";
import type { FileRowTooltipHandles, FileRowTooltipPayload } from "./file-row-tooltip.ts";

type Props = {
	handle: Tooltip.Handle<FileRowTooltipPayload>;
};

const SharedFileRowTooltipRoot: FC<Props> = (p) => (
	<Tooltip.Root handle={p.handle} disableHoverablePopup>
		{({ payload }) => (
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup
						render={
							<TooltipPopup
								key={payload?.kbdScope}
								kbd={payload?.kbd}
								kbdScope={payload?.kbdScope}
							/>
						}
					>
						{payload?.content}
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		)}
	</Tooltip.Root>
);

export const FileRowTooltipRoot: FC<{ handles: FileRowTooltipHandles }> = (p) => (
	<>
		<SharedFileRowTooltipRoot handle={p.handles.row} />
		<SharedFileRowTooltipRoot handle={p.handles.control} />
	</>
);
