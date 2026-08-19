import type { Address } from "#ui/addresses.ts";
import type { AddressSpace } from "#ui/workspace/address-space.ts";
import { createContext } from "react";

export const AddressSpaceContext = createContext<AddressSpace<Address> | null>(null);
AddressSpaceContext.displayName = "AddressSpaceContext";
