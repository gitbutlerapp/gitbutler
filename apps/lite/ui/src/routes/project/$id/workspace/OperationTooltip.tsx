import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { Operand } from "#ui/operands.ts";
import { getOperation } from "#ui/operations/operation.ts";
import { type OutlineMode } from "#ui/outline/mode.ts";
import { Tooltip, useRender } from "@base-ui/react";
import { Match } from "effect";
import { FC } from "react";

export const OperationTooltip: FC<
	{
		target: Operand;
		outlineMode: OutlineMode;
		isActive: boolean;
	} & useRender.ComponentProps<"div">
> = ({ target, outlineMode, isActive, render, ...props }) => {
	const tooltip = isActive
		? Match.value(outlineMode).pipe(
				Match.tags({
					Absorb: () => <>Absorb target</>,
					Transfer: ({ value: mode }) => {
						if (mode.operationType === null) return null;
						const operation = getOperation({
							source: mode.source,
							target,
							operationType: mode.operationType,
						});
						if (!operation) return null;

						return operation.label;
					},
				}),
				Match.orElse(() => null),
			)
		: null;

	const trigger = useRender({ render, props });

	return (
		<Tooltip.Root open={tooltip !== null} disableHoverablePopup>
			<Tooltip.Trigger render={trigger} />
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8} side="right">
					<Tooltip.Popup render={<TooltipPopup />}>{tooltip}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
