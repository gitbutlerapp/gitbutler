import { AddressSpaceContext } from "../AddressSpaceContext.ts";
import { setCursor, useIsCursorAt } from "#ui/use-cursor.ts";
import { Row } from "../Row.tsx";
import { addressIdentityKey, type Address } from "#ui/addresses.ts";
import { addressSpaceIncludes } from "#ui/workspace/address-space.ts";
import { type ComponentProps, type FC, use } from "react";
import { assert } from "#ui/assert.ts";

export const ItemRow: FC<
	{
		address: Address;
	} & Omit<ComponentProps<typeof Row>, "inert" | "isSelected" | "onSelect">
> = ({ address, ...props }) => {
	const addressSpace = assert(use(AddressSpaceContext));
	const isSelected = useIsCursorAt("applied", addressSpace, address);
	const selectItem = () => {
		setCursor("applied", address);
	};

	return (
		<Row
			{...props}
			inert={!addressSpaceIncludes(addressSpace, address, addressIdentityKey)}
			isSelected={isSelected}
			onSelect={selectItem}
		/>
	);
};
