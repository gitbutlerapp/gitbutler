import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import type { FocusScope } from "#ui/focus-scopes.ts";
import { Tooltip } from "@base-ui/react";
import type { HotkeySequence } from "@tanstack/react-hotkeys";
import type { FC } from "react";

export type FileRowTooltipPayload = {
	content: string;
	kbd?: string | HotkeySequence;
	kbdScope?: FocusScope;
};

type Props = {
	handle: Tooltip.Handle<FileRowTooltipPayload>;
};

export const FileRowTooltipRoot: FC<Props> = (p) => (
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
