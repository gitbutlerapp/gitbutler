import { NavigationIndexContext } from "../SidebarNavigationIndexContext.ts";
import { setCursor, useIsCursorAt } from "#ui/use-cursor.ts";
import { Row } from "../Row.tsx";
import { addressIdentityKey, type Address } from "#ui/addresses.ts";
import { navigationIndexIncludes } from "#ui/workspace/navigation-index.ts";
import { type ComponentProps, type FC, use } from "react";
import { assert } from "#ui/assert.ts";

export const ItemRow: FC<
	{
		address: Address;
	} & Omit<ComponentProps<typeof Row>, "inert" | "isSelected" | "onSelect">
> = ({ address, ...props }) => {
	const navigationIndex = assert(use(NavigationIndexContext));
	const isSelected = useIsCursorAt("applied", navigationIndex, address);
	const selectItem = () => {
		setCursor("applied", address);
	};

	return (
		<Row
			{...props}
			inert={!navigationIndexIncludes(navigationIndex, address, addressIdentityKey)}
			isSelected={isSelected}
			onSelect={selectItem}
		/>
	);
};
