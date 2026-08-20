// oxlint-disable react/only-export-components -- The context module exports its provider and hooks together by design.
import { assert } from "#ui/assert.ts";
import type { Address } from "#ui/addresses.ts";
import type { AddressSpace } from "#ui/workspace/address-space.ts";
import { createContext, use, type FC, type ReactNode } from "react";

/**
 * Shared with every row: the address space rows resolve against, and the
 * commits the pending absorption would land in. One context because the two
 * change together and their consumers overlap. Only the hooks below read
 * it, and they assert the provider, so the `null` default never reaches a
 * consumer.
 */
type WorkspaceListsContext = {
	addressSpace: AddressSpace<Address>;
	absorptionTargetCommitIds: ReadonlySet<string>;
};

const Context = createContext<WorkspaceListsContext | null>(null);
Context.displayName = "WorkspaceListsContext";

export const WorkspaceListsProvider: FC<WorkspaceListsContext & { children: ReactNode }> = ({
	addressSpace,
	absorptionTargetCommitIds,
	children,
}) => <Context value={{ addressSpace, absorptionTargetCommitIds }}>{children}</Context>;

export const useAddressSpace = (): AddressSpace<Address> => assert(use(Context)).addressSpace;

export const useAbsorptionTargetCommitIds = (): ReadonlySet<string> =>
	assert(use(Context)).absorptionTargetCommitIds;
