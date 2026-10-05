import { Toast } from "@base-ui/react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { hostedMachinesQueryOptions, hostedPresenceQueryOptions } from "#ui/api/queries.ts";
import { sendFromHub } from "#ui/hosted-session.ts";
import { type NativeMenuItem, nativeMenuItem, nativeMenuSeparator } from "#ui/native-menu.ts";

/**
 * Sending another machine's published branch on to one more of yours, as menu items, from the
 * hosted page only: what's sent is what `from` last published.
 */
export const useHubSendMenu = ({
	projectId,
	from,
	branch,
}: {
	/** On the hub, the project's root commit. */
	projectId: string;
	from: string;
	branch: string;
}): Array<NativeMenuItem> => {
	const enabled = window.lite.hosted === true;
	const toastManager = Toast.useToastManager();
	const { data: published = [] } = useQuery({ ...hostedMachinesQueryOptions(projectId), enabled });
	const { data: online = [] } = useQuery({ ...hostedPresenceQueryOptions(projectId), enabled });
	const { isPending, mutate: send } = useMutation({
		mutationFn: sendFromHub,
		onSuccess: (_, { to }) => toastManager.add({ title: `Sent ${branch} to ${to}` }),
		meta: { failureTitle: "Failed to send the branch" },
	});
	if (!enabled) return [];

	// Every machine of the account this page knows, but the one it's from.
	const recipients = [...new Set([...published.map((machine) => machine.name), ...online])]
		.filter((name) => name !== from)
		.toSorted();
	return [
		nativeMenuItem({
			label: "Send Branch To",
			enabled: !isPending && recipients.length > 0,
			submenu: recipients.map((to) =>
				nativeMenuItem({
					label: online.includes(to) ? to : `${to} (offline)`,
					onSelect: () => send({ project: projectId, from, branch, to }),
				}),
			),
		}),
		nativeMenuSeparator,
	];
};
