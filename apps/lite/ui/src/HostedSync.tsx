import type { ToastManager } from "@base-ui/react";
import type { SyncOutcome } from "@gitbutler/but-sdk";
import { Button } from "@gitbutler/ui-react/Button.tsx";
import { Modal, ModalFooter, ModalHeader } from "@gitbutler/ui-react/Popup.tsx";
import { createContext, type FC, type ReactNode, useContext, useState } from "react";

type Choice = {
	title: string;
	keepLabel: string;
	reason: string;
	overwrite: () => void;
};

type Settle = (outcome: SyncOutcome, ask: Omit<Choice, "reason">) => void;

const SettleContext = createContext<Settle>(() => {});

/**
 * Settles a publish to, or pull from, the hosted server: done is reported, and
 * work it would replace is asked about first.
 */
// oxlint-disable-next-line react/only-export-components -- The hook reads the provider's own context.
export const useHostedSync = (): Settle => useContext(SettleContext);

/** The one dialog every hosted sync asks through, wherever it started. */
export const HostedSyncProvider: FC<{ toastManager: ToastManager; children: ReactNode }> = ({
	toastManager,
	children,
}) => {
	// Kept after closing, so the dialog doesn't empty while it animates out.
	const [choice, setChoice] = useState<Choice>();
	const [open, setOpen] = useState(false);

	const settle: Settle = (outcome, ask) => {
		if (outcome.type === "done") {
			toastManager.add({ title: outcome.subject });
			return;
		}
		setChoice({ ...ask, reason: outcome.subject });
		setOpen(true);
	};

	return (
		<SettleContext value={settle}>
			{children}
			<Modal open={open} onOpenChange={setOpen}>
				<ModalHeader title={choice?.title} description={choice?.reason} />
				<ModalFooter>
					<Button variant="ghost" onClick={() => setOpen(false)}>
						{choice?.keepLabel}
					</Button>
					<Button
						variant="gray"
						onClick={() => {
							choice?.overwrite();
							setOpen(false);
						}}
					>
						Overwrite
					</Button>
				</ModalFooter>
			</Modal>
		</SettleContext>
	);
};
