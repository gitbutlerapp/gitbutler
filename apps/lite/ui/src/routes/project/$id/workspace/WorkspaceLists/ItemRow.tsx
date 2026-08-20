import { setCursor, useIsCursorAt } from "#ui/use-cursor.ts";
import { Row } from "../Row.tsx";
import { useAddressSpace } from "./context.tsx";
import { addressIdentityKey, type Address } from "#ui/addresses.ts";
import { addressSpaceIncludes } from "#ui/workspace/address-space.ts";
import type { ComponentProps, FC } from "react";

export const ItemRow: FC<
	{
		address: Address;
	} & Omit<ComponentProps<typeof Row>, "inert" | "isSelected" | "onSelect">
> = ({ address, ...props }) => {
	const addressSpace = useAddressSpace();
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
