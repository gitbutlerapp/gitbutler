import { NavigationIndexContext } from "../OutlineNavigationIndexContext.ts";
import { setCursor, useIsCursorAt } from "#ui/use-cursor.ts";
import { Row } from "../Row.tsx";
import { operandIdentityKey, type Operand } from "#ui/operands.ts";
import { navigationIndexIncludes } from "#ui/workspace/navigation-index.ts";
import { type ComponentProps, type FC, use } from "react";
import { assert } from "#ui/assert.ts";

export const ItemRow: FC<
	{
		operand: Operand;
	} & Omit<ComponentProps<typeof Row>, "inert" | "isSelected" | "onSelect">
> = ({ operand, ...props }) => {
	const navigationIndex = assert(use(NavigationIndexContext));
	const isSelected = useIsCursorAt("stacks", navigationIndex, operand);
	const selectItem = () => {
		setCursor("stacks", operand);
	};

	return (
		<Row
			{...props}
			inert={!navigationIndexIncludes(navigationIndex, operand, operandIdentityKey)}
			isSelected={isSelected}
			onSelect={selectItem}
		/>
	);
};
